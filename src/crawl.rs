//! Forward crawl (bubblemap style): from core wallets, follow outgoing SOL transfers down to `depth` levels and
//! collect every connected wallet, until `max_wallets`. Exchanges/services (data/p3/exchange_list.txt) are not
//! expanded. Each wallet: up to `pages` x 100 newest transactions since `since` (gTFA, 10 credits per 100).
//! Output: one edge per line {parent, child, lam, slot, bt, sig, depth, root}.
use crate::decode;
use crate::graph::read_lines;
use crate::rpc::Rpc;
use serde_json::json;
use std::collections::{HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;

pub fn run(roots_file: &str, depth: u32, max_wallets: usize, pages: u32, since: i64, out: &str) -> Result<(), String> {
    let skip: HashSet<String> = fs::read_to_string("data/p3/exchange_list.txt").unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut expanded: HashSet<String> = HashSet::new();
    // resume: wallets already expanded = parents present in the output
    for v in read_lines(out) {
        if let (Some(p), Some(c)) = (v["parent"].as_str(), v["child"].as_str()) {
            expanded.insert(p.to_string());
            seen.insert(p.to_string());
            seen.insert(c.to_string());
        }
    }
    let mut q: VecDeque<(String, u32, String)> = VecDeque::new();
    for r in fs::read_to_string(roots_file).map_err(|e| e.to_string())?.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        seen.insert(r.to_string());
        q.push_back((r.to_string(), 0, r.to_string()));
    }
    // re-queue children of expanded wallets (resume) is approximated by re-reading edges
    for v in read_lines(out) {
        let (c, d, root) = (v["child"].as_str().unwrap_or(""), v["depth"].as_u64().unwrap_or(0) as u32, v["root"].as_str().unwrap_or(""));
        if !expanded.contains(c) && d < depth {
            q.push_back((c.to_string(), d, root.to_string()));
        }
    }
    let mut rpc = Rpc::new("crawl")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    while let Some((w, d, root)) = q.pop_front() {
        if seen.len() >= max_wallets {
            break;
        }
        if expanded.contains(&w) || skip.contains(&w) || d >= depth {
            continue;
        }
        expanded.insert(w.clone());
        let mut token: Option<String> = None;
        for _ in 0..pages {
            let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
                "filters": {"status": "succeeded", "blockTime": {"gte": since}}});
            if let Some(t) = &token {
                o["paginationToken"] = json!(t);
            }
            let r = match rpc.call("getTransactionsForAddress", json!([w, o]), 10) {
                Ok(r) => r,
                Err(e) if e.starts_with("RPC") => break,
                Err(e) => {
                    rpc.save();
                    return Err(e);
                }
            };
            let data = r["data"].as_array().cloned().unwrap_or_default();
            for tx in &data {
                let sig = tx["transaction"]["signatures"][0].as_str().unwrap_or("");
                for (s, c, lam) in decode::sol_moves(tx) {
                    if s != w || c == w || lam < 1_000_000 {
                        continue; // ignore dust (< 0.001 SOL)
                    }
                    writeln!(f, "{}", json!({"parent": w, "child": c, "lam": lam, "slot": tx["slot"], "bt": tx["blockTime"], "sig": sig, "depth": d + 1, "root": root}))
                        .map_err(|e| e.to_string())?;
                    if seen.insert(c.clone()) {
                        q.push_back((c, d + 1, root.clone()));
                    }
                }
            }
            token = r["paginationToken"].as_str().map(String::from);
            if data.is_empty() || token.is_none() {
                break;
            }
        }
        if expanded.len().is_multiple_of(50) {
            rpc.save();
            println!("expanded={} wallets={} queue={} credits_total={}", expanded.len(), seen.len(), q.len(), rpc.used());
        }
    }
    rpc.save();
    println!("DONE expanded={} wallets={} credits_total={}", expanded.len(), seen.len(), rpc.used());
    Ok(())
}
