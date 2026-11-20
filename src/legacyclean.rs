//! Clean the legacy 18 Nov family data (owner 21 Nov: "data lama kamu bersihkan"). Non-destructive: the original
//! files are read only; cleaned output goes to <out>. Removal reasons per edge (V3 7 / 8.1 / traps T1-T9):
//!   BUSY_ENDPOINT     : an endpoint was flagged busy in the legacy run (exchange/bot/service-like, >= 300 txs)
//!   INFRA_LIST        : an endpoint is in the legacy exchange / service / bot lists
//!   NON_WALLET        : an endpoint is not a System-owned non-executable account now (program, PDA, token acct)
//!   DISPERSE          : the transaction sent to more than disperse_max recipients (airdrop / bot distribution)
//!   DUST              : amount below the dust floor of config/family.toml
//! Families are rebuilt from the kept edges: everything reachable from the legacy root over kept edges, root must
//! itself be clean; a family needs >= 2 wallets. Account checks use getMultipleAccounts (1 credit per 100).
//! Usage: jaringan legacyclean <family_a_dir> <export_dir> <out_dir>
use crate::rpc::{cfg_u64, Rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Reason an edge is removed, or None to keep it.
pub fn reason(e: &Value, busy: &HashSet<String>, infra: &HashSet<String>, non_wallet: &HashSet<String>, per_sig: &HashMap<String, usize>, disperse_max: usize, dust_sol: f64) -> Option<&'static str> {
    let p = e["parent"].as_str().unwrap_or("");
    let c = e["child"].as_str().unwrap_or("");
    if infra.contains(p) || infra.contains(c) {
        return Some("INFRA_LIST");
    }
    if busy.contains(p) || busy.contains(c) {
        return Some("BUSY_ENDPOINT");
    }
    if non_wallet.contains(p) || non_wallet.contains(c) {
        return Some("NON_WALLET");
    }
    if per_sig.get(e["sig"].as_str().unwrap_or("")).copied().unwrap_or(0) > disperse_max {
        return Some("DISPERSE");
    }
    if e["sol"].as_f64().unwrap_or(0.0) < dust_sol {
        return Some("DUST");
    }
    None
}

