//! V3 family builder v1 (owner 20 Nov 2026: find families = a core wallet (induk) with the wallets linked to it;
//! a dev that switches wallets but is still funded from the same induk belongs to the same family).
//!
//! For every dev wallet (devtrace output, newest row per dev) walk UP the material first-funding chain (V3 E1/E7):
//!   wallet w, as-of time t (its first create for a dev, the funding time for an ancestor)
//!   -> oldest <= 100 txs of w before t (one Helius call, 10 credits), successful txs only
//!   -> SOL into w from System transfer / createAccount (outer + inner, decode::sol_moves), amount >= dust floor
//!   -> funder = the sender with the largest single inbound; material only if it is >= material_pct of all SOL
//!      w received before t (V3 T6: first MATERIAL inbound, not the first inbound)
//!   -> funder class as of the funding time (V3 7, fail closed):
//!        not a System-owned, non-executable account          -> NON_WALLET (program / PDA / token account) stop
//!        >= busy_sigs signatures within busy_window_s before -> INFRA_BUSY (exchange / bot / service, behaviour) stop
//!        lookup failed                                      -> UNCLASSIFIED stop
//!        otherwise                                          -> ORDINARY, climb further (max_depth)
//! Induk (root) = the highest ORDINARY wallet of the chain. Family = all chains with the same root; members = every
//! wallet on those chains. A dev whose first material funder is infrastructure has no family (root = itself).
//! Wallet results are cached (data/v3/family/wallets.jsonl) so shared ancestors are fetched once.
//! Usage: jaringan famv3 <dev_dir> <out_dir>
use crate::rpc::{cfg_u64, Rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

const CFG: &str = "config/family.toml";
const SYSTEM: &str = "11111111111111111111111111111111";

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn append(path: &str, v: &Value) -> Result<(), String> {
    let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    writeln!(f, "{v}").map_err(|e| e.to_string())
}

/// Material funder of `w` from its oldest txs (all before `before_bt`): (funder, lamports, sig, bt, share, n_inbound).
pub fn material_funder(txs: &[Value], w: &str, before_bt: i64, dust: u64, material_pct: f64) -> Option<(String, u64, String, i64, f64, usize)> {
    let mut best: Option<(String, u64, String, i64)> = None;
    let (mut total, mut n) = (0u64, 0usize);
    for tx in txs {
        if !tx["meta"]["err"].is_null() {
            continue;
        }
        let bt = tx["blockTime"].as_i64().unwrap_or(i64::MAX);
        if bt >= before_bt {
            continue;
        }
        for (s, d, lam) in crate::decode::sol_moves(tx) {
            if d != w || s == w || lam < dust {
                continue;
            }
            total += lam;
            n += 1;
            if best.as_ref().map(|b| lam > b.1).unwrap_or(true) {
                best = Some((s, lam, tx["transaction"]["signatures"][0].as_str().unwrap_or("").to_string(), bt));
            }
        }
    }
    let (s, lam, sig, bt) = best?;
    let share = lam as f64 / total.max(1) as f64;
    (share >= material_pct).then_some((s, lam, sig, bt, share, n))
}

/// Behavioural infra check from a signature page (newest first, all before t): busy if the page is full and spans
/// less than `window_s`.
pub fn is_busy(bts: &[i64], busy_sigs: usize, window_s: i64) -> bool {
    if bts.len() < busy_sigs {
        return false;
    }
    let (hi, lo) = (bts.iter().max().copied().unwrap_or(0), bts.iter().min().copied().unwrap_or(0));
    hi - lo <= window_s
}

struct Ctx {
    rpc: Rpc,
    cache: HashMap<String, Value>, // wallet -> {class, funder, lam, sig, bt, share}
    cache_p: String,
    dust: u64,
    material: f64,
    busy_sigs: usize,
    busy_window: i64,
}

impl Ctx {
    /// Class of `a` as of time `t` (cached per (wallet) with the first t asked; v1 simplification, logged).
    fn class(&mut self, a: &str, t: i64) -> Result<String, String> {
        if let Some(c) = self.cache.get(a).and_then(|v| v["class"].as_str().map(String::from)) {
            return Ok(c);
        }
        let info = match self.rpc.call("getAccountInfo", json!([a, {"encoding": "jsonParsed"}]), 1) {
            Ok(r) => r,
            Err(e) if e.starts_with("BUDGET") => return Err(e),
            Err(_) => return Ok("UNCLASSIFIED".into()),
        };
        let v = &info["value"];
        // a wallet that was emptied may no longer exist: treat a missing account as a wallet candidate
        let wallet_like = v.is_null() || (v["owner"] == SYSTEM && v["executable"] != true);
        let class = if !wallet_like {
            "NON_WALLET".to_string()
        } else {
            let r = match self.rpc.call("getTransactionsForAddress", json!([a, {"transactionDetails": "signatures", "limit": self.busy_sigs,
                "filters": {"blockTime": {"lt": t}}}]), 10) {
                Ok(r) => r,
                Err(e) if e.starts_with("BUDGET") => return Err(e),
                Err(_) => return Ok("UNCLASSIFIED".into()),
            };
            let bts: Vec<i64> = r["data"].as_array().into_iter().flatten().filter_map(|s| s["blockTime"].as_i64()).collect();
            if is_busy(&bts, self.busy_sigs, self.busy_window) { "INFRA_BUSY".into() } else { "ORDINARY".into() }
        };
        let e = self.cache.entry(a.to_string()).or_insert_with(|| json!({"w": a}));
        e["class"] = json!(class);
        e["class_as_of"] = json!(t);
        e["owner"] = v["owner"].clone();
        Ok(class)
    }

    /// Material funder of `w` before `t` (cached).
    fn funder(&mut self, w: &str, t: i64) -> Result<Value, String> {
        if let Some(v) = self.cache.get(w) {
            if v.get("funder_done").is_some() {
                return Ok(v.clone());
            }
        }
        let r = match self.rpc.call("getTransactionsForAddress", json!([w, {"transactionDetails": "full", "limit": 100, "sortOrder": "asc",
            "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1, "filters": {"blockTime": {"lt": t}}}]), 10) {
            Ok(r) => r,
            Err(e) if e.starts_with("BUDGET") => return Err(e),
            Err(e) => {
                let e2 = self.cache.entry(w.to_string()).or_insert_with(|| json!({"w": w}));
                e2["funder_done"] = json!(true);
                e2["error"] = json!(e.chars().take(80).collect::<String>());
                return Ok(e2.clone());
            }
        };
        let txs = r["data"].as_array().cloned().unwrap_or_default();
        let f = material_funder(&txs, w, t, self.dust, self.material);
        let e = self.cache.entry(w.to_string()).or_insert_with(|| json!({"w": w}));
        e["funder_done"] = json!(true);
        e["n_tx_read"] = json!(txs.len());
        match f {
            Some((s, lam, sig, bt, share, n)) => {
                e["funder"] = json!(s);
                e["sol"] = json!(lam as f64 / 1e9);
                e["sig"] = json!(sig);
                e["bt"] = json!(bt);
                e["share"] = json!(share);
                e["n_inbound"] = json!(n);
            }
            None => e["funder"] = Value::Null,
        }
        let out = e.clone();
        append(&self.cache_p, &out)?;
        Ok(out)
    }
}

pub fn run(dev_dir: &str, out: &str, looping: bool) -> Result<(), String> {
    let max_depth = cfg_u64(CFG, "max_depth").ok_or("config/family.toml: max_depth missing")? as usize;
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let cache_p = format!("{out}/wallets.jsonl");
    let mut cache: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&cache_p) {
        if let Some(w) = v["w"].as_str() {
            let e = cache.entry(w.to_string()).or_insert_with(|| json!({"w": w}));
            if let Some(o) = v.as_object() {
                for (k, x) in o {
                    e[k] = x.clone();
                }
            }
        }
    }
    let mut cx = Ctx {
        rpc: Rpc::new("famv3")?,
        cache,
        cache_p,
        dust: cfg_u64(CFG, "dust_lamports").ok_or("dust_lamports missing")?,
        material: cfg_u64(CFG, "material_pct").ok_or("material_pct missing")? as f64 / 100.0,
        busy_sigs: cfg_u64(CFG, "busy_sigs").ok_or("busy_sigs missing")? as usize,
        busy_window: cfg_u64(CFG, "busy_window_s").ok_or("busy_window_s missing")? as i64,
    };
    loop {
    // devs: newest row per dev; as-of = the dev's first create time we know
    let mut devs: BTreeMap<String, (i64, Vec<String>)> = BTreeMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        let Some(d) = v["dev"].as_str() else { continue };
        let coins: Vec<String> = v["coins"].as_array().into_iter().flatten().filter_map(|c| c["mint"].as_str().map(String::from)).collect();
        let first = v["coins"].as_array().into_iter().flatten().filter_map(|c| c["bt"].as_i64()).min().unwrap_or(i64::MAX);
        devs.insert(d.to_string(), (first, coins));
    }
    let chains_p = format!("{out}/chains.jsonl");
    let done: HashSet<String> = read_jsonl(&chains_p).iter().filter_map(|v| v["dev"].as_str().map(String::from)).collect();
    let mut n_new = 0usize;
    for (dev, (t0, _)) in &devs {
        if done.contains(dev) || *t0 == i64::MAX {
            continue;
        }
        let mut chain: Vec<Value> = Vec::new();
        let mut seen: HashSet<String> = HashSet::from([dev.clone()]);
        let (mut w, mut t, mut end) = (dev.clone(), *t0, "max_depth".to_string());
        for _ in 0..max_depth {
            let f = cx.funder(&w, t)?;
            let Some(s) = f["funder"].as_str().map(String::from) else {
                end = if f.get("error").is_some() { "UNCLASSIFIED_fetch".into() } else { "NO_MATERIAL_FUNDER".into() };
                break;
            };
            let fbt = f["bt"].as_i64().unwrap_or(t);
            let class = cx.class(&s, fbt)?;
            chain.push(json!({"child": w, "parent": s, "sol": f["sol"], "share": f["share"], "sig": f["sig"], "bt": fbt, "parent_class": class}));
            if class != "ORDINARY" {
                end = class;
                break;
            }
            if !seen.insert(s.clone()) {
                end = "LOOP".into();
                break;
            }
            w = s;
            t = fbt;
        }
        // root = highest ORDINARY wallet: the parent of the last ORDINARY edge, else the dev itself
        let root = chain.iter().rev().find(|e| e["parent_class"] == "ORDINARY").and_then(|e| e["parent"].as_str()).unwrap_or(dev).to_string();
        append(&chains_p, &json!({"dev": dev, "as_of_bt": t0, "root": root, "depth": chain.iter().filter(|e| e["parent_class"] == "ORDINARY").count(),
            "end": end, "chain": chain}))?;
        n_new += 1;
        if n_new.is_multiple_of(20) {
            cx.rpc.save();
            println!("{}", json!({"devs_done": n_new, "credits_total": cx.rpc.used()}));
        }
    }
    cx.rpc.save();
    report(dev_dir, out)?;
    println!("{}", json!({"devs_new": n_new, "credits_total": cx.rpc.used()}));
    if !looping {
        return Ok(());
    }
    std::thread::sleep(std::time::Duration::from_secs(300));
    }
}

/// Families from chains.jsonl: group devs by root; family = root != dev for at least one dev (>= 2 wallets).
pub fn report(dev_dir: &str, out: &str) -> Result<(), String> {
    let chains = read_jsonl(&format!("{out}/chains.jsonl"));
    let mut labels: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            labels.insert(d.to_string(), v);
        }
    }
    let mut fam: BTreeMap<String, (HashSet<String>, HashSet<String>)> = BTreeMap::new(); // root -> (devs, members)
    let mut ends: BTreeMap<String, usize> = BTreeMap::new();
    for c in &chains {
        let (Some(dev), Some(root)) = (c["dev"].as_str(), c["root"].as_str()) else { continue };
        *ends.entry(c["end"].as_str().unwrap_or("").split(':').next().unwrap_or("").to_string()).or_insert(0) += 1;
        let e = fam.entry(root.to_string()).or_default();
        e.0.insert(dev.to_string());
        e.1.insert(dev.to_string());
        for x in c["chain"].as_array().into_iter().flatten() {
            if x["parent_class"] == "ORDINARY" {
                e.1.insert(x["child"].as_str().unwrap_or("").to_string());
                e.1.insert(x["parent"].as_str().unwrap_or("").to_string());
            }
        }
    }
    let mut f = fs::File::create(format!("{out}/families.jsonl")).map_err(|e| e.to_string())?;
    let (mut n_fam, mut n_multi_dev) = (0usize, 0usize);
    let mut sizes: Vec<usize> = Vec::new();
    for (root, (devs, members)) in &fam {
        if members.len() < 2 {
            continue;
        }
        n_fam += 1;
        if devs.len() >= 2 {
            n_multi_dev += 1;
        }
        sizes.push(members.len());
        let coins: Vec<Value> = devs.iter().filter_map(|d| labels.get(d)).flat_map(|v| v["coins"].as_array().cloned().unwrap_or_default()).collect();
        let n_dump = coins.iter().filter(|c| c["fast_dump"] == true).count();
        let best = coins.iter().filter_map(|c| c["market_cap"].as_f64()).fold(0.0, f64::max);
        let dl: Vec<Value> = devs.iter().map(|d| json!({"dev": d, "label": labels.get(d).map(|v| v["label"].clone())})).collect();
        let mut m: Vec<&String> = members.iter().collect();
        m.sort();
        writeln!(f, "{}", json!({"root": root, "n_wallets": members.len(), "n_devs": devs.len(), "devs": dl, "members": m,
            "n_coins": coins.len(), "n_fast_dump": n_dump, "best_mcap_now": best})).map_err(|e| e.to_string())?;
    }
    sizes.sort();
    let pct = |p: f64| sizes.get(((sizes.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0);
    let rep = json!({"devs_traced": chains.len(), "families_ge2_wallets": n_fam, "families_ge2_devs": n_multi_dev,
        "size_p50": pct(0.5), "size_p90": pct(0.9), "size_max": sizes.last(), "chain_end": ends});
    fs::write(format!("{out}/summary.json"), rep.to_string()).map_err(|e| e.to_string())?;
    println!("{rep}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tx(bt: i64, err: bool, moves: &[(&str, &str, u64)]) -> Value {
        let ix: Vec<Value> = moves.iter().map(|(s, d, l)| json!({"programId": SYSTEM, "parsed": {"type": "transfer", "info": {"source": s, "destination": d, "lamports": l}}})).collect();
        json!({"blockTime": bt, "meta": {"err": if err { json!({"x": 1}) } else { Value::Null }, "innerInstructions": []},
            "transaction": {"signatures": [format!("s{bt}")], "message": {"instructions": ix}}})
    }
    #[test]
    fn dust_and_failed_and_late_ignored() {
        let txs = vec![
            tx(1, false, &[("DUSTER", "W", 1_000)]),           // dust
            tx(2, true, &[("FAILED", "W", 5_000_000_000)]),    // failed tx
            tx(3, false, &[("A", "W", 2_000_000_000)]),        // material
            tx(9, false, &[("LATE", "W", 9_000_000_000)]),     // after as-of time
        ];
        let f = material_funder(&txs, "W", 5, 10_000_000, 0.5).expect("funder");
        assert_eq!(f.0, "A");
        assert_eq!(f.5, 1);
    }
    #[test]
    fn not_material_when_split() {
        let txs = vec![tx(1, false, &[("A", "W", 1_000_000_000)]), tx(2, false, &[("B", "W", 1_100_000_000)]), tx(3, false, &[("C", "W", 1_200_000_000)])];
        assert!(material_funder(&txs, "W", 5, 10_000_000, 0.5).is_none());
        assert_eq!(material_funder(&txs, "W", 5, 10_000_000, 0.3).map(|f| f.0), Some("C".to_string()));
    }
    #[test]
    fn outbound_is_not_funding() {
        let txs = vec![tx(1, false, &[("W", "X", 3_000_000_000)])];
        assert!(material_funder(&txs, "W", 5, 10_000_000, 0.5).is_none());
    }
    #[test]
    fn busy_rule() {
        let full: Vec<i64> = (0..1000).map(|i| 1000 + i).collect();
        assert!(is_busy(&full, 1000, 86_400));
        let slow: Vec<i64> = (0..1000).map(|i| i * 1000).collect();
        assert!(!is_busy(&slow, 1000, 86_400));
        assert!(!is_busy(&full[..10], 1000, 86_400));
    }
}
