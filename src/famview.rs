//! Clean, typed family view (owner 21 Nov 2026): only families whose devs launched a coin within the last
//! active_days (stale ones out), every wallet typed, independent devs and one-time wallets kept APART from families.
//! Roles: DEV (coin creator), INDUK (top funder: has children, no parent), PERANTARA (relay inside a funding chain),
//! PENERIMA (wallet the dev paid). ONE_TIME flag = dev wallet with a single coin and a tiny, complete history.
//! Inputs: <fam2_dir>/{families,edges,chains}.jsonl, <dev_dir>/{devs,roles}.jsonl.
//! Output: <out>/families_active.jsonl, independent_devs.tsv, one_time_wallets.tsv, summary.json.
//! Usage: jaringan famview <fam2_dir> <dev_dir> <out_dir>
use crate::rpc::cfg_u64;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Last launch time of a dev: newest coin in its history OR the create tx of the coin it was found by (history can be
/// cut before the newest coin; bug fixed 21 Nov).
pub fn last_launch(dev_row: Option<&Value>, role_bt: Option<i64>) -> Option<i64> {
    dev_row.and_then(|v| v["coins"].as_array()).into_iter().flatten().filter_map(|c| c["bt"].as_i64()).chain(role_bt).max()
}

/// One-time wallet: exactly one coin, complete history, at most `max_tx` transactions read.
pub fn one_time(dev_row: &Value, max_tx: u64) -> bool {
    dev_row["n_coins"].as_u64() == Some(1) && dev_row["history_complete"] == true && dev_row["txs_read"].as_u64().unwrap_or(u64::MAX) <= max_tx
}

pub fn run(fam: &str, dev_dir: &str, out: &str) -> Result<(), String> {
    let active_s = cfg_u64("config/quality.toml", "active_days").ok_or("config/quality.toml: active_days missing")? as i64 * 86_400;
    let max_tx = cfg_u64("config/family.toml", "one_time_max_tx").ok_or("config/family.toml: one_time_max_tx missing")?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut dev_rows: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            dev_rows.insert(d.to_string(), v);
        }
    }
    let mut role_bt: HashMap<String, i64> = HashMap::new();
    for r in read_jsonl(&format!("{dev_dir}/roles.jsonl")) {
        if let (Some(c), Some(bt)) = (r["creator"].as_str(), r["bt"].as_i64()) {
            let e = role_bt.entry(c.to_string()).or_insert(bt);
            *e = (*e).max(bt);
        }
    }
    let edges = read_jsonl(&format!("{fam}/edges.jsonl"));
    let (mut up_child, mut up_parent, mut paid): (HashSet<String>, HashSet<String>, HashSet<String>) = Default::default();
    for e in &edges {
        let (p, c) = (e["parent"].as_str().unwrap_or("").to_string(), e["child"].as_str().unwrap_or("").to_string());
        if e["kind"] == "dev_out" {
            paid.insert(c);
        } else {
            up_parent.insert(p);
            up_child.insert(c);
        }
    }
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut ff = String::new();
    let (mut n_active, mut n_stale, mut wallets) = (0usize, 0usize, 0usize);
    let mut in_family: HashSet<String> = HashSet::new();
    let mut roles_count: HashMap<&str, usize> = HashMap::new();
    for f in read_jsonl(&format!("{fam}/families.jsonl")) {
        let devs: Vec<String> = f["devs"].as_array().into_iter().flatten().filter_map(|d| d["dev"].as_str().map(String::from)).collect();
        let last = devs.iter().filter_map(|d| last_launch(dev_rows.get(d), role_bt.get(d).copied())).max();
        if !last.is_some_and(|t| now - t <= active_s) {
            n_stale += 1;
            continue;
        }
        n_active += 1;
        let dset: HashSet<&String> = devs.iter().collect();
        let mut members = Vec::new();
        for w in f["members"].as_array().into_iter().flatten().filter_map(|m| m.as_str()) {
            let ws = w.to_string();
            let role = if dset.contains(&ws) { "DEV" } else if up_parent.contains(w) && !up_child.contains(w) { "INDUK" }
                else if up_parent.contains(w) && up_child.contains(w) { "PERANTARA" } else if paid.contains(w) { "PENERIMA" } else { "TERHUBUNG" };
            *roles_count.entry(role).or_insert(0) += 1;
            let ot = dev_rows.get(w).is_some_and(|v| one_time(v, max_tx));
            members.push(json!({"w": w, "role": role, "one_time": ot}));
            in_family.insert(ws);
            wallets += 1;
        }
        ff.push_str(&json!({"root": f["root"], "n_wallets": members.len(), "n_devs": devs.len(), "last_launch_h": last.map(|t| (now - t) / 3600),
            "flag": f["flag"], "members": members}).to_string());
        ff.push('\n');
    }
    fs::write(format!("{out}/families_active.jsonl"), ff).map_err(|e| e.to_string())?;
    // independent devs: traced, not in any family, launched within the window; one-time wallets listed apart
    let (mut ind, mut once) = (String::from("dev\tkoin\tjam_sejak_koin_terakhir\tsekali_pakai\n"), String::from("wallet\tkoin\ttx_dibaca\tjam_sejak_koin\n"));
    let (mut n_ind, mut n_once) = (0usize, 0usize);
    for (d, v) in &dev_rows {
        let last = last_launch(Some(v), role_bt.get(d).copied());
        let active = last.is_some_and(|t| now - t <= active_s);
        let ot = one_time(v, max_tx);
        if ot && active {
            n_once += 1;
            once.push_str(&format!("{d}\t{}\t{}\t{}\n", v["n_coins"], v["txs_read"], last.map(|t| (now - t) / 3600).unwrap_or(-1)));
        }
        if !in_family.contains(d) && active {
            n_ind += 1;
            ind.push_str(&format!("{d}\t{}\t{}\t{}\n", v["n_coins"], last.map(|t| (now - t) / 3600).unwrap_or(-1), if ot { "ya" } else { "tidak" }));
        }
    }
    fs::write(format!("{out}/independent_devs.tsv"), ind).map_err(|e| e.to_string())?;
    fs::write(format!("{out}/one_time_wallets.tsv"), once).map_err(|e| e.to_string())?;
    let rep = json!({"families_active": n_active, "families_stale_removed": n_stale, "wallets_in_active_families": wallets, "roles": roles_count,
        "independent_devs_active": n_ind, "one_time_dev_wallets_active": n_once, "active_days": active_s / 86_400});
    fs::write(format!("{out}/summary.json"), rep.to_string()).map_err(|e| e.to_string())?;
    println!("{rep}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn last_launch_uses_role_time_too() {
        let v = json!({"coins": [{"bt": 100}, {"bt": 50}]});
        assert_eq!(last_launch(Some(&v), Some(400)), Some(400));
        assert_eq!(last_launch(Some(&v), None), Some(100));
        assert_eq!(last_launch(None, None), None);
    }
    #[test]
    fn one_time_rule() {
        assert!(one_time(&json!({"n_coins": 1, "history_complete": true, "txs_read": 12}), 30));
        assert!(!one_time(&json!({"n_coins": 1, "history_complete": false, "txs_read": 12}), 30));
        assert!(!one_time(&json!({"n_coins": 2, "history_complete": true, "txs_read": 12}), 30));
        assert!(!one_time(&json!({"n_coins": 1, "history_complete": true, "txs_read": 300}), 30));
    }
}
