//! Take-profit ladder simulation on the bonding curve (owner 18 Nov: buy in the first seconds, take profit on the way
//! to graduation; what happens on the DEX afterwards is only a label).
//! Entry: first curve trade in slot >= create_slot + k (k = reachable delay), post-state, 0.1 SOL, real fee bps.
//! Ladder: equal parts sold when the sell value of the WHOLE position reaches each multiple of cost (e.g. 2,3,5,10).
//! Exit of the rest: creator / create-signer sells (we sell at the next trade), graduation (curve complete: last curve
//! state), 30 min time stop, or data end (flagged censored). Costs: fee per side (trade fee_bps), tip, tx cost.
use crate::graph::read_lines;
use crate::outcomes::{buy_out, parse_t, sell_out, T};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;

const X: u64 = 100_000_000;
const TIP: f64 = 0.00146;
const TXC: f64 = 0.000_315;

pub struct Res {
    pub ret: f64,
    pub reason: &'static str,

    pub tps_hit: usize,
}

/// `tr` sorted by (slot, txi); `cs` create slot; `grad_slot` if the coin graduated.
#[allow(clippy::too_many_arguments)]
pub fn sim(tr: &[T], cs: u64, k: u64, creator: &str, signer: &str, ladder: &[f64], grad_slot: Option<(u64, i64)>, tstop: i64) -> Option<Res> {
    let e = tr.iter().position(|t| t.slot >= cs + k && t.slot > cs)?;
    let t0 = &tr[e];
    let tok = buy_out(t0.vsol, t0.vtok, X, t0.fee_bps.max(1));
    if tok == 0 {
        return None;
    }
    let cost = X as f64 / 1e9 + TIP + TXC;
    let part = tok / ladder.len() as u64;
    let mut left = tok;
    let mut got = 0.0f64;
    let mut hit = 0usize;
    let mut reason = "data_end";
    let mut last = t0;
    for t in &tr[e + 1..] {
        if t.bt - t0.bt > tstop {
            reason = "time_stop";
            break;
        }
        last = t;
        // ladder: whole-position value vs cost
        while hit < ladder.len() && left > 0 {
            let whole = sell_out(t.vsol, t.vtok, tok, t.fee_bps.max(1)) as f64 / 1e9;
            if whole < ladder[hit] * cost {
                break;
            }
            let q = if hit + 1 == ladder.len() { left } else { part.min(left) };
            got += sell_out(t.vsol, t.vtok, q, t.fee_bps.max(1)) as f64 / 1e9;
            left -= q;
            hit += 1;
        }
        if left == 0 {
            reason = "all_tp";
            break;
        }
        // owner 18 Nov variant: JAR_NO_DEV_EXIT=1 -> do not follow the dev's sell, hold for TP / graduation / time stop
        if !t.buy && (t.user == creator || t.user == signer) && std::env::var("JAR_NO_DEV_EXIT").is_err() {
            reason = "dev_sell";
            break; // sell the rest at the next trade's state (handled below with `last` = next trade if any)
        }
        if grad_slot.map(|g| t.slot >= g.0).unwrap_or(false) {
            reason = "graduated";
            break;
        }
    }
    // Truncated curve data (page cap) on a coin that graduated within the hold window: the curve's final state is
    // deterministic (all 793.1M real tokens sold, vtok = 279.9M, same k) -> every TP below it was reached; sell the rest there.
    if reason == "data_end" && left > 0 {
        if let Some((_, _gbt)) = grad_slot.filter(|g| g.1 - t0.bt <= tstop) {

            let vtok_f: u64 = 279_900_000_000_000;
            let vsol_f = (t0.vsol as u128 * t0.vtok as u128 / vtok_f as u128) as u64;
            while hit < ladder.len() && left > 0 {
                if (sell_out(vsol_f, vtok_f, tok, 125) as f64 / 1e9) < ladder[hit] * cost {
                    break;
                }
                // filled on the way up at the TP level (not at the top): value = TP multiple of cost for that part
                let q = if hit + 1 == ladder.len() { left } else { part.min(left) };
                got += ladder[hit] * cost * q as f64 / tok as f64;
                left -= q;
                hit += 1;
            }
            if left > 0 {
                got += sell_out(vsol_f, vtok_f, left, 125) as f64 / 1e9;
                left = 0;
            }
            reason = "graduated_final_state";
        }
    }
    if left > 0 {
        let st = if reason == "dev_sell" { tr.iter().find(|x| (x.slot, x.txi) > (last.slot, last.txi)).unwrap_or(last) } else { last };
        got += sell_out(st.vsol, st.vtok, left, st.fee_bps.max(1)) as f64 / 1e9;
    }
    Some(Res { ret: (got - TXC) / cost - 1.0, reason, tps_hit: hit })
}

