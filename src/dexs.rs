//! V3 Stage 1: DexScreener universe of NEW, ACTIVE Solana coins (owner 20 Nov 2026: no old coins, poll every
//! 5 minutes, trace devs of coins with market cap >= tier floor, bigger is better).
//!
//! Discovery sources (tagged per mint, V3 4.1 seed-bias rule):
//!   ONCHAIN_PUMP_MIGRATION : Pump.fun graduations seen on chain (amm::migrations, Helius, ~10 credits / 100)
//!   DS_PROMOTED:<path>     : DexScreener profiles / boosts / takeovers (paid promotion, not a random sample)
//! Every poll: all watched mints (age <= max_age_h) are queried with /tokens/v1 (<= 30 per call), raw responses
//! are stored, one aggregate row per mint is written, and a mint is QUALIFIED the first time it is active and its
//! marketCap (never FDV) is >= min_mcap_usd. Tier upgrades are logged too.
//! Usage: jaringan dexs <out_dir> <cycles>   (cycles = 0 -> run forever)
use crate::rpc::cfg_u64;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CFG: &str = "config/dexscreener.toml";

fn cfg_s(name: &str) -> Option<String> {
    let t = fs::read_to_string(CFG).ok()?;
    t.lines().find_map(|l| {
        let l = l.split('#').next()?.trim();
        let (k, v) = l.split_once('=')?;
        (k.trim() == name).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn now_s() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn tiers() -> Vec<f64> {
    let t = fs::read_to_string("config/tiers.toml").unwrap_or_default();
    t.lines()
        .find_map(|l| l.trim().strip_prefix("tiers_usd").map(|r| r.trim_start_matches([' ', '=']).trim().trim_matches('"').to_string()))
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.trim().replace('_', "").parse().ok())
        .collect()
}

/// Highest tier <= v (0 if below all tiers).
pub fn tier_of(v: f64, tiers: &[f64]) -> f64 {
    tiers.iter().copied().filter(|t| v >= *t).fold(0.0, f64::max)
}

struct Http {
    agent: ureq::Agent,
    last: Instant,
    gap: Duration,
}

impl Http {
    fn new(rpm: u64) -> Self {
        Http {
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(40)).build(),
            last: Instant::now() - Duration::from_secs(60),
            gap: Duration::from_millis(60_000 / rpm.max(1)),
        }
    }
    /// GET with rate limit, retry and backoff (429 -> wait 60 s). Err after 4 attempts.
    fn get(&mut self, url: &str) -> Result<Value, String> {
        let mut err = String::new();
        for attempt in 0..4u32 {
            let since = self.last.elapsed();
            if since < self.gap {
                sleep(self.gap - since);
            }
            self.last = Instant::now();
            match self.agent.get(url).call() {
                Ok(r) => match r.into_string().map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string())) {
                    Ok(v) => return Ok(v),
                    Err(e) => err = format!("parse: {e}"),
                },
                Err(ureq::Error::Status(429, _)) => {
                    err = "http 429".into();
                    sleep(Duration::from_secs(60));
                    continue;
                }
                Err(e) => err = e.to_string().chars().take(160).collect(),
            }
            sleep(Duration::from_secs(2u64.pow(attempt + 1)));
        }
        Err(err)
    }
}

/// One aggregate row per mint from a /tokens/v1 response (pairs deduplicated by pairAddress; only pairs where the
/// mint is the base token). marketCap and fdv kept apart; fdv_proxy = true when no pair has marketCap.
pub fn aggregate(mint: &str, pairs: &[Value], t: i64) -> Option<Value> {
    let mut seen = HashSet::new();
    let mut ps: Vec<&Value> = Vec::new();
    for p in pairs {
        if p["chainId"] == "solana" && p["baseToken"]["address"] == mint && seen.insert(p["pairAddress"].as_str().unwrap_or("").to_string()) {
            ps.push(p);
        }
    }
    if ps.is_empty() {
        return None;
    }
    let f = |p: &Value, k: &str| p[k].as_f64();
    let liq = |p: &Value| p["liquidity"]["usd"].as_f64().unwrap_or(0.0);
    let top = ps.iter().copied().max_by(|a, b| liq(a).total_cmp(&liq(b)))?;
    let mcap = ps.iter().filter_map(|p| f(p, "marketCap")).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |x| x.max(v))));
    let fdv = ps.iter().filter_map(|p| f(p, "fdv")).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |x| x.max(v))));
    let tx = |p: &Value, w: &str| p["txns"][w]["buys"].as_u64().unwrap_or(0) + p["txns"][w]["sells"].as_u64().unwrap_or(0);
    let created = ps.iter().filter_map(|p| p["pairCreatedAt"].as_i64()).min();
    let mut dex: Vec<String> = ps.iter().filter_map(|p| p["dexId"].as_str().map(String::from)).collect();
    dex.sort();
    dex.dedup();
    Some(json!({
        "t": t, "mint": mint, "sym": top["baseToken"]["symbol"], "n_pairs": ps.len(), "dex": dex,
        "top_pair": top["pairAddress"], "top_dex": top["dexId"], "top_quote": top["quoteToken"]["address"],
        "market_cap": mcap, "fdv": fdv, "fdv_proxy": mcap.is_none(),
        "liq_usd": ps.iter().map(|p| liq(p)).sum::<f64>(),
        "price_usd": top["priceUsd"].as_str().and_then(|s| s.parse::<f64>().ok()),
        "txns_m5": ps.iter().map(|p| tx(p, "m5")).sum::<u64>(), "txns_h1": ps.iter().map(|p| tx(p, "h1")).sum::<u64>(),
        "vol_h1": ps.iter().filter_map(|p| p["volume"]["h1"].as_f64()).sum::<f64>(),
        "vol_h24": ps.iter().filter_map(|p| p["volume"]["h24"].as_f64()).sum::<f64>(),
        "pair_created_ms": created,
    }))
}

