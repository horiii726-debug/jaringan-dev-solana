//! Bitquery (owner's token, BITQUERY_TOKEN in .env): native SOL transfers sent by a set of wallets, in ONE query
//! per page (Sender in [...]), to find every wallet connected to the team wallets. Bitquery's Solana data
//! (streaming.bitquery.io/eap) covers roughly the last day, so this is for recent and live discovery.
//! Output: one edge per line {parent, child, sol, time, sig}. Points are saved by batching all senders together.
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Duration;

pub fn run(senders_file: &str, since: &str, max_rows: usize, out: &str) -> Result<(), String> {
    let env = fs::read_to_string(".env").map_err(|e| e.to_string())?;
    let token = env.lines().find_map(|l| l.strip_prefix("BITQUERY_TOKEN=")).ok_or("BITQUERY_TOKEN missing")?.trim().to_string();
    let senders: Vec<String> = fs::read_to_string(senders_file).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).build();
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    let mut total = 0usize;
    for chunk in senders.chunks(100) {
        let mut after = since.to_string();
        loop {
            let list = chunk.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",");
            let q = format!("{{ Solana {{ Transfers(limit: {{count: 2000}}, orderBy: {{ascending: Block_Time}}, where: {{Block: {{Time: {{after: \"{after}\"}}}}, \
                Transfer: {{Sender: {{Address: {{in: [{list}]}}}}, Currency: {{Native: true}}, Amount: {{ge: \"0.05\"}}}}}}) \
                {{ Transfer {{ Sender {{ Address }} Receiver {{ Address }} Amount }} Block {{ Time }} Transaction {{ Signature }} }} }} }}");
            std::thread::sleep(Duration::from_secs(5));
            let resp = agent.post("https://streaming.bitquery.io/eap").set("content-type", "application/json")
                .set("Authorization", &format!("Bearer {token}")).send_string(&json!({"query": q}).to_string());
            let v: Value = match resp {
                Ok(r) => serde_json::from_str(&r.into_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?,
                Err(ureq::Error::Status(429, _)) => {
                    std::thread::sleep(Duration::from_secs(65));
                    continue;
                }
                Err(ureq::Error::Status(c, r)) => return Err(format!("bitquery http {c}: {}", r.into_string().unwrap_or_default().chars().take(200).collect::<String>())),
                Err(e) => return Err(format!("bitquery transport {}", e.to_string().replace(&token, "***"))),
            };
            if let Some(e) = v.get("errors") {
                return Err(format!("bitquery error: {}", e.to_string().chars().take(300).collect::<String>()));
            }
            let rows = v["data"]["Solana"]["Transfers"].as_array().cloned().unwrap_or_default();
            for r in &rows {
                let (p, c) = (r["Transfer"]["Sender"]["Address"].as_str().unwrap_or(""), r["Transfer"]["Receiver"]["Address"].as_str().unwrap_or(""));
                if p == c {
                    continue;
                }
                writeln!(f, "{}", json!({"parent": p, "child": c, "sol": r["Transfer"]["Amount"], "time": r["Block"]["Time"], "sig": r["Transaction"]["Signature"]}))
                    .map_err(|e| e.to_string())?;
            }
            total += rows.len();
            println!("senders {}..: rows {} total {total} last {}", chunk.len(), rows.len(), rows.last().map(|r| r["Block"]["Time"].clone()).unwrap_or(Value::Null));
            if rows.len() < 2000 || total >= max_rows {
                break;
            }
            after = rows.last().and_then(|r| r["Block"]["Time"].as_str()).unwrap_or(&after).to_string();
        }
        if total >= max_rows {
            break;
        }
    }
    println!("DONE rows={total}");
    Ok(())
}

/// Creator of each mint from its create tx (oldest tx of the mint, Helius gTFA, 10 credits per mint).
pub fn creators(list: &str, out: &str) -> Result<(), String> {
    let mut rpc = crate::rpc::Rpc::new("creators")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for m in fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let r = rpc.call("getTransactionsForAddress", json!([m, {"transactionDetails": "full", "limit": 1, "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 1, "sortOrder": "asc", "filters": {"status": "succeeded"}}]), 10)?;
        for tx in r["data"].as_array().into_iter().flatten() {
            for e in crate::decode::events(tx).0 {
                if let crate::decode::Ev::Create(c) = e {
                    if c.mint == m {
                        writeln!(f, "{}", json!({"mint": m, "creator": c.creator.clone().unwrap_or(c.user.clone()), "user": c.user, "bt": tx["blockTime"]}))
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}

/// First `n` buyers of each mint (Bitquery DEXTrades, one batched query per 100 mints, limitBy mint).
pub fn early(list: &str, n: u32, out: &str) -> Result<(), String> {
    let env = fs::read_to_string(".env").map_err(|e| e.to_string())?;
    let token = env.lines().find_map(|l| l.strip_prefix("BITQUERY_TOKEN=")).ok_or("BITQUERY_TOKEN missing")?.trim().to_string();
    let mints: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(300)).build();
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for chunk in mints.chunks(100) {
        let l = chunk.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",");
        let q = format!("{{ Solana {{ DEXTrades(limitBy: {{by: Trade_Buy_Currency_MintAddress, count: {n}}}, orderBy: {{ascending: Block_Time}}, \
            where: {{Trade: {{Buy: {{Currency: {{MintAddress: {{in: [{l}]}}}}}}}}, Transaction: {{Result: {{Success: true}}}}}}) \
            {{ Block {{ Time Slot }} Trade {{ Buy {{ Account {{ Owner }} Amount AmountInUSD Currency {{ MintAddress }} }} Sell {{ Amount }} }} Transaction {{ Signature Signer }} }} }} }}");
        let resp = agent.post("https://streaming.bitquery.io/eap").set("content-type", "application/json")
            .set("Authorization", &format!("Bearer {token}")).send_string(&json!({"query": q}).to_string());
        let v: Value = match resp {
            Ok(r) => serde_json::from_str(&r.into_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?,
            Err(ureq::Error::Status(c, r)) => return Err(format!("bitquery http {c}: {}", r.into_string().unwrap_or_default().chars().take(200).collect::<String>())),
            Err(e) => return Err(format!("bitquery transport {}", e.to_string().replace(&token, "***"))),
        };
        if let Some(e) = v.get("errors") {
            return Err(format!("bitquery error: {}", e.to_string().chars().take(300).collect::<String>()));
        }
        for r in v["data"]["Solana"]["DEXTrades"].as_array().into_iter().flatten() {
            writeln!(f, "{}", json!({"mint": r["Trade"]["Buy"]["Currency"]["MintAddress"], "buyer": r["Trade"]["Buy"]["Account"]["Owner"],
                "signer": r["Transaction"]["Signer"], "sol": r["Trade"]["Sell"]["Amount"], "usd": r["Trade"]["Buy"]["AmountInUSD"], "time": r["Block"]["Time"], "slot": r["Block"]["Slot"]}))
                .map_err(|e| e.to_string())?;
        }
        println!("chunk {} done", chunk.len());
    }
    Ok(())
}
