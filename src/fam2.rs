//! Family builder v2 (owner 21 Nov 2026): quality devs first, transfer-level evidence, and the dev's own outgoing
//! transfers. Uses Helius getTransfersByAddress (parsed SOL transfers with signature + instruction index) instead of
//! full transactions; wallet classes are checked on Tatum / public RPC (Aux) so Helius quota stays for history.
//!
//! Per dev (devtrace output; devs of the best coins first; serial fast-sell devs are skipped = blacklisted):
//!   UP   : inbound native-SOL transfers before the dev's first create (oldest first, <= 100) -> material funder
//!          (largest single inbound >= dust, >= material_pct of all inbound before t; V3 E1 / T2 / T6); classify the
//!          funder as of that transfer; climb while ORDINARY (max_depth). Root (induk) = highest ORDINARY wallet.
//!   DOWN : the dev's outbound native-SOL transfers (newest <= out_limit) to wallets that are ORDINARY, >= dust, and
//!          not part of a transaction paying > disperse_max recipients (airdrop / bot) -> "dev knows this wallet"
//!          (owner rule). Recipients are family members.
//! Classes (fail closed): NON_WALLET (program / PDA / token account), INFRA_BUSY (>= busy_sigs signatures in
//! busy_window_s before the transfer: exchange / bot / service), INFRA_LIST (legacy lists), UNCLASSIFIED, ORDINARY.
//! Families = union-find over all kept edges. Output: edges.jsonl, removed.jsonl, chains.jsonl, families.jsonl,
//! summary.json. Usage: jaringan fam2 <dev_dir> <out_dir> <loop 0|1>
use crate::famv3::is_busy;
use crate::rpc::{cfg_u64, Aux, Rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

const CFG: &str = "config/family.toml";
const SOL: &str = "So11111111111111111111111111111111111111111";
const SYSTEM: &str = "11111111111111111111111111111111";

/// (from, to, lamports, signature, blockTime)
pub type Tr = (String, String, u64, String, i64);

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn app(path: &str, v: &Value) -> Result<(), String> {
    // one write_all per line: parallel workers append to the same files (O_APPEND keeps whole lines intact)
    let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    f.write_all(format!("{v}\n").as_bytes()).map_err(|e| e.to_string())
}

/// A parsed transfer row -> (from, to, lamports, sig, bt).
pub fn tr(v: &Value) -> Option<Tr> {
    Some((
        v["fromUserAccount"].as_str()?.to_string(),
        v["toUserAccount"].as_str()?.to_string(),
        v["amount"].as_str().and_then(|s| s.parse().ok()).or(v["amount"].as_u64())?,
        v["signature"].as_str()?.to_string(),
        v["blockTime"].as_i64()?,
    ))
}

/// Material funder among inbound transfers to `w` strictly before `t`.
pub fn material(rows: &[Tr], w: &str, t: i64, dust: u64, pct: f64) -> Option<(String, u64, String, i64, f64)> {
    let ins: Vec<&Tr> = rows.iter().filter(|r| r.1 == w && r.0 != w && r.2 >= dust && r.4 < t).collect();
    let total: u64 = ins.iter().map(|r| r.2).sum();
    let best = ins.iter().max_by_key(|r| r.2)?;
    let share = best.2 as f64 / total.max(1) as f64;
    (share >= pct).then(|| (best.0.clone(), best.2, best.3.clone(), best.4, share))
}

/// Outbound recipients of `w`: per recipient (lamports, first sig, first bt), excluding dust and transactions in which
/// `w` paid more than `disperse_max` distinct recipients.
pub fn outbound(rows: &[Tr], w: &str, dust: u64, disperse_max: usize) -> BTreeMap<String, (u64, String, i64)> {
    let mut per_sig: HashMap<&str, HashSet<&str>> = HashMap::new();
    for r in rows.iter().filter(|r| r.0 == w && r.1 != w) {
        per_sig.entry(r.3.as_str()).or_default().insert(r.1.as_str());
    }
    let mut out: BTreeMap<String, (u64, String, i64)> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.0 == w && r.1 != w && r.2 >= dust) {
        if per_sig.get(r.3.as_str()).map(|s| s.len()).unwrap_or(0) > disperse_max {
            continue;
        }
        let e = out.entry(r.1.clone()).or_insert((0, r.3.clone(), r.4));
        e.0 += r.2;
        if r.4 < e.2 {
            e.1 = r.3.clone();
            e.2 = r.4;
        }
    }
    out
}

