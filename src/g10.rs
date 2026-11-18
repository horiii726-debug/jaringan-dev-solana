//! Gate G10: re-fetch a random sample of creates with plain getTransaction from a second, public RPC endpoint
//! and compare mint, creator and create signer with our decoded rows.
use crate::decode::{self, Ev};
use crate::graph::read_lines;
use serde_json::{json, Value};
use std::thread::sleep;
use std::time::Duration;

fn h(s: &str) -> u64 {
    // FNV-1a: deterministic sample independent of outcome
    s.bytes().fold(0xcbf29ce484222325u64, |a, b| (a ^ b as u64).wrapping_mul(0x100000001b3))
}

pub fn run(files: &[String], n: usize) -> Result<(), String> {
    let url = "https://api.mainnet-beta.solana.com";
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build();
    let mut rows: Vec<Value> = files.iter().flat_map(|p| read_lines(p)).filter(|v| v["kind"] == "create").collect();
    rows.sort_by_key(|v| h(v["sig"].as_str().unwrap_or("")));
    let (mut ok, mut bad, mut miss) = (0, 0, 0);
    for v in rows.iter().take(n) {
        let sig = v["sig"].as_str().unwrap_or("");
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "getTransaction", "params": [sig, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 1}]}).to_string();
        let mut got: Option<Value> = None;
        for _ in 0..8 {
            sleep(Duration::from_millis(1200));
            if let Ok(r) = agent.post(url).set("content-type", "application/json").send_string(&body) {
                if let Ok(j) = r.into_string().map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string())) {
                    if !j["result"].is_null() {
                        got = Some(j["result"].clone());
                        break;
                    }
                }
            }
            sleep(Duration::from_secs(3));
        }
        let Some(tx) = got else { miss += 1; continue };
        let (evs, _) = decode::events(&tx);
        let c = evs.iter().find_map(|e| if let Ev::Create(c) = e { Some(c.clone()) } else { None });
        let same = c.map(|c| c.mint == v["mint"] && c.creator.unwrap_or(c.user.clone()) == v["creator"] && c.user == v["user"]).unwrap_or(false);
        if same {
            ok += 1;
        } else {
            bad += 1;
            println!("MISMATCH {sig}");
        }
    }
    println!("G10 sample={} match={ok} mismatch={bad} not_fetched={miss}", ok + bad + miss);
    Ok(())
}
