//! Quality dev seeds from MadeOnSol FREE endpoints (owner 21 Nov: take quality wallets, not random ones).
//!   /deployer-hunter/leaderboard  tier in (elite, good, moderate), sort total_bonded, 50 per page
//!   /deployer-hunter/recent-bonds peak_mc_min = min_peak_usd, 50 per call
//! Each row -> {dev, mint, peak_mc, tier, source} appended to <uni>/quality_devs.jsonl (devtrace reads it first).
//! Raw responses kept in <uni>/raw/madeonsol_quality.jsonl. Third-party list = a SEED for on-chain tracing only;
//! every dev is still traced and judged on chain. Waits for the daily quota (200/day) when it is used up.
//! Usage: jaringan mosq <universe_dir> <max_calls>
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

fn first_str<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v[*k].as_str().filter(|s| !s.is_empty()))
}
fn first_f64(v: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| v[*k].as_f64())
}

/// Rows of any list response: the first array found under common keys (or the body itself).
pub fn rows(body: &Value) -> Vec<Value> {
    for k in ["deployers", "leaderboard", "bonds", "tokens", "data", "results", "items"] {
        if let Some(a) = body[k].as_array() {
            return a.clone();
        }
    }
    body.as_array().cloned().unwrap_or_default()
}

/// Seed from one row: (dev, mint, peak, tier).
pub fn seed(r: &Value) -> Option<(String, String, f64, String)> {
    let dev = first_str(r, &["wallet_address", "deployer_wallet", "deployer", "creator", "wallet"])
        .or_else(|| r["deployer"]["wallet_address"].as_str())?;
    let mint = first_str(r, &["token_mint", "best_token_mint", "mint"]).unwrap_or("");
    let peak = first_f64(r, &["peak_market_cap", "best_token_peak_mc", "peak_mc", "avg_peak_mc"]).unwrap_or(0.0);
    let tier = first_str(r, &["tier", "deployer_tier"]).or_else(|| r["deployer"]["tier"].as_str()).unwrap_or("");
    Some((dev.to_string(), mint.to_string(), peak, tier.to_string()))
}

pub fn run(uni: &str, max_calls: u64) -> Result<(), String> {
    let key = fs::read_to_string(".madeonsol.env").map_err(|e| format!(".madeonsol.env: {e}"))?
        .lines().find_map(|l| l.strip_prefix("MADEONSOL_KEY=").map(|s| s.trim().to_string())).ok_or("MADEONSOL_KEY missing")?;
    let min_peak = crate::rpc::cfg_u64("config/devtrace.toml", "seed_min_peak_usd").unwrap_or(100_000);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(40)).build();
    fs::create_dir_all(format!("{uni}/raw")).map_err(|e| e.to_string())?;
    let out_p = format!("{uni}/quality_devs.jsonl");
    let mut have: HashSet<String> = fs::read_to_string(&out_p).unwrap_or_default().lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|v| v["dev"].as_str().map(String::from)).collect();
    let mut urls: Vec<(String, String)> = Vec::new();
    for tier in ["elite", "good", "moderate"] {
        for off in (0..500).step_by(50) {
            urls.push((format!("leaderboard:{tier}"), format!("/deployer-hunter/leaderboard?tier={tier}&sort=total_bonded&limit=50&offset={off}")));
        }
    }
    urls.push(("recent-bonds".into(), format!("/deployer-hunter/recent-bonds?limit=50&peak_mc_min={min_peak}")));
    let (mut calls, mut added) = (0u64, 0usize);
    let mut raw = OpenOptions::new().create(true).append(true).open(format!("{uni}/raw/madeonsol_quality.jsonl")).map_err(|e| e.to_string())?;
    let mut out = OpenOptions::new().create(true).append(true).open(&out_p).map_err(|e| e.to_string())?;
    let mut i = 0usize;
    while i < urls.len() && calls < max_calls {
        let (src, path) = &urls[i];
        sleep(Duration::from_millis(1500));
        match agent.get(&format!("https://madeonsol.com/api/v1{path}")).set("Authorization", &format!("Bearer {key}")).call() {
            Ok(r) => {
                calls += 1;
                let body: Value = r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
                writeln!(raw, "{}", json!({"path": path, "body": body})).map_err(|e| e.to_string())?;
                let rs = rows(&body);
                for r in &rs {
                    if let Some((dev, mint, peak, tier)) = seed(r) {
                        if have.insert(dev.clone()) {
                            out.write_all(format!("{}\n", json!({"dev": dev, "mint": mint, "peak_mc": peak, "tier": tier, "source": src})).as_bytes()).map_err(|e| e.to_string())?;
                            added += 1;
                        }
                    }
                }
                println!("{}", json!({"path": path, "rows": rs.len(), "added_total": added}));
                if rs.is_empty() && src.starts_with("leaderboard") {
                    // no more pages for this tier: skip its remaining offsets
                    let t = src.clone();
                    while i + 1 < urls.len() && urls[i + 1].0 == t {
                        i += 1;
                    }
                }
                i += 1;
            }
            Err(ureq::Error::Status(429, _)) => {
                println!("{}", json!({"quota": "used up, waiting 30 min"}));
                sleep(Duration::from_secs(1800));
            }
            Err(e) => {
                println!("{}", json!({"path": path, "error": e.to_string().chars().take(120).collect::<String>()}));
                i += 1;
            }
        }
    }
    println!("{}", json!({"done": true, "calls": calls, "devs_added": added}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeds_from_varied_rows() {
        let r = json!({"wallet_address": "D1", "best_token_peak_mc": 250000.0, "tier": "elite"});
        assert_eq!(seed(&r), Some(("D1".into(), "".into(), 250000.0, "elite".into())));
        let r = json!({"token_mint": "M", "peak_market_cap": 1.0e6, "deployer": {"wallet_address": "D2", "tier": "good"}});
        assert_eq!(seed(&r), Some(("D2".into(), "M".into(), 1.0e6, "good".into())));
        assert!(seed(&json!({"x": 1})).is_none());
        assert_eq!(rows(&json!({"deployers": [1, 2]})).len(), 2);
    }
}