fn stats(v: &[f64]) -> Value {
    if v.is_empty() {
        return json!({"n": 0});
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len() as f64;
    let w: f64 = s.iter().filter(|x| **x > 0.0).sum();
    let l: f64 = -s.iter().filter(|x| **x <= 0.0).sum::<f64>();
    let r = |x: f64| (x * 1000.0).round() / 1000.0;
    json!({"n": s.len(), "win": r(s.iter().filter(|x| **x > 0.0).count() as f64 / n), "mean": r(s.iter().sum::<f64>() / n), "median": r(s[s.len() / 2]),
        "no_top5": r(if s.len() > 5 { s[..s.len() - 5].iter().sum::<f64>() / (n - 5.0) } else { f64::NAN }), "pf": r(if l > 0.0 { w / l } else { f64::INFINITY })})
}

/// tpsim <launches,..> <trades,..> <migrations,..> <creators.txt|-> <out.json>
pub fn run(launch_files: &[String], trade_files: &[String], mig_files: &[String], sel: &str, out: &str) -> Result<(), String> {
    let sel: HashSet<String> = if sel == "-" { HashSet::new() } else { fs::read_to_string(sel).unwrap_or_default().lines().map(|l| l.trim().to_string()).collect() };
    let mut ls: HashMap<String, (u64, String, String, i64)> = HashMap::new();
    for f in launch_files {
        for v in read_lines(f) {
            if v["kind"] == "create" {
                ls.insert(v["mint"].as_str().unwrap_or("").into(), (v["slot"].as_u64().unwrap_or(0), v["creator"].as_str().unwrap_or("").into(), v["user"].as_str().unwrap_or("").into(), v["bt"].as_i64().unwrap_or(0)));
            }
        }
    }
    let mut grad: HashMap<String, (u64, i64)> = HashMap::new();
    for f in mig_files {
        for v in read_lines(f) {
            if let (Some(m), Some(s)) = (v["mint"].as_str(), v["slot"].as_u64()) {
                grad.entry(m.into()).or_insert((s, v["bt"].as_i64().unwrap_or(0)));
            }
        }
    }
    let ladders: [(&str, Vec<f64>); 5] = [("tp2", vec![2.0]), ("tp3", vec![3.0]), ("tp5", vec![5.0]), ("tp10", vec![10.0]), ("ladder_2_3_5_10", vec![2.0, 3.0, 5.0, 10.0])];
    let mut res: HashMap<String, Vec<f64>> = HashMap::new();
    let mut reasons: HashMap<String, usize> = HashMap::new();
    let (mut n, mut ng, mut nss) = (0usize, 0usize, 0usize);
    let mut seen = HashSet::new();
    let mut rows = String::new();
    for f in trade_files {
        for v in read_lines(f) {
            let m = v["mint"].as_str().unwrap_or("").to_string();
            let Some((cs, cr, us, cbt)) = ls.get(&m) else { continue };
            if (!sel.is_empty() && !sel.contains(cr)) || !seen.insert(m.clone()) {
                continue;
            }
            let g = grad.get(&m).copied();
            if g.map(|x| x.0) == Some(*cs) {
                nss += 1; // graduated inside the create slot: not buyable cheap, counted, skipped
                continue;
            }
            let mut tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).collect()).unwrap_or_default();
            tr.sort_by_key(|t| (t.slot, t.txi));
            n += 1;
            ng += g.is_some() as usize;
            for k in [1u64, 2, 3] {
                for (name, lad) in &ladders {
                    let key = format!("k{k}_{name}");
                    match sim(&tr, *cs, k, cr, us, lad, g, 1800) {
                        Some(r) => {
                            res.entry(key.clone()).or_default().push(r.ret);
                            rows.push_str(&format!("{cbt}\t{key}\t{}\t{m}\n", r.ret));
                            if k == 2 && *name == "ladder_2_3_5_10" {
                                *reasons.entry(format!("tp_levels_hit_{}", r.tps_hit)).or_default() += 1;
                                *reasons.entry(r.reason.to_string()).or_default() += 1;
                            }
                        }
                        None => *res.entry(format!("{key}_not_filled_count")).or_default() = vec![],
                    }
                }
            }
        }
    }
    let mut o = json!({"coins": n, "graduated_normal": ng, "skipped_same_slot_bundle": nss, "exit_reasons_k2_ladder": reasons});
    let mut keys: Vec<&String> = res.keys().filter(|k| !k.ends_with("_count")).collect();
    keys.sort();
    for k in keys {
        o[k] = stats(&res[k]);
        println!("{k}\t{}", o[k]);
    }
    println!("coins={n} graduated_normal={ng} skipped_bundle={nss} reasons(k2 ladder)={}", o["exit_reasons_k2_ladder"]);
    fs::write(out, o.to_string()).map_err(|e| e.to_string())?;
    fs::write(format!("{out}.rows.tsv"), rows).map_err(|e| e.to_string())?;
    Ok(())
}

