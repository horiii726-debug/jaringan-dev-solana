//! Family of a dev (owner 18 Nov): find the core wallet (induk) behind the devs of big DexScreener coins.
//!  up   : from each dev follow the first funder until an exchange/bot (labels) or a busy wallet (>= 3000 tx)
//!         or no funder -> the highest own wallet = induk.
//!  down : from each induk follow outgoing SOL (top-level System transfers >= 0.05 SOL, since 1 Oct) through
//!         relays, skipping exchanges/bots; busy wallets are not expanded -> family wallets + every Pump.fun
//!         coin they created.
//!  report: per family: wallets, devs (perantara or not), coins: lulus / DexScreener / mcap now / ATH (candles
//!         already on disk only, no download).
//! Usage: jaringan family <devs.txt> <out_dir> <max_wallets_total>
use crate::decode;
use crate::fund;
use crate::graph::read_lines;
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;

const MIN_LAM: u64 = 50_000_000; // 0.05 SOL
const SINCE: i64 = 1_790_812_800; // 2026-10-01 00:00 UTC
const SYSTEM: &str = "11111111111111111111111111111111";
const GRAD_PX: f64 = 4.1e-7; // SOL per token at Pump.fun graduation
const FAMILY_CAP: usize = 250; // wallets per family

fn bad_set() -> HashMap<String, String> {
    let mut m = HashMap::new();
    for l in fs::read_to_string("data/p3/exchange_list.txt").unwrap_or_default().lines() {
        let w = l.split_whitespace().next().unwrap_or("");
        if !w.is_empty() {
            m.insert(w.to_string(), "EXCHANGE".to_string());
        }
    }
    for f in ["data/roots/labels.tsv"] {
        for l in fs::read_to_string(f).unwrap_or_default().lines() {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() >= 2 && matches!(p[1], "EXCHANGE_SERVICE" | "BOT_HUB" | "KNOWN_LIST") {
                m.insert(p[0].to_string(), p[1].to_string());
            }
        }
    }
    m.insert(SYSTEM.to_string(), "SYSTEM".to_string());
    m
}

/// Real recipients of SOL that `w` sent in this tx, by balance change (relays route through temporary accounts
/// that are closed in the same tx, so plain transfer pairs miss the real receiver). Only when `w` lost
/// >= 0.05 SOL; txs with Pump.fun trades/creates are skipped (curve, fees, tips are not family).
fn transfers(tx: &Value, w: &str) -> Vec<(String, u64)> {
    if !decode::events(tx).0.is_empty() {
        return Vec::new();
    }
    let keys = tx["transaction"]["message"]["accountKeys"].as_array().cloned().unwrap_or_default();
    let pre = tx["meta"]["preBalances"].as_array().cloned().unwrap_or_default();
    let post = tx["meta"]["postBalances"].as_array().cloned().unwrap_or_default();
    let delta = |i: usize| post.get(i).and_then(|v| v.as_i64()).unwrap_or(0) - pre.get(i).and_then(|v| v.as_i64()).unwrap_or(0);
    let Some(wi) = keys.iter().position(|k| k["pubkey"] == w) else { return Vec::new() };
    if -delta(wi) < MIN_LAM as i64 {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let pk = k["pubkey"].as_str().unwrap_or("");
        if i != wi && delta(i) >= MIN_LAM as i64 && pk != SYSTEM {
            v.push((pk.to_string(), delta(i) as u64));
        }
    }
    v
}

