//! Quality dev ranking (owner 21 Nov 2026 definition): a quality dev's coins rise hundreds to thousands of percent,
//! it rarely rugs, rarely sells within 60 s, has many moonshots in its history, and keeps launching new coins.
//! Inputs: devtrace history (coins, dev first-sell time, fast_dump) + coinrise per-coin rise (reachable entry -> peak).
//! Rule (config/quality.toml, owner-defined, PROVISIONAL numbers set before seeing the ranking):
//!   coins_measured >= min_coins, rug_rate <= max_rug_pct, fast_sell_rate (<= fast_s) <= max_fast_pct,
//!   moonshots (rise >= moon_x) >= min_moons, last create within active_days.
//! Output: <out>/quality_devs.tsv (all devs with metrics, quality flag first) + summary json on stdout.
//! Usage: jaringan qdev <dev_dir> <rise_dir> <out_dir>
use crate::rpc::cfg_u64;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Owner rule v2 (21 Nov): "dump" = the dev sold >= 90% of what it bought; early rug window = 1 hour.
/// (Kept as constants mirrored in config/quality.toml version 2 for the record.)
pub const DUMP_SHARE: f64 = 0.9;
pub const RUG_WINDOW_S: i64 = 3600;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct M {
    pub coins: usize,
    pub measured: usize,
    pub rugs: usize,
    pub fast: usize,
    pub moons: usize,
    pub ge100: usize,
    pub median_x: f64,
    pub max_x: f64,
    pub last_bt: i64,
}

