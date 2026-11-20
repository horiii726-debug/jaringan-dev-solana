//! One clean family dataset from the legacy 18 Nov graph and the new V3 chains (owner 21 Nov: "bersihkan benar-benar
//! bersih, jangan ada error, transfer exchange dan bug; gabungkan jadi 1 data baru dan data lama").
//! Every edge, old or new, passes the SAME checks against the raw transaction (no balance-delta edges):
//!   NO_TX / FAILED_TX : signature not found or transaction failed
//!   NO_DIRECT_TRANSFER: no System transfer/createAccount parent -> child in the tx (legacy balance-delta defect)
//!   DUST              : direct amount < dust floor (config/family.toml)
//!   DISPERSE          : the parent paid > legacy_disperse_max distinct recipients in that tx (airdrop / bot)
//!   NON_WALLET        : an endpoint is not a System-owned, non-executable account (program, PDA, token account)
//!   INFRA_BUSY        : an endpoint had >= busy_sigs signatures within busy_window_s before the edge (exchange /
//!                       bot / service behaviour; checked as of the LATEST edge of that wallet = conservative)
//!   INFRA_LIST        : an endpoint is in the legacy exchange / service / bot lists
//!   UNCLASSIFIED      : a check could not run (fail closed, V3 7.4)
//! Families = connected groups over the kept edges (union-find); >= 2 wallets; groups above giant_cap wallets are
//! flagged GIANT_REVIEW (V3 8.3 giant-component alarm). Caches make re-runs cheap.
//! Usage: jaringan unify <v3_family_dir> <legacy_clean_dir> <dev_dir> <out_dir>
use crate::famv3::is_busy;
use crate::rpc::{cfg_u64, Rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

const SYSTEM: &str = "11111111111111111111111111111111";

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn appender(path: &str) -> Result<fs::File, String> {
    OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())
}

/// Direct lamports parent -> child in a parsed tx, and the number of distinct recipients the parent paid.
pub fn direct(tx: &Value, parent: &str, child: &str) -> (u64, usize) {
    let moves = crate::decode::sol_moves(tx);
    let lam = moves.iter().filter(|(s, d, _)| s == parent && d == child).map(|m| m.2).sum();
    let mut rec: Vec<&String> = moves.iter().filter(|(s, d, _)| s == parent && d != parent).map(|m| &m.1).collect();
    rec.sort();
    rec.dedup();
    (lam, rec.len())
}

struct Uf {
    p: Vec<usize>,
}
impl Uf {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.p[r] != r {
            r = self.p[r];
        }
        let mut y = x;
        while self.p[y] != r {
            let n = self.p[y];
            self.p[y] = r;
            y = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.p[ra.max(rb)] = ra.min(rb);
        }
    }
}

