//! P3: first funder of a wallet = source of the first incoming SOL (oldest successful tx of the wallet).
//! Cost: 1 credit per 1000 signatures + 1 credit for the oldest transaction.
use crate::decode;
use crate::rpc::{cfg_u64, issue, Rpc};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};

/// Funder from one parsed transaction: first System transfer/createAccount into `w`,
/// else the other account with the largest SOL decrease (flagged "balance").
pub fn funder_in(tx: &Value, w: &str) -> (Option<String>, u64, &'static str) {
    if let Some((s, _, l)) = decode::sol_moves(tx).into_iter().find(|(s, d, _)| d == w && s != w) {
        return (Some(s), l, "transfer");
    }
    let keys = tx["transaction"]["message"]["accountKeys"].as_array().cloned().unwrap_or_default();
    let pre = tx["meta"]["preBalances"].as_array().cloned().unwrap_or_default();
    let post = tx["meta"]["postBalances"].as_array().cloned().unwrap_or_default();
    let mut best: Option<(String, i64)> = None;
    for (i, k) in keys.iter().enumerate() {
        let pk = k["pubkey"].as_str().unwrap_or("");
        if pk == w {
            continue;
        }
        let d = pre.get(i).and_then(|v| v.as_i64()).unwrap_or(0) - post.get(i).and_then(|v| v.as_i64()).unwrap_or(0);
        if d > 0 && best.as_ref().map(|b| d > b.1).unwrap_or(true) {
            best = Some((pk.to_string(), d));
        }
    }
    match best {
        Some((p, d)) => (Some(p), d as u64, "balance"),
        None => (None, 0, "none"),
    }
}

pub fn one(rpc: &mut Rpc, w: &str, max_pages: u64) -> Result<Value, String> {
    let mut sigs: Vec<Value> = Vec::new();
    let mut before: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut o = json!({"limit": 1000});
        if let Some(b) = &before {
            o["before"] = json!(b);
        }
        let r = rpc.call("getSignaturesForAddress", json!([w, o]), 1)?;
        let a = r.as_array().cloned().unwrap_or_default();
        pages += 1;
        let full = a.len() == 1000;
        before = a.last().and_then(|s| s["signature"].as_str()).map(String::from);
        sigs.extend(a);
        if !full {
            break;
        }
        if pages >= max_pages {
            return Ok(json!({"w": w, "n_sigs": sigs.len(), "deep": true}));
        }
    }
    let n = sigs.len();
    let Some(first) = sigs.iter().rev().find(|s| s["err"].is_null()) else {
        return Ok(json!({"w": w, "n_sigs": n, "deep": false, "kind": "no_success_tx"}));
    };
    let sig = first["signature"].as_str().unwrap_or("").to_string();
    let tx = rpc.call("getTransaction", json!([sig, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 1}]), 1)?;
    if tx.is_null() {
        issue("fund_tx_null", json!({"w": w, "sig": sig}));
        return Ok(json!({"w": w, "n_sigs": n, "deep": false, "kind": "tx_null", "first_sig": sig}));
    }
    let (f, lam, kind) = funder_in(&tx, w);
    let payer = tx["transaction"]["message"]["accountKeys"][0]["pubkey"].clone();
    Ok(json!({"w": w, "n_sigs": n, "deep": false, "first_sig": sig, "first_slot": tx["slot"], "first_bt": tx["blockTime"],
        "funder": f, "lam": lam, "kind": kind, "payer": payer, "last_bt": sigs.first().map(|s| s["blockTime"].clone())}))
}