/// Metrics of one dev from its coins (devtrace) and the per-coin rise rows (by mint).
pub fn metrics(coins: &[Value], rise: &HashMap<String, Value>, fast_s: i64, moon_x: f64) -> M {
    let mut m = M { coins: coins.len(), ..Default::default() };
    let mut xs = Vec::new();
    for c in coins {
        m.last_bt = m.last_bt.max(c["bt"].as_i64().unwrap_or(0));
        let first_sell = c["dev_first_sell_after_s"].as_i64();
        // v2 (owner 21 Nov): selling within fast_s is fine unless the dev dumps (sold share >= DUMP_SHARE) right there
        if first_sell.is_some_and(|s| s <= fast_s) && c["dev_sold_share"].as_f64().unwrap_or(0.0) >= DUMP_SHARE {
            m.fast += 1;
        }
        let Some(r) = c["mint"].as_str().and_then(|k| rise.get(k)) else { continue };
        let Some(x) = r["rise_x"].as_f64() else { continue };
        m.measured += 1;
        // v2 (owner 21 Nov): a rug counts only when the dev was already selling within the first RUG_WINDOW_S
        if r["rug"] == true && first_sell.is_some_and(|s| s <= RUG_WINDOW_S) {
            m.rugs += 1;
        }
        if x >= moon_x {
            m.moons += 1;
        }
        if x >= 2.0 {
            m.ge100 += 1;
        }
        xs.push(x);
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    if !xs.is_empty() {
        m.median_x = xs[xs.len() / 2];
        m.max_x = xs[xs.len() - 1];
    }
    m
}

/// Newest create time per creator from devtrace roles (the coin a dev was found by may be missing from a cut history).
pub fn role_times(dev_dir: &str) -> HashMap<String, i64> {
    let mut m: HashMap<String, i64> = HashMap::new();
    for r in read_jsonl(&format!("{dev_dir}/roles.jsonl")) {
        if let (Some(c), Some(bt)) = (r["creator"].as_str(), r["bt"].as_i64()) {
            let e = m.entry(c.to_string()).or_insert(bt);
            *e = (*e).max(bt);
        }
    }
    m
}

pub struct Rule {
    pub min_coins: usize,
    pub max_rug: f64,
    pub max_fast: f64,
    pub min_moons: usize,
    pub active_s: i64,
}

/// Quality verdict and the first failed condition (for the table).
pub fn verdict(m: &M, r: &Rule, now: i64) -> (bool, &'static str) {
    if m.measured < r.min_coins {
        return (false, "too_few_coins");
    }
    if m.rugs as f64 > r.max_rug * m.measured as f64 {
        return (false, "rugs_often");
    }
    if m.fast as f64 > r.max_fast * m.coins.max(1) as f64 {
        return (false, "sells_fast");
    }
    if m.moons < r.min_moons {
        return (false, "few_moonshots");
    }
    if now - m.last_bt > r.active_s {
        return (false, "not_active");
    }
    (true, "QUALITY")
}

/// Family level (owner: a dev that switches wallets but stays linked to the same induk is the same operator): all
/// coins of all devs in one family are judged together. `fam_file` = families.jsonl (fam2 / unified format).
pub fn run_family(dev_dir: &str, rise_dir: &str, fam_file: &str, out: &str) -> Result<(), String> {
    let c = |k: &str| cfg_u64("config/quality.toml", k).ok_or(format!("config/quality.toml: {k} missing"));
    let rule = Rule { min_coins: c("min_coins")? as usize, max_rug: c("max_rug_pct")? as f64 / 100.0, max_fast: c("max_fast_pct")? as f64 / 100.0,
        min_moons: c("min_moons")? as usize, active_s: c("active_days")? as i64 * 86_400 };
    let (fast_s, moon_x) = (c("fast_s")? as i64, c("moon_pct")? as f64 / 100.0 + 1.0);
    let mut rise: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{rise_dir}/coins.jsonl")) {
        if let Some(m) = v["mint"].as_str() {
            rise.insert(m.to_string(), v);
        }
    }
    let mut devs: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            devs.insert(d.to_string(), v);
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let role_bt = role_times(dev_dir);
    let mut rows: Vec<(bool, usize, f64, String)> = Vec::new();
    let mut why: HashMap<&str, usize> = HashMap::new();
    for f in read_jsonl(fam_file) {
        let ds: Vec<String> = f["devs"].as_array().into_iter().flatten().filter_map(|d| d["dev"].as_str().or(d.as_str()).map(String::from)).collect();
        let mut coins: Vec<Value> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for d in &ds {
            for c in devs.get(d).and_then(|v| v["coins"].as_array()).into_iter().flatten() {
                if seen.insert(c["mint"].as_str().unwrap_or("").to_string()) {
                    coins.push(c.clone());
                }
            }
        }
        if coins.is_empty() {
            continue;
        }
        let mut m = metrics(&coins, &rise, fast_s, moon_x);
        m.last_bt = ds.iter().filter_map(|d| role_bt.get(d).copied()).fold(m.last_bt, i64::max);
        let (ok, w) = verdict(&m, &rule, now);
        *why.entry(w).or_insert(0) += 1;
        let root = f["root"].as_str().or(f["family_id"].as_str()).unwrap_or("");
        let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{}\t{}", if ok { "BERKUALITAS" } else { "-" }, w, root,
            f["n_wallets"].as_u64().unwrap_or(0), ds.len(), m.coins, m.measured, m.moons, m.ge100, m.rugs, (m.median_x - 1.0) * 100.0,
            (m.max_x - 1.0) * 100.0, m.fast, (now - m.last_bt) / 3600);
        rows.push((ok, m.moons, m.max_x, line));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.total_cmp(&a.2)));
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let head = "status\talasan\tinduk\twallet\tdev\tkoin\tkoin_terukur\tmoonshot\tnaik_ge100pct\trug\tmedian_naik_pct\tmax_naik_pct\tjual_le60s\tjam_sejak_koin_terakhir";
    let body: Vec<String> = rows.iter().map(|r| r.3.clone()).collect();
    fs::write(format!("{out}/quality_families.tsv"), format!("{head}\n{}\n", body.join("\n"))).map_err(|e| e.to_string())?;
    println!("{}", json!({"families_with_coins": rows.len(), "quality": rows.iter().filter(|r| r.0).count(), "reasons": why}));
    Ok(())
}