/// Outbound native-SOL rows of `w` from a CoinStats /wallet/transactions page (action "Sent", coin solana, from = w).
/// Incoming rows carry no sender in CoinStats data (checked 2026-11-21), so CoinStats is used for DOWN only.
pub fn cs_rows(v: &Value, w: &str) -> Vec<Tr> {
    let mut out = Vec::new();
    for t in v["result"].as_array().into_iter().flatten() {
        let sig = t["hash"]["id"].as_str().unwrap_or("").to_string();
        let bt = t["date"].as_str().and_then(iso_secs).unwrap_or(0);
        for g in t["transactions"].as_array().into_iter().flatten() {
            if g["action"] != "Sent" {
                continue;
            }
            for it in g["items"].as_array().into_iter().flatten() {
                if it["coin"]["id"] != "solana" || it["fromAddress"].as_str() != Some(w) {
                    continue;
                }
                let (Some(to), Some(c)) = (it["toAddress"].as_str(), it["count"].as_f64()) else { continue };
                out.push((w.to_string(), to.to_string(), (c.abs() * 1e9).round() as u64, sig.clone(), bt));
            }
        }
    }
    out
}

/// "2026-10-30T16:14:50.000Z" -> unix seconds (UTC), std only.
pub fn iso_secs(s: &str) -> Option<i64> {
    let (d, t) = s.split_once('T')?;
    let mut dp = d.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (dp.next()??, dp.next()??, dp.next()??);
    let mut tp = t.trim_end_matches('Z').split(':');
    let (hh, mm) = (tp.next()?.parse::<i64>().ok()?, tp.next()?.parse::<i64>().ok()?);
    let ss = tp.next()?.split('.').next()?.parse::<i64>().ok()?;
    // days from civil (Howard Hinnant)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400 + hh * 3600 + mm * 60 + ss)
}

struct Cs {
    key: String,
    agent: ureq::Agent,
}

impl Cs {
    fn new() -> Option<Self> {
        let key = fs::read_to_string(".coinstats.env").ok()?.lines().find_map(|l| l.strip_prefix("COINSTATS_API_KEY=").map(|k| k.trim().to_string()))?;
        Some(Cs { key, agent: ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60)).build() })
    }
    /// Sync then read the newest `limit` transactions of `w` (Solana). Err on any HTTP failure (caller falls back).
    fn outbound(&self, w: &str, limit: u64) -> Result<Vec<Tr>, String> {
        let base = "https://openapiv1.coinstats.app/wallet/transactions";
        let q = format!("{base}?address={w}&connectionId=solana");
        let _ = self.agent.request("PATCH", &q).set("X-API-KEY", &self.key).call();
        for attempt in 0..6u64 {
            std::thread::sleep(std::time::Duration::from_millis(500 + attempt * 1500));
            match self.agent.get(&format!("{q}&limit={limit}")).set("X-API-KEY", &self.key).call() {
                Ok(r) => {
                    let v: Value = r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).ok_or("coinstats json")?;
                    return Ok(cs_rows(&v, w));
                }
                Err(ureq::Error::Status(409, _)) | Err(ureq::Error::Status(429, _)) => continue,
                Err(e) => return Err(format!("coinstats {}", e.to_string().chars().take(80).collect::<String>())),
            }
        }
        Err("coinstats not synced".into())
    }
}

struct Ctx {
    rpc: Rpc,
    aux: Aux,
    cls: HashMap<String, String>,
    cls_p: String,
    infra: HashSet<String>,
    busy_sigs: usize,
    busy_window: i64,
}

