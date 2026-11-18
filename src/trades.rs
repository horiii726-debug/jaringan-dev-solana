//! P2.1: trades per coin on the bonding curve (gTFA on the curve account, oldest first, up to the horizon).
//! A coin whose page cap is hit before the horizon is marked censored at the last seen time.
use crate::decode::{self, Ev};
use crate::rpc::{cfg_u64, issue, Rpc};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;

/// One coin: input line "mint,curve,create_bt".
fn one(rpc: &mut Rpc, mint: &str, curve: &str, cbt: i64, horizon: i64, max_pages: u64) -> Result<Value, String> {
    let mut token: Option<String> = None;
    let mut tr: Vec<Value> = Vec::new();
    let (mut n_tx, mut n_failed, mut pages) = (0u64, 0u64, 0u64);
    let mut complete_slot: Option<u64> = None;
    let mut last_bt = cbt;
    let mut reached = false;
    loop {
        let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
            "sortOrder": "asc", "filters": {"status": "succeeded", "blockTime": {"gte": cbt - 5, "lte": cbt + horizon}}});
        if let Some(t) = &token {
            o["paginationToken"] = json!(t);
        }
        let mut r = rpc.call("getTransactionsForAddress", json!([curve, o]), 10)?;
        pages += 1;
        let data = r["data"].take();
        let data = data.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        for tx in data {
            n_tx += 1;
            let slot = tx["slot"].as_u64().unwrap_or(0);
            let txi = tx["transactionIndex"].as_u64().unwrap_or(0);
            let bt = tx["blockTime"].as_i64().unwrap_or(0);
            last_bt = last_bt.max(bt);
            if !tx["meta"]["err"].is_null() {
                n_failed += 1;
                continue;
            }
            let (evs, _) = decode::events(tx);
            for (ei, e) in evs.iter().enumerate() {
                match e {
                    Ev::Trade(t) if t.mint == mint => tr.push(json!([slot, txi, ei, bt, t.buy as u8, t.sol, t.tok, t.user, t.vsol, t.vtok,
                        t.fee_bps.unwrap_or(0) + t.cfee_bps.unwrap_or(0), t.mayhem.unwrap_or(false) as u8])),
                    Ev::Complete(m) if m == mint => complete_slot = complete_slot.or(Some(slot)),
                    _ => {}
                }
            }
        }
        token = r["paginationToken"].as_str().map(String::from);
        if data.is_empty() || token.is_none() {
            reached = true;
            break;
        }
        if pages >= max_pages {
            break;
        }
    }
    Ok(json!({"mint": mint, "cbt": cbt, "n_tx": n_tx, "n_failed": n_failed, "pages": pages, "complete_to_horizon": reached,
        "last_bt": last_bt, "graduated_slot": complete_slot, "t": tr}))
}

pub fn run(list: &str, out: &str) -> Result<(), String> {
    let horizon = cfg_u64("config/labels.toml", "horizon_s").ok_or("config/labels.toml horizon_s missing")? as i64;
    let max_pages = cfg_u64("config/budget.toml", "trades_max_pages").unwrap_or(5);
    let stem = std::path::Path::new(out).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut rpc = Rpc::new(&format!("trades_{stem}"))?;
    let mut done: HashSet<String> = HashSet::new();
    if let Ok(s) = fs::read_to_string(out) {
        let mut good = String::new();
        for l in s.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                done.insert(v["mint"].as_str().unwrap_or("").to_string());
                good.push_str(l);
                good.push('\n');
            }
        }
        fs::write(out, good).map_err(|e| e.to_string())?;
    }
    let items: Vec<(String, String, i64)> = fs::read_to_string(list)
        .map_err(|e| e.to_string())?
        .lines()
        .filter_map(|l| {
            let mut p = l.split(',');
            Some((p.next()?.to_string(), p.next()?.to_string(), p.next()?.trim().parse().ok()?))
        })
        .filter(|(m, _, _)| !done.contains(m))
        .collect();
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    println!("todo={} done={}", items.len(), done.len());
    for (i, (m, c, bt)) in items.iter().enumerate() {
        match one(&mut rpc, m, c, *bt, horizon, max_pages) {
            Ok(v) => writeln!(f, "{v}").map_err(|e| e.to_string())?,
            Err(e) if e.starts_with("RPC") => {
                issue("trades_skip", json!({"mint": m, "err": e}));
                writeln!(f, "{}", json!({"mint": m, "error": e})).map_err(|e| e.to_string())?;
            }
            Err(e) => {
                rpc.save();
                return Err(e);
            }
        }
        if (i + 1).is_multiple_of(200) {
            rpc.save();
            println!("{} / {} credits_total={}", i + 1, items.len(), rpc.used());
        }
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}
