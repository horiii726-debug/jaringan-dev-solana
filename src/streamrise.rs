//! streamrise (21 Nov): how far the coins caught live on the ShredStream gRPC (data/shred/creates.jsonl) rose.
//! Per coin: mint (from the stream, else from the creator's create tx near the stream time), then the bonding-curve
//! trades (Helius, oldest first, up to <pages> x 100 txs) through coinrise::Curve:
//!   entry = first trade in a slot after the create slot (the price we can reach), peak, last, graduated.
//!   rise from entry = peak / entry; rise from launch = peak mcap / 27.96 SOL (Pump.fun start).
//! Detection delay = our stream receive time - the create block time (block time has 1 s resolution).
//! Usage: jaringan streamrise <creates.jsonl> <out_dir> <pages>

use crate::coinrise::{stats, Curve};
use crate::decode::{self, Ev};
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;

const LAUNCH_SOL: f64 = 27.96;

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn resolve_mint(rpc: &mut Rpc, creator: &str, name: &str, t_s: i64) -> Result<Option<String>, String> {
    let r = rpc.call("getTransactionsForAddress", json!([creator, {"transactionDetails": "full", "limit": 20, "sortOrder": "desc",
        "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1, "filters": {"blockTime": {"gte": t_s - 90, "lte": t_s + 90}}}]), 10)?;
    // spam creators launch the same name again and again: take the create whose block time is closest to when the
    // stream showed it (name match first), never just the newest one
    let mut best: Option<(bool, i64, String)> = None;
    for tx in r["data"].as_array().into_iter().flatten() {
        let bt = tx["blockTime"].as_i64().unwrap_or(0);
        for e in decode::events(tx).0 {
            if let Ev::Create(c) = e {
                let cand = (c.name != name, (bt - t_s).abs(), c.mint);
                if best.as_ref().is_none_or(|b| (cand.0, cand.1) < (b.0, b.1)) {
                    best = Some(cand);
                }
            }
        }
    }
    Ok(best.filter(|b| b.1 <= 5).map(|b| b.2))
}

pub fn run(creates: &str, out: &str, pages: u32) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let path = format!("{out}/rise.jsonl");
    let done: HashSet<String> = read_jsonl(&path).iter().filter_map(|v| v["key"].as_str().map(String::from)).collect();
    let mut rpc = Rpc::new("streamrise")?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| e.to_string())?;
    let mut seen = HashSet::new();
    for c in read_jsonl(creates) {
        let (creator, name) = (c["creator"].as_str().unwrap_or("").to_string(), c["name"].as_str().unwrap_or("").to_string());
        let key = format!("{creator}|{name}|{}", c["slot"]);
        if !seen.insert(key.clone()) || done.contains(&key) {
            continue;
        }
        let t_ms = c["t_ms"].as_i64().unwrap_or(0);
        let mint = match c["mint"].as_str() {
            Some(m) => Some(m.to_string()),
            None => match resolve_mint(&mut rpc, &creator, &name, t_ms / 1000) {
                Ok(m) => m,
                Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                Err(_) => None,
            },
        };
        let Some(mint) = mint else {
            f.write_all(format!("{}\n", json!({"key": key, "symbol": c["symbol"], "creator": creator, "error": "mint not found"})).as_bytes()).ok();
            continue;
        };
        let mut cv = Curve::default();
        let (mut token, mut p, mut complete) = (None::<String>, 0u32, false);
        while p < pages {
            let mut o = json!({"transactionDetails": "full", "limit": 100, "sortOrder": "asc", "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1});
            if let Some(t) = &token {
                o["paginationToken"] = json!(t);
            }
            let r = match rpc.call("getTransactionsForAddress", json!([mint, o]), 10) {
                Ok(r) => r,
                Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                Err(_) => break,
            };
            p += 1;
            let data = r["data"].as_array().cloned().unwrap_or_default();
            for tx in &data {
                cv.feed(tx, &mint);
            }
            token = r["paginationToken"].as_str().map(String::from);
            if data.is_empty() || token.is_none() || cv.graduated {
                complete = true;
                break;
            }
        }
        let x_entry = cv.entry.filter(|e| *e > 0.0).map(|e| cv.peak / e);
        let row = json!({"key": key, "mint": mint, "symbol": c["symbol"], "name": name, "creator": creator, "hit": c["hit"],
            "t_ms": t_ms, "create_bt": cv.create_bt, "delay_ms": cv.create_bt.map(|b| t_ms - b * 1000),
            "n_trades": cv.n_trades, "complete": complete, "graduated": cv.graduated,
            "x_entry": x_entry, "x_launch": (cv.peak > 0.0).then(|| cv.peak * 1e6 / LAUNCH_SOL),
            "last_vs_entry": cv.entry.zip(cv.last).map(|(e, l)| l / e), "peak_after_s": cv.peak_bt.zip(cv.entry_bt).map(|(a, b)| a - b),
            "peak_mcap_sol": cv.peak * 1e6});
        f.write_all(format!("{row}\n").as_bytes()).map_err(|e| e.to_string())?;
    }
    rpc.save();
    let rows: Vec<Value> = read_jsonl(&path).into_iter().filter(|v| v["x_entry"].is_number()).collect();
    let xe: Vec<f64> = rows.iter().filter_map(|v| v["x_entry"].as_f64()).collect();
    let xl: Vec<f64> = rows.iter().filter_map(|v| v["x_launch"].as_f64()).collect();
    let mut d: Vec<i64> = rows.iter().filter_map(|v| v["delay_ms"].as_i64()).collect();
    d.sort();
    let bucket = |lo: f64, hi: f64| xe.iter().filter(|x| **x >= lo && **x < hi).count();
    println!("{}", json!({"coins": rows.len(), "graduated": rows.iter().filter(|v| v["graduated"] == true).count(),
        "rise_from_entry": stats(&xe), "rise_from_launch": stats(&xl),
        "entry_buckets": {"naik_<10%": bucket(0.0, 1.1), "10-100%": bucket(1.1, 2.0), "100-1000%": bucket(2.0, 11.0), ">=1000%": bucket(11.0, f64::MAX)},
        "delay_ms": if d.is_empty() { json!(null) } else { json!({"p10": d[d.len() / 10], "median": d[d.len() / 2], "p90": d[d.len() * 9 / 10]}) },
        "credits": rpc.used()}));
    Ok(())
}