impl Ctx {
    fn transfers(&mut self, w: &str, direction: &str, order: &str, limit: u64, before: Option<i64>) -> Result<Vec<Tr>, String> {
        let mut o = json!({"direction": direction, "mint": SOL, "limit": limit, "sortOrder": order});
        if let Some(t) = before {
            o["filters"] = json!({"blockTime": {"lt": t}});
        }
        let r = self.rpc.call("getTransfersByAddress", json!([w, o]), 10)?;
        Ok(r["data"].as_array().into_iter().flatten().filter_map(tr).collect())
    }

    /// Class of `a` as of the transfer `sig` (busy = activity BEFORE that signature). Cached per wallet.
    fn class(&mut self, a: &str, sig: &str) -> String {
        if let Some(c) = self.cls.get(a) {
            return c.clone();
        }
        let c = if self.infra.contains(a) {
            "INFRA_LIST".to_string()
        } else {
            match self.aux.call("getAccountInfo", json!([a, {"encoding": "base64", "dataSlice": {"offset": 0, "length": 0}}])) {
                Err(_) => "UNCLASSIFIED".to_string(),
                Ok(r) if !r["value"].is_null() && (r["value"]["owner"] != SYSTEM || r["value"]["executable"] == true) => "NON_WALLET".to_string(),
                Ok(_) => match self.aux.call("getSignaturesForAddress", json!([a, {"limit": self.busy_sigs, "before": sig}])) {
                    Err(_) => "UNCLASSIFIED".to_string(),
                    Ok(r) => {
                        let bts: Vec<i64> = r.as_array().into_iter().flatten().filter_map(|s| s["blockTime"].as_i64()).collect();
                        if is_busy(&bts, self.busy_sigs, self.busy_window) { "INFRA_BUSY".into() } else { "ORDINARY".into() }
                    }
                },
            }
        };
        let _ = app(&self.cls_p, &json!({"w": a, "class": c, "as_of_sig": sig}));
        self.cls.insert(a.to_string(), c.clone());
        c
    }
}

