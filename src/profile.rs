//! Behaviour profile of a wallet from its newest 100 transactions (gTFA, 10 credits): is it an exchange-like
//! hot wallet, a team bank (funds devs and gets proceeds back), a trader (trades on Pump.fun/DEX itself),
//! or a bot/service. Classification is by measured behaviour only; no external labels.
use crate::decode::{self, Ev, SYSTEM};
use crate::graph::read_lines;
use crate::rpc::Rpc;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

pub fn run(list: &str, devs_file: &str, out: &str) -> Result<(), String> {
    let devs: HashSet<String> = fs::read_to_string(devs_file).unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
    let done: HashSet<String> = read_lines(out).iter().filter_map(|v| v["w"].as_str().map(String::from)).collect();
    let ws: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !done.contains(l)).collect();
    let mut rpc = Rpc::new("profile")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for w in &ws {
        let r = rpc.call("getTransactionsForAddress", json!([w, {"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 1, "filters": {"status": "succeeded"}}]), 10)?;
        let txs = r["data"].as_array().cloned().unwrap_or_default();
        let (mut out_n, mut in_n, mut pump_trades, mut signer_n) = (0u64, 0u64, 0u64, 0u64);
        let (mut out_sol, mut in_sol) = (0f64, 0f64);
        let mut to: HashSet<String> = HashSet::new();
        let mut from: HashSet<String> = HashSet::new();
        let mut progs: HashMap<String, u64> = HashMap::new();
        let (mut t_min, mut t_max) = (i64::MAX, i64::MIN);
        for tx in &txs {
            let bt = tx["blockTime"].as_i64().unwrap_or(0);
            t_min = t_min.min(bt);
            t_max = t_max.max(bt);
            let keys = tx["transaction"]["message"]["accountKeys"].as_array().cloned().unwrap_or_default();
            if keys.iter().any(|k| k["pubkey"] == w.as_str() && k["signer"] == true) {
                signer_n += 1;
            }
            for ix in tx["transaction"]["message"]["instructions"].as_array().into_iter().flatten() {
                if let Some(p) = ix["programId"].as_str() {
                    if p != SYSTEM && !p.starts_with("ComputeBudget") {
                        *progs.entry(p.to_string()).or_default() += 1;
                    }
                }
            }
            for (s, d, lam) in decode::sol_moves(tx) {
                if s == *w && d != *w {
                    out_n += 1;
                    out_sol += lam as f64 / 1e9;
                    to.insert(d);
                } else if d == *w && s != *w {
                    in_n += 1;
                    in_sol += lam as f64 / 1e9;
                    from.insert(s);
                }
            }
            let (evs, _) = decode::events(tx);
            pump_trades += evs.iter().filter(|e| matches!(e, Ev::Trade(t) if t.user == *w)).count() as u64;
        }
        let n = txs.len() as f64;
        let hours = if t_max > t_min { (t_max - t_min) as f64 / 3600.0 } else { 0.0 };
        let to_devs = to.iter().filter(|x| devs.contains(*x)).count();
        let from_devs = from.iter().filter(|x| devs.contains(*x)).count();
        let mut top: Vec<(String, u64)> = progs.into_iter().collect();
        top.sort_by_key(|x| std::cmp::Reverse(x.1));
        // behaviour class
        let tph = if hours > 0.0 { n / hours } else { 0.0 };
        let class = if n >= 100.0 && tph >= 150.0 && to.len() + from.len() >= 60 && pump_trades == 0 {
            "EXCHANGE_ATAU_LAYANAN"
        } else if pump_trades as f64 >= 0.3 * n {
            "TRADER_PUMP"
        } else if to_devs >= 2 && from_devs >= 1 {
            "BANK_TIM (danai dev + terima hasil balik)"
        } else if to_devs >= 2 {
            "PENDANA_DEV"
        } else if signer_n as f64 <= 0.2 * n {
            "PENERIMA_PASIF"
        } else {
            "LAINNYA"
        };
        let row = json!({"w": w, "tx": txs.len(), "hours": (hours * 10.0).round() / 10.0, "tx_per_hour": tph.round(), "signer": signer_n,
            "sol_out_n": out_n, "sol_out": out_sol.round(), "to_unique": to.len(), "to_devs": to_devs,
            "sol_in_n": in_n, "sol_in": in_sol.round(), "from_unique": from.len(), "from_devs": from_devs,
            "pump_trades": pump_trades, "top_programs": top.iter().take(3).map(|(p, c)| format!("{}:{c}", &p[..8.min(p.len())])).collect::<Vec<_>>(), "class": class});
        writeln!(f, "{row}").map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("profiled {} credits_total={}", ws.len(), rpc.used());
    Ok(())
}