fn up(rpc: &mut Rpc, dev: &str, bad: &HashMap<String, String>) -> Result<Value, String> {
    let mut chain: Vec<Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut w = dev.to_string();
    let mut end = "max_depth".to_string();
    seen.insert(w.clone());
    for _ in 0..15 {
        let v = fund::one(rpc, &w, 3)?;
        let deep = v["deep"] == true;
        let fu = v["funder"].as_str().map(String::from);
        chain.push(json!({"w": w, "n_sigs": v["n_sigs"], "deep": deep, "first_bt": v["first_bt"], "funder": fu,
            "sol": v["lam"].as_u64().unwrap_or(0) as f64 / 1e9, "kind": v["kind"]}));
        // hub: this wallet passed big money down (>= 1 SOL) but was itself started with dust -> it collects
        // money from many sources (profits) = the core wallet; its first funder is irrelevant.
        let own = v["lam"].as_u64().unwrap_or(0) as f64 / 1e9;
        let sent = if chain.len() >= 2 { chain[chain.len() - 2]["sol"].as_f64().unwrap_or(0.0) } else { 0.0 };
        if !deep && chain.len() >= 2 && sent >= 1.0 && own < 0.5 {
            end = "induk_akumulasi".into();
            break;
        }
        if deep {
            end = "wallet_ramai".into();
            break;
        }
        let Some(f) = fu else {
            end = "tanpa_funder".into();
            break;
        };
        if let Some(l) = bad.get(&f) {
            end = format!("{l}:{f}");
            break;
        }
        if !seen.insert(f.clone()) {
            end = "loop".into();
            break;
        }
        w = f;
    }
    // induk = highest wallet that is not busy (busy = exchange/bot/service-like, not an own wallet)
    let induk = chain.iter().rev().find(|c| c["deep"] != true).or(chain.first()).and_then(|c| c["w"].as_str()).unwrap_or(dev).to_string();
    Ok(json!({"dev": dev, "induk": induk, "end": end, "levels": chain.len(), "chain": chain}))
}