pub fn run(dev_dir: &str, rise_dir: &str, out: &str) -> Result<(), String> {
    let c = |k: &str| cfg_u64("config/quality.toml", k).ok_or(format!("config/quality.toml: {k} missing"));
    let rule = Rule { min_coins: c("min_coins")? as usize, max_rug: c("max_rug_pct")? as f64 / 100.0, max_fast: c("max_fast_pct")? as f64 / 100.0,
        min_moons: c("min_moons")? as usize, active_s: c("active_days")? as i64 * 86_400 };
    let (fast_s, moon_x) = (c("fast_s")? as i64, c("moon_pct")? as f64 / 100.0 + 1.0);
    let mut rise: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{rise_dir}/coins.jsonl")) {
        if let Some(m) = v["mint"].as_str() {
            rise.insert(m.to_string(), v);
        }
    }
    let mut devs: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            devs.insert(d.to_string(), v);
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let role_bt = role_times(dev_dir);
    let mut rows: Vec<(bool, usize, f64, String)> = Vec::new();
    let mut why: HashMap<&str, usize> = HashMap::new();
    for (d, v) in &devs {
        let coins = v["coins"].as_array().cloned().unwrap_or_default();
        let mut m = metrics(&coins, &rise, fast_s, moon_x);
        m.last_bt = role_bt.get(d).copied().unwrap_or(0).max(m.last_bt);
        let (ok, w) = verdict(&m, &rule, now);
        *why.entry(w).or_insert(0) += 1;
        let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{}\t{}", if ok { "BERKUALITAS" } else { "-" }, w, d, m.coins, m.measured,
            m.moons, m.ge100, m.rugs, (m.median_x - 1.0) * 100.0, (m.max_x - 1.0) * 100.0, m.fast, (now - m.last_bt) / 3600);
        rows.push((ok, m.moons, m.max_x, line));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.total_cmp(&a.2)));
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let head = "status\talasan\tdev\tkoin\tkoin_terukur\tmoonshot\tnaik_ge100pct\trug\tmedian_naik_pct\tmax_naik_pct\tjual_le60s\tjam_sejak_koin_terakhir";
    let body: Vec<String> = rows.iter().map(|r| r.3.clone()).collect();
    fs::write(format!("{out}/quality_devs.tsv"), format!("{head}\n{}\n", body.join("\n"))).map_err(|e| e.to_string())?;
    let n_ok = rows.iter().filter(|r| r.0).count();
    println!("{}", json!({"devs": devs.len(), "quality": n_ok, "reasons": why}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn coin(m: &str, bt: i64, sell: Option<i64>) -> Value {
        json!({"mint": m, "bt": bt, "dev_first_sell_after_s": sell})
    }
    #[test]
    fn quality_rule() {
        let mut rise = HashMap::new();
        for (m, x, rug) in [("a", 8.0, false), ("b", 12.0, false), ("c", 1.5, false), ("d", 1.0, true)] {
            rise.insert(m.to_string(), json!({"rise_x": x, "rug": rug}));
        }
        let mut coins = vec![coin("a", 100, Some(900)), coin("b", 200, None), coin("c", 300, Some(5000)), coin("d", 400, Some(30))];
        coins[3]["dev_sold_share"] = json!(0.99);
        let m = metrics(&coins, &rise, 60, 5.0);
        assert_eq!((m.measured, m.moons, m.rugs, m.fast, m.ge100, m.last_bt), (4, 2, 1, 1, 2, 400));
        // partial early sell is not a dump; a rug whose dev only sold after 1 h does not count
        coins[3]["dev_sold_share"] = json!(0.25);
        coins[3]["dev_first_sell_after_s"] = json!(30);
        assert_eq!(metrics(&coins, &rise, 60, 5.0).fast, 0);
        coins[3]["dev_first_sell_after_s"] = json!(7200);
        assert_eq!(metrics(&coins, &rise, 60, 5.0).rugs, 0);
        let r = Rule { min_coins: 3, max_rug: 0.3, max_fast: 0.3, min_moons: 2, active_s: 1000 };
        assert_eq!(verdict(&m, &r, 1000), (true, "QUALITY"));
        assert_eq!(verdict(&m, &r, 5000).1, "not_active");
        let strict = Rule { max_rug: 0.2, ..r };
        assert_eq!(verdict(&m, &strict, 1000).1, "rugs_often");
    }
}