pub fn run(v3: &str, legacy: &str, dev_dir: &str, out: &str) -> Result<(), String> {
    let c = |k: &str| cfg_u64("config/family.toml", k).ok_or(format!("config/family.toml: {k} missing"));
    let (dust, disperse_max, busy_sigs, busy_window, giant) = (c("dust_lamports")?, c("legacy_disperse_max")? as usize, c("busy_sigs")? as usize, c("busy_window_s")? as i64, c("giant_cap")? as usize);
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    // 1. candidate edges
    let mut cand: Vec<Value> = Vec::new();
    for ch in read_jsonl(&format!("{v3}/chains.jsonl")) {
        for e in ch["chain"].as_array().into_iter().flatten() {
            if e["parent_class"] == "ORDINARY" {
                cand.push(json!({"parent": e["parent"], "child": e["child"], "sig": e["sig"], "bt": e["bt"], "src": "v3_funding"}));
            }
        }
    }
    for e in read_jsonl(&format!("{legacy}/clean_edges.jsonl")) {
        cand.push(json!({"parent": e["parent"], "child": e["child"], "sig": e["sig"], "bt": e["bt"], "src": "legacy_18nov"}));
    }
    let mut seen_key = HashSet::new();
    cand.retain(|e| seen_key.insert(format!("{}|{}|{}", e["parent"], e["child"], e["sig"])));
    let mut rpc = Rpc::new("unify")?;
    // 2. raw tx check per signature (cached)
    let txc_p = format!("{out}/tx_check.jsonl");
    let mut txc: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&txc_p) {
        if let Some(k) = v["k"].as_str() {
            txc.insert(k.to_string(), v);
        }
    }
    let mut f = appender(&txc_p)?;
    let mut by_sig: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, e) in cand.iter().enumerate() {
        by_sig.entry(e["sig"].as_str().unwrap_or("").to_string()).or_default().push(i);
    }
    let mut n_fetch = 0u64;
    for (sig, idx) in &by_sig {
        if idx.iter().all(|i| txc.contains_key(&format!("{}|{}|{sig}", cand[*i]["parent"].as_str().unwrap_or(""), cand[*i]["child"].as_str().unwrap_or("")))) {
            continue;
        }
        let r = match rpc.call("getTransaction", json!([sig, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 0}]), 1) {
            Ok(r) => Some(r),
            Err(e) if e.starts_with("BUDGET") => return Err(e),
            Err(_) => None,
        };
        n_fetch += 1;
        for i in idx {
            let (p, ch) = (cand[*i]["parent"].as_str().unwrap_or(""), cand[*i]["child"].as_str().unwrap_or(""));
            let k = format!("{p}|{ch}|{sig}");
            let row = match &r {
                None => json!({"k": k, "status": "UNCLASSIFIED"}),
                Some(tx) if tx.is_null() => json!({"k": k, "status": "NO_TX"}),
                Some(tx) if !tx["meta"]["err"].is_null() => json!({"k": k, "status": "FAILED_TX"}),
                Some(tx) => {
                    let (lam, nrec) = direct(tx, p, ch);
                    json!({"k": k, "status": "OK", "lam": lam, "n_recipients": nrec, "bt": tx["blockTime"], "slot": tx["slot"]})
                }
            };
            writeln!(f, "{row}").map_err(|e| e.to_string())?;
            txc.insert(k, row);
        }
        if n_fetch.is_multiple_of(500) {
            rpc.save();
            println!("{}", json!({"tx_checked": n_fetch, "credits_total": rpc.used()}));
        }
    }
    // 3. per-edge tx verdict
    let mut verdict: Vec<Option<&'static str>> = Vec::with_capacity(cand.len());
    let mut lam_of: Vec<u64> = Vec::with_capacity(cand.len());
    let mut bt_of: Vec<i64> = Vec::with_capacity(cand.len());
    for e in &cand {
        let k = format!("{}|{}|{}", e["parent"].as_str().unwrap_or(""), e["child"].as_str().unwrap_or(""), e["sig"].as_str().unwrap_or(""));
        let t = txc.get(&k).cloned().unwrap_or(json!({"status": "UNCLASSIFIED"}));
        let lam = t["lam"].as_u64().unwrap_or(0);
        lam_of.push(lam);
        bt_of.push(t["bt"].as_i64().or(e["bt"].as_i64()).unwrap_or(0));
        verdict.push(match t["status"].as_str().unwrap_or("UNCLASSIFIED") {
            "OK" if lam == 0 => Some("NO_DIRECT_TRANSFER"),
            "OK" if lam < dust => Some("DUST"),
            "OK" if t["n_recipients"].as_u64().unwrap_or(0) as usize > disperse_max => Some("DISPERSE"),
            "OK" => None,
            "NO_TX" => Some("NO_TX"),
            "FAILED_TX" => Some("FAILED_TX"),
            _ => Some("UNCLASSIFIED"),
        });
    }
    // 4. endpoint classes for edges that survived step 3 (cached)
    let mut latest: HashMap<String, i64> = HashMap::new();
    for (i, e) in cand.iter().enumerate() {
        if verdict[i].is_none() {
            for w in [e["parent"].as_str().unwrap_or(""), e["child"].as_str().unwrap_or("")] {
                let x = latest.entry(w.to_string()).or_insert(0);
                *x = (*x).max(bt_of[i]);
            }
        }
    }
    let mut infra: HashSet<String> = HashSet::new();
    for fl in ["data/p3/exchange_list.txt", "dataset/wallets/exchange_list.txt"] {
        for l in fs::read_to_string(fl).unwrap_or_default().lines() {
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
    let cls_p = format!("{out}/wallet_class.jsonl");
    let mut cls: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&cls_p) {
        if let Some(w) = v["w"].as_str() {
            cls.insert(w.to_string(), v);
        }
    }
    let mut cf = appender(&cls_p)?;
    let mut todo: Vec<String> = latest.keys().filter(|w| !cls.contains_key(*w)).cloned().collect();
    todo.sort();
    for chunk in todo.chunks(100) {
        let r = match rpc.call("getMultipleAccounts", json!([chunk, {"encoding": "jsonParsed", "dataSlice": {"offset": 0, "length": 0}}]), 1) {
            Ok(r) => r,
            Err(e) if e.starts_with("BUDGET") => return Err(e),
            Err(_) => json!({"value": []}),
        };
        let vals = r["value"].as_array().cloned().unwrap_or_default();
        for (i, w) in chunk.iter().enumerate() {
            let v = vals.get(i).cloned();
            let class = if infra.contains(w) {
                "INFRA_LIST".to_string()
            } else {
                match v {
                    None => "UNCLASSIFIED".to_string(),
                    Some(v) if !v.is_null() && (v["owner"] != SYSTEM || v["executable"] == true) => "NON_WALLET".to_string(),
                    Some(_) => {
                        let t = latest[w];
                        match rpc.call("getTransactionsForAddress", json!([w, {"transactionDetails": "signatures", "limit": busy_sigs, "filters": {"blockTime": {"lte": t}}}]), 10) {
                            Ok(r) => {
                                let bts: Vec<i64> = r["data"].as_array().into_iter().flatten().filter_map(|s| s["blockTime"].as_i64()).collect();
                                if is_busy(&bts, busy_sigs, busy_window) { "INFRA_BUSY".to_string() } else { "ORDINARY".to_string() }
                            }
                            Err(e) if e.starts_with("BUDGET") => return Err(e),
                            Err(_) => "UNCLASSIFIED".to_string(),
                        }
                    }
                }
            };
            let row = json!({"w": w, "class": class, "as_of_bt": latest[w]});
            writeln!(cf, "{row}").map_err(|e| e.to_string())?;
            cls.insert(w.clone(), row);
        }
        rpc.save();
    }
    // 5. final edge verdict, families
    let class_of = |w: &str| cls.get(w).and_then(|v| v["class"].as_str()).unwrap_or("UNCLASSIFIED").to_string();
    let mut removed: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut ef = fs::File::create(format!("{out}/edges.jsonl")).map_err(|e| e.to_string())?;
    let mut rf = fs::File::create(format!("{out}/removed_edges.jsonl")).map_err(|e| e.to_string())?;
    let mut idx: HashMap<String, usize> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut kept: Vec<(usize, usize, usize)> = Vec::new();
    for (i, e) in cand.iter().enumerate() {
        let (p, ch) = (e["parent"].as_str().unwrap_or(""), e["child"].as_str().unwrap_or(""));
        let why = verdict[i].map(String::from).or_else(|| {
            let (a, b) = (class_of(p), class_of(ch));
            if a != "ORDINARY" { Some(a) } else if b != "ORDINARY" { Some(b) } else { None }
        });
        let src = e["src"].as_str().unwrap_or("").to_string();
        let mut row = e.clone();
        row["sol"] = json!(lam_of[i] as f64 / 1e9);
        row["bt"] = json!(bt_of[i]);
        match why {
            Some(r) => {
                *removed.entry(src).or_default().entry(r.clone()).or_insert(0) += 1;
                row["reason"] = json!(r);
                writeln!(rf, "{row}").map_err(|e| e.to_string())?;
            }
            None => {
                writeln!(ef, "{row}").map_err(|e| e.to_string())?;
                let mut id = |w: &str| *idx.entry(w.to_string()).or_insert_with(|| { names.push(w.to_string()); names.len() - 1 });
                let (a, b) = (id(p), id(ch));
                kept.push((a, b, i));
            }
        }
    }
    let mut uf = Uf { p: (0..names.len()).collect() };
    for (a, b, _) in &kept {
        uf.union(*a, *b);
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..names.len() {
        let r = uf.find(i);
        groups.entry(r).or_default().push(i);
    }
    // devs and their coins (new: devtrace; legacy: export role DEV / koin_family.csv)
    let mut dev_coins: HashMap<String, Vec<Value>> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            dev_coins.insert(d.to_string(), v["coins"].as_array().cloned().unwrap_or_default());
        }
    }
    for l in fs::read_to_string("family_export/koin_family.csv").unwrap_or_default().lines().skip(1) {
        let p: Vec<&str> = l.split(',').collect();
        if p.len() >= 7 {
            let list = dev_coins.entry(p[3].to_string()).or_default();
            if !list.iter().any(|c| c["mint"] == p[1]) {
                list.push(json!({"mint": p[1], "symbol": p[2], "bt": p[4].parse::<i64>().ok(), "legacy_status": p[5], "legacy_mcap_usd": p[6].parse::<f64>().ok()}));
            }
        }
    }
    let mut ff = fs::File::create(format!("{out}/families.jsonl")).map_err(|e| e.to_string())?;
    let (mut n, mut n_dev, mut n_giant, mut n_both) = (0usize, 0usize, 0usize, 0usize);
    let mut sizes = Vec::new();
    for members in groups.values() {
        if members.len() < 2 {
            continue;
        }
        let set: HashSet<usize> = members.iter().copied().collect();
        let es: Vec<&(usize, usize, usize)> = kept.iter().filter(|(a, _, _)| set.contains(a)).collect();
        let has_parent: HashSet<usize> = es.iter().map(|(_, b, _)| *b).collect();
        let mut roots: Vec<&String> = members.iter().filter(|m| !has_parent.contains(m)).map(|m| &names[*m]).collect();
        roots.sort();
        let srcs: HashSet<&str> = es.iter().map(|(_, _, i)| cand[*i]["src"].as_str().unwrap_or("")).collect();
        let mut m: Vec<&String> = members.iter().map(|i| &names[*i]).collect();
        m.sort();
        let devs: Vec<&&String> = m.iter().filter(|w| dev_coins.contains_key(w.as_str())).collect();
        let coins: Vec<Value> = devs.iter().flat_map(|d| dev_coins[d.as_str()].clone()).collect();
        let flag = if m.len() > giant { "GIANT_REVIEW" } else { "OK" };
        n += 1;
        if !devs.is_empty() {
            n_dev += 1;
        }
        if flag != "OK" {
            n_giant += 1;
        }
        if srcs.len() > 1 {
            n_both += 1;
        }
        sizes.push(m.len());
        writeln!(ff, "{}", json!({"family_id": format!("F{}", n), "roots": roots, "n_wallets": m.len(), "n_edges": es.len(), "n_devs": devs.len(),
            "devs": devs, "n_coins": coins.len(), "coins": coins, "sources": srcs, "flag": flag, "members": m})).map_err(|e| e.to_string())?;
    }
    sizes.sort();
    let pct = |p: f64| sizes.get(((sizes.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0);
    let cc: BTreeMap<String, usize> = cls.values().fold(BTreeMap::new(), |mut b, v| { *b.entry(v["class"].as_str().unwrap_or("").to_string()).or_insert(0) += 1; b });
    rpc.save();
    let rep = json!({"candidate_edges": cand.len(), "kept_edges": kept.len(), "removed_by_source_and_reason": removed, "wallet_classes": cc,
        "families_ge2": n, "families_with_dev": n_dev, "families_old_and_new_joined": n_both, "giant_review": n_giant,
        "size_p50": pct(0.5), "size_p90": pct(0.9), "size_max": sizes.last(), "credits_total": rpc.used(), "config": "config/family.toml"});
    fs::write(format!("{out}/summary.json"), rep.to_string()).map_err(|e| e.to_string())?;
    println!("{rep}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tx(moves: &[(&str, &str, u64)]) -> Value {
        let ix: Vec<Value> = moves.iter().map(|(s, d, l)| json!({"programId": SYSTEM, "parsed": {"type": "transfer", "info": {"source": s, "destination": d, "lamports": l}}})).collect();
        json!({"meta": {"err": null, "innerInstructions": []}, "transaction": {"message": {"instructions": ix}}})
    }
    #[test]
    fn direct_transfer_and_recipients() {
        let t = tx(&[("P", "C", 5), ("P", "C", 7), ("P", "X", 1), ("Q", "C", 100)]);
        assert_eq!(direct(&t, "P", "C"), (12, 2));
        assert_eq!(direct(&t, "Z", "C"), (0, 0)); // balance-delta-only edge has no direct transfer
    }
    #[test]
    fn union_find_groups() {
        let mut u = Uf { p: (0..5).collect() };
        u.union(0, 1);
        u.union(3, 4);
        u.union(1, 4);
        assert_eq!(u.find(3), u.find(0));
        assert_ne!(u.find(2), u.find(0));
    }
}
