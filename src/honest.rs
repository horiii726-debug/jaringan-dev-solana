//! F3 + F4 (18 Nov review): what a real bot could have obtained.
//! Entry (reachable): the first market state with blockTime >= create_bt + latency AND slot > create_slot.
//!   Curve while the coin trades on the curve; PumpSwap pool if it graduated before that (same-slot bundle graduations
//!   always enter on the pool). We land right AFTER that trade (its post-state) = conservative. No trade after the
//!   latency -> NOT_FILLED (counted, never dropped).
//! Value at t (honest): post-state of the last trade at or before t, NO liquidity filter (a drained pool is worth what
//!   selling into it returns). If the data does not reach t -> CENSORED (excluded from win/mean, counted).
//!   Never the pre-graduation curve price for a graduated coin.
//! PumpSwap Buy/SellEvent pool reserves are PRE-trade (checked on the data: pb[i+1] == pb[i] -/+ base[i]); the
//! post-state is rebuilt from the event amounts.
use crate::graph::read_lines;
use crate::outcomes::{buy_out, parse_t, sell_out, T};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;

pub const X: u64 = 100_000_000; // 0.1 SOL
pub const TIP: f64 = 0.00146;
pub const TXC: f64 = 0.000_315;
pub const POOL_FEE_BPS: u64 = 125; // [U] until fees are read from the events (amm.rs stores them for new pulls)
pub const CURVE_HORIZON_S: i64 = 1800; // labels.toml horizon_s: curve trades were pulled 30 min after create

#[derive(Clone, Debug)]
pub struct P {
    pub slot: u64,
    pub bt: i64,
    pub buy: bool,
    pub base: u64,
    pub q: u64,
    pub pb: u64,
    pub pq: u64,
}

impl P {
    /// pool reserves after this trade (event reserves are pre-trade)
    pub fn post(&self) -> (u64, u64) {
        if self.buy {
            (self.pb.saturating_sub(self.base), self.pq + self.q)
        } else {
            (self.pb + self.base, self.pq.saturating_sub(self.q))
        }
    }
}

pub fn parse_p(a: &Value) -> Option<P> {
    Some(P { slot: a[0].as_u64()?, bt: a[1].as_i64()?, buy: a[2].as_u64()? == 1, base: a[3].as_u64()?, q: a[4].as_u64()?, pb: a[5].as_u64()?, pq: a[6].as_u64()? })
}

pub fn pool_buy(pb: u64, pq: u64, x: u64, fee_bps: u64) -> u64 {
    let eff = x as u128 * (10_000 - fee_bps as u128) / 10_000;
    let k = pb as u128 * pq as u128;
    (pb as u128 - k.div_ceil(pq as u128 + eff)) as u64
}

pub fn pool_sell(pb: u64, pq: u64, tok: u64, fee_bps: u64) -> u64 {
    if pb == 0 {
        return 0;
    }
    let k = pb as u128 * pq as u128;
    let out = pq as u128 - k.div_ceil(pb as u128 + tok as u128);
    (out * (10_000 - fee_bps as u128) / 10_000) as u64
}

#[derive(Debug, PartialEq)]
pub enum Venue {
    Curve,
    Pool,
}

#[derive(Debug)]
pub struct Entry {
    pub venue: Venue,
    pub slot: u64,
    pub bt: i64,
    pub tokens: u64,
    pub mcap_sol: f64,
}

#[derive(Debug, PartialEq)]
pub enum EntryErr {
    NotFilled,
    NoCurveData,
    NoPoolData,
}

pub struct Coin<'a> {
    pub cs: u64,
    pub cbt: i64,
    pub curve: Option<&'a [T]>,
    pub curve_end: i64,
    pub mig: Option<(u64, i64)>,
    pub pool: Option<&'a [P]>,
    pub pool_end: i64,
}