pub fn run(devs_file: &str, out_dir: &str, max_wallets: usize) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let bad = bad_set();
    let mut rpc = Rpc::new("family")?;
    let devs: Vec<String> = fs::read_to_string(devs_file).map_err(|e| e.to_string())?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

    // ---- up (resumable)
    let up_path = format!("{out_dir}/up.jsonl");
    let done: HashSet<String> = read_lines(&up_path).iter().filter_map(|v| v["dev"].as_str().map(String::from)).collect();
    let mut fup = OpenOptions::new().create(true).append(true).open(&up_path).map_err(|e| e.to_string())?;
    for d in devs.iter().filter(|d| !done.contains(*d)) {
        match up(&mut rpc, d, &bad) {
            Ok(v) => writeln!(fup, "{v}").map_err(|e| e.to_string())?,
            Err(e) if e.starts_with("BUDGET") => {
                rpc.save();
                return Err(e);
            }
            Err(e) => eprintln!("up {d}: {e}"),
        }
    }
    rpc.save();
    let ups = read_lines(&up_path);
    println!("UP_DONE devs={} credits_total={}", ups.len(), rpc.used());

    // ---- down from each induk (resumable per wallet)
    let edges_path = format!("{out_dir}/edges.jsonl");
    let creates_path = format!("{out_dir}/creates.jsonl");
    let wal_path = format!("{out_dir}/wallets.jsonl");
    let mut expanded: HashSet<String> = read_lines(&wal_path).iter().filter_map(|v| v["w"].as_str().map(String::from)).collect();
    let mut fe = OpenOptions::new().create(true).append(true).open(&edges_path).map_err(|e| e.to_string())?;
    let mut fc = OpenOptions::new().create(true).append(true).open(&creates_path).map_err(|e| e.to_string())?;
    let mut fw = OpenOptions::new().create(true).append(true).open(&wal_path).map_err(|e| e.to_string())?;
    let mut roots: Vec<String> = Vec::new();
    // Owner 18 Nov: only families with a KNOWN origin are crawled down: the dev was traced up to a core
    // wallet (induk) through at least one own wallet, and the trace did not die in a busy wallet.
    // Devs funded straight from an exchange or sitting on a busy wallet are dropped (we will never buy them).
    for u in &ups {
        let i = u["induk"].as_str().unwrap_or("").to_string();
        let end = u["end"].as_str().unwrap_or("");
        let traced = i != u["dev"].as_str().unwrap_or("") && end != "wallet_ramai";
        if traced && !i.is_empty() && !roots.contains(&i) {
            roots.push(i);
        }
    }
    println!("families with known origin: {} of {} devs", roots.len(), ups.len());
    let old_edges = read_lines(&edges_path);
    let mut total = expanded.len();
    for root in &roots {
        let mut fam: HashSet<String> = HashSet::new();
        let mut q: VecDeque<(String, u32)> = VecDeque::new();
        fam.insert(root.clone());
        q.push_back((root.clone(), 0));
        // known members: every wallet on the up-chains that end at this induk (relays + devs)
        for u in ups.iter().filter(|u| u["induk"] == root.as_str()) {
            for c in u["chain"].as_array().into_iter().flatten() {
                if let Some(x) = c["w"].as_str() {
                    if c["deep"] != true && fam.insert(x.to_string()) {
                        q.push_back((x.to_string(), 1));
                    }
                }
            }
        }
        while let Some((w, d)) = q.pop_front() {
            if fam.len() >= FAMILY_CAP {
                break;
            }
            if expanded.contains(&w) {
                // resume: replay children already on disk
                for e in old_edges.iter().filter(|e| e["parent"] == w.as_str()) {
                    let c = e["child"].as_str().unwrap_or("").to_string();
                    if d < 12 && fam.insert(c.clone()) {
                        q.push_back((c, d + 1));
                    }
                }
                continue;
            }
            if total >= max_wallets {
                break;
            }
            expanded.insert(w.clone());
            total += 1;
            let pages_max = if w == *root { 10 } else { 3 };
            let (mut token, mut pages, mut ntx): (Option<String>, u32, usize) = (None, 0, 0);
            let mut kids: Vec<(String, u64)> = Vec::new();
            let mut asc = false; // busy wallet: one extra page of its OLDEST txs (create + first moves live there)
            loop {
                let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
                    "filters": {"status": "succeeded", "blockTime": {"gte": SINCE}}});
                if asc {
                    o["sortOrder"] = json!("asc");
                } else if let Some(t) = &token {
                    o["paginationToken"] = json!(t);
                }
                let r = match rpc.call("getTransactionsForAddress", json!([w, o]), 10) {
                    Ok(r) => r,
                    Err(e) if e.starts_with("BUDGET") => {
                        rpc.save();
                        return Err(e);
                    }
                    Err(_) => break,
                };
                pages += 1;
                let data = r["data"].as_array().cloned().unwrap_or_default();
                ntx += data.len();
                for tx in &data {
                    let sig = tx["transaction"]["signatures"][0].as_str().unwrap_or("");
                    for (c, lam) in transfers(tx, &w) {
                        if !bad.contains_key(&c) {
                            writeln!(fe, "{}", json!({"root": root, "parent": w, "child": c, "sol": lam as f64 / 1e9, "bt": tx["blockTime"], "sig": sig})).map_err(|e| e.to_string())?;
                            kids.push((c, lam));
                        }
                    }
                    for e in decode::events(tx).0 {
                        if let decode::Ev::Create(c) = e {
                            if c.user == w || c.creator.as_deref() == Some(w.as_str()) {
                                writeln!(fc, "{}", json!({"root": root, "w": w, "mint": c.mint, "symbol": c.symbol, "bt": tx["blockTime"]})).map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }
                if asc {
                    break;
                }
                token = r["paginationToken"].as_str().map(String::from);
                if data.is_empty() || token.is_none() || pages >= pages_max {
                    if token.is_some() && !data.is_empty() {
                        asc = true;
                        continue;
                    }
                    break;
                }
            }
            let busy = token.is_some() && w != *root; // more history than we read -> service/bot/trader, do not expand
            writeln!(fw, "{}", json!({"root": root, "w": w, "depth": d, "ntx": ntx, "busy": busy, "kids": kids.len()})).map_err(|e| e.to_string())?;
            if !busy && d < 12 {
                for (c, _) in kids {
                    if fam.insert(c.clone()) {
                        q.push_back((c, d + 1));
                    }
                }
            }
            if total.is_multiple_of(25) {
                rpc.save();
                println!("wallets={total} root={} fam={} credits_total={}", &root[..8], fam.len(), rpc.used());
            }
        }
    }
    rpc.save();
    println!("DOWN_DONE wallets={total} credits_total={}", rpc.used());
    report(out_dir)
}