pub fn run(fa: &str, ex: &str, out: &str) -> Result<(), String> {
    let disperse_max = cfg_u64("config/family.toml", "legacy_disperse_max").ok_or("config/family.toml: legacy_disperse_max missing")? as usize;
    let dust_sol = cfg_u64("config/family.toml", "dust_lamports").ok_or("dust_lamports missing")? as f64 / 1e9;
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let edges = read_jsonl(&format!("{fa}/edges.jsonl"));
    let wallets = read_jsonl(&format!("{fa}/wallets.jsonl"));
    let busy: HashSet<String> = wallets.iter().filter(|w| w["busy"] == true).filter_map(|w| w["w"].as_str().map(String::from)).collect();
    let mut infra: HashSet<String> = HashSet::new();
    for f in ["data/p3/exchange_list.txt", "dataset/wallets/exchange_list.txt"] {
        for l in fs::read_to_string(f).unwrap_or_default().lines() {
            if let Some(w) = l.split_whitespace().next() {
                infra.insert(w.to_string());
            }
        }
    }
    for l in fs::read_to_string("data/roots/labels.tsv").unwrap_or_default().lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() >= 2 && matches!(p[1], "EXCHANGE_SERVICE" | "BOT_HUB" | "KNOWN_LIST") {
            infra.insert(p[0].to_string());
        }
    }
    // every address in the legacy graph -> account owner check (cached in out/accounts.jsonl)
    let acc_p = format!("{out}/accounts.jsonl");
    let mut owner: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&acc_p) {
        if let Some(a) = v["a"].as_str() {
            owner.insert(a.to_string(), v);
        }
    }
    let mut addrs: Vec<String> = edges.iter().flat_map(|e| [e["parent"].as_str(), e["child"].as_str()]).flatten().map(String::from)
        .chain(wallets.iter().filter_map(|w| w["w"].as_str().map(String::from))).collect();
    addrs.sort();
    addrs.dedup();
    let todo: Vec<String> = addrs.iter().filter(|a| !owner.contains_key(*a)).cloned().collect();
    if !todo.is_empty() {
        let mut rpc = Rpc::new("legacyclean")?;
        let mut f = OpenOptions::new().create(true).append(true).open(&acc_p).map_err(|e| e.to_string())?;
        for chunk in todo.chunks(100) {
            let r = rpc.call("getMultipleAccounts", json!([chunk, {"encoding": "jsonParsed", "dataSlice": {"offset": 0, "length": 0}}]), 1)?;
            let vals = r["value"].as_array().cloned().unwrap_or_default();
            for (a, v) in chunk.iter().zip(vals.iter()) {
                let row = json!({"a": a, "exists": !v.is_null(), "owner": v["owner"], "executable": v["executable"]});
                writeln!(f, "{row}").map_err(|e| e.to_string())?;
                owner.insert(a.clone(), row);
            }
        }
        rpc.save();
        println!("{}", json!({"accounts_checked": todo.len(), "credits_total": rpc.used()}));
    }
    // a missing (closed) account was a wallet that was emptied: kept as wallet-like
    let non_wallet: HashSet<String> = owner.values()
        .filter(|v| v["exists"] == true && (v["owner"] != "11111111111111111111111111111111" || v["executable"] == true))
        .filter_map(|v| v["a"].as_str().map(String::from)).collect();
    let mut per_sig: HashMap<String, usize> = HashMap::new();
    for e in &edges {
        *per_sig.entry(e["sig"].as_str().unwrap_or("").to_string()).or_insert(0) += 1;
    }
    let mut kept: Vec<&Value> = Vec::new();
    let mut removed: BTreeMap<&str, usize> = BTreeMap::new();
    let mut rf = fs::File::create(format!("{out}/removed_edges.jsonl")).map_err(|e| e.to_string())?;
    let mut kf = fs::File::create(format!("{out}/clean_edges.jsonl")).map_err(|e| e.to_string())?;
    for e in &edges {
        match reason(e, &busy, &infra, &non_wallet, &per_sig, disperse_max, dust_sol) {
            Some(r) => {
                *removed.entry(r).or_insert(0) += 1;
                let mut x = e.clone();
                x["reason"] = json!(r);
                writeln!(rf, "{x}").map_err(|e| e.to_string())?;
            }
            None => {
                kept.push(e);
                writeln!(kf, "{e}").map_err(|e| e.to_string())?;
            }
        }
    }
    // devs of the legacy export (role DEV), for counting devs per clean family
    let devs: HashSet<String> = fs::read_to_string(format!("{ex}/wallet_family.csv")).unwrap_or_default().lines().skip(1)
        .filter_map(|l| { let p: Vec<&str> = l.split(',').collect(); (p.get(2) == Some(&"DEV")).then(|| p.get(1).map(|s| s.to_string())).flatten() }).collect();
    let mut kids: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &kept {
        kids.entry(e["parent"].as_str().unwrap_or("")).or_default().push(e["child"].as_str().unwrap_or(""));
    }
    let roots: HashSet<&str> = edges.iter().filter_map(|e| e["root"].as_str()).collect();
    let mut ff = fs::File::create(format!("{out}/clean_families.jsonl")).map_err(|e| e.to_string())?;
    let (mut n_fam, mut n_dev_fam, mut dropped_root) = (0usize, 0usize, 0usize);
    let mut sizes = Vec::new();
    let mut roots_sorted: Vec<&&str> = roots.iter().collect();
    roots_sorted.sort();
    for root in roots_sorted {
        if busy.contains(*root) || infra.contains(*root) || non_wallet.contains(*root) {
            dropped_root += 1;
            continue;
        }
        let mut seen: HashSet<&str> = HashSet::from([*root]);
        let mut q: VecDeque<&str> = VecDeque::from([*root]);
        while let Some(w) = q.pop_front() {
            for c in kids.get(w).into_iter().flatten() {
                if seen.insert(c) {
                    q.push_back(c);
                }
            }
        }
        if seen.len() < 2 {
            continue;
        }
        let mut m: Vec<&str> = seen.into_iter().collect();
        m.sort();
        let d: Vec<&&str> = m.iter().filter(|w| devs.contains(**w)).collect();
        n_fam += 1;
        if !d.is_empty() {
            n_dev_fam += 1;
        }
        sizes.push(m.len());
        writeln!(ff, "{}", json!({"root": root, "n_wallets": m.len(), "n_devs": d.len(), "devs": d, "members": m})).map_err(|e| e.to_string())?;
    }
    sizes.sort();
    let pct = |p: f64| sizes.get(((sizes.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0);
    let rep = json!({"edges_in": edges.len(), "edges_kept": kept.len(), "edges_removed": removed, "non_wallet_addresses": non_wallet.len(),
        "busy_wallets": busy.len(), "infra_list": infra.len(), "legacy_roots": roots.len(), "roots_dropped_unclean": dropped_root,
        "clean_families_ge2": n_fam, "clean_families_with_dev": n_dev_fam, "size_p50": pct(0.5), "size_p90": pct(0.9), "size_max": sizes.last()});
    fs::write(format!("{out}/summary.json"), rep.to_string()).map_err(|e| e.to_string())?;
    println!("{rep}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removal_reasons() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<HashSet<String>>();
        let mut per = HashMap::new();
        per.insert("S1".to_string(), 1usize);
        per.insert("S9".to_string(), 9usize);
        let e = |p: &str, c: &str, sig: &str, sol: f64| json!({"parent": p, "child": c, "sig": sig, "sol": sol});
        let (busy, infra, nw) = (s(&["B"]), s(&["X"]), s(&["P"]));
        assert_eq!(reason(&e("A", "C", "S1", 1.0), &busy, &infra, &nw, &per, 5, 0.01), None);
        assert_eq!(reason(&e("X", "C", "S1", 1.0), &busy, &infra, &nw, &per, 5, 0.01), Some("INFRA_LIST"));
        assert_eq!(reason(&e("A", "B", "S1", 1.0), &busy, &infra, &nw, &per, 5, 0.01), Some("BUSY_ENDPOINT"));
        assert_eq!(reason(&e("P", "C", "S1", 1.0), &busy, &infra, &nw, &per, 5, 0.01), Some("NON_WALLET"));
        assert_eq!(reason(&e("A", "C", "S9", 1.0), &busy, &infra, &nw, &per, 5, 0.01), Some("DISPERSE"));
        assert_eq!(reason(&e("A", "C", "S1", 0.001), &busy, &infra, &nw, &per, 5, 0.01), Some("DUST"));
    }
}