pub fn run(dev_dir: &str, out: &str, looping: bool) -> Result<(), String> {
    let c = |k: &str| cfg_u64(CFG, k).ok_or(format!("config/family.toml: {k} missing"));
    let (max_depth, dust, material_pct) = (c("max_depth")? as usize, c("dust_lamports")?, c("material_pct")? as f64 / 100.0);
    let (disperse_max, out_limit) = (c("legacy_disperse_max")? as usize, c("out_limit")?);
    let cr = |k: &str| cfg_u64("config/coinrise.toml", k).ok_or(format!("config/coinrise.toml: {k} missing"));
    let (bl_min, bl_s, bl_share) = (cr("bl_min_coins")? as usize, cr("bl_fast_s")? as i64, cr("bl_fast_share_pct")? as f64 / 100.0);
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut infra = HashSet::new();
    for fl in ["data/p3/exchange_list.txt", "dataset/wallets/exchange_list.txt"] {
        for l in fs::read_to_string(fl).unwrap_or_default().lines() {
            if let Some(w) = l.split_whitespace().next() {
                infra.insert(w.to_string());
            }
        }
    }
    let cls_p = format!("{out}/wallet_class.jsonl");
    let mut cls = HashMap::new();
    for v in read_jsonl(&cls_p) {
        if let (Some(w), Some(k)) = (v["w"].as_str(), v["class"].as_str()) {
            cls.insert(w.to_string(), k.to_string());
        }
    }
    let mut cx = Ctx { rpc: Rpc::new("fam2")?, aux: Aux::new("fam2")?, cls, cls_p, infra, busy_sigs: c("busy_sigs")? as usize, busy_window: c("busy_window_s")? as i64 };
    let cs = Cs::new();
    let (edges_p, removed_p, chains_p) = (format!("{out}/edges.jsonl"), format!("{out}/removed.jsonl"), format!("{out}/chains.jsonl"));
    loop {
        // devs skipped only for a missing create time are retried (fixed 21 Nov: fall back to the create tx in roles)
        let done: HashSet<String> = read_jsonl(&chains_p).iter().filter(|v| v["skipped"] != "NO_CREATE_TIME")
            .filter_map(|v| v["dev"].as_str().map(String::from)).collect();
        let mut create_bt: HashMap<String, i64> = HashMap::new();
        for r in read_jsonl(&format!("{dev_dir}/roles.jsonl")) {
            if let (Some(c), Some(bt)) = (r["creator"].as_str(), r["bt"].as_i64()) {
                let e = create_bt.entry(c.to_string()).or_insert(bt);
                *e = (*e).min(bt);
            }
        }
        // newest devtrace row per dev, best coins first (tier, then market cap of its best coin)
        let mut rows: HashMap<String, Value> = HashMap::new();
        for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
            if let Some(d) = v["dev"].as_str() {
                rows.insert(d.to_string(), v);
            }
        }
        let mut order: Vec<(String, f64, f64)> = rows.iter().filter(|(d, _)| !done.contains(*d)).map(|(d, v)| {
            let best = v["coins"].as_array().into_iter().flatten().filter_map(|c| c["market_cap"].as_f64()).fold(0.0, f64::max);
            (d.clone(), v["tier"].as_f64().unwrap_or(0.0), best)
        }).collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then(b.2.total_cmp(&a.2)).then(a.0.cmp(&b.0)));
        // parallel workers (owner 21 Nov: all sources at once): JAR_SHARD="i/n" -> this worker takes devs with
        // (byte sum of address) % n == i; workers never touch the same dev
        if let Some((i, n)) = std::env::var("JAR_SHARD").ok().and_then(|s| { let (a, b) = s.split_once('/')?; Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)) }) {
            order.retain(|(d, _, _)| d.bytes().map(u64::from).sum::<u64>() % n.max(1) == i);
        }
        // owner 21 Nov: only creators of DexScreener coins (JAR_SCOPE_FILE, written by reput)
        if let Some(sc) = crate::reput::scope_set() {
            order.retain(|(d, _, _)| sc.contains(d));
        }
        let mut n_new = 0usize;
        for (dev, _, _) in &order {
            let v = &rows[dev];
            let coins = v["coins"].as_array().cloned().unwrap_or_default();
            if let Some((fast, n)) = crate::coinrise::serial_rugger(&coins, bl_min, bl_s, bl_share) {
                app(&chains_p, &json!({"dev": dev, "skipped": "SERIAL_FAST_SELL_DEV", "fast_sell_coins": fast, "coins": n}))?;
                continue;
            }
            // history may be cut before the dev's older creates: also use the create tx of the coin it was found by
            let Some(t0) = coins.iter().filter_map(|c| c["bt"].as_i64()).chain(create_bt.get(dev).copied()).min() else {
                app(&chains_p, &json!({"dev": dev, "skipped": "NO_CREATE_TIME"}))?;
                continue;
            };
            // UP
            let (mut w, mut t, mut end, mut chain) = (dev.clone(), t0, "max_depth".to_string(), Vec::new());
            let mut seen = HashSet::from([dev.clone()]);
            for _ in 0..max_depth {
                let ins = match cx.transfers(&w, "in", "asc", 100, Some(t)) {
                    Ok(x) => x,
                    Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                    Err(_) => { end = "UNCLASSIFIED_fetch".into(); break; }
                };
                let Some((s, lam, sig, bt, share)) = material(&ins, &w, t, dust, material_pct) else { end = "NO_MATERIAL_FUNDER".into(); break };
                let k = cx.class(&s, &sig);
                let e = json!({"parent": s, "child": w, "sol": lam as f64 / 1e9, "share": share, "sig": sig, "bt": bt, "kind": "up_funding", "dev": dev, "parent_class": k});
                if k != "ORDINARY" {
                    app(&removed_p, &e)?;
                    end = k;
                    break;
                }
                app(&edges_p, &e)?;
                chain.push(e);
                if !seen.insert(s.clone()) { end = "LOOP".into(); break; }
                w = s;
                t = bt;
            }
            // DOWN: dev -> wallets it paid
            // CoinStats first (keeps Helius quota for UP), Helius getTransfersByAddress as fallback
            let (outs, down_src) = match cs.as_ref().map(|c| c.outbound(dev, out_limit)) {
                Some(Ok(x)) => (x, "coinstats"),
                _ => match cx.transfers(dev, "out", "desc", out_limit, None) {
                    Ok(x) => (x, "helius"),
                    Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                    Err(_) => (Vec::new(), "none"),
                },
            };
            let mut n_out = 0usize;
            for (to, (lam, sig, bt)) in outbound(&outs, dev, dust, disperse_max) {
                let k = cx.class(&to, &sig);
                let e = json!({"parent": dev, "child": to, "sol": lam as f64 / 1e9, "sig": sig, "bt": bt, "kind": "dev_out", "dev": dev, "child_class": k, "src": down_src});
                if k == "ORDINARY" {
                    app(&edges_p, &e)?;
                    n_out += 1;
                } else {
                    app(&removed_p, &e)?;
                }
            }
            let root = chain.last().and_then(|e| e["parent"].as_str()).unwrap_or(dev).to_string();
            app(&chains_p, &json!({"dev": dev, "root": root, "depth": chain.len(), "end": end, "dev_out_kept": n_out}))?;
            n_new += 1;
            if n_new.is_multiple_of(20) {
                cx.rpc.save();
                families(dev_dir, out)?;
            }
        }
        cx.rpc.save();
        families(dev_dir, out)?;
        println!("{}", json!({"devs_new": n_new, "helius_credits_total": cx.rpc.used()}));
        if !looping {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(300));
    }
}