fn append(path: &str, v: &Value) -> Result<(), String> {
    let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    writeln!(f, "{v}").map_err(|e| e.to_string())
}

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

pub fn run(out: &str, cycles: u64) -> Result<(), String> {
    let base = cfg_s("base").ok_or("config/dexscreener.toml: base missing")?;
    let tokens_path = cfg_s("tokens_path").ok_or("tokens_path missing")?;
    let promoted: Vec<String> = cfg_s("promoted").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let batch = cfg_u64(CFG, "batch").unwrap_or(30).clamp(1, 30) as usize;
    let poll_s = cfg_u64(CFG, "poll_s").unwrap_or(300);
    let max_age_s = cfg_u64(CFG, "max_age_h").ok_or("max_age_h missing")? as i64 * 3600;
    let min_mcap = cfg_u64(CFG, "min_mcap_usd").ok_or("min_mcap_usd missing")? as f64;
    let min_txns_h1 = cfg_u64(CFG, "min_txns_h1").unwrap_or(1);
    let tiers = tiers();
    if tiers.is_empty() {
        return Err("config/tiers.toml: tiers_usd missing".into());
    }
    let mut tok_http = Http::new(cfg_u64(CFG, "rpm_tokens").unwrap_or(240));
    let mut meta_http = Http::new(cfg_u64(CFG, "rpm_meta").unwrap_or(50));
    fs::create_dir_all(format!("{out}/raw")).map_err(|e| e.to_string())?;
    let (reg_p, snap_p, q_p, mig_p, log_p) =
        (format!("{out}/registry.jsonl"), format!("{out}/snap.jsonl"), format!("{out}/qualified.jsonl"), format!("{out}/migrations.jsonl"), format!("{out}/runs.jsonl"));

    // restart: rebuild state from files (no gap, no duplicates)
    let mut reg: HashMap<String, (i64, String)> = HashMap::new(); // mint -> (first_seen, source)
    for v in read_jsonl(&reg_p) {
        if let Some(m) = v["mint"].as_str() {
            reg.entry(m.to_string()).or_insert((v["t"].as_i64().unwrap_or(0), v["source"].as_str().unwrap_or("").to_string()));
        }
    }
    let mut best_tier: HashMap<String, f64> = HashMap::new();
    for v in read_jsonl(&q_p) {
        if let (Some(m), Some(t)) = (v["mint"].as_str(), v["tier"].as_f64()) {
            let e = best_tier.entry(m.to_string()).or_insert(0.0);
            *e = e.max(t);
        }
    }
    let mut created: HashMap<String, i64> = HashMap::new(); // mint -> pair created (s), for the age rule
    let snap_every = cfg_u64(CFG, "snap_every_s").unwrap_or(3600) as i64;
    let snap_min = cfg_u64(CFG, "snap_min_mcap_usd").unwrap_or(10_000) as f64;
    let mut last_snap: HashMap<String, i64> = HashMap::new();
    let mut raw_seen: HashSet<String> = HashSet::new();
    for v in read_jsonl(&snap_p) {
        if let (Some(m), Some(t)) = (v["mint"].as_str(), v["t"].as_i64()) {
            let e = last_snap.entry(m.to_string()).or_insert(0);
            *e = (*e).max(t);
            if let Some(c) = v["pair_created_ms"].as_i64() {
                created.insert(m.to_string(), c / 1000);
            }
        }
    }
    for l in fs::read_to_string(format!("{out}/raw/tokens.jsonl")).unwrap_or_default().lines() {
        // old format rows hold "mints" (batch); new rows hold "mint"
        if let Ok(v) = serde_json::from_str::<Value>(l) {
            if let Some(m) = v["mint"].as_str() {
                raw_seen.insert(m.to_string());
            }
            for m in v["mints"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                raw_seen.insert(m.to_string());
            }
        }
    }
    let mut mig_from = read_jsonl(&mig_p).iter().filter_map(|v| v["bt"].as_i64()).max().map(|b| b + 1).unwrap_or(now_s() - max_age_s);

    let mut cycle = 0u64;
    loop {
        cycle += 1;
        let t0 = now_s();
        let mut errors = 0u64;
        // 1. on-chain Pump.fun graduations since the last one stored
        let mig_to = now_s() - 30;
        if mig_to > mig_from {
            match crate::amm::migrations(mig_from, mig_to, &mig_p) {
                Ok(()) => mig_from = mig_to,
                Err(e) => {
                    errors += 1;
                    append(&log_p, &json!({"t": now_s(), "error": format!("migrations: {e}")}))?;
                }
            }
        }
        for v in read_jsonl(&mig_p) {
            if let (Some(m), Some(bt)) = (v["mint"].as_str(), v["bt"].as_i64()) {
                if !reg.contains_key(m) {
                    reg.insert(m.to_string(), (bt, "ONCHAIN_PUMP_MIGRATION".into()));
                    append(&reg_p, &json!({"mint": m, "t": bt, "source": "ONCHAIN_PUMP_MIGRATION", "sig": v["sig"], "slot": v["slot"]}))?;
                }
            }
        }
        // 2. DexScreener promoted lists (chainId solana only)
        for p in &promoted {
            match meta_http.get(&format!("{base}{p}")) {
                Ok(v) => {
                    append(&format!("{out}/raw/promoted.jsonl"), &json!({"t": now_s(), "path": p, "body": v}))?;
                    let arr = v.as_array().cloned().unwrap_or_default();
                    for it in arr {
                        if it["chainId"] != "solana" {
                            continue;
                        }
                        if let Some(m) = it["tokenAddress"].as_str() {
                            if !reg.contains_key(m) {
                                let src = format!("DS_PROMOTED:{p}");
                                reg.insert(m.to_string(), (now_s(), src.clone()));
                                append(&reg_p, &json!({"mint": m, "t": now_s(), "source": src}))?;
                            }
                        }
                    }
                }
                Err(e) => {
                    errors += 1;
                    append(&log_p, &json!({"t": now_s(), "error": format!("{p}: {e}")}))?;
                }
            }
        }
        // 3. watch set: registered mints that are not known to be older than max_age
        let now = now_s();
        let mut watch: Vec<String> = reg
            .iter()
            .filter(|(m, (first, _))| match created.get(*m) {
                Some(c) => now - c <= max_age_s,
                None => now - first <= max_age_s,
            })
            .map(|(m, _)| m.clone())
            .collect();
        watch.sort();
        let (mut n_rows, mut n_missing, mut n_new_q) = (0u64, 0u64, 0u64);
        for chunk in watch.chunks(batch) {
            let url = format!("{base}{tokens_path}{}", chunk.join(","));
            let v = match tok_http.get(&url) {
                Ok(v) => v,
                Err(e) => {
                    errors += 1;
                    append(&log_p, &json!({"t": now_s(), "error": format!("tokens: {e}"), "n": chunk.len()}))?;
                    continue;
                }
            };
            let t = now_s();
            let pairs = v.as_array().cloned().unwrap_or_default();
            for m in chunk {
                let Some(a) = aggregate(m, &pairs, t) else {
                    n_missing += 1;
                    continue;
                };
                n_rows += 1;
                let first_sight = !created.contains_key(m) && !raw_seen.contains(m);
                if let Some(c) = a["pair_created_ms"].as_i64() {
                    created.insert(m.clone(), c / 1000);
                }
                let src = reg.get(m).map(|r| r.1.clone()).unwrap_or_default();
                let mut row = a.clone();
                row["source"] = json!(src);
                // disk budget (16 GB VPS): raw pairs of a mint at first sight and at every qualification; snapshot
                // rows at most every snap_every_s per mint and only for mints with marketCap >= snap_min_mcap_usd
                let mine: Vec<&Value> = pairs.iter().filter(|p| p["baseToken"]["address"] == m.as_str()).collect();
                if first_sight {
                    raw_seen.insert(m.clone());
                    append(&format!("{out}/raw/tokens.jsonl"), &json!({"t": t, "mint": m, "why": "first_sight", "pairs": mine}))?;
                }
                let mc_now = a["market_cap"].as_f64().unwrap_or(0.0);
                if mc_now >= snap_min && t - last_snap.get(m).copied().unwrap_or(0) >= snap_every {
                    last_snap.insert(m.clone(), t);
                    append(&snap_p, &row)?;
                }
                let age_ok = a["pair_created_ms"].as_i64().map(|c| t - c / 1000 <= max_age_s).unwrap_or(false);
                let active = a["txns_h1"].as_u64().unwrap_or(0) >= min_txns_h1;
                if let (Some(mc), true, true) = (a["market_cap"].as_f64(), age_ok, active) {
                    let tier = tier_of(mc, &tiers);
                    if mc >= min_mcap && tier > *best_tier.get(m).unwrap_or(&0.0) {
                        best_tier.insert(m.clone(), tier);
                        row["tier"] = json!(tier);
                        append(&q_p, &row)?;
                        append(&format!("{out}/raw/tokens.jsonl"), &json!({"t": t, "mint": m, "why": "qualified", "pairs": mine}))?;
                        n_new_q += 1;
                    }
                }
            }
        }
        let tally: BTreeMap<String, usize> = reg.values().fold(BTreeMap::new(), |mut b, (_, s)| {
            *b.entry(s.split(':').next().unwrap_or("").to_string()).or_insert(0) += 1;
            b
        });
        let rec = json!({"t": now_s(), "cycle": cycle, "secs": now_s() - t0, "registry": reg.len(), "by_source": tally, "watched": watch.len(),
            "rows": n_rows, "not_on_ds": n_missing, "new_qualified_or_tier_up": n_new_q, "qualified_mints": best_tier.len(), "errors": errors});
        append(&log_p, &rec)?;
        println!("{rec}");
        if cycles != 0 && cycle >= cycles {
            return Ok(());
        }
        let spent = (now_s() - t0) as u64;
        sleep(Duration::from_secs(poll_s.saturating_sub(spent).max(5)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pair(addr: &str, base: &str, mc: Option<f64>, fdv: f64, liq: f64, m5: u64) -> Value {
        let mut p = json!({"chainId": "solana", "dexId": "pumpswap", "pairAddress": addr, "baseToken": {"address": base, "symbol": "X"},
            "quoteToken": {"address": "So11111111111111111111111111111111111111112"}, "fdv": fdv, "liquidity": {"usd": liq},
            "txns": {"m5": {"buys": m5, "sells": 0}, "h1": {"buys": m5, "sells": 1}}, "volume": {"h1": 10.0, "h24": 20.0}, "pairCreatedAt": 1_000_000});
        if let Some(m) = mc {
            p["marketCap"] = json!(m);
        }
        p
    }
    #[test]
    fn dedup_pairs_and_keep_mcap_fdv_apart() {
        let ps = vec![pair("P1", "M", Some(60_000.0), 70_000.0, 5_000.0, 2), pair("P1", "M", Some(60_000.0), 70_000.0, 5_000.0, 2), pair("P2", "M", Some(61_000.0), 71_000.0, 1_000.0, 1), pair("P3", "OTHER", Some(9e9), 9e9, 9e9, 9)];
        let a = aggregate("M", &ps, 5).expect("row");
        assert_eq!(a["n_pairs"], 2);
        assert_eq!(a["market_cap"], 61_000.0);
        assert_eq!(a["fdv"], 71_000.0);
        assert_eq!(a["fdv_proxy"], false);
        assert_eq!(a["liq_usd"], 6_000.0);
        assert_eq!(a["top_pair"], "P1");
        assert_eq!(a["txns_m5"], 3);
    }
    #[test]
    fn fdv_never_used_as_mcap() {
        let a = aggregate("M", &[pair("P1", "M", None, 90_000.0, 5_000.0, 1)], 5).expect("row");
        assert!(a["market_cap"].is_null());
        assert_eq!(a["fdv_proxy"], true);
        assert!(aggregate("M", &[pair("P1", "Z", None, 1.0, 1.0, 1)], 5).is_none());
    }
    #[test]
    fn tier_floor() {
        let t = [50_000.0, 100_000.0, 1_000_000.0];
        assert_eq!(tier_of(49_999.0, &t), 0.0);
        assert_eq!(tier_of(50_000.0, &t), 50_000.0);
        assert_eq!(tier_of(999_999.0, &t), 100_000.0);
        assert_eq!(tier_of(5e6, &t), 1_000_000.0);
    }
}