/// Reachable entry. `lat_s` = detection + send/land latency in whole seconds (blockTime resolution).
pub fn entry(c: &Coin, lat_s: i64, x: u64) -> Result<Entry, EntryErr> {
    let t_min = c.cbt + lat_s;
    let cur = c.curve.and_then(|tr| tr.iter().find(|t| t.slot > c.cs && t.bt >= t_min && c.mig.map(|m| t.slot < m.0).unwrap_or(true)));
    let pl = c.pool.and_then(|p| p.iter().find(|t| t.slot > c.cs && t.bt >= t_min));
    // curve trade before the migration wins if it is earlier; otherwise the pool
    let use_curve = match (cur, pl) {
        (Some(a), Some(b)) => a.slot < b.slot,
        (Some(_), None) => true,
        _ => false,
    };
    if use_curve {
        let t = cur.unwrap();
        let tokens = buy_out(t.vsol, t.vtok, x, t.fee_bps.max(1));
        return Ok(Entry { venue: Venue::Curve, slot: t.slot, bt: t.bt, tokens, mcap_sol: t.vsol as f64 / t.vtok as f64 * 1e6 });
    }
    if let Some(t) = pl {
        let (pb, pq) = t.post();
        return Ok(Entry { venue: Venue::Pool, slot: t.slot, bt: t.bt, tokens: pool_buy(pb, pq, x, POOL_FEE_BPS), mcap_sol: pq as f64 / pb.max(1) as f64 * 1e6 });
    }
    match (c.mig, c.pool, c.curve) {
        (Some(_), None, _) => Err(EntryErr::NoPoolData),
        (None, _, None) => Err(EntryErr::NoCurveData),
        // graduated after the latency but the curve trades are missing
        (Some(m), Some(_), None) if m.1 >= t_min => Err(EntryErr::NoCurveData),
        _ => Err(EntryErr::NotFilled),
    }
}

/// Honest sell value (lamports) of `tok` at time t, or None = CENSORED.
pub fn value_at(c: &Coin, tok: u64, t: i64) -> Option<u64> {
    let graduated_by_t = c.mig.map(|m| m.1 <= t).unwrap_or(false);
    if graduated_by_t {
        let p = c.pool?;
        if t > c.pool_end {
            return None;
        }
        let last = p.iter().take_while(|x| x.bt <= t).last()?;
        let (pb, pq) = last.post();
        return Some(pool_sell(pb, pq, tok, POOL_FEE_BPS));
    }
    let tr = c.curve?;
    if t > c.curve_end {
        return None;
    }
    let last = tr.iter().take_while(|x| x.bt <= t).last()?;
    Some(sell_out(last.vsol, last.vtok, tok, last.fee_bps.max(1)))
}

/// Last state in the sample (the brief's "exit = last trade in the sample") and best exit in the sample (upper bound).
pub fn last_and_best(c: &Coin, tok: u64, from_slot: u64) -> (Option<u64>, Option<u64>) {
    let mut best: Option<u64> = None;
    let mut last: Option<u64> = None;
    if let Some(tr) = c.curve {
        for t in tr.iter().filter(|t| t.slot >= from_slot && c.mig.map(|m| t.slot < m.0).unwrap_or(true)) {
            let v = sell_out(t.vsol, t.vtok, tok, t.fee_bps.max(1));
            best = Some(best.unwrap_or(0).max(v));
            last = Some(v);
        }
    }
    if c.mig.is_some() {
        if let Some(p) = c.pool {
            for t in p.iter().filter(|t| t.slot >= from_slot) {
                let (pb, pq) = t.post();
                let v = pool_sell(pb, pq, tok, POOL_FEE_BPS);
                best = Some(best.unwrap_or(0).max(v));
                last = Some(v);
            }
        }
    }
    (last, best)
}

fn ret(v: u64, cost_mult: f64) -> f64 {
    let cost = X as f64 / 1e9 + (TIP + TXC) * cost_mult;
    // cost_mult also doubles the 1.25% fee on both sides (approximately: value * (1 - 0.0125*(m-1)) twice)
    let fee_extra = (1.0 - 0.0125 * (cost_mult - 1.0)).powi(2);
    v as f64 / 1e9 * fee_extra / cost - 1.0
}