/// Quarter-Kelly compounding (owner 18 Nov). Kelly fraction f* = argmax mean(ln(1 + f r)) on the TRAIN trades only
/// (earlier period), then f*/4 of the bankroll per trade on the TEST trades in time order, stake capped at `cap_sol`
/// (returns were simulated for 0.1 SOL; bigger buys move the curve more, so the cap keeps the simulation honest).
/// Rows: bt<TAB>key<TAB>ret<TAB>mint.
pub fn kelly(train: &str, test: &str, key: &str, start_usd: f64, sol_usd: f64, cap_sol: f64) -> Result<(), String> {
    let load = |p: &str| -> Vec<(i64, f64)> {
        let mut v: Vec<(i64, f64)> = fs::read_to_string(p).unwrap_or_default().lines().filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.get(1) != Some(&key) {
                return None;
            }
            Some((f.first()?.parse().ok()?, f.get(2)?.parse().ok()?))
        }).collect();
        v.sort_by_key(|x| x.0);
        v
    };
    let (tr, te) = (load(train), load(test));
    if tr.is_empty() || te.is_empty() {
        return Err(format!("no rows for {key}: train {} test {}", tr.len(), te.len()));
    }
    let g = |f: f64| tr.iter().map(|(_, r)| (1.0 + f * r).max(1e-9).ln()).sum::<f64>() / tr.len() as f64;
    let (mut fbest, mut gbest) = (0.0, 0.0);
    for i in 1..=1000 {
        let f = i as f64 / 1000.0;
        if g(f) > gbest {
            gbest = g(f);
            fbest = f;
        }
    }
    let q = fbest / 4.0;
    let cap = cap_sol * sol_usd;
    let (mut bank, mut peak, mut mdd, mut skipped, mut capped) = (start_usd, start_usd, 0.0f64, 0, 0);
    let mut wins = 0;
    for (_, r) in &te {
        let mut stake = bank * q;
        if stake > cap {
            stake = cap;
            capped += 1;
        }
        if stake < 1.0 {
            skipped += 1;
            continue;
        }
        bank += stake * r;
        wins += (*r > 0.0) as usize;
        peak = peak.max(bank);
        mdd = mdd.max(1.0 - bank / peak);
    }
    let mean_tr = tr.iter().map(|x| x.1).sum::<f64>() / tr.len() as f64;
    let mean_te = te.iter().map(|x| x.1).sum::<f64>() / te.len() as f64;
    println!("strategy={key} train_trades={} train_mean={:+.1}% full_kelly={fbest:.3} quarter={q:.4}", tr.len(), 100.0 * mean_tr);
    println!("TEST trades={} test_mean={:+.1}% win={:.1}% start=${start_usd:.0} end=${bank:.2} max_drawdown={:.1}% capped_at_{cap_sol}SOL={capped} skipped_below_$1={skipped}",
        te.len(), 100.0 * mean_te, 100.0 * wins as f64 / te.len().max(1) as f64, 100.0 * mdd);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(slot: u64, bt: i64, buy: bool, user: &str, vsol: u64) -> T {
        // keep k = vsol * vtok constant like the curve
        let k: u128 = 30_000_000_000u128 * 1_073_000_000_000_000u128;
        T { slot, txi: 0, bt, buy, sol: 0, user: user.into(), vsol, vtok: (k / vsol as u128) as u64, fee_bps: 125 }
    }
    #[test]
    fn ladder_takes_profit_before_dev_dump() {
        let tr = vec![t(101, 0, true, "a", 31_000_000_000), t(102, 1, true, "b", 70_000_000_000), t(103, 2, true, "c", 120_000_000_000), t(104, 3, false, "dev", 31_000_000_000), t(105, 4, false, "x", 30_500_000_000)];
        let r = sim(&tr, 100, 1, "dev", "dev", &[2.0, 3.0], None, 1800).unwrap();
        assert!(r.tps_hit >= 1, "hit {}", r.tps_hit);
        assert!(r.ret > 0.5, "ret {}", r.ret);
    }
    #[test]
    fn truncated_data_on_graduated_coin_uses_final_curve_state() {
        let tr = vec![t(101, 0, true, "a", 31_000_000_000), t(102, 1, true, "b", 33_000_000_000)];
        let r = sim(&tr, 100, 1, "dev", "dev", &[2.0, 3.0], Some((900, 120)), 1800).unwrap();
        assert_eq!(r.reason, "graduated_final_state");
        assert_eq!(r.tps_hit, 2);
        assert!(r.ret > 1.0, "ret {}", r.ret);
        // graduated after the hold window: no free win
        let r2 = sim(&tr, 100, 1, "dev", "dev", &[2.0], Some((900, 5000)), 1800).unwrap();
        assert_eq!(r2.reason, "data_end");
    }
    #[test]
    fn dev_dump_without_tp_loses() {
        let tr = vec![t(101, 0, true, "a", 31_000_000_000), t(102, 1, false, "dev", 30_100_000_000), t(103, 2, false, "x", 30_050_000_000)];
        let r = sim(&tr, 100, 1, "dev", "dev", &[2.0], None, 1800).unwrap();
        assert_eq!(r.reason, "dev_sell");
        assert!(r.ret < 0.0);
    }
}
