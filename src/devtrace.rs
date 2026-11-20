//! V3 Stage 2 (owner 20 Nov 2026): for every QUALIFIED coin (new, active, marketCap >= tier floor; highest tier
//! first) find its creator from the create transaction and the creator's full launch history: every Pump.fun coin
//! the creator made, the creator's own buys/sells on it, and what each coin looks like on DexScreener now.
//! Roles stay separate (V3 6 / T5): `creator` = CreateEvent.creator only; if the event has no creator field it is
//! UNKNOWN (user, fee payer and signers are stored, never copied into creator).
//! Labels are descriptive with the raw numbers next to them (config/devtrace.toml, version logged); they are not
//! tuned on outcomes and are not a trading signal.
//! Usage: jaringan devtrace <universe_dir> <out_dir> <loop 0|1>
use crate::decode::{self, Ev};
use crate::rpc::{cfg_u64, Rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

const CFG: &str = "config/devtrace.toml";

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// One write() per line (O_APPEND): parallel shards append to the same files without interleaving lines
/// (writeln! on a File issues many small writes; that is how fam2 edges.jsonl got corrupted on 20 Nov).
fn append(path: &str, v: &Value) -> Result<(), String> {
    let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    f.write_all(format!("{v}\n").as_bytes()).map_err(|e| e.to_string())
}

/// Roles from the oldest transaction of a mint.
pub fn roles(tx: &Value, mint: &str) -> Value {
    let keys = tx["transaction"]["message"]["accountKeys"].as_array().cloned().unwrap_or_default();
    let signers: Vec<String> = keys.iter().filter(|k| k["signer"] == true).filter_map(|k| k["pubkey"].as_str().map(String::from)).collect();
    let fee_payer = keys.first().and_then(|k| k["pubkey"].as_str()).unwrap_or("").to_string();
    let mut progs: Vec<String> = tx["transaction"]["message"]["instructions"].as_array().into_iter().flatten().filter_map(|i| i["programId"].as_str().map(String::from)).collect();
    for g in tx["meta"]["innerInstructions"].as_array().into_iter().flatten() {
        progs.extend(g["instructions"].as_array().into_iter().flatten().filter_map(|i| i["programId"].as_str().map(String::from)));
    }
    let ev = decode::events(tx).0.into_iter().find_map(|e| match e {
        Ev::Create(c) if c.mint == mint => Some(c),
        _ => None,
    });
    let base = json!({"mint": mint, "venue": crate::svc::launchpad(&progs), "sig": tx["transaction"]["signatures"][0], "slot": tx["slot"],
        "bt": tx["blockTime"], "fee_payer": fee_payer, "signers": signers, "failed": !tx["meta"]["err"].is_null()});
    let mut r = base;
    match ev {
        Some(c) => {
            r["creator"] = json!(c.creator.clone().unwrap_or_else(|| "UNKNOWN".into()));
            r["create_user"] = json!(c.user);
            r["symbol"] = json!(c.symbol);
            r["evidence"] = json!(if c.creator.is_some() { "Pump CreateEvent.creator in oldest tx" } else { "Pump CreateEvent without creator field" });
        }
        None => {
            r["creator"] = json!("UNKNOWN");
            r["evidence"] = json!("no Pump CreateEvent for this mint in its oldest tx (other launchpad or not a create)");
        }
    }
    r
}

#[derive(Default, Clone)]
pub struct CoinAcc {
    pub bt: i64,
    pub symbol: String,
    pub buy_sol: u64,
    pub sell_sol: u64,
    pub buy_tok: u64,
    pub sell_tok: u64,
    pub first_sell_bt: Option<i64>,
}

/// Fold one transaction of the dev wallet into per-coin accumulators: creates made by `w` (CreateEvent.creator
/// == w, or user == w when the event has no creator field) and w's own trades.
pub fn fold_tx(tx: &Value, w: &str, acc: &mut BTreeMap<String, CoinAcc>) {
    if !tx["meta"]["err"].is_null() {
        return;
    }
    let bt = tx["blockTime"].as_i64().unwrap_or(0);
    for e in decode::events(tx).0 {
        match e {
            Ev::Create(c) => {
                let mine = match &c.creator {
                    Some(cr) => cr == w,
                    None => c.user == w,
                };
                if mine {
                    let a = acc.entry(c.mint.clone()).or_default();
                    a.bt = bt;
                    a.symbol = c.symbol.clone();
                }
            }
            Ev::Trade(t) if t.user == w => {
                let a = acc.entry(t.mint.clone()).or_default();
                if t.buy {
                    a.buy_sol += t.sol;
                    a.buy_tok += t.tok;
                } else {
                    a.sell_sol += t.sol;
                    a.sell_tok += t.tok;
                    a.first_sell_bt = Some(a.first_sell_bt.map_or(bt, |x| x.min(bt)));
                }
            }
            _ => {}
        }
    }
}

/// Descriptive dev label from its created coins (each: on_dex, liq_usd, market_cap, fast_dump).
pub fn dev_label(coins: &[Value], min_liq: f64, good_mcap: f64, min_coins_pattern: usize, dump_share: f64) -> &'static str {
    let n = coins.len();
    if n <= 1 {
        return "BARU_1_KOIN";
    }
    let good = coins.iter().filter(|c| c["market_cap"].as_f64().unwrap_or(0.0) >= good_mcap && c["liq_usd"].as_f64().unwrap_or(0.0) >= min_liq).count();
    let dumps = coins.iter().filter(|c| c["fast_dump"] == true).count();
    let dump_rate = dumps as f64 / n as f64;
    if n >= min_coins_pattern && dump_rate >= dump_share {
        "POLA_RUG"
    } else if good >= 2 {
        "BAGUS"
    } else if good == 1 && dump_rate < dump_share {
        "CAMPUR"
    } else {
        "LEMAH"
    }
}