pub fn run(list: &str, out: &str) -> Result<(), String> {
    let max_pages = cfg_u64("config/budget.toml", "fund_max_pages").unwrap_or(3);
    let stem = std::path::Path::new(out).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut rpc = Rpc::new(&format!("fund_{stem}"))?;
    // Resume: keep only complete lines, collect finished wallets.
    let mut done: HashSet<String> = HashSet::new();
    if let Ok(s) = fs::read_to_string(out) {
        let mut good = String::new();
        for l in s.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                done.insert(v["w"].as_str().unwrap_or("").to_string());
                good.push_str(l);
                good.push('\n');
            }
        }
        fs::write(out, good).map_err(|e| e.to_string())?;
    }
    let ws: Vec<String> = BufReader::new(fs::File::open(list).map_err(|e| e.to_string())?)
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    let todo: Vec<&String> = ws.iter().filter(|w| !done.contains(*w)).collect();
    println!("wallets={} done={} todo={}", ws.len(), done.len(), todo.len());
    // Batches of 10 wallets: one batch of signature lists, one batch of oldest transactions.
    // Items that failed are not written, so a re-run picks them up.
    for (ci, chunk) in todo.chunks(10).enumerate() {
        let reqs: Vec<(&str, Value)> = chunk.iter().map(|w| ("getSignaturesForAddress", json!([w, {"limit": 1000}]))).collect();
        let res = match rpc.call_batch(&reqs) {
            Ok(r) => r,
            Err(e) => {
                rpc.save();
                return Err(e);
            }
        };
        let mut need: Vec<(&str, String, usize, Value)> = Vec::new();
        for (w, r) in chunk.iter().zip(res) {
            let Some(a) = r.as_array() else { continue };
            if a.len() == 1000 {
                match one(&mut rpc, w, max_pages) {
                    Ok(v) => writeln!(f, "{v}").map_err(|e| e.to_string())?,
                    Err(e) if e.starts_with("RPC") => issue("fund_skip", json!({"w": w, "err": e})),
                    Err(e) => {
                        rpc.save();
                        return Err(e);
                    }
                }
                continue;
            }
            match a.iter().rev().find(|s| s["err"].is_null()) {
                None => writeln!(f, "{}", json!({"w": w, "n_sigs": a.len(), "deep": false, "kind": "no_success_tx"})).map_err(|e| e.to_string())?,
                Some(s) => need.push((w.as_str(), s["signature"].as_str().unwrap_or("").to_string(), a.len(), a.first().map(|x| x["blockTime"].clone()).unwrap_or(Value::Null))),
            }
        }
        if !need.is_empty() {
            let reqs: Vec<(&str, Value)> = need.iter().map(|(_, s, _, _)| ("getTransaction", json!([s, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 1}]))).collect();
            let txs = match rpc.call_batch(&reqs) {
                Ok(r) => r,
                Err(e) => {
                    rpc.save();
                    return Err(e);
                }
            };
            for ((w, sig, n, last_bt), tx) in need.into_iter().zip(txs) {
                if tx.is_null() {
                    issue("fund_tx_null", json!({"w": w, "sig": sig}));
                    continue;
                }
                let (fu, lam, kind) = funder_in(&tx, w);
                let payer = tx["transaction"]["message"]["accountKeys"][0]["pubkey"].clone();
                writeln!(f, "{}", json!({"w": w, "n_sigs": n, "deep": false, "first_sig": sig, "first_slot": tx["slot"], "first_bt": tx["blockTime"],
                    "funder": fu, "lam": lam, "kind": kind, "payer": payer, "last_bt": last_bt})).map_err(|e| e.to_string())?;
            }
        }
        if (ci + 1).is_multiple_of(50) {
            rpc.save();
            println!("{} / {} credits_total={} retries={}", (ci + 1) * 10, todo.len(), rpc.used(), rpc.retries);
        }
    }
    rpc.save();
    println!("DONE {} credits_total={}", todo.len(), rpc.used());
    Ok(())
}


/// Activity of wallets (e.g. funders): newest 1000 signatures, count and time span. Busy wallets
/// (many tx per hour) are services/exchanges/bots, not a team's own wallet. 1 credit per wallet.
pub fn activity(list: &str, out: &str) -> Result<(), String> {
    let mut rpc = Rpc::new("activity")?;
    let done: HashSet<String> = fs::read_to_string(out).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|v| v["w"].as_str().map(String::from)).collect();
    let ws: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !done.contains(l)).collect();
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for chunk in ws.chunks(10) {
        let reqs: Vec<(&str, Value)> = chunk.iter().map(|w| ("getSignaturesForAddress", json!([w, {"limit": 1000}]))).collect();
        let res = rpc.call_batch(&reqs)?;
        for (w, r) in chunk.iter().zip(res) {
            let Some(a) = r.as_array() else { continue };
            let span = match (a.first().and_then(|s| s["blockTime"].as_i64()), a.last().and_then(|s| s["blockTime"].as_i64())) {
                (Some(x), Some(y)) => x - y,
                _ => -1,
            };
            writeln!(f, "{}", json!({"w": w, "n": a.len(), "span_s": span, "newest_bt": a.first().map(|s| s["blockTime"].clone())})).map_err(|e| e.to_string())?;
        }
    }
    rpc.save();
    println!("activity done {} credits_total={}", ws.len(), rpc.used());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn funder_prefers_system_transfer() {
        let tx = json!({"transaction": {"message": {"accountKeys": [{"pubkey": "P"}, {"pubkey": "W"}, {"pubkey": "F"}],
            "instructions": [{"programId": decode::SYSTEM, "parsed": {"type": "transfer", "info": {"source": "F", "destination": "W", "lamports": 5}}}]}},
            "meta": {"preBalances": [100, 0, 50], "postBalances": [90, 5, 45], "innerInstructions": []}});
        assert_eq!(funder_in(&tx, "W"), (Some("F".into()), 5, "transfer"));
    }
    #[test]
    fn funder_falls_back_to_balance() {
        let tx = json!({"transaction": {"message": {"accountKeys": [{"pubkey": "P"}, {"pubkey": "W"}], "instructions": []}},
            "meta": {"preBalances": [100, 0], "postBalances": [60, 40], "innerInstructions": []}});
        assert_eq!(funder_in(&tx, "W"), (Some("P".into()), 40, "balance"));
    }
}
