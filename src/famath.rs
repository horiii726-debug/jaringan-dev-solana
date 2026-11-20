//! ATH per family coin (owner 21 Nov: "coin yang dia bikin kenaikan tertinggi berapa persen, ATH market cap").
//! Source: MadeOnSol /deployer-hunter/{wallet}/tokens (FREE tier, 1 call per dev, raw response stored with date).
//! Third-party numbers: labelled SOURCE=MadeOnSol, cross-check only (V3 4.2), not on-chain verified.
//! Multiples: peak / mc_at_bond (from graduation, a buyable point) — never from the creation price (T13).
//! Usage: jaringan famath <family_dir> <out_dir>
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Per-coin summary from one MadeOnSol token row.
pub fn coin_row(t: &Value) -> Value {
    let peak = t["peak_market_cap"].as_f64();
    let bond = t["mc_at_bond"].as_f64();
    let mult = match (peak, bond) {
        (Some(p), Some(b)) if b > 0.0 => Some(p / b),
        _ => None,
    };
    json!({"mint": t["token_mint"], "symbol": t["token_symbol"], "deployed_at": t["deployed_at"], "bonded": t["bonded_at"].is_string(),
        "mc_at_bond": bond, "peak_market_cap": peak, "peak_over_bond_x": mult})
}

pub fn run(fam: &str, out: &str) -> Result<(), String> {
    let key = fs::read_to_string(".madeonsol.env").map_err(|e| format!(".madeonsol.env: {e}"))?
        .lines().find_map(|l| l.strip_prefix("MADEONSOL_KEY=").map(|s| s.trim().to_string())).ok_or("MADEONSOL_KEY missing")?;
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let raw_p = format!("{out}/madeonsol_raw.jsonl");
    let mut cache: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&raw_p) {
        if let Some(w) = v["dev"].as_str() {
            cache.insert(w.to_string(), v["body"].clone());
        }
    }
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(40)).build();
    let fams = read_jsonl(&format!("{fam}/families.jsonl"));
    let mut rf = OpenOptions::new().create(true).append(true).open(&raw_p).map_err(|e| e.to_string())?;
    let (mut calls, mut quota_left) = (0u64, None::<String>);
    let mut of = fs::File::create(format!("{out}/family_ath.jsonl")).map_err(|e| e.to_string())?;
    for f in &fams {
        let mut coins: Vec<Value> = Vec::new();
        for d in f["devs"].as_array().into_iter().flatten() {
            let Some(dev) = d["dev"].as_str() else { continue };
            if !cache.contains_key(dev) {
                sleep(Duration::from_millis(1100)); // burst limit 60/min
                let url = format!("https://madeonsol.com/api/v1/deployer-hunter/{dev}/tokens?limit=50");
                match agent.get(&url).set("Authorization", &format!("Bearer {key}")).call() {
                    Ok(r) => {
                        quota_left = r.header("x-ratelimit-remaining").map(String::from);
                        let body: Value = r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
                        writeln!(rf, "{}", json!({"dev": dev, "fetched": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_secs()).unwrap_or(0), "body": body})).map_err(|e| e.to_string())?;
                        cache.insert(dev.to_string(), body);
                        calls += 1;
                    }
                    Err(ureq::Error::Status(c, _)) => {
                        println!("{}", json!({"dev": dev, "http": c}));
                        if c == 429 {
                            break;
                        }
                        continue;
                    }
                    Err(e) => {
                        println!("{}", json!({"dev": dev, "error": e.to_string()}));
                        continue;
                    }
                }
            }
            for t in cache[dev]["tokens"].as_array().into_iter().flatten() {
                let mut c = coin_row(t);
                c["dev"] = json!(dev);
                coins.push(c);
            }
        }
        coins.sort_by(|a, b| b["peak_market_cap"].as_f64().unwrap_or(0.0).total_cmp(&a["peak_market_cap"].as_f64().unwrap_or(0.0)));
        let best = coins.first().cloned().unwrap_or(Value::Null);
        writeln!(of, "{}", json!({"root": f["root"], "n_wallets": f["n_wallets"], "n_devs": f["n_devs"], "n_coins_mos": coins.len(),
            "n_bonded": coins.iter().filter(|c| c["bonded"] == true).count(),
            "best_symbol": best["symbol"], "best_mint": best["mint"], "best_peak_mcap": best["peak_market_cap"], "best_mc_at_bond": best["mc_at_bond"],
            "best_peak_over_bond_x": best["peak_over_bond_x"], "source": "MadeOnSol deployer-hunter tokens (cross-check, not on-chain)", "coins": coins}))
            .map_err(|e| e.to_string())?;
    }
    println!("{}", json!({"families": fams.len(), "madeonsol_calls": calls, "quota_remaining": quota_left}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiple_from_bond_not_creation() {
        let r = coin_row(&json!({"token_mint": "M", "peak_market_cap": 500000.0, "mc_at_bond": 50000.0, "bonded_at": "x"}));
        assert_eq!(r["peak_over_bond_x"], 10.0);
        let r = coin_row(&json!({"token_mint": "M", "peak_market_cap": 9000.0, "mc_at_bond": null}));
        assert!(r["peak_over_bond_x"].is_null());
        assert_eq!(r["bonded"], false);
    }
}
