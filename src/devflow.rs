//! Where does the dev move the coin? For each coin: transactions of the creator and create signer after the
//! create (gTFA, up to 3 pages), every token transfer of that mint leaving them -> recipient owners.
//! Those recipients join the "dev family": when any family wallet sells, we sell.
//! Input lines: mint,creator,user,create_bt. Output: {mint, family:[wallets that received the coin from the dev]}.
use crate::graph::read_lines;
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;

/// Owner of each token-balance account index in a parsed tx, for one mint.
fn owners(tx: &Value, mint: &str) -> Vec<(u64, String, f64)> {
    let mut v = Vec::new();
    for b in tx["meta"]["postTokenBalances"].as_array().into_iter().flatten() {
        if b["mint"] == mint {
            v.push((b["accountIndex"].as_u64().unwrap_or(0), b["owner"].as_str().unwrap_or("").to_string(),
                b["uiTokenAmount"]["uiAmount"].as_f64().unwrap_or(0.0)));
        }
    }
    v
}

pub fn run(list: &str, out: &str) -> Result<(), String> {
    let done: HashSet<String> = read_lines(out).iter().filter_map(|v| v["mint"].as_str().map(String::from)).collect();
    let mut rpc = Rpc::new("devflow")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for line in fs::read_to_string(list).map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = line.split(',').collect();
        if p.len() < 4 || done.contains(p[0]) {
            continue;
        }
        let (mint, bt0): (&str, i64) = (p[0], p[3].parse().unwrap_or(0));
        let devs: Vec<&str> = if p[1] == p[2] { vec![p[1]] } else { vec![p[1], p[2]] };
        let mut fam: HashSet<String> = HashSet::new();
        for d in &devs {
            let mut token: Option<String> = None;
            for _ in 0..3 {
                let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
                    "sortOrder": "asc", "filters": {"status": "succeeded", "blockTime": {"gte": bt0, "lte": bt0 + 86_400}}});
                if let Some(t) = &token {
                    o["paginationToken"] = json!(t);
                }
                let r = match rpc.call("getTransactionsForAddress", json!([d, o]), 10) {
                    Ok(r) => r,
                    Err(e) if e.starts_with("RPC") => break,
                    Err(e) => return Err(e),
                };
                let data = r["data"].as_array().cloned().unwrap_or_default();
                for tx in &data {
                    // token moved from a dev-owned account to an account owned by someone else (not a pool/curve sell:
                    // sells are detected separately as trades) -> recipient owner joins the family
                    let pre: Vec<(u64, String, f64)> = tx["meta"]["preTokenBalances"].as_array().into_iter().flatten()
                        .filter(|b| b["mint"] == mint)
                        .map(|b| (b["accountIndex"].as_u64().unwrap_or(0), b["owner"].as_str().unwrap_or("").to_string(), b["uiTokenAmount"]["uiAmount"].as_f64().unwrap_or(0.0)))
                        .collect();
                    let post = owners(tx, mint);
                    let dev_out = pre.iter().filter(|x| x.1 == *d).map(|x| x.2).sum::<f64>()
                        - post.iter().filter(|x| x.1 == *d).map(|x| x.2).sum::<f64>();
                    if dev_out <= 0.0 {
                        continue;
                    }
                    let is_trade = crate::decode::events(tx).0.iter().any(|e| matches!(e, crate::decode::Ev::Trade(_)));
                    if is_trade {
                        continue;
                    }
                    for (idx, own, amt) in &post {
                        let before = pre.iter().find(|x| x.0 == *idx).map(|x| x.2).unwrap_or(0.0);
                        if own != d && amt > &before {
                            fam.insert(own.clone());
                        }
                    }
                }
                token = r["paginationToken"].as_str().map(String::from);
                if data.is_empty() || token.is_none() {
                    break;
                }
            }
        }
        writeln!(f, "{}", json!({"mint": mint, "family": fam.into_iter().collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}
