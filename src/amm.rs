//! P2 after graduation: find the PumpSwap pool of each graduated coin (CreatePoolEvent in the migrate tx) and pull
//! its trades (BuyEvent / SellEvent, layouts from the official pump_amm IDL) for `amm_horizon_s` after graduation.
//! Price = pool quote reserves / pool base reserves (real reserves in the event).
use crate::graph::read_lines;
use crate::rpc::{cfg_u64, issue, Rpc};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;

pub const PAMM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
const TAG: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
pub const PAMM_CREATE_POOL: [u8; 8] = [177, 49, 12, 210, 160, 118, 167, 116];
pub const BUY: [u8; 8] = [103, 244, 82, 31, 44, 245, 119, 119];
pub const SELL: [u8; 8] = [62, 47, 55, 10, 165, 3, 220, 42];

pub fn u64_at(d: &[u8], o: usize) -> Option<u64> {
    d.get(o..o + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
}
pub fn pk_at(d: &[u8], o: usize) -> Option<String> {
    d.get(o..o + 32).map(|s| bs58::encode(s).into_string())
}

/// pAMM events of one tx: (disc, body)
pub fn amm_events(tx: &Value) -> Vec<([u8; 8], Vec<u8>)> {
    let mut out = Vec::new();
    for g in tx["meta"]["innerInstructions"].as_array().into_iter().flatten() {
        for ix in g["instructions"].as_array().into_iter().flatten() {
            if ix["programId"] != PAMM {
                continue;
            }
            let Some(d) = ix["data"].as_str().and_then(|s| bs58::decode(s).into_vec().ok()) else { continue };
            if d.len() >= 16 && d[..8] == TAG {
                let mut disc = [0u8; 8];
                disc.copy_from_slice(&d[8..16]);
                out.push((disc, d[16..].to_vec()));
            }
        }
    }
    out
}

fn find_pool(rpc: &mut Rpc, curve: &str, grad_slot: u64) -> Result<Option<String>, String> {
    let r = rpc.call("getTransactionsForAddress", json!([curve, {"transactionDetails": "full", "limit": 20, "encoding": "jsonParsed",
        "maxSupportedTransactionVersion": 1, "sortOrder": "asc", "filters": {"status": "succeeded", "slot": {"gte": grad_slot}}}]), 10)?;
    for tx in r["data"].as_array().into_iter().flatten() {
        for (disc, b) in amm_events(tx) {
            if disc == PAMM_CREATE_POOL {
                // ts 8, index 2, creator 32, base 32, quote 32, dec 1, dec 1, 7 x u64 = 56, bump 1 -> pool at 165
                return Ok(pk_at(&b, 8 + 2 + 32 * 3 + 2 + 56 + 1));
            }
        }
    }
    Ok(None)
}

pub fn run(list: &str, out: &str) -> Result<(), String> {
    let horizon = cfg_u64("config/labels.toml", "amm_horizon_s").unwrap_or(86_400) as i64;
    let max_pages = std::env::var("JAR_AMM_MAX_PAGES").ok().and_then(|v| v.parse().ok()).or_else(|| cfg_u64("config/budget.toml", "amm_max_pages")).unwrap_or(10);
    let done: HashSet<String> = read_lines(out).iter().filter_map(|v| v["mint"].as_str().map(String::from)).collect();
    let items: Vec<(String, String, u64, i64)> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines().filter_map(|l| {
        let p: Vec<&str> = l.split(',').collect();
        Some((p.first()?.to_string(), p.get(1)?.to_string(), p.get(2)?.parse().ok()?, p.get(3)?.parse().ok()?))
    }).filter(|x| !done.contains(&x.0)).collect();
    let stem = std::path::Path::new(out).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut rpc = Rpc::new(&format!("amm_{stem}"))?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    println!("todo={}", items.len());
    for (mint, curve, gslot, gbt) in &items {
        let direct = curve.strip_prefix("pool=").map(String::from);
        let pool = match direct.map(|p| Ok(Some(p))).unwrap_or_else(|| find_pool(&mut rpc, curve, *gslot)) {
            Ok(Some(p)) => p,
            Ok(None) => {
                writeln!(f, "{}", json!({"mint": mint, "error": "no_pool"})).map_err(|e| e.to_string())?;
                continue;
            }
            Err(e) if e.starts_with("RPC") => {
                issue("amm_skip", json!({"mint": mint, "err": e}));
                continue;
            }
            Err(e) => return Err(e),
        };
        let mut token: Option<String> = None;
        let (mut pages, mut n_tx, mut reached) = (0u64, 0u64, false);
        let mut tr: Vec<Value> = Vec::new();
        loop {
            let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
                "sortOrder": "asc", "filters": {"status": "succeeded", "blockTime": {"gte": gbt - 5, "lte": gbt + horizon}}});
            if let Some(t) = &token {
                o["paginationToken"] = json!(t);
            }
            let mut r = rpc.call("getTransactionsForAddress", json!([pool, o]), 10)?;
            pages += 1;
            let data = r["data"].take();
            let data = data.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            for tx in data {
                n_tx += 1;
                let (slot, bt) = (tx["slot"].as_u64().unwrap_or(0), tx["blockTime"].as_i64().unwrap_or(0));
                for (disc, b) in amm_events(tx) {
                    let buy = disc == BUY;
                    if !buy && disc != SELL {
                        continue;
                    }
                    // ts 0, amt 8, limit 16, user_base 24, user_quote 32, pool_base 40, pool_quote 48, quote amount 56
                    // only events of THIS pool (router / arbitrage txs carry events of other pools)
                    if pk_at(&b, 112).as_deref() != Some(pool.as_str()) {
                        continue;
                    }
                    let (Some(base), Some(pb), Some(pq), Some(q)) = (u64_at(&b, 8), u64_at(&b, 40), u64_at(&b, 48), u64_at(&b, 56)) else { continue };
                    // fee bps: lp @64, protocol @80, coin creator @336 (idl/pump_amm.json Buy/SellEvent)
                    tr.push(json!([slot, bt, buy as u8, base, q, pb, pq, pk_at(&b, 144), u64_at(&b, 64), u64_at(&b, 80), u64_at(&b, 336)]));
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
        writeln!(f, "{}", json!({"mint": mint, "pool": pool, "gbt": gbt, "pages": pages, "n_tx": n_tx, "complete": reached, "t": tr})).map_err(|e| e.to_string())?;
    }
    rpc.save();
    println!("DONE credits_total={}", rpc.used());
    Ok(())
}

/// All graduations in a time window, cheaply: every migrate tx touches the Pump.fun migration withdraw authority.
/// Output per migration: {mint, pool, quote, slot, bt, sig}. 10 credits per 100 migrations.
pub fn migrations(start: i64, end: i64, out: &str) -> Result<(), String> {
    const WITHDRAW_AUTH: &str = "39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg";
    let mut rpc = Rpc::new("migrations")?;
    let mut f = OpenOptions::new().create(true).append(true).open(out).map_err(|e| e.to_string())?;
    let mut token: Option<String> = None;
    let (mut n, mut pages) = (0u64, 0u64);
    loop {
        let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
            "sortOrder": "asc", "filters": {"status": "succeeded", "blockTime": {"gte": start, "lt": end}}});
        if let Some(t) = &token {
            o["paginationToken"] = json!(t);
        }
        let r = rpc.call("getTransactionsForAddress", json!([WITHDRAW_AUTH, o]), 10)?;
        pages += 1;
        let data = r["data"].as_array().cloned().unwrap_or_default();
        for tx in &data {
            for (disc, b) in amm_events(tx) {
                if disc != PAMM_CREATE_POOL {
                    continue;
                }
                // ts 8, index 2, creator 32, base_mint @42, quote_mint @74, ..., pool @165
                writeln!(f, "{}", json!({"mint": pk_at(&b, 42), "quote": pk_at(&b, 74), "pool": pk_at(&b, 165), "slot": tx["slot"], "bt": tx["blockTime"],
                    "sig": tx["transaction"]["signatures"][0]})).map_err(|e| e.to_string())?;
                n += 1;
            }
        }
        token = r["paginationToken"].as_str().map(String::from);
        if data.is_empty() || token.is_none() {
            break;
        }
        if pages.is_multiple_of(20) {
            rpc.save();
            println!("pages={pages} migrations={n} credits_total={}", rpc.used());
        }
    }
    rpc.save();
    println!("DONE migrations={n} credits_total={}", rpc.used());
    Ok(())
}
