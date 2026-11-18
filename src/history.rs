//! Full history of a wallet from its FIRST transaction (gTFA ascending, up to `pages` x 100 tx):
//! every Pump.fun coin it created (as creator or signer), its own buys and sells of those coins (time, SOL),
//! and SOL in/out. Used to judge reputation from the start, not only inside our download window.
//! Output per wallet: {w, first_bt, last_bt, n_tx, complete, coins:[{mint, sym, bt, buy_sol, sell_sol, first_sell_s}], sol_in, sol_out}
use crate::decode::{self, Ev};
use crate::graph::read_lines;
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

pub fn run(list: &str, pages: u32, out: &str) -> Result<(), String> {
    let done: HashSet<String> = read_lines(out).iter().filter_map(|v| v["w"].as_str().map(String::from)).collect();
    let ws: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !done.contains(l)).collect();
    let stem = std::path::Path::new(out).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut rpc = Rpc::new(&format!("history_{stem}"))?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for w in &ws {
        let mut token: Option<String> = None;
        let (mut n_tx, mut first_bt, mut last_bt, mut complete) = (0u64, 0i64, 0i64, false);
        let (mut sol_in, mut sol_out) = (0u64, 0u64);
        // mint -> (symbol, create_bt, buy lamports, sell lamports, first sell time)
        let mut coins: BTreeMap<String, (String, i64, u64, u64, Option<i64>)> = BTreeMap::new();
        for _ in 0..pages {
            let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
                "sortOrder": "asc", "filters": {"status": "succeeded"}});
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
                n_tx += 1;
                let bt = tx["blockTime"].as_i64().unwrap_or(0);
                if first_bt == 0 {
                    first_bt = bt;
                }
                last_bt = bt;
                for (s, d, l) in decode::sol_moves(tx) {
                    if d == *w && s != *w {
                        sol_in += l;
                    } else if s == *w && d != *w {
                        sol_out += l;
                    }
                }
                for e in decode::events(tx).0 {
                    match e {
                        Ev::Create(c) if c.creator.as_deref() == Some(w.as_str()) || c.user == *w => {
                            coins.entry(c.mint.clone()).or_insert((c.symbol.chars().take(16).collect(), bt, 0, 0, None));
                        }
                        Ev::Trade(t) if t.user == *w => {
                            if let Some(e) = coins.get_mut(&t.mint) {
                                if t.buy {
                                    e.2 += t.sol;
                                } else {
                                    e.3 += t.sol;
                                    if e.4.is_none() {
                                        e.4 = Some(bt - e.1);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            token = r["paginationToken"].as_str().map(String::from);
            if data.is_empty() || token.is_none() {
                complete = true;
                break;
            }
        }
        let cs: Vec<Value> = coins.iter().map(|(m, (s, bt, b, sl, fs))| json!({"mint": m, "sym": s, "bt": bt, "buy_sol": *b as f64 / 1e9,
            "sell_sol": *sl as f64 / 1e9, "first_sell_s": fs})).collect();
        writeln!(f, "{}", json!({"w": w, "first_bt": first_bt, "last_bt": last_bt, "n_tx": n_tx, "complete": complete,
            "sol_in": sol_in as f64 / 1e9, "sol_out": sol_out as f64 / 1e9, "coins": cs})).map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE {} credits_total={}", ws.len(), rpc.used());
    Ok(())
}