fn stats(v: &[f64]) -> Value {
    if v.is_empty() {
        return json!({"n": 0});
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len() as f64;
    let mean = s.iter().sum::<f64>() / n;
    let med = s[s.len() / 2];
    let wins: Vec<f64> = s.iter().copied().filter(|&x| x > 0.0).collect();
    let losses: Vec<f64> = s.iter().copied().filter(|&x| x <= 0.0).collect();
    let aw = if wins.is_empty() { 0.0 } else { wins.iter().sum::<f64>() / wins.len() as f64 };
    let al = if losses.is_empty() { 0.0 } else { losses.iter().sum::<f64>() / losses.len() as f64 };
    let pf = if losses.is_empty() { f64::INFINITY } else { wins.iter().sum::<f64>() / -losses.iter().sum::<f64>() };
    let no5 = if s.len() > 5 { s[..s.len() - 5].iter().sum::<f64>() / (n - 5.0) } else { f64::NAN };
    let r = |x: f64| (x * 10_000.0).round() / 10_000.0;
    json!({"n": s.len(), "win": r(wins.len() as f64 / n), "mean": r(mean), "median": r(med), "mean_no_top5": r(no5),
        "avg_win": r(aw), "avg_loss": r(al), "pf": r(pf), "breakeven_win": r(if aw - al > 0.0 { -al / (aw - al) } else { f64::NAN })})
}

fn read_mints(path: &str, col: usize, skip_header: bool) -> HashSet<String> {
    fs::read_to_string(path).unwrap_or_default().lines().skip(skip_header as usize)
        .filter_map(|l| l.split('\t').nth(col).map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect()
}

/// honest <launches,..> <trades,..> <amm> <migrations> <latency_ms> <out_dir>
type Sel<'a> = &'a dyn Fn(&Value) -> bool;

pub fn run(launch_files: &[String], trade_files: &[String], amm_file: &str, mig_file: &str, lat_ms: i64, out_dir: &str) -> Result<(), String> {
    let lat_s = (lat_ms + 999) / 1000;
    let group_a = read_mints(&std::env::var("JAR_GROUP_A").unwrap_or("dataset/reputasi/reputasi_pool_now.tsv".into()), 0, false);
    let group_l = read_mints(&std::env::var("JAR_GROUP_LABEL").unwrap_or("dataset/coins/min50k_fix2.tsv".into()), 0, false);
    let mut mig: HashMap<String, (u64, i64)> = HashMap::new();
    for v in read_lines(mig_file) {
        if v["quote"].as_str().unwrap_or("").starts_with("So111") {
            if let (Some(m), Some(s), Some(b)) = (v["mint"].as_str(), v["slot"].as_u64(), v["bt"].as_i64()) {
                mig.entry(m.to_string()).or_insert((s, b));
            }
        }
    }
    let mut pools: HashMap<String, (Vec<P>, i64)> = HashMap::new();
    for v in read_lines(amm_file) {
        let Some(t) = v["t"].as_array() else { continue };
        let mut p: Vec<P> = t.iter().filter_map(parse_p).collect();
        p.sort_by_key(|x| x.slot);
        let gbt = v["gbt"].as_i64().unwrap_or(0);
        let end = if v["complete"] == true { gbt + 86_400 } else { p.last().map(|x| x.bt).unwrap_or(gbt) };
        let m = v["mint"].as_str().unwrap_or("").to_string();
        if pools.get(&m).map(|e| e.0.len() >= p.len()).unwrap_or(false) {
            continue;
        }
        pools.insert(m, (p, end));
    }
    let mut curves: HashMap<String, (Vec<T>, i64)> = HashMap::new();
    for f in trade_files {
        for v in read_lines(f) {
            let m = v["mint"].as_str().unwrap_or("").to_string();
            let mut tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).collect()).unwrap_or_default();
            tr.sort_by_key(|t| (t.slot, t.txi));
            let cbt = v["cbt"].as_i64().unwrap_or(0);
            let end = if v["complete_to_horizon"] == true { cbt + CURVE_HORIZON_S } else { tr.last().map(|t| t.bt).unwrap_or(cbt) };
            if curves.get(&m).map(|e| e.0.len() >= tr.len()).unwrap_or(false) {
                continue;
            }
            curves.insert(m, (tr, end));
        }
    }
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mut rows: Vec<Value> = Vec::new();
    let horizons: [(&str, i64); 4] = [("30m", 1800), ("1h", 3600), ("6h", 21_600), ("24h", 86_400)];
    for lf in launch_files {
        for l in read_lines(lf) {
            if l["kind"] != "create" || !l["quote"].as_str().unwrap_or("").starts_with("1111") {
                continue;
            }
            let m = l["mint"].as_str().unwrap_or("").to_string();
            let in_a = group_a.contains(&m);
            let in_l = group_l.contains(&m);
            let grad = mig.contains_key(&m);
            if !in_a && !in_l && !grad && !curves.contains_key(&m) {
                continue;
            }
            let (cs, cbt) = (l["slot"].as_u64().unwrap_or(0), l["bt"].as_i64().unwrap_or(0));
            let c = Coin {
                cs,
                cbt,
                curve: curves.get(&m).map(|x| x.0.as_slice()),
                curve_end: curves.get(&m).map(|x| x.1).unwrap_or(cbt),
                mig: mig.get(&m).copied(),
                pool: pools.get(&m).map(|x| x.0.as_slice()),
                pool_end: pools.get(&m).map(|x| x.1).unwrap_or(cbt),
            };
            let day = crate::outcomes::day_of(cbt);
            let mut row = json!({"mint": m, "day": day, "A": in_a, "label": in_l, "grad": grad, "mayhem": l["mayhem"],
                "same_slot_grad": c.mig.map(|x| x.0 == cs).unwrap_or(false), "mig_s": c.mig.map(|x| x.1 - cbt)});
            match entry(&c, lat_s, X) {
                Err(e) => row["status"] = json!(format!("{e:?}")),
                Ok(en) => {
                    row["status"] = json!("FILLED");
                    row["venue"] = json!(format!("{:?}", en.venue));
                    row["entry_s"] = json!(en.bt - cbt);
                    row["entry_mcap_sol"] = json!(en.mcap_sol.round());
                    for (k, h) in horizons {
                        row[format!("r_{k}")] = value_at(&c, en.tokens, cbt + h).map(|v| json!(ret(v, 1.0))).unwrap_or(json!("CENSORED"));
                        row[format!("r2_{k}")] = value_at(&c, en.tokens, cbt + h).map(|v| json!(ret(v, 2.0))).unwrap_or(json!("CENSORED"));
                    }
                    let (last, best) = last_and_best(&c, en.tokens, en.slot);
                    row["r_last"] = json!(last.map(|v| ret(v, 1.0)));
                    row["r2_last"] = json!(last.map(|v| ret(v, 2.0)));
                    row["r_best"] = json!(best.map(|v| ret(v, 1.0)));
                    row["data_end_s"] = json!(if c.mig.is_some() { c.pool_end - cbt } else { c.curve_end - cbt });
                }
            }
            rows.push(row);
        }
    }
    // ---- summaries: group x day x horizon ----
    let groups: [(&str, Sel); 4] = [
        ("class_A", &|r| r["A"] == true),
        ("label_50k", &|r| r["label"] == true),
        ("graduated_sol", &|r| r["grad"] == true),
        ("all_with_curve_data", &|r| r["grad"] == false && r["A"] == false && r["label"] == false),
    ];
    let mut days: Vec<String> = rows.iter().filter_map(|r| r["day"].as_str().map(String::from)).collect::<HashSet<_>>().into_iter().collect();
    days.sort();
    days.push("ALL".into());
    let mut out = json!({"latency_ms": lat_ms, "latency_s_used": lat_s, "x_sol": 0.1, "tip_sol": TIP, "tx_sol": TXC, "pool_fee_bps_assumed": POOL_FEE_BPS,
        "note": "reachable entry, honest value, censored excluded; r2 = costs x2", "groups": {}});
    let mut tsv = String::from("group\tday\tn\tfilled\tnot_filled\tno_data\tentry_on_pool\tentry_mcap_sol_med\thorizon\tcensored\tn_eval\twin\tmean\tmedian\tmean_no_top5\tpf\tmean_cost_x2\n");
    for (g, sel) in groups.iter() {
        for d in &days {
            let v: Vec<&Value> = rows.iter().filter(|r| sel(r) && (d == "ALL" || r["day"] == d.as_str())).collect();
            if v.is_empty() {
                continue;
            }
            let filled: Vec<&&Value> = v.iter().filter(|r| r["status"] == "FILLED").collect();
            let nf = v.iter().filter(|r| r["status"] == "NotFilled").count();
            let nd = v.len() - filled.len() - nf;
            let on_pool = filled.iter().filter(|r| r["venue"] == "Pool").count();
            let mut mc: Vec<f64> = filled.iter().filter_map(|r| r["entry_mcap_sol"].as_f64()).collect();
            mc.sort_by(|a, b| a.total_cmp(b));
            let mc_med = mc.get(mc.len() / 2).copied().unwrap_or(f64::NAN);
            let mut gj = json!({"n": v.len(), "filled": filled.len(), "not_filled": nf, "no_data": nd, "entry_on_pool": on_pool, "entry_mcap_sol_median": mc_med});
            for k in ["30m", "1h", "6h", "24h", "last", "best"] {
                let key = format!("r_{k}");
                let vals: Vec<f64> = filled.iter().filter_map(|r| r[&key].as_f64()).collect();
                let vals2: Vec<f64> = filled.iter().filter_map(|r| r[format!("r2_{k}")].as_f64()).collect();
                let cens = filled.len() - vals.len();
                let st = stats(&vals);
                let st2 = stats(&vals2);
                tsv.push_str(&format!("{g}\t{d}\t{}\t{}\t{nf}\t{nd}\t{on_pool}\t{mc_med:.0}\t{k}\t{cens}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", v.len(), filled.len(),
                    st["n"], st["win"], st["mean"], st["median"], st["mean_no_top5"], st["pf"], st2["mean"]));
                gj[k] = json!({"censored": cens, "stats": st, "stats_cost_x2": st2});
            }
            out["groups"][*g][d] = gj;
        }
    }
    fs::write(format!("{out_dir}/honest_{lat_ms}ms.json"), out.to_string()).map_err(|e| e.to_string())?;
    fs::write(format!("{out_dir}/honest_{lat_ms}ms.tsv"), &tsv).map_err(|e| e.to_string())?;
    fs::write(format!("{out_dir}/honest_{lat_ms}ms_rows.jsonl"), rows.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("\n")).map_err(|e| e.to_string())?;
    println!("rows={} -> {out_dir}/honest_{lat_ms}ms.tsv", rows.len());
    Ok(())
}

