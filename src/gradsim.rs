//! "Jito first" hold simulation across bonding curve and PumpSwap.
//! Entry: 0.1 SOL right after the create tx (state after the create tx's own trades), tip as a cost fraction.
//! Value at a time = what our tokens sell for then (curve constant product before graduation, pool constant
//! product after), minus 1.25% fee. Strategies: hold 1 h / 6 h / 24 h, and PEAK (upper bound, not tradable).
use crate::graph::read_lines;
use crate::outcomes::{buy_out, parse_t, sell_out, T};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;

const X: u64 = 100_000_000;
const FEE_BPS: u64 = 125;

fn pool_sell(pb: u64, pq: u64, tok: u64) -> u64 {
    let k = pb as u128 * pq as u128;
    let out = pq as u128 - k.div_ceil(pb as u128 + tok as u128);
    (out * (10_000 - FEE_BPS as u128) / 10_000) as u64
}

pub fn run(launch_files: &[String], trade_files: &[String], amm_file: &str, tip_frac: f64, sol_usd: f64) -> Result<(), String> {
    // 18 Nov review: entry right after the create tx is unreachable and value_at hides crashes (liquidity filter) -> this
    // output is an UNREACHABLE UPPER BOUND only. Honest numbers: jaringan honest (src/honest.rs).
    if std::env::var("JAR_UPPER_BOUND").as_deref() != Ok("1") {
        return Err("gradsim = unreachable upper bound; set JAR_UPPER_BOUND=1 to run it, use jaringan honest for real numbers".into());
    }
    let mut ls: HashMap<String, Value> = HashMap::new();
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] == "create" {
                ls.insert(v["mint"].as_str().unwrap_or("").into(), v);
            }
        }
    }
    let mut amm: HashMap<String, Vec<(i64, u64, u64)>> = HashMap::new(); // bt, pool_base, pool_quote
    for v in read_lines(amm_file) {
        if let Some(t) = v["t"].as_array() {
            amm.insert(v["mint"].as_str().unwrap_or("").into(), t.iter().filter_map(|x| Some((x[1].as_i64()?, x[5].as_u64()?, x[6].as_u64()?))).collect());
        }
    }
    // good teams from the scoreboard: dev -> team funder
    let sc: Value = serde_json::from_str(&fs::read_to_string("reports/web/score.json").unwrap_or_default()).unwrap_or(Value::Null);
    let mut team_of: HashMap<String, String> = HashMap::new();
    for t in sc["teams"].as_array().into_iter().flatten().filter(|t| t["good"] == true) {
        for d in t["dev_list"].as_array().into_iter().flatten() {
            team_of.insert(d.as_str().unwrap_or("").into(), t["funder"].as_str().unwrap_or("").into());
        }
    }
    // dev family inputs: devflow recipients (JAR_FAMILY), team members (reports/team_members.tsv), pool trades with trader
    let mut family: HashMap<String, Vec<String>> = HashMap::new();
    if let Ok(p) = std::env::var("JAR_FAMILY") {
        for v in read_lines(&p) {
            family.insert(v["mint"].as_str().unwrap_or("").into(),
                v["family"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect());
        }
    }
    let mut team_key: HashMap<String, String> = HashMap::new();
    let mut team_devs: HashMap<String, Vec<String>> = HashMap::new();
    for l in fs::read_to_string("reports/team_members.tsv").unwrap_or_default().lines() {
        if let Some((k, d)) = l.split_once('\t') {
            team_key.insert(d.to_string(), k.to_string());
            team_devs.entry(k.to_string()).or_default().push(d.to_string());
        }
    }
    let mut amm_users: HashMap<String, Vec<(i64, bool, String)>> = HashMap::new();
    for v in read_lines(amm_file) {
        if let Some(t) = v["t"].as_array() {
            amm_users.insert(v["mint"].as_str().unwrap_or("").into(),
                t.iter().filter_map(|x| Some((x[1].as_i64()?, x[2].as_u64()? == 1, x.get(7).and_then(|u| u.as_str()).unwrap_or("").to_string()))).collect());
        }
    }
    let sim_list: HashSet<String> = std::env::var("JAR_SIM_LIST").ok().and_then(|p| fs::read_to_string(p).ok()).map(|s| s.lines().map(|l| l.trim().to_string()).collect()).unwrap_or_default();
    let cost = X as f64 / 1e9 * (1.0 + tip_frac) + 0.000_21;
    let horizons = [3600i64, 21_600, 86_400];
    let mut rows: Vec<Value> = Vec::new();
    let mut seen = HashSet::new();
    for p in trade_files {
        for v in read_lines(p) {
            let m = v["mint"].as_str().unwrap_or("").to_string();
            let Some(l) = ls.get(&m) else { continue };
            if !seen.insert(m.clone()) || l["mayhem"] == true || !l["quote"].as_str().unwrap_or("").starts_with("1111") {
                continue;
            }
            let creator = l["creator"].as_str().unwrap_or("").to_string();
            let grad = !v["graduated_slot"].is_null();
            let in_team = team_of.contains_key(&creator);
            let in_list = sim_list.contains(&m);
            if !grad && !in_team && !in_list {
                continue;
            }
            let mut tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).collect()).unwrap_or_default();
            tr.sort_by_key(|t| (t.slot, t.txi));
            let cbt = l["bt"].as_i64().unwrap_or(0);
            let (mut vs, mut vt) = (l["after_vsol"].as_u64().or(l["vsol"].as_u64()).unwrap_or(30_000_000_000), l["after_vtok"].as_u64().or(l["vtok"].as_u64()).unwrap_or(1_073_000_000_000_000));
            // JAR_POS = k: we are buyer number k+1 after the create tx (k more trades of the create slot land before us)
            let pos_k: usize = std::env::var("JAR_POS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            if pos_k > 0 {
                let cs = l["slot"].as_u64().unwrap_or(0);
                let ctxi = l["txi"].as_u64().unwrap_or(0);
                let after: Vec<&T> = tr.iter().filter(|t| t.slot == cs && t.txi > ctxi).collect();
                if let Some(t) = after.get(pos_k.min(after.len()).saturating_sub(1)).filter(|_| !after.is_empty()) {
                    vs = t.vsol;
                    vt = t.vtok;
                }
            }
            let tok = buy_out(vs, vt, X, FEE_BPS);
            let a = amm.get(&m);
            if grad && a.is_none() {
                continue; // graduated but pool not pulled yet
            }
            let value_at = |t_end: i64| -> f64 {
                if let Some(a) = a {
                    if let Some(&(_, pb, pq)) = a.iter().take_while(|x| x.0 <= t_end).filter(|x| x.2 >= 20_000_000_000 && x.1 >= 50_000_000_000_000).last() {
                        return pool_sell(pb, pq, tok) as f64 / 1e9;
                    }
                }
                let last = tr.iter().take_while(|t| t.bt <= t_end).last();
                last.map(|t| sell_out(t.vsol, t.vtok, tok, FEE_BPS) as f64 / 1e9).unwrap_or(sell_out(vs, vt, tok, FEE_BPS) as f64 / 1e9)
            };
            let mut peak = tr.iter().map(|t| sell_out(t.vsol, t.vtok, tok, FEE_BPS)).max().unwrap_or(0) as f64 / 1e9;
            let mut peak_mcap_sol = tr.iter().map(|t| t.vsol as f64 / t.vtok as f64 * 1e6).fold(0.0, f64::max);
            if let Some(a) = a {
                for &(_, pb, pq) in a.iter().filter(|x| x.2 >= 20_000_000_000 && x.1 >= 50_000_000_000_000) {
                    peak = peak.max(pool_sell(pb, pq, tok) as f64 / 1e9);
                    if pb > 0 {
                        peak_mcap_sol = peak_mcap_sol.max(pq as f64 / pb as f64 * 1e6);
                    }
                }
            }
            let r: Vec<f64> = horizons.iter().map(|h| (value_at(cbt + h) - 0.000_105) / cost - 1.0).collect();
            // exit with the dev family: first sell (curve or pool) by creator, signer, create-tx bundle wallets,
            // team wallets, or wallets the dev moved the coin to; we sell one second later. No time limit.
            let mut fam: HashSet<&str> = HashSet::new();
            fam.insert(creator.as_str());
            fam.insert(l["user"].as_str().unwrap_or(""));
            for b in l["tx_buyers"].as_array().into_iter().flatten() {
                fam.insert(b.as_str().unwrap_or(""));
            }
            if let Some(fl) = family.get(&m) {
                fam.extend(fl.iter().map(|s| s.as_str()));
            }
            if let Some(k) = team_key.get(&creator) {
                fam.extend(team_devs.get(k).into_iter().flatten().map(|s| s.as_str()));
            }
            let cs = l["slot"].as_u64().unwrap_or(0);
            let ctxi = l["txi"].as_u64().unwrap_or(0);
            let curve_sell = tr.iter().find(|t| !t.buy && (t.slot, t.txi) > (cs, ctxi) && fam.contains(t.user.as_str())).map(|t| t.bt);
            let pool_sell_t = amm_users.get(&m).and_then(|v| v.iter().find(|x| !x.1 && fam.contains(x.2.as_str())).map(|x| x.0));
            let last_seen = tr.last().map(|t| t.bt).unwrap_or(cbt).max(a.and_then(|v| v.last().map(|x| x.0)).unwrap_or(cbt));
            let (t_exit, why) = match (curve_sell, pool_sell_t) {
                (Some(c), Some(p)) => (c.min(p), "dev_family_sell"),
                (Some(c), None) => (c, "dev_family_sell"),
                (None, Some(p)) => (p, "dev_family_sell"),
                (None, None) => (last_seen, "no_family_sell_in_data"),
            };
            let r_dev = (value_at(t_exit + 1) - 0.000_105) / cost - 1.0;
            rows.push(json!({"r_dev": r_dev, "t_exit": t_exit, "exit": why, "bt": cbt, "in_list": in_list, "mint": m, "sym": l["symbol"], "creator": creator, "team": team_of.get(&creator), "grad": grad,
                "peak_mcap_sol": peak_mcap_sol.round(), "peak_mcap_usd": (peak_mcap_sol * sol_usd).round(),
                "peak_x": ((peak - 0.000_105) / cost * 100.0).round() / 100.0, "r1h": r[0], "r6h": r[1], "r24h": r[2]}));
        }
    }
    let summ = |sel: &dyn Fn(&Value) -> bool| -> Value {
        let v: Vec<&Value> = rows.iter().filter(|r| sel(r)).collect();
        let n = v.len().max(1) as f64;
        let mean = |k: &str| v.iter().map(|r| r[k].as_f64().unwrap_or(0.0)).sum::<f64>() / n;
        let win = |k: &str| v.iter().filter(|r| r[k].as_f64().unwrap_or(0.0) > 0.0).count() as f64 / n;
        let mut r1: Vec<f64> = v.iter().map(|r| r["r1h"].as_f64().unwrap_or(0.0)).collect();
        r1.sort_by(|a, b| a.total_cmp(b));
        let med1 = r1.get(r1.len() / 2).copied().unwrap_or(0.0);
        let no5 = if r1.len() > 5 { r1[..r1.len() - 5].iter().sum::<f64>() / (r1.len() - 5) as f64 } else { f64::NAN };
        let mut mc: Vec<f64> = v.iter().map(|r| r["peak_mcap_usd"].as_f64().unwrap_or(0.0)).collect();
        mc.sort_by(|a, b| a.total_cmp(b));
        let mut rd: Vec<f64> = v.iter().map(|r| r["r_dev"].as_f64().unwrap_or(0.0)).collect();
        rd.sort_by(|a, b| a.total_cmp(b));
        let rd_med = rd.get(rd.len() / 2).copied().unwrap_or(0.0);
        let rd_no5 = if rd.len() > 5 { rd[..rd.len() - 5].iter().sum::<f64>() / (rd.len() - 5) as f64 } else { f64::NAN };
        let rd_sold = v.iter().filter(|r| r["exit"] == "dev_family_sell").count();
        json!({"rdev": mean("r_dev"), "wdev": win("r_dev"), "rdev_median": rd_med, "rdev_no_top5": rd_no5, "dev_sold": rd_sold,
            "r1h_median": med1, "r1h_no_top5": no5, "n": v.len(), "r1h": mean("r1h"), "w1h": win("r1h"), "r6h": mean("r6h"), "w6h": win("r6h"), "r24h": mean("r24h"), "w24h": win("r24h"),
            "peak_x_mean": mean("peak_x"), "mcap_usd_median": mc.get(mc.len() / 2), "mcap_usd_max": mc.last(),
            "ge_100k": mc.iter().filter(|&&x| x >= 100_000.0).count(), "ge_1m": mc.iter().filter(|&&x| x >= 1_000_000.0).count()})
    };
    let mut top: Vec<Value> = rows.iter().filter(|r| r["grad"] == true).cloned().collect();
    top.sort_by(|a, b| b["peak_mcap_usd"].as_f64().unwrap_or(0.0).total_cmp(&a["peak_mcap_usd"].as_f64().unwrap_or(0.0)));
    let out = json!({"unreachable_upper_bound": true, "sol_usd": sol_usd, "tip_frac": tip_frac, "graduated": summ(&|r| r["grad"] == true),
        "list": summ(&|r| r["in_list"] == true),
        "team_all_coins": summ(&|r| !r["team"].is_null()), "team_graduated": summ(&|r| !r["team"].is_null() && r["grad"] == true),
        "top": top.into_iter().take(200).collect::<Vec<_>>(), "list_rows": rows.iter().filter(|r| r["in_list"] == true).cloned().collect::<Vec<_>>()});
    fs::write("reports/web/grad.json", out.to_string()).map_err(|e| e.to_string())?;
    for k in ["graduated", "team_all_coins", "team_graduated"] {
        println!("{k}: {}", out[k]);
    }
    Ok(())
}
