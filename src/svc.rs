//! Funder classification (owner 18 Nov: never mistake an exchange / service transfer for a founder).
//! Helius paid labels (wallet-api identity / funded-by) are 403 on our plan, so the label is behavioural and every
//! label carries its evidence. Per wallet: newest <= 1000 signatures (activity rate), newest 100 full transactions
//! (SOL counterparties), balance.
//!   KNOWN_LIST        : in data/p3/exchange_list.txt (manually checked earlier)
//!   EXCHANGE_SERVICE  : >= 40 tx/h over its newest 1000 txs AND >= 30 distinct SOL counterparties in 100 txs
//!   BOT_HUB           : >= 40 tx/h, few counterparties (program-driven / router / bot)
//!   OPERATOR          : wallet that funds wallets but is not busy (candidate team funder)
//!   UNKNOWN           : no data
//! Founder links are allowed only through OPERATOR wallets.
use crate::decode;
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;

pub fn classify(tx_per_h: f64, n_sigs: usize, counterparties: usize, known: bool) -> (&'static str, String) {
    if known {
        return ("KNOWN_LIST", "in data/p3/exchange_list.txt".into());
    }
    if n_sigs == 0 {
        return ("UNKNOWN", "no signatures".into());
    }
    let busy = n_sigs >= 1000 && tx_per_h >= 40.0;
    match (busy, counterparties >= 30) {
        (true, true) => ("EXCHANGE_SERVICE", format!("{tx_per_h:.0} tx/h, {counterparties} SOL counterparties in 100 tx")),
        (true, false) => ("BOT_HUB", format!("{tx_per_h:.0} tx/h, {counterparties} SOL counterparties in 100 tx")),
        _ => ("OPERATOR", format!("{n_sigs} sigs, {tx_per_h:.1} tx/h, {counterparties} counterparties")),
    }
}