/// F6 step 1: funding-coverage leak check per day from a feat_all table (label col 0, day col 1, has_fund col 18).
/// Rule: a funding feature may be used only if |cov_pos - cov_neg| < 5 points AND the has_fund-alone AUC < 0.55.
pub fn coverage(launch_files: &[String], fund_files: &[String], labels: &str) -> Result<(), String> {
    let tmp = "data/ml/coverage_fall.tsv";
    crate::feat_all::run(launch_files, fund_files, labels, tmp)?;
    let s = fs::read_to_string(tmp).map_err(|e| e.to_string())?;
    let mut d: std::collections::BTreeMap<String, [u64; 4]> = Default::default(); // pos, pos_fund, neg, neg_fund
    for l in s.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        let (Some(lab), Some(day), Some(h)) = (f.first(), f.get(1), f.get(18)) else { continue };
        let e = d.entry(day.to_string()).or_default();
        let pos = *lab == "1";
        let fund = *h == "1";
        e[if pos { 0 } else { 2 }] += 1;
        e[if pos { 1 } else { 3 }] += fund as u64;
    }
    println!("day\tpos\tcov_pos\tneg\tcov_neg\tgap_pts\tauc_has_fund\tfunding_feature_allowed");
    let mut all_ok = true;
    for (day, e) in &d {
        let cp = e[1] as f64 / e[0].max(1) as f64;
        let cn = e[3] as f64 / e[2].max(1) as f64;
        let auc = 0.5 + (cp - cn) / 2.0;
        let ok = e[0] > 0 && (cp - cn).abs() < 0.05 && auc < 0.55;
        all_ok &= ok || e[0] == 0;
        println!("{day}\t{}\t{:.1}%\t{}\t{:.1}%\t{:.1}\t{auc:.3}\t{}", e[0], 100.0 * cp, e[2], 100.0 * cn, 100.0 * (cp - cn), if ok { "yes" } else { "NO" });
    }
    println!("funding features usable on all days: {}", if all_ok { "yes" } else { "NO -> drop has_fund, fund_lag_s, team_* for every day" });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(slot: u64, bt: i64, vsol: u64, vtok: u64) -> T {
        T { slot, txi: 0, bt, buy: true, sol: 0, user: "u".into(), vsol, vtok, fee_bps: 125 }
    }
    fn p(slot: u64, bt: i64, pb: u64, pq: u64) -> P {
        P { slot, bt, buy: true, base: 0, q: 0, pb, pq }
    }

    #[test]
    fn entry_slot_after_create_and_after_latency() {
        let tr = vec![t(100, 1000, 31e9 as u64, 1.06e15 as u64), t(101, 1000, 32e9 as u64, 1.05e15 as u64), t(108, 1003, 40e9 as u64, 0.8e15 as u64)];
        let c = Coin { cs: 100, cbt: 1000, curve: Some(&tr), curve_end: 2800, mig: None, pool: None, pool_end: 0 };
        let e = entry(&c, 3, X).unwrap();
        assert!(e.slot > 100);
        assert_eq!(e.slot, 108);
        assert_eq!(e.venue, Venue::Curve);
    }

    #[test]
    fn same_slot_graduation_enters_on_pool() {
        let tr = vec![t(100, 1000, 85e9 as u64, 0.28e15 as u64)];
        let pl = vec![p(100, 1000, 206e12 as u64, 67e9 as u64), p(105, 1003, 150e12 as u64, 120e9 as u64)];
        let c = Coin { cs: 100, cbt: 1000, curve: Some(&tr), curve_end: 1000, mig: Some((100, 1000)), pool: Some(&pl), pool_end: 90_000 };
        let e = entry(&c, 3, X).unwrap();
        assert_eq!(e.venue, Venue::Pool);
        assert_eq!(e.slot, 105);
        // graduated but no pool data -> NoPoolData, never the curve
        let c2 = Coin { pool: None, ..c };
        assert_eq!(entry(&c2, 3, X).unwrap_err(), EntryErr::NoPoolData);
    }

    #[test]
    fn causal_truncation_same_entry() {
        let pl: Vec<P> = (0..50).map(|i| p(100 + i, 1000 + i as i64 / 2, (200 - i) * 1_000_000_000_000, (70 + i) * 1_000_000_000)).collect();
        let c = Coin { cs: 100, cbt: 1000, curve: None, curve_end: 0, mig: Some((100, 1000)), pool: Some(&pl), pool_end: 90_000 };
        let a = entry(&c, 3, X).unwrap();
        let cut: Vec<P> = pl.iter().filter(|x| x.slot <= a.slot).cloned().collect();
        let c2 = Coin { pool: Some(&cut), ..c };
        let b = entry(&c2, 3, X).unwrap();
        assert_eq!((a.slot, a.tokens), (b.slot, b.tokens));
        // one slot of the future shifted in breaks it
        let shifted: Vec<P> = pl.iter().map(|x| P { slot: x.slot + 1, ..x.clone() }).collect();
        let c3 = Coin { pool: Some(&shifted), ..c };
        assert_ne!(entry(&c3, 3, X).unwrap().slot, a.slot);
    }

    #[test]
    fn not_filled_when_no_trade_after_latency() {
        let tr = vec![t(101, 1001, 31e9 as u64, 1.06e15 as u64)];
        let c = Coin { cs: 100, cbt: 1000, curve: Some(&tr), curve_end: 2800, mig: None, pool: None, pool_end: 0 };
        assert_eq!(entry(&c, 3, X).unwrap_err(), EntryErr::NotFilled);
    }

    #[test]
    fn drained_pool_is_a_loss_and_short_data_is_censored() {
        // peak at +10 s, drained by +20 min
        let pl = vec![p(105, 1003, 150e12 as u64, 120e9 as u64), p(110, 1010, 100e12 as u64, 180e9 as u64), p(2000, 2200, 900e12 as u64, 0.5e9 as u64)];
        let c = Coin { cs: 100, cbt: 1000, curve: None, curve_end: 0, mig: Some((100, 1000)), pool: Some(&pl), pool_end: 1000 + 86_400 };
        let e = entry(&c, 3, X).unwrap();
        let v = value_at(&c, e.tokens, 1000 + 3600).unwrap();
        assert!(ret(v, 1.0) < -0.9, "ret {}", ret(v, 1.0));
        // data ends at +30 s (complete = false) -> CENSORED at 1 h
        let c2 = Coin { pool_end: 1030, ..c };
        assert!(value_at(&c2, e.tokens, 1000 + 3600).is_none());
    }

    #[test]
    fn pool_events_pre_trade_post_rebuild() {
        let a = P { slot: 1, bt: 0, buy: true, base: 2_376_302_863_285, q: 987_478_891, pb: 206_900_000_000_000, pq: 67_405_853_768 };
        assert_eq!(a.post().0, 204_523_697_136_715); // next event's pool_base in the real data
    }
}