fn ds_batch(agent: &ureq::Agent, mints: &[String]) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    for chunk in mints.chunks(30) {
        let url = format!("https://api.dexscreener.com/tokens/v1/solana/{}", chunk.join(","));
        for attempt in 0..3u64 {
            sleep(Duration::from_millis(260));
            match agent.get(&url).call().map_err(|e| e.to_string()).and_then(|r| r.into_string().map_err(|e| e.to_string())) {
                Ok(s) => {
                    let pairs: Vec<Value> = serde_json::from_str::<Value>(&s).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                    for m in chunk {
                        if let Some(a) = crate::dexs::aggregate(m, &pairs, t) {
                            out.insert(m.clone(), a);
                        }
                    }
                    break;
                }
                Err(_) => sleep(Duration::from_secs(5 * (attempt + 1))),
            }
        }
    }
    out
}

pub fn run(uni: &str, out: &str, looping: bool) -> Result<(), String> {
    let max_pages = cfg_u64(CFG, "max_pages").ok_or("config/devtrace.toml: max_pages missing")? as u32;
    let fast_dump_s = cfg_u64(CFG, "fast_dump_s").ok_or("fast_dump_s missing")? as i64;
    let dump_pct = cfg_u64(CFG, "fast_dump_sold_pct").ok_or("fast_dump_sold_pct missing")? as f64 / 100.0;
    let min_liq = cfg_u64(CFG, "alive_min_liq_usd").ok_or("alive_min_liq_usd missing")? as f64;
    let good_mcap = cfg_u64(CFG, "good_mcap_usd").ok_or("good_mcap_usd missing")? as f64;
    let min_pat = cfg_u64(CFG, "pattern_min_coins").ok_or("pattern_min_coins missing")? as usize;
    let dump_share = cfg_u64(CFG, "pattern_dump_share_pct").ok_or("pattern_dump_share_pct missing")? as f64 / 100.0;
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let (roles_p, devs_p) = (format!("{out}/roles.jsonl"), format!("{out}/devs.jsonl"));
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(40)).build();
    // JAR_SHARD=i/n (21 Nov): parallel workers; a worker takes the mints and devs whose address byte sum % n == i
    let (si, sn) = std::env::var("JAR_SHARD").ok().and_then(|s| {
        let (a, b) = s.split_once('/')?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?.max(1)))
    }).unwrap_or((0, 1));
    let mine = |k: &str| k.bytes().map(|b| b as usize).sum::<usize>() % sn == si;
    let mut rpc = Rpc::new(&if sn > 1 { format!("devtrace{si}") } else { "devtrace".to_string() })?;
    loop {
        let done_mints: HashSet<String> = read_jsonl(&roles_p).iter().filter_map(|v| v["mint"].as_str().map(String::from)).collect();
        // a dev is done when its history is complete or the oldest-first pass already ran (v2); older truncated
        // rows are redone and the newest row per dev is the valid one
        let mut done_devs: HashSet<String> = read_jsonl(&devs_p)
            .iter()
            .filter(|v| v["history_complete"] == true || v["asc_pass"] == true)
            .filter_map(|v| v["dev"].as_str().map(String::from))
            .collect();
        let redo: Vec<(String, String, f64)> = {
            let rows = read_jsonl(&devs_p);
            let mut seen = HashSet::new();
            rows.iter()
                .rev()
                .filter_map(|v| Some((v["dev"].as_str()?.to_string(), v["from_mint"].as_str()?.to_string(), v["tier"].as_f64()?)))
                .filter(|(d, _, _)| !done_devs.contains(d) && seen.insert(d.clone()))
                .collect()
        };
        for (d, _, _) in &redo {
            done_devs.insert(d.clone());
        }
        // best tier per qualified mint, highest first
        let mut q: HashMap<String, f64> = HashMap::new();
        let mut fb: HashMap<String, f64> = HashMap::new();
        for v in read_jsonl(&format!("{uni}/qualified.jsonl")) {
            if let (Some(m), Some(t)) = (v["mint"].as_str(), v["tier"].as_f64()) {
                let e = q.entry(m.to_string()).or_insert(0.0);
                *e = e.max(t);
            }
        }
        // v7 (owner 21 Nov: "gua mau creator coinnya ada di DexScreener"): every coin seen on DexScreener (snap rows +
        // qualified); with ds_creators_only = 1 only creators of such coins get a history run
        let ds_only = cfg_u64(CFG, "ds_creators_only").unwrap_or(0) == 1;
        let ds_mints: HashSet<String> = read_jsonl(&format!("{uni}/snap.jsonl")).iter().filter_map(|v| v["mint"].as_str().map(String::from))
            .chain(q.keys().cloned()).collect();
        let roles_rows = read_jsonl(&roles_p);
        let ds_creators: HashSet<String> = roles_rows.iter().filter(|v| v["mint"].as_str().is_some_and(|m| ds_mints.contains(m)))
            .filter_map(|v| v["creator"].as_str().map(String::from)).collect();
        // owner 20 Nov (goal 1000 families): after the qualified coins, also coins below the tier floor, highest
        // marketCap seen first, down to fallback_min_mcap_usd (priority value = max marketCap seen; tier stays 0)
        let floor = cfg_u64(CFG, "fallback_min_mcap_usd").unwrap_or(u64::MAX) as f64;
        for v in read_jsonl(&format!("{uni}/snap.jsonl")) {
            if let (Some(m), Some(mc)) = (v["mint"].as_str(), v["market_cap"].as_f64()) {
                if mc >= floor && !q.contains_key(m) {
                    // v5 (owner 21 Nov): coins whose DexScreener pair is <= 24 h old and still trading go first
                    let fresh = v["pair_created_ms"].as_i64().is_some_and(|c| v["t"].as_i64().unwrap_or(0) - c / 1000 <= 86_400)
                        && v["txns_h1"].as_u64().unwrap_or(0) > 0;
                    let pri = if fresh { 1e12 + mc } else { mc };
                    let e = fb.entry(m.to_string()).or_insert(0.0);
                    *e = e.max(pri);
                }
            }
        }
        let mut todo: Vec<(String, f64)> = q.into_iter().filter(|(m, _)| !done_mints.contains(m)).collect();
        todo.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut rest: Vec<(String, f64)> = fb.iter().filter(|(m, _)| !done_mints.contains(*m)).map(|(m, mc)| (m.clone(), *mc)).collect();
        rest.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        todo.extend(rest.into_iter().map(|(m, _)| (m, 0.0)));
        // v3 (21 Nov, goal 1000 active families): then every coin that graduated within the window (a Pump.fun
        // graduation completes the curve, i.e. the coin reached the DEX), newest first
        // v4 (owner 21 Nov: "wallet berkualitas, bukan asal"): the all-graduated fallback is OFF unless
        // graduated_all = 1; it traced thousands of low-quality coins before any dev history ran
        if cfg_u64(CFG, "graduated_all").unwrap_or(0) == 1 {
            let have: HashSet<String> = todo.iter().map(|t| t.0.clone()).chain(done_mints.iter().cloned()).collect();
            let mut grads: Vec<(String, i64)> = read_jsonl(&format!("{uni}/migrations.jsonl")).iter()
                .filter_map(|v| Some((v["mint"].as_str()?.to_string(), v["bt"].as_i64()?)))
                .filter(|(m, _)| !have.contains(m)).collect();
            grads.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            grads.dedup_by(|a, b| a.0 == b.0);
            todo.extend(grads.into_iter().map(|(m, _)| (m, 0.0)));
        }
        // roles are resolved in batches so dev histories start early (best coins first)
        let rb = cfg_u64(CFG, "roles_batch").unwrap_or(200) as usize;
        todo.retain(|(m, _)| mine(m));
        todo.truncate(rb);
        let mut new_devs = 0usize;
        let mut jobs: Vec<(String, String, f64)> = Vec::new();
        // quality devs seeded from outside lists (e.g. MadeOnSol leaderboard / bonds with high peak), best first
        let mut seeds: Vec<(String, String, f64)> = read_jsonl(&format!("{uni}/quality_devs.jsonl")).iter()
            .filter_map(|v| Some((v["dev"].as_str()?.to_string(), v["mint"].as_str().unwrap_or("").to_string(), v["peak_mc"].as_f64().unwrap_or(0.0))))
            .collect();
        seeds.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        for (d, m, p) in seeds {
            if mine(&d) && done_devs.insert(d.clone()) {
                jobs.push((d, m, p));
            }
        }
        for (m, tier) in &todo {
            let r = match rpc.call("getTransactionsForAddress", json!([m, {"transactionDetails": "full", "limit": 1, "sortOrder": "asc", "encoding": "jsonParsed",
                "maxSupportedTransactionVersion": 1}]), 10) {
                Ok(r) => r,
                Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                Err(e) => {
                    append(&roles_p, &json!({"mint": m, "tier": tier, "error": e.chars().take(120).collect::<String>()}))?;
                    continue;
                }
            };
            let Some(tx) = r["data"].as_array().and_then(|a| a.first()).cloned() else {
                append(&roles_p, &json!({"mint": m, "tier": tier, "error": "no_tx"}))?;
                continue;
            };
            let mut ro = roles(&tx, m);
            ro["tier"] = json!(tier);
            append(&roles_p, &ro)?;
            let dev = ro["creator"].as_str().unwrap_or("UNKNOWN").to_string();
            // a creator owned by another worker is left to it (it picks the roles row up as an orphan)
            if dev != "UNKNOWN" && mine(&dev) && done_devs.insert(dev.clone()) {
                jobs.push((dev, m.clone(), *tier));
            }
        }
        // v6 (21 Nov): creators whose coin roles were resolved in an earlier loop but whose history never ran (the
        // process stopped between the roles batch and the histories, e.g. the v3 all-graduated pass left ~7.8k
        // creators untraced; their mints count as done so they never came back). Best coin first, orphan_batch per loop.
        let mut orphans: Vec<(String, String, f64, f64, i64)> = roles_rows.iter()
            .filter_map(|v| {
                let m = v["mint"].as_str()?;
                Some((v["creator"].as_str()?.to_string(), m.to_string(), v["tier"].as_f64().unwrap_or(0.0),
                    fb.get(m).copied().unwrap_or(0.0), v["bt"].as_i64().unwrap_or(0)))
            })
            .filter(|(d, ..)| d != "UNKNOWN" && mine(d) && !done_devs.contains(d) && (!ds_only || ds_creators.contains(d)))
            .collect();
        orphans.sort_by(|a, b| b.2.total_cmp(&a.2).then(b.3.total_cmp(&a.3)).then(b.4.cmp(&a.4)).then(a.0.cmp(&b.0)));
        let ob = cfg_u64(CFG, "orphan_batch").unwrap_or(200) as usize;
        let mut n_orphan = 0usize;
        for (d, m, t, _, _) in orphans {
            if n_orphan >= ob {
                break;
            }
            if done_devs.insert(d.clone()) {
                jobs.push((d, m, t));
                n_orphan += 1;
            }
        }
        let redo: Vec<(String, String, f64)> =
            redo.into_iter().filter(|(d, m, _)| mine(d) && (!ds_only || ds_creators.contains(d) || ds_mints.contains(m))).collect();
        for (dev, m, tier) in redo.into_iter().chain(jobs) {
            // launch history of the dev: newest first (max_pages x 100 txs); if that does not reach the wallet's first
            // tx, a second pass oldest first (max_pages x 100). Txs are counted once (dedup by signature). Complete =
            // the newest-first pass ended, or the two passes overlap.
            let mut acc: BTreeMap<String, CoinAcc> = BTreeMap::new();
            let mut sigs: HashSet<String> = HashSet::new();
            let (mut pages, mut complete, mut ntx, mut asc_pass) = (0u32, false, 0usize, false);
            for order in ["desc", "asc"] {
                if order == "asc" {
                    if complete {
                        break;
                    }
                    asc_pass = true;
                }
                let (mut token, mut p) = (None::<String>, 0u32);
                'pages: while p < max_pages {
                    let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1, "sortOrder": order});
                    if let Some(t) = &token {
                        o["paginationToken"] = json!(t);
                    }
                    let r = match rpc.call("getTransactionsForAddress", json!([dev, o]), 10) {
                        Ok(r) => r,
                        Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                        Err(_) => break,
                    };
                    p += 1;
                    pages += 1;
                    let data = r["data"].as_array().cloned().unwrap_or_default();
                    for t in &data {
                        let s = t["transaction"]["signatures"][0].as_str().unwrap_or("").to_string();
                        if !sigs.insert(s) {
                            complete = true; // asc pass reached a tx the desc pass already read
                            break 'pages;
                        }
                        ntx += 1;
                        fold_tx(t, &dev, &mut acc);
                    }
                    token = r["paginationToken"].as_str().map(String::from);
                    if data.is_empty() || token.is_none() {
                        complete = true;
                        break;
                    }
                }
            }
            let created: Vec<String> = acc.iter().filter(|(_, a)| a.bt > 0).map(|(k, _)| k.clone()).collect();
            let ds = ds_batch(&agent, &created);
            let mut coins = Vec::new();
            for mint in &created {
                let a = &acc[mint];
                let sold_share = if a.buy_tok > 0 { a.sell_tok as f64 / a.buy_tok as f64 } else { 0.0 };
                let fast = a.first_sell_bt.map(|s| s - a.bt <= fast_dump_s).unwrap_or(false) && sold_share >= dump_pct;
                let d = ds.get(mint);
                coins.push(json!({"mint": mint, "symbol": a.symbol, "bt": a.bt,
                    "dev_buy_sol": a.buy_sol as f64 / 1e9, "dev_sell_sol": a.sell_sol as f64 / 1e9, "dev_sold_share": sold_share,
                    "dev_first_sell_after_s": a.first_sell_bt.map(|s| s - a.bt), "fast_dump": fast,
                    "on_dex": d.is_some(), "market_cap": d.and_then(|x| x["market_cap"].as_f64()), "fdv": d.and_then(|x| x["fdv"].as_f64()),
                    "liq_usd": d.and_then(|x| x["liq_usd"].as_f64()), "dex": d.map(|x| x["dex"].clone())}));
            }
            coins.sort_by_key(|c| c["bt"].as_i64().unwrap_or(0));
            let label = dev_label(&coins, min_liq, good_mcap, min_pat, dump_share);
            let row = json!({"dev": dev, "from_mint": m, "tier": tier, "pages": pages, "txs_read": ntx, "history_complete": complete, "asc_pass": asc_pass,
                "n_coins": coins.len(), "n_on_dex": coins.iter().filter(|c| c["on_dex"] == true).count(),
                "n_fast_dump": coins.iter().filter(|c| c["fast_dump"] == true).count(), "label": label,
                "label_note": if complete { "full history" } else { "history truncated at max_pages: counts are a lower bound" },
                "config_version": cfg_u64(CFG, "version"), "coins": coins});
            append(&devs_p, &row)?;
            new_devs += 1;
            rpc.save();
        }
        rpc.save();
        println!("{}", json!({"todo": todo.len(), "orphans": n_orphan, "new_devs": new_devs, "credits_total": rpc.used()}));
        if !looping {
            return Ok(());
        }
        // backlog left (a full batch ran): go again at once; otherwise wait for new coins
        if todo.len() < rb && n_orphan < ob {
            sleep(Duration::from_secs(300));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(mc: f64, liq: f64, dump: bool) -> Value {
        json!({"market_cap": mc, "liq_usd": liq, "fast_dump": dump})
    }
    #[test]
    fn labels_from_counts() {
        assert_eq!(dev_label(&[c(1e6, 1e5, false)], 5e3, 5e4, 3, 0.5), "BARU_1_KOIN");
        assert_eq!(dev_label(&[c(0.0, 0.0, true), c(0.0, 0.0, true), c(1e6, 1e5, false)], 5e3, 5e4, 3, 0.5), "POLA_RUG");
        assert_eq!(dev_label(&[c(1e5, 1e4, false), c(6e4, 6e3, false), c(0.0, 0.0, true)], 5e3, 5e4, 3, 0.5), "BAGUS");
        assert_eq!(dev_label(&[c(1e5, 1e4, false), c(0.0, 0.0, false)], 5e3, 5e4, 3, 0.5), "CAMPUR");
        assert_eq!(dev_label(&[c(1e5, 100.0, false), c(0.0, 0.0, false)], 5e3, 5e4, 3, 0.5), "LEMAH");
    }
    #[test]
    fn failed_tx_and_foreign_trades_ignored() {
        let mut acc = BTreeMap::new();
        let tx = json!({"meta": {"err": {"x": 1}}, "blockTime": 5});
        fold_tx(&tx, "W", &mut acc);
        assert!(acc.is_empty());
        let tx = json!({"meta": {"err": null, "innerInstructions": []}, "blockTime": 5, "transaction": {"message": {"instructions": []}}});
        fold_tx(&tx, "W", &mut acc);
        assert!(acc.is_empty());
    }
}