/// ATH and now from candles already on disk (no download). Returns (ath_x_from_grad, ath_usd, mcap_now_usd).
fn ath(mint: &str, mcap_now: f64) -> Option<(f64, f64)> {
    let v: Value = serde_json::from_str(&fs::read_to_string(format!("data/roots/gt/{mint}.hour.json")).ok()?).ok()?;
    let mut c: Vec<(i64, f64)> = v["data"]["attributes"]["ohlcv_list"].as_array()?.iter()
        .filter_map(|x| Some((x[0].as_i64()?, x[4].as_f64()?))).filter(|x| x.1 > 0.0).collect();
    if c.len() < 2 {
        return None;
    }
    c.sort_by_key(|x| x.0);
    let now = c.last()?.1;
    let hi = c.iter().map(|x| x.1).fold(0.0, f64::max);
    let usd = if mcap_now > 0.0 { mcap_now / now * hi } else { 0.0 };
    Some((hi / GRAD_PX, usd))
}

pub fn report(out_dir: &str) -> Result<(), String> {
    let ups = read_lines(&format!("{out_dir}/up.jsonl"));
    let edges = read_lines(&format!("{out_dir}/edges.jsonl"));
    let creates = read_lines(&format!("{out_dir}/creates.jsonl"));
    let wallets = read_lines(&format!("{out_dir}/wallets.jsonl"));
    let mut grad: HashSet<String> = HashSet::new();
    let pcs = read_lines("data/roots/pool_creators.jsonl");
    for v in &pcs {
        if let Some(m) = v["mint"].as_str() {
            grad.insert(m.to_string());
        }
    }
    let mut ds: HashMap<String, (String, f64)> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), (v["sym"].as_str().unwrap_or("").to_string(), v["mcap"].as_f64().unwrap_or(0.0)));
    }
    let max_bt = read_lines("data/roots/roles.jsonl").iter().filter_map(|v| v["bt"].as_i64()).max().unwrap_or(0);
    let mut by_root: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for u in &ups {
        let traced = u["induk"] != u["dev"] && u["end"] != "wallet_ramai";
        if traced {
            by_root.entry(u["induk"].as_str().unwrap_or("").to_string()).or_default().push(u);
        }
    }
    let mut out = String::new();
    let mut summary: Vec<(usize, String)> = Vec::new();
    for (root, us) in &by_root {
        let fam_w: HashSet<&str> = wallets.iter().filter(|w| w["root"] == root.as_str()).filter_map(|w| w["w"].as_str()).collect();
        let busy = wallets.iter().filter(|w| w["root"] == root.as_str() && w["busy"] == true).count();
        let mut coins: BTreeMap<String, (String, String, i64)> = BTreeMap::new();
        for c in creates.iter().filter(|c| c["root"] == root.as_str()) {
            coins.entry(c["mint"].as_str().unwrap_or("").to_string()).or_insert((c["symbol"].as_str().unwrap_or("").to_string(), c["w"].as_str().unwrap_or("").to_string(), c["bt"].as_i64().unwrap_or(0)));
        }
        let devs_up: Vec<&str> = us.iter().filter_map(|u| u["dev"].as_str()).collect();
        // coins of the seed devs already known from the graduated list (their create may be older than the pages read)
        for v in pcs.iter().filter(|v| devs_up.contains(&v["coin_creator"].as_str().unwrap_or(""))) {
            let m = v["mint"].as_str().unwrap_or("").to_string();
            let sym = ds.get(&m).map(|x| x.0.clone()).unwrap_or_default();
            coins.entry(m).or_insert((sym, v["coin_creator"].as_str().unwrap_or("").to_string(), 0));
        }
        let n_grad = coins.keys().filter(|m| grad.contains(*m)).count();
        let n_ds = coins.keys().filter(|m| ds.contains_key(*m)).count();
        let n_new = coins.values().filter(|c| c.2 > max_bt).count();
        let n_fail = coins.len() - n_grad - n_new.min(coins.len() - n_grad);
        let end = us[0]["end"].as_str().unwrap_or("");
        out.push_str(&format!("\n=== FAMILY induk {root}\n  ujung atas: {end} | wallet keluarga: {} (ramai/tak dibuka: {busy}) | koin dibuat: {} | lulus: {n_grad} | DexScreener: {n_ds} | gagal lulus: {n_fail} | terlalu baru (>{max_bt}): {n_new}\n",
            fam_w.len(), coins.len()));
        for u in us {
            let chain: Vec<String> = u["chain"].as_array().into_iter().flatten()
                .map(|c| format!("{}({}tx,{:.2}SOL)", &c["w"].as_str().unwrap_or("")[..8], c["n_sigs"], c["sol"].as_f64().unwrap_or(0.0))).collect();
            let dev = u["dev"].as_str().unwrap_or("");
            let back: f64 = edges.iter().filter(|e| e["parent"] == dev && fam_w.contains(e["child"].as_str().unwrap_or(""))).filter_map(|e| e["sol"].as_f64()).sum();
            let dev_coins = creates.iter().filter(|c| c["w"] == dev).count();
            let c0 = &u["chain"][0];
            let perantara = c0["n_sigs"].as_u64().unwrap_or(9999) < 300 && dev_coins <= 3 && u["levels"].as_u64().unwrap_or(0) >= 2;
            out.push_str(&format!("  dev {} | {} | koin oleh dev: {dev_coins} | SOL balik ke keluarga: {back:.1} | rantai: {}\n",
                &dev[..8], if perantara { "PERANTARA (wallet sekali pakai)" } else { "dev aktif/lama" }, chain.join(" <- ")));
        }
        let mut rows: Vec<(f64, String)> = Vec::new();
        for (m, (sym, w, bt)) in &coins {
            let (st, mc) = match ds.get(m) {
                Some((_, mc)) => ("DEXSCREENER", *mc),
                None if grad.contains(m) => ("lulus, hilang", 0.0),
                None if *bt > max_bt => ("baru (belum dinilai)", 0.0),
                None => ("GAGAL lulus", 0.0),
            };
            let a = ath(m, mc);
            let ath_s = a.map(|(x, usd)| format!("ATH {:.0}x dari lulus (~${:.0}k)", x, usd / 1e3)).unwrap_or_else(|| "ATH -".into());
            rows.push((mc, format!("    {sym:<12} {m} | {st} | mcap ${:.0}k | {ath_s} | dibuat oleh {} | bt {bt}\n", mc / 1e3, &w[..8])));
        }
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for r in rows {
            out.push_str(&r.1);
        }
        summary.push((coins.len(), format!("{} | wallet {} | koin {} | lulus {n_grad} | DS {n_ds} | gagal {n_fail} | dev {}\n", &root[..8], fam_w.len(), coins.len(), devs_up.len())));
    }
    summary.sort_by(|a, b| b.0.cmp(&a.0));
    let head: String = summary.into_iter().map(|s| s.1).collect();
    let txt = format!("RINGKASAN FAMILY (induk | wallet | koin | lulus | DexScreener | gagal | dev awal)\n{head}{out}");
    fs::write(format!("{out_dir}/report.txt"), &txt).map_err(|e| e.to_string())?;
    println!("REPORT {out_dir}/report.txt families={}", by_root.len());
    Ok(())
}