/// Union-find over kept edges -> families.jsonl (format read by coinrise / watchlist) + summary.json.
pub fn families(dev_dir: &str, out: &str) -> Result<(), String> {
    let edges = read_jsonl(&format!("{out}/edges.jsonl"));
    let devs: HashSet<String> = read_jsonl(&format!("{dev_dir}/devs.jsonl")).iter().filter_map(|v| v["dev"].as_str().map(String::from)).collect();
    let mut id: HashMap<String, usize> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut p: Vec<usize> = Vec::new();
    fn find(p: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while p[r] != r { r = p[r]; }
        let mut y = x;
        while p[y] != r { let n = p[y]; p[y] = r; y = n; }
        r
    }
    let mut ix = |w: &str, names: &mut Vec<String>, p: &mut Vec<usize>| -> usize {
        *id.entry(w.to_string()).or_insert_with(|| { names.push(w.to_string()); p.push(names.len() - 1); names.len() - 1 })
    };
    let mut parents: HashSet<usize> = HashSet::new();
    let mut children: HashSet<usize> = HashSet::new();
    let mut kinds: HashMap<usize, BTreeMap<String, usize>> = HashMap::new();
    let mut pairs = Vec::new();
    for e in &edges {
        let (Some(a), Some(b)) = (e["parent"].as_str(), e["child"].as_str()) else { continue };
        let (x, y) = (ix(a, &mut names, &mut p), ix(b, &mut names, &mut p));
        parents.insert(x);
        children.insert(y);
        pairs.push((x, y, e["kind"].as_str().unwrap_or("").to_string()));
    }
    for (x, y, _) in &pairs {
        let (rx, ry) = (find(&mut p, *x), find(&mut p, *y));
        if rx != ry { p[rx.max(ry)] = rx.min(ry); }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..names.len() {
        let r = find(&mut p, i);
        groups.entry(r).or_default().push(i);
    }
    for (x, _, k) in &pairs {
        let r = find(&mut p, *x);
        *kinds.entry(r).or_default().entry(k.clone()).or_insert(0) += 1;
    }
    let giant = cfg_u64(CFG, "giant_cap").unwrap_or(100) as usize;
    let mut f = fs::File::create(format!("{out}/families.jsonl")).map_err(|e| e.to_string())?;
    let (mut n, mut nd, mut ng, mut sizes) = (0usize, 0usize, 0usize, Vec::new());
    for (r, m) in &groups {
        if m.len() < 2 { continue; }
        let roots: Vec<&String> = m.iter().filter(|i| parents.contains(i) && !children.contains(i)).map(|i| &names[*i]).collect();
        let mut mem: Vec<&String> = m.iter().map(|i| &names[*i]).collect();
        mem.sort();
        let d: Vec<Value> = mem.iter().filter(|w| devs.contains(w.as_str())).map(|w| json!({"dev": w})).collect();
        n += 1;
        if d.len() >= 2 { nd += 1; }
        let flag = if m.len() > giant { ng += 1; "GIANT_REVIEW" } else { "OK" };
        sizes.push(m.len());
        writeln!(f, "{}", json!({"root": roots.first().map(|s| s.as_str()).unwrap_or(names[*r].as_str()), "roots": roots, "n_wallets": m.len(),
            "n_devs": d.len(), "devs": d, "edge_kinds": kinds.get(r), "flag": flag, "members": mem})).map_err(|e| e.to_string())?;
    }
    sizes.sort();
    let pct = |q: f64| sizes.get(((sizes.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0);
    let chains = read_jsonl(&format!("{out}/chains.jsonl"));
    let ends: BTreeMap<String, usize> = chains.iter().fold(BTreeMap::new(), |mut b, c| {
        *b.entry(c["end"].as_str().or(c["skipped"].as_str()).unwrap_or("").to_string()).or_insert(0) += 1; b });
    let rep = json!({"devs_processed": chains.len(), "families_ge2": n, "families_ge2_devs": nd, "giant_review": ng, "size_p50": pct(0.5),
        "size_p90": pct(0.9), "size_max": sizes.last(), "edges_kept": edges.len(), "chain_end_or_skip": ends});
    fs::write(format!("{out}/summary.json"), rep.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(f: &str, t: &str, lam: u64, sig: &str, bt: i64) -> Tr {
        (f.into(), t.into(), lam, sig.into(), bt)
    }
    #[test]
    fn material_rules() {
        let rows = vec![r("DUST", "W", 1_000, "a", 1), r("A", "W", 3_000_000_000, "b", 2), r("B", "W", 1_000_000_000, "c", 3), r("LATE", "W", 9e9 as u64, "d", 9)];
        let m = material(&rows, "W", 5, 10_000_000, 0.5).expect("funder");
        assert_eq!(m.0, "A");
        assert!((m.4 - 0.75).abs() < 1e-9);
        assert!(material(&rows, "W", 5, 10_000_000, 0.8).is_none());
    }
    #[test]
    fn outbound_skips_dust_and_disperse() {
        let mut rows = vec![r("W", "X", 2_000_000_000, "s1", 1), r("W", "X", 1_000_000_000, "s2", 2), r("W", "D", 1_000, "s3", 3)];
        for i in 0..7 {
            rows.push(r("W", &format!("R{i}"), 1_000_000_000, "air", 4));
        }
        let o = outbound(&rows, "W", 10_000_000, 5);
        assert_eq!(o.len(), 1);
        assert_eq!(o["X"].0, 3_000_000_000);
        assert_eq!(o["X"].1, "s1");
    }
    #[test]
    fn coinstats_rows_and_time() {
        assert_eq!(iso_secs("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(iso_secs("2026-10-30T16:14:50.000Z"), Some(1_793_376_890));
        let v = json!({"result": [
            {"date": "1970-01-01T00:00:10.000Z", "hash": {"id": "S1"}, "transactions": [{"action": "Sent", "items": [
                {"coin": {"id": "solana"}, "fromAddress": "W", "toAddress": "X", "count": -1.5},
                {"coin": {"id": "usd-coin"}, "fromAddress": "W", "toAddress": "Y", "count": 5.0}]}]},
            {"date": "1970-01-01T00:00:20.000Z", "hash": {"id": "S2"}, "transactions": [{"action": "Received", "items": [
                {"coin": {"id": "solana"}, "toAddress": "W", "count": 2.0}]}]}]});
        assert_eq!(cs_rows(&v, "W"), vec![("W".to_string(), "X".to_string(), 1_500_000_000, "S1".to_string(), 10)]);
    }
    #[test]
    fn parses_transfer_row() {
        let v = json!({"fromUserAccount": "A", "toUserAccount": "B", "amount": "200000000", "signature": "S", "blockTime": 7});
        assert_eq!(tr(&v), Some(("A".into(), "B".into(), 200_000_000, "S".into(), 7)));
    }
}