pub fn run(list: &str, out: &str) -> Result<(), String> {
    let known: HashSet<String> = fs::read_to_string("data/p3/exchange_list.txt").unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
    let done: HashSet<String> = crate::graph::read_lines(out).iter().filter_map(|v| v["w"].as_str().map(String::from)).collect();
    let ws: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !done.contains(l)).collect();
    let mut rpc = Rpc::new("svc")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    println!("todo={}", ws.len());
    for w in &ws {
        let sigs = rpc.call("getSignaturesForAddress", json!([w, {"limit": 1000}]), 1).map(|v| v.as_array().cloned().unwrap_or_default()).unwrap_or_default();
        let (newest, oldest) = (sigs.first().and_then(|s| s["blockTime"].as_i64()), sigs.last().and_then(|s| s["blockTime"].as_i64()));
        let span_h = match (newest, oldest) { (Some(a), Some(b)) => ((a - b) as f64 / 3600.0).max(1.0 / 60.0), _ => 1.0 };
        let rate = sigs.len() as f64 / span_h;
        let mut cps: HashSet<String> = HashSet::new();
        if !known.contains(w) && sigs.len() >= 1000 && rate >= 40.0 {
            let r = rpc.call("getTransactionsForAddress", json!([w, {"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1}]), 10).unwrap_or(Value::Null);
            for tx in r["data"].as_array().into_iter().flatten() {
                for (s, d, _) in decode::sol_moves(tx) {
                    if s == *w {
                        cps.insert(d);
                    } else if d == *w {
                        cps.insert(s);
                    }
                }
            }
        }
        let bal = rpc.call("getBalance", json!([w]), 1).ok().and_then(|v| v["value"].as_u64()).unwrap_or(0);
        let (label, why) = classify(rate, sigs.len(), cps.len(), known.contains(w));
        writeln!(f, "{}", json!({"w": w, "label": label, "why": why, "n_sigs": sigs.len(), "tx_per_h": rate, "counterparties": cps.len(),
            "balance_sol": bal as f64 / 1e9, "newest_bt": newest, "oldest_of_1000_bt": oldest})).map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}


/// Creator of graduated coins without launch data: PumpSwap Pool account field coin_creator (@211,
/// idl/pump_amm.json Pool: disc 8, bump 1, index 2, creator 32, base 32, quote 32, lp 32, base_ta 32, quote_ta 32, lp_supply 8).
/// Input: lines "mint<TAB>pool". Output: {mint, pool, coin_creator}. getMultipleAccounts, 100 pools per call.
pub fn pool_creators(list: &str, out: &str) -> Result<(), String> {
    use base64::Engine;
    let rows: Vec<(String, String)> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines()
        .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_string(), b.trim().to_string()))).collect();
    let mut rpc = Rpc::new("poolcreator")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    for chunk in rows.chunks(100) {
        let pools: Vec<&String> = chunk.iter().map(|r| &r.1).collect();
        let r = rpc.call("getMultipleAccounts", json!([pools, {"encoding": "base64"}]), 1)?;
        for (i, (m, p)) in chunk.iter().enumerate() {
            let cc = r["value"][i]["data"][0].as_str()
                .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
                .and_then(|d| d.get(211..243).map(|x| bs58::encode(x).into_string()));
            writeln!(f, "{}", json!({"mint": m, "pool": p, "coin_creator": cc})).map_err(|e| e.to_string())?;
        }
    }
    rpc.save();
    println!("DONE {} credits_total={}", rows.len(), rpc.used());
    Ok(())
}

/// Stage-2 roles from the create transaction itself: oldest successful tx of the mint (getTransactionsForAddress asc,
/// limit 1) -> CreateEvent creator / user, fee payer (account 0), all signers. Input: mint per line.
pub fn create_roles(list: &str, out: &str) -> Result<(), String> {
    let done: HashSet<String> = crate::graph::read_lines(out).iter().filter_map(|v| v["mint"].as_str().map(String::from)).collect();
    let mints: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !done.contains(l)).collect();
    let mut rpc = Rpc::new("roles")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    println!("todo={}", mints.len());
    for m in &mints {
        let r = match rpc.call("getTransactionsForAddress", json!([m, {"transactionDetails": "full", "limit": 1, "sortOrder": "asc", "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 1, "filters": {"status": "succeeded"}}]), 10) {
            Ok(r) => r,
            Err(e) if e.starts_with("BUDGET") => return Err(e),
            Err(e) => {
                writeln!(f, "{}", json!({"mint": m, "error": e.chars().take(120).collect::<String>()})).map_err(|e| e.to_string())?;
                continue;
            }
        };
        let Some(tx) = r["data"].as_array().and_then(|a| a.first()) else {
            writeln!(f, "{}", json!({"mint": m, "error": "no_tx"})).map_err(|e| e.to_string())?;
            continue;
        };
        let keys = tx["transaction"]["message"]["accountKeys"].as_array().cloned().unwrap_or_default();
        let signers: Vec<String> = keys.iter().filter(|k| k["signer"] == true).filter_map(|k| k["pubkey"].as_str().map(String::from)).collect();
        let fee_payer = keys.first().and_then(|k| k["pubkey"].as_str()).unwrap_or("").to_string();
        // launch venue = known launchpad program among the programs the create tx invokes (outer + inner)
        let mut progs: Vec<String> = tx["transaction"]["message"]["instructions"].as_array().into_iter().flatten().filter_map(|i| i["programId"].as_str().map(String::from)).collect();
        for g in tx["meta"]["innerInstructions"].as_array().into_iter().flatten() {
            progs.extend(g["instructions"].as_array().into_iter().flatten().filter_map(|i| i["programId"].as_str().map(String::from)));
        }
        let venue = launchpad(&progs);
        let ev = decode::events(tx).0.into_iter().find_map(|e| if let decode::Ev::Create(c) = e { if c.mint == *m { Some(c) } else { None } } else { None });
        let row = match ev {
            Some(c) => json!({"mint": m, "venue": venue, "sig": tx["transaction"]["signatures"][0], "slot": tx["slot"], "bt": tx["blockTime"], "creator": c.creator.clone().unwrap_or(c.user.clone()),
                "user": c.user, "curve": c.curve, "fee_payer": fee_payer, "signers": signers, "symbol": c.symbol, "evidence": "Pump CreateEvent in oldest tx of mint"}),
            // other launchpads: creator candidate = fee payer (signer 0); roles kept separate, evidence = oldest tx
            None => json!({"mint": m, "venue": venue, "sig": tx["transaction"]["signatures"][0], "slot": tx["slot"], "bt": tx["blockTime"], "creator": fee_payer,
                "fee_payer": fee_payer, "signers": signers, "evidence": "fee payer of oldest tx of mint (no Pump CreateEvent)"}),
        };
        writeln!(f, "{row}").map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}

/// Dev profile around one launch (owner 18 Nov: the first SOL sender is not automatically the controller; check
/// whether the dev is a one-time wallet; check the wallets behind it). Per (wallet, create_bt):
///   signatures (<= 1000) -> activity size and the oldest time seen (age, if < 1000 sigs = full history)
///   50 txs BEFORE the launch -> incoming SOL (sender, SOL, seconds before launch) and earlier Pump creates by it
///   50 txs AFTER the launch  -> outgoing SOL (recipient, SOL) = where the money goes
/// one_time = full history < 100 txs, first activity < 3 days before launch, no earlier create in the sample.
/// Input lines: wallet<TAB>create_bt<TAB>mint.
pub fn dev_profile(list: &str, out: &str) -> Result<(), String> {
    let done: HashSet<String> = crate::graph::read_lines(out).iter().filter_map(|v| v["mint"].as_str().map(String::from)).collect();
    let rows: Vec<(String, i64, String)> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((f.first()?.to_string(), f.get(1)?.parse().ok()?, f.get(2)?.to_string()))
    }).filter(|r| !done.contains(&r.2)).collect();
    let mut rpc = Rpc::new("devprof")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    let page = |rpc: &mut Rpc, w: &str, filt: Value, order: &str| -> Vec<Value> {
        rpc.call("getTransactionsForAddress", json!([w, {"transactionDetails": "full", "limit": 50, "sortOrder": order, "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 1, "filters": filt}]), 10).ok().and_then(|r| r["data"].as_array().cloned()).unwrap_or_default()
    };
    for (w, cbt, mint) in &rows {
        let sigs = rpc.call("getSignaturesForAddress", json!([w, {"limit": 1000}]), 1).map(|v| v.as_array().cloned().unwrap_or_default()).unwrap_or_default();
        let full = sigs.len() < 1000;
        let oldest = sigs.last().and_then(|s| s["blockTime"].as_i64());
        let before = page(&mut rpc, w, json!({"status": "succeeded", "blockTime": {"lt": cbt}}), "desc");
        let after = page(&mut rpc, w, json!({"status": "succeeded", "blockTime": {"gt": cbt}}), "asc");
        let mut inflow: Vec<Value> = Vec::new();
        let mut earlier_creates = 0;
        for tx in &before {
            let bt = tx["blockTime"].as_i64().unwrap_or(0);
            for (s, d, l) in decode::sol_moves(tx) {
                if d == *w && s != *w && l >= 10_000_000 {
                    inflow.push(json!({"from": s, "sol": l as f64 / 1e9, "s_before": cbt - bt}));
                }
            }
            earlier_creates += decode::events(tx).0.iter().filter(|e| matches!(e, decode::Ev::Create(c) if c.user == *w || c.creator.as_deref() == Some(w.as_str()))).count();
        }
        let mut outflow: Vec<Value> = Vec::new();
        for tx in &after {
            let bt = tx["blockTime"].as_i64().unwrap_or(0);
            for (s, d, l) in decode::sol_moves(tx) {
                if s == *w && d != *w && l >= 10_000_000 {
                    outflow.push(json!({"to": d, "sol": l as f64 / 1e9, "s_after": bt - cbt}));
                }
            }
        }
        let age_days = oldest.map(|o| (cbt - o) as f64 / 86_400.0);
        let one_time = full && sigs.len() < 100 && age_days.map(|a| a < 3.0).unwrap_or(false) && earlier_creates == 0;
        writeln!(f, "{}", json!({"mint": mint, "w": w, "create_bt": cbt, "n_sigs": sigs.len(), "history_complete": full, "age_days_at_launch": age_days,
            "earlier_creates_in_50tx": earlier_creates, "one_time_wallet": one_time, "inflow_before": inflow, "outflow_after": outflow})).map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE {} credits_total={}", rows.len(), rpc.used());
    Ok(())
}

/// Launchpad of a create tx from the programs it invokes (program ids from the official programs).
pub fn launchpad(progs: &[String]) -> &'static str {
    const PADS: [(&str, &str); 6] = [
        ("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P", "pump.fun"),
        ("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN", "meteora-dbc"),
        ("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj", "raydium-launchlab"),
        ("MoonCVVNZFSYkqNXP6bxHLPL6QQJiMagDL3qcqUQTrG", "moonshot"),
        ("boop8hVGQGqehUK2iVEMEnMrL5RbjywRzHKBmBE7ry4", "boop"),
        ("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA", "pumpswap-direct"),
    ];
    for (id, name) in PADS {
        if progs.iter().any(|p| p == id) {
            return name;
        }
    }
    "other"
}

/// Full launch history of a wallet (owner 18 Nov: count the failures too, not only the winners): reads the wallet's
/// transactions (getTransactionsForAddress, newest first, up to `max_pages` x 100) and lists every Pump.fun create
/// signed by it (CreateEvent user or creator == wallet). Output per wallet: {w, pages, complete, creates:[{mint, bt}]}.
pub fn dev_history(list: &str, max_pages: u32, out: &str) -> Result<(), String> {
    let done: HashSet<String> = crate::graph::read_lines(out).iter().filter_map(|v| v["w"].as_str().map(String::from)).collect();
    let ws: Vec<String> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !done.contains(l)).collect();
    let mut rpc = Rpc::new("devhist")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    println!("todo={}", ws.len());
    for w in &ws {
        let mut token: Option<String> = None;
        let (mut pages, mut complete) = (0u32, false);
        let mut creates: Vec<Value> = Vec::new();
        while pages < max_pages {
            let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1, "filters": {"status": "succeeded"}});
            if let Some(t) = &token {
                o["paginationToken"] = json!(t);
            }
            let r = match rpc.call("getTransactionsForAddress", json!([w, o]), 10) {
                Ok(r) => r,
                Err(e) if e.starts_with("BUDGET") => return Err(e),
                Err(_) => break,
            };
            pages += 1;
            let data = r["data"].as_array().cloned().unwrap_or_default();
            for tx in &data {
                for e in decode::events(tx).0 {
                    if let decode::Ev::Create(c) = e {
                        if c.user == *w || c.creator.as_deref() == Some(w.as_str()) {
                            creates.push(json!({"mint": c.mint, "bt": tx["blockTime"], "symbol": c.symbol}));
                        }
                    }
                }
            }
            token = r["paginationToken"].as_str().map(String::from);
            if data.is_empty() || token.is_none() {
                complete = true;
                break;
            }
        }
        writeln!(f, "{}", json!({"w": w, "pages": pages, "complete": complete, "n_creates": creates.len(), "creates": creates})).map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels() {
        assert_eq!(classify(500.0, 1000, 80, false).0, "EXCHANGE_SERVICE");
        assert_eq!(classify(500.0, 1000, 3, false).0, "BOT_HUB");
        assert_eq!(classify(0.5, 40, 5, false).0, "OPERATOR");
        assert_eq!(classify(0.5, 40, 5, true).0, "KNOWN_LIST");
        assert_eq!(classify(0.0, 0, 0, false).0, "UNKNOWN");
    }
}
