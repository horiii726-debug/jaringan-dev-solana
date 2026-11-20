//! Rise and rug per coin for every coin made by a family dev, from the start of the dev's history (owner 21 Nov:
//! "kenaikan coin yang dia bikin: banyak naik atau banyak rugpull, paling kecil, paling besar, total").
//! On-chain, Pump.fun bonding curve (decode::events on the mint's own transactions, oldest first):
//!   entry  = price after the first trade in a slot AFTER the create slot (reachable for a buyer, V3 T13)
//!   peak   = highest post-trade curve price; a graduated coin's curve ends at its highest point (the AMM part
//!            after graduation is not decoded here -> peak is a LOWER BOUND for graduated coins, flagged)
//!   rise_x = peak / entry ; low_x = lowest post-entry price / entry (how far it fell below our entry)
//!   rug    = not graduated AND last price <= rug_last_pct of peak, or the dev fast-dumped (devtrace flag)
//! Pages per coin are capped (config/coinrise.toml); a coin whose history was cut is flagged truncated.
//! Usage: jaringan coinrise <family_dir> <dev_dir> <out_dir> <loop 0|1>
use crate::decode::{self, Ev};
use crate::rpc::{cfg_u64, Rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

const CFG: &str = "config/coinrise.toml";

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

#[derive(Default, Debug, Clone)]
pub struct Curve {
    pub create_slot: Option<u64>,
    pub create_bt: Option<i64>,
    pub entry: Option<f64>,
    pub entry_bt: Option<i64>,
    pub peak: f64,
    pub peak_bt: Option<i64>,
    pub low_after_entry: Option<f64>,
    pub last: Option<f64>,
    pub last_bt: Option<i64>,
    pub graduated: bool,
    pub n_trades: u64,
}

impl Curve {
    /// Feed one successful transaction (oldest first).
    pub fn feed(&mut self, tx: &Value, mint: &str) {
        if !tx["meta"]["err"].is_null() {
            return;
        }
        let slot = tx["slot"].as_u64().unwrap_or(0);
        let bt = tx["blockTime"].as_i64();
        for e in decode::events(tx).0 {
            match e {
                Ev::Create(c) if c.mint == mint => {
                    self.create_slot = Some(slot);
                    self.create_bt = bt;
                }
                Ev::Trade(t) if t.mint == mint && t.vtok > 0 => {
                    let p = t.vsol as f64 / t.vtok as f64;
                    self.n_trades += 1;
                    if self.entry.is_none() && self.create_slot.is_some_and(|cs| slot > cs) {
                        self.entry = Some(p);
                        self.entry_bt = bt;
                    }
                    if self.entry.is_some() {
                        self.low_after_entry = Some(self.low_after_entry.map_or(p, |l| l.min(p)));
                    }
                    if p > self.peak {
                        self.peak = p;
                        self.peak_bt = bt;
                    }
                    self.last = Some(p);
                    self.last_bt = bt;
                }
                Ev::Complete(m) if m == mint => self.graduated = true,
                _ => {}
            }
        }
    }
}

/// Summary of rises: (n, min, median, max, mean, n>=2x, n>=10x) over finite values.
pub fn stats(xs: &[f64]) -> Value {
    let mut v: Vec<f64> = xs.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() {
        return json!({"n": 0});
    }
    json!({"n": v.len(), "min_x": v[0], "median_x": v[v.len() / 2], "max_x": v[v.len() - 1], "mean_x": v.iter().sum::<f64>() / v.len() as f64,
        "n_ge2x": v.iter().filter(|x| **x >= 2.0).count(), "n_ge10x": v.iter().filter(|x| **x >= 10.0).count(), "n_lt1x": v.iter().filter(|x| **x < 1.0001).count()})
}

/// Seconds from the dev's create to its first sell: min / median / max and buckets; `never` = no sell seen.
pub fn sell_stats(xs: &[i64], never: usize) -> Value {
    let mut v = xs.to_vec();
    v.sort();
    if v.is_empty() {
        return json!({"n_sold": 0, "n_never_sold": never});
    }
    let c = |lim: i64| v.iter().filter(|s| **s <= lim).count();
    json!({"n_sold": v.len(), "n_never_sold": never, "min_s": v[0], "median_s": v[v.len() / 2], "max_s": v[v.len() - 1],
        "le_5s": c(5), "le_60s": c(60), "le_300s": c(300), "le_3600s": c(3600)})
}

pub fn run(fam: &str, dev_dir: &str, out: &str, looping: bool) -> Result<(), String> {
    let max_pages = cfg_u64(CFG, "max_pages").ok_or("config/coinrise.toml: max_pages missing")? as u32;
    let rug_last = cfg_u64(CFG, "rug_last_pct").ok_or("rug_last_pct missing")? as f64 / 100.0;
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let coins_p = format!("{out}/coins.jsonl");
    let mut rpc = Rpc::new("coinrise")?;
    loop {
        let mut done: HashMap<String, Value> = HashMap::new();
        for v in read_jsonl(&coins_p) {
            if let Some(m) = v["mint"].as_str() {
                done.insert(m.to_string(), v);
            }
        }
        // newest devtrace row per dev
        let mut dev_rows: HashMap<String, Value> = HashMap::new();
        for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
            if let Some(d) = v["dev"].as_str() {
                dev_rows.insert(d.to_string(), v);
            }
        }
        // owner 21 Nov: "ALLDEVS" = every traced dev (one entry per dev), not only family devs
        let fams: Vec<Value> = if fam == "ALLDEVS" {
            let mut ds: Vec<&String> = dev_rows.keys().collect();
            ds.sort();
            // owner 21 Nov: only creators of DexScreener coins (JAR_SCOPE_FILE, written by reput)
            if let Some(sc) = crate::reput::scope_set() {
                ds.retain(|d| sc.contains(*d));
            }
            ds.into_iter().map(|d| json!({"root": d, "n_wallets": 1, "n_devs": 1, "devs": [{"dev": d}], "members": [d]})).collect()
        } else {
            read_jsonl(&format!("{fam}/families.jsonl"))
        };
        let mut f = OpenOptions::new().create(true).append(true).open(&coins_p).map_err(|e| e.to_string())?;
        let mut n_new = 0usize;
        for fm in &fams {
            for d in fm["devs"].as_array().into_iter().flatten() {
                let dev = d["dev"].as_str().or(d.as_str()).unwrap_or("");
                let Some(row) = dev_rows.get(dev) else { continue };
                for c in row["coins"].as_array().into_iter().flatten() {
                    let Some(mint) = c["mint"].as_str() else { continue };
                    if done.contains_key(mint) {
                        continue;
                    }
                    let mut cv = Curve::default();
                    let (mut token, mut pages, mut complete) = (None::<String>, 0u32, false);
                    while pages < max_pages {
                        let mut o = json!({"transactionDetails": "full", "limit": 100, "sortOrder": "asc", "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1});
                        if let Some(t) = &token {
                            o["paginationToken"] = json!(t);
                        }
                        let r = match rpc.call("getTransactionsForAddress", json!([mint, o]), 10) {
                            Ok(r) => r,
                            Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                            Err(_) => break,
                        };
                        pages += 1;
                        let data = r["data"].as_array().cloned().unwrap_or_default();
                        for tx in &data {
                            cv.feed(tx, mint);
                        }
                        token = r["paginationToken"].as_str().map(String::from);
                        if data.is_empty() || token.is_none() || cv.graduated {
                            complete = true;
                            break;
                        }
                    }
                    let rise = match (cv.entry, cv.peak) {
                        (Some(e), p) if e > 0.0 && p >= e => Some(p / e),
                        (Some(e), _) if e > 0.0 => Some(1.0),
                        _ => None,
                    };
                    let collapsed = !cv.graduated && cv.last.zip(Some(cv.peak)).is_some_and(|(l, p)| p > 0.0 && l <= rug_last * p);
                    let rug = collapsed || c["fast_dump"] == true;
                    let rowc = json!({"mint": mint, "dev": dev, "symbol": c["symbol"], "create_bt": cv.create_bt.or(c["bt"].as_i64()),
                        "pages": pages, "history_complete": complete, "n_trades": cv.n_trades, "graduated": cv.graduated,
                        "entry_price": cv.entry, "peak_price": cv.peak, "rise_x": rise, "peak_after_s": cv.peak_bt.zip(cv.entry_bt).map(|(p, e)| p - e),
                        "low_x": cv.low_after_entry.zip(cv.entry).map(|(l, e)| l / e), "last_x": cv.last.zip(cv.entry).map(|(l, e)| l / e),
                        "peak_is_lower_bound": cv.graduated, "collapsed": collapsed, "dev_fast_dump": c["fast_dump"], "rug": rug,
                        "peak_mcap_sol_curve": cv.peak * 1e6, "dex_market_cap_now": c["market_cap"]});
                    writeln!(f, "{rowc}").map_err(|e| e.to_string())?;
                    done.insert(mint.to_string(), rowc);
                    n_new += 1;
                    if n_new.is_multiple_of(50) {
                        rpc.save();
                    }
                }
            }
        }
        rpc.save();
        summarize(&fams, &dev_rows, &done, out)?;
        println!("{}", json!({"coins_new": n_new, "coins_total": done.len(), "credits_total": rpc.used()}));
        if !looping {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(300));
    }
}

/// Owner 21 Nov: a dev that sells fast on (almost) every coin is a serial rugger; its WHOLE family (every affiliated
/// wallet) leaves the watchlist. Rule (config/coinrise.toml): >= bl_min_coins coins and >= bl_fast_share_pct of
/// them with the dev's first sell <= bl_fast_s seconds after create (devtrace history).
pub fn serial_rugger(coins: &[Value], min_coins: usize, fast_s: i64, share: f64) -> Option<(usize, usize)> {
    let n = coins.len();
    let fast = coins.iter().filter(|c| c["dev_first_sell_after_s"].as_i64().is_some_and(|s| s <= fast_s)).count();
    (n >= min_coins && fast as f64 >= share * n as f64).then_some((fast, n))
}

fn watchlist(fams: &[Value], dev_rows: &HashMap<String, Value>, out: &str) -> Result<Value, String> {
    let c = |k: &str| cfg_u64(CFG, k).ok_or(format!("config/coinrise.toml: {k} missing"));
    let (min_coins, fast_s, share) = (c("bl_min_coins")? as usize, c("bl_fast_s")? as i64, c("bl_fast_share_pct")? as f64 / 100.0);
    let mut wf = fs::File::create(format!("{out}/watchlist.jsonl")).map_err(|e| e.to_string())?;
    let mut bf = fs::File::create(format!("{out}/blacklist.jsonl")).map_err(|e| e.to_string())?;
    let mut bad_wallets: Vec<String> = Vec::new();
    let (mut n_ok, mut n_bad) = (0usize, 0usize);
    for fm in fams {
        let mut bad: Vec<Value> = Vec::new();
        for d in fm["devs"].as_array().into_iter().flatten() {
            let dev = d["dev"].as_str().or(d.as_str()).unwrap_or("");
            let cs = dev_rows.get(dev).and_then(|r| r["coins"].as_array()).cloned().unwrap_or_default();
            if let Some((fast, n)) = serial_rugger(&cs, min_coins, fast_s, share) {
                bad.push(json!({"dev": dev, "coins": n, "fast_sell_coins": fast}));
            }
        }
        let row = json!({"family": fm["root"].as_str().or(fm["family_id"].as_str()), "n_wallets": fm["n_wallets"], "devs": fm["devs"], "members": fm["members"]});
        if bad.is_empty() {
            n_ok += 1;
            writeln!(wf, "{row}").map_err(|e| e.to_string())?;
        } else {
            n_bad += 1;
            let mut r = row.clone();
            r["reason"] = json!(format!("serial fast-sell dev: >= {}% of >= {} coins sold <= {} s", (share * 100.0) as u64, min_coins, fast_s));
            r["bad_devs"] = json!(bad);
            writeln!(bf, "{r}").map_err(|e| e.to_string())?;
            for m in fm["members"].as_array().into_iter().flatten().filter_map(|m| m.as_str()) {
                bad_wallets.push(m.to_string());
            }
        }
    }
    bad_wallets.sort();
    bad_wallets.dedup();
    fs::write(format!("{out}/blacklist_wallets.txt"), bad_wallets.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok(json!({"watchlist_families": n_ok, "blacklisted_families": n_bad, "blacklisted_wallets": bad_wallets.len()}))
}

fn summarize(fams: &[Value], dev_rows: &HashMap<String, Value>, coins: &HashMap<String, Value>, out: &str) -> Result<(), String> {
    let wl = watchlist(fams, dev_rows, out)?;
    let mut ff = fs::File::create(format!("{out}/family_rise.jsonl")).map_err(|e| e.to_string())?;
    let mut all_rise = Vec::new();
    let (mut tot, mut tot_rug, mut tot_grad, mut seen) = (0usize, 0usize, 0usize, HashSet::new());
    let mut by_label: BTreeMap<String, usize> = BTreeMap::new();
    let (mut all_sells, mut all_never, mut seen_sell) = (Vec::new(), 0usize, HashSet::new());
    for fm in fams {
        let mut rs = Vec::new();
        let (mut sells, mut never) = (Vec::new(), 0usize);
        let (mut n, mut nr, mut ng) = (0usize, 0usize, 0usize);
        let mut best: Option<&Value> = None;
        for d in fm["devs"].as_array().into_iter().flatten() {
            let dev = d["dev"].as_str().or(d.as_str()).unwrap_or("");
            for c in dev_rows.get(dev).and_then(|r| r["coins"].as_array()).into_iter().flatten() {
                let Some(row) = c["mint"].as_str().and_then(|m| coins.get(m)) else { continue };
                n += 1;
                // dev's first sell after its own create (seconds, devtrace); None = dev never sold in the history read
                match c["dev_first_sell_after_s"].as_i64() {
                    Some(s) => sells.push(s),
                    None => never += 1,
                }
                if seen_sell.insert(row["mint"].as_str().unwrap_or("").to_string()) {
                    match c["dev_first_sell_after_s"].as_i64() {
                        Some(s) => all_sells.push(s),
                        None => all_never += 1,
                    }
                }
                if row["rug"] == true {
                    nr += 1;
                }
                if row["graduated"] == true {
                    ng += 1;
                }
                if let Some(x) = row["rise_x"].as_f64() {
                    rs.push(x);
                    if best.is_none_or(|b| x > b["rise_x"].as_f64().unwrap_or(0.0)) {
                        best = Some(row);
                    }
                }
                if seen.insert(row["mint"].as_str().unwrap_or("").to_string()) {
                    tot += 1;
                    if row["rug"] == true {
                        tot_rug += 1;
                    }
                    if row["graduated"] == true {
                        tot_grad += 1;
                    }
                    if let Some(x) = row["rise_x"].as_f64() {
                        all_rise.push(x);
                    }
                }
            }
        }
        if n == 0 {
            continue;
        }
        let label = if nr * 2 > n { "BANYAK_RUG" } else if rs.iter().filter(|x| **x >= 2.0).count() * 2 > n { "BANYAK_NAIK" } else { "CAMPUR" };
        *by_label.entry(label.into()).or_insert(0) += 1;
        writeln!(ff, "{}", json!({"family": fm["root"].as_str().or(fm["family_id"].as_str()), "n_wallets": fm["n_wallets"], "n_devs": fm["n_devs"],
            "n_coins": n, "n_graduated": ng, "n_rug": nr, "rise": stats(&rs), "label": label, "dev_first_sell": sell_stats(&sells, never),
            "best": best.map(|b| json!({"mint": b["mint"], "symbol": b["symbol"], "rise_x": b["rise_x"], "graduated": b["graduated"]}))})).map_err(|e| e.to_string())?;
    }
    let rep = json!({"watchlist": wl, "families_with_coins": by_label.values().sum::<usize>(), "family_labels": by_label, "coins": tot, "coins_rug": tot_rug,
        "coins_graduated": tot_grad, "rise_all_coins": stats(&all_rise), "dev_first_sell_all_coins": sell_stats(&all_sells, all_never),
        "note": "rise from first reachable trade (slot after create); curve only, graduated peaks are lower bounds; thresholds config/coinrise.toml (provisional)"});
    fs::write(format!("{out}/summary.json"), rep.to_string()).map_err(|e| e.to_string())?;
    println!("{rep}");
    Ok(())
}

/// Rebuild watchlist / blacklist right now from the current families and dev histories (no provider calls).
pub fn watchlist_now(fam: &str, dev_dir: &str, out: &str) -> Result<(), String> {
    let mut dev_rows: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            dev_rows.insert(d.to_string(), v);
        }
    }
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let r = watchlist(&read_jsonl(&format!("{fam}/families.jsonl")), &dev_rows, out)?;
    println!("{r}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stats_basic() {
        let s = stats(&[1.0, 3.0, 12.0, f64::NAN]);
        assert_eq!(s["n"], 3);
        assert_eq!(s["min_x"], 1.0);
        assert_eq!(s["median_x"], 3.0);
        assert_eq!(s["max_x"], 12.0);
        assert_eq!(s["n_ge2x"], 2);
        assert_eq!(s["n_ge10x"], 1);
        assert_eq!(stats(&[])["n"], 0);
    }
    #[test]
    fn serial_rugger_rule() {
        let c = |s: Option<i64>| json!({"dev_first_sell_after_s": s});
        let four_fast = vec![c(Some(1)), c(Some(3)), c(Some(50)), c(Some(2))];
        assert_eq!(serial_rugger(&four_fast, 3, 60, 0.8), Some((4, 4)));
        let mixed = vec![c(Some(1)), c(Some(3)), c(None), c(Some(900))];
        assert_eq!(serial_rugger(&mixed, 3, 60, 0.8), None);
        assert_eq!(serial_rugger(&four_fast[..2], 3, 60, 0.8), None); // too few coins
    }
    #[test]
    fn failed_tx_ignored_and_no_entry_before_create() {
        let mut c = Curve::default();
        c.feed(&json!({"meta": {"err": {"x": 1}}, "slot": 5}), "M");
        assert!(c.entry.is_none() && c.n_trades == 0);
    }
}
