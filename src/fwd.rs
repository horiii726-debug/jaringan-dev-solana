//! Fresh forward holdout (18 Nov, data never looked at before): does a coin from a watched team / dev do well for a
//! buyer who enters as fast as a real bot can?
//! `fwd link`: link each new create to the watchlist (direct = creator/signer is a watched wallet; funder = one of the
//!   creator's first SOL funders is a watched wallet; exchange funders ignored) + a random baseline -> targets.
//! `fwd eval`: reachable entry (first trade with bt >= create + latency AND slot > create slot) at the trade price,
//!   value = last trade price at/before t (fee 1.25% per side, tip + tx cost), CENSORED if the data does not reach t.
//!   Price-based (Bitquery Trade.Price in SOL per token); slippage of 0.1 SOL ignored [U].
use crate::graph::read_lines;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;

const FEE: f64 = 0.0125;
const COST: f64 = 0.1 + 0.00146 + 0.000_315;

pub struct Tr {
    pub slot: u64,
    pub bt: i64,
    pub price: f64,
    pub amm: bool,
}

pub fn entry_idx(tr: &[Tr], cs: u64, cbt: i64, lat_s: i64) -> Option<usize> {
    tr.iter().position(|t| t.slot > cs && t.bt >= cbt + lat_s && t.price > 0.0)
}

/// return of 0.1 SOL bought at tr[e] and sold at price p
pub fn ret_at(pe: f64, p: f64) -> f64 {
    let tokens = 0.1 * (1.0 - FEE) / pe;
    tokens * p * (1.0 - FEE) / COST - 1.0
}

fn load_watch(path: &str) -> HashMap<String, (String, String)> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((f.first()?.to_string(), (f.get(1)?.to_string(), f.get(2)?.to_string())))
    }).collect()
}

pub fn link(dir: &str, watch_file: &str, exch_file: &str, n_base: usize) -> Result<(), String> {
    let watch = load_watch(watch_file);
    let exch: HashSet<String> = fs::read_to_string(exch_file).unwrap_or_default().lines().map(|l| l.split_whitespace().next().unwrap_or("").to_string()).collect();
    let mut funders: HashMap<String, Vec<String>> = HashMap::new();
    for v in read_lines(&format!("{dir}/funders.jsonl")) {
        let (Some(w), Some(f)) = (v["w"].as_str(), v["funder"].as_str()) else { continue };
        if !exch.contains(f) {
            funders.entry(w.to_string()).or_default().push(f.to_string());
        }
    }
    // funder fan-out inside the window: a funder that pays many creators is a service/bot, not a team
    let mut fan: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (w, fs_) in &funders {
        for f in fs_ {
            fan.entry(f.as_str()).or_default().insert(w.as_str());
        }
    }
    let creates = read_lines(&format!("{dir}/creates.jsonl"));
    let mut out = String::new();
    let mut targets: Vec<String> = Vec::new();
    let (mut nd, mut nf) = (0, 0);
    for (i, c) in creates.iter().enumerate() {
        let s = c["signer"].as_str().unwrap_or("");
        let u = c["user"].as_str().unwrap_or("");
        let mint = c["mint"].as_str().unwrap_or("");
        let direct = watch.get(s).or_else(|| watch.get(u));
        let via = funders.get(s).into_iter().flatten().find(|f| watch.contains_key(f.as_str()));
        let (kind, class, root) = if let Some((cl, r)) = direct {
            nd += 1;
            ("direct", cl.clone(), r.clone())
        } else if let Some(f) = via {
            nf += 1;
            let (cl, r) = &watch[f.as_str()];
            ("funder", cl.clone(), r.clone())
        } else {
            ("none", String::new(), String::new())
        };
        // deterministic pseudo-random baseline: every k-th create
        let base = n_base > 0 && i % (creates.len() / n_base).max(1) == 0;
        if kind == "none" && !base {
            continue;
        }
        let fan_n = funders.get(s).and_then(|v| v.first()).and_then(|f| fan.get(f.as_str()).map(|x| x.len())).unwrap_or(0);
        out.push_str(&json!({"mint": mint, "kind": kind, "class": class, "root": root, "base": base, "signer": s, "slot": c["slot"], "bt": c["bt"],
            "funder": funders.get(s).and_then(|v| v.first()), "funder_fan_out": fan_n}).to_string());
        out.push('\n');
        targets.push(mint.to_string());
    }
    fs::write(format!("{dir}/link.jsonl"), out).map_err(|e| e.to_string())?;
    fs::write(format!("{dir}/targets.txt"), targets.join("\n") + "\n").map_err(|e| e.to_string())?;
    println!("creates={} direct={nd} funder_linked={nf} targets={}", creates.len(), targets.len());
    Ok(())
}

fn stats(v: &[f64]) -> Value {
    if v.is_empty() {
        return json!({"n": 0});
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len() as f64;
    let r = |x: f64| (x * 1000.0).round() / 1000.0;
    let no5 = if s.len() > 5 { s[..s.len() - 5].iter().sum::<f64>() / (n - 5.0) } else { f64::NAN };
    json!({"n": s.len(), "win": r(s.iter().filter(|&&x| x > 0.0).count() as f64 / n), "mean": r(s.iter().sum::<f64>() / n), "median": r(s[s.len() / 2]), "no_top5": r(no5)})
}

pub fn eval(dir: &str, lat_s: i64, window_end: i64) -> Result<(), String> {
    let link: Vec<Value> = read_lines(&format!("{dir}/link.jsonl"));
    let mut trades: HashMap<String, Vec<Tr>> = HashMap::new();
    let mut capped: HashSet<String> = HashSet::new();
    for v in read_lines(&format!("{dir}/trades.jsonl")) {
        let m = v["mint"].as_str().unwrap_or("").to_string();
        if v["capped"] == true {
            capped.insert(m.clone());
            continue;
        }
        trades.entry(m).or_default().push(Tr { slot: v["slot"].as_u64().unwrap_or(0), bt: v["bt"].as_i64().unwrap_or(0),
            price: v["price"].as_f64().unwrap_or(0.0), amm: v["amm"] == true });
    }
    for t in trades.values_mut() {
        t.sort_by_key(|x| (x.slot, x.bt));
    }
    let hz: [(&str, i64); 4] = [("5m", 300), ("30m", 1800), ("1h", 3600), ("6h", 21_600)];
    let mut rows: Vec<Value> = Vec::new();
    for l in &link {
        let m = l["mint"].as_str().unwrap_or("");
        let (cs, cbt) = (l["slot"].as_u64().unwrap_or(0), l["bt"].as_i64().unwrap_or(0));
        let empty = Vec::new();
        let tr = trades.get(m).unwrap_or(&empty);
        let mut row = l.clone();
        let first_amm = tr.iter().find(|t| t.amm).map(|t| t.slot);
        row["graduated"] = json!(first_amm.is_some());
        row["same_slot_grad"] = json!(first_amm == Some(cs));
        let data_end = if capped.contains(m) { tr.last().map(|t| t.bt).unwrap_or(cbt) } else { window_end };
        match entry_idx(tr, cs, cbt, lat_s) {
            None => row["status"] = json!(if tr.is_empty() { "NO_TRADES" } else { "NOT_FILLED" }),
            Some(e) => {
                let pe = tr[e].price;
                row["status"] = json!("FILLED");
                row["entry_mcap_sol"] = json!((pe * 1e9).round());
                row["entry_on_pool"] = json!(tr[e].amm);
                for (k, h) in hz {
                    let t = cbt + h;
                    row[format!("r_{k}")] = if t > data_end { json!("CENSORED") } else {
                        let p = tr.iter().take_while(|x| x.bt <= t).last().map(|x| x.price).unwrap_or(pe);
                        json!(ret_at(pe, p))
                    };
                }
                row["r_last"] = json!(ret_at(pe, tr.last().map(|x| x.price).unwrap_or(pe)));
                row["r_best"] = json!(ret_at(pe, tr[e..].iter().map(|x| x.price).fold(0.0, f64::max)));
                row["peak_mcap_sol"] = json!((tr.iter().map(|x| x.price).fold(0.0, f64::max) * 1e9).round());
            }
        }
        rows.push(row);
    }
    #[allow(clippy::type_complexity)]
    let groups: Vec<(&str, Box<dyn Fn(&Value) -> bool>)> = vec![
        ("direct_A", Box::new(|r: &Value| r["kind"] == "direct" && r["class"].as_str().unwrap_or("").starts_with('A'))),
        ("direct_B", Box::new(|r: &Value| r["kind"] == "direct" && r["class"].as_str().unwrap_or("").starts_with('B'))),
        ("direct_C", Box::new(|r: &Value| r["kind"] == "direct" && r["class"].as_str().unwrap_or("").starts_with('C'))),
        ("funder_linked_any", Box::new(|r: &Value| r["kind"] == "funder")),
        ("funder_linked_fanout_le5", Box::new(|r: &Value| r["kind"] == "funder" && r["funder_fan_out"].as_u64().unwrap_or(0) <= 5)),
        ("baseline_random", Box::new(|r: &Value| r["base"] == true)),
    ];
    let mut out = json!({"latency_s": lat_s, "window_end": window_end, "note": "fresh holdout 17-18 Nov, reachable entry, price-based"});
    println!("group\tn\tfilled\tnot_filled\tno_trades\tgrad\tsame_slot_grad\tentry_mcap_sol_med\thorizon\tcensored\tn_eval\twin\tmean\tmedian\tno_top5");
    for (g, sel) in &groups {
        let v: Vec<&Value> = rows.iter().filter(|r| sel(r)).collect();
        let filled: Vec<&&Value> = v.iter().filter(|r| r["status"] == "FILLED").collect();
        let nf = v.iter().filter(|r| r["status"] == "NOT_FILLED").count();
        let nt = v.iter().filter(|r| r["status"] == "NO_TRADES").count();
        let grad = v.iter().filter(|r| r["graduated"] == true).count();
        let ss = v.iter().filter(|r| r["same_slot_grad"] == true).count();
        let mut mc: Vec<f64> = filled.iter().filter_map(|r| r["entry_mcap_sol"].as_f64()).collect();
        mc.sort_by(|a, b| a.total_cmp(b));
        let mcm = mc.get(mc.len() / 2).copied().unwrap_or(f64::NAN);
        let mut gj = json!({"n": v.len(), "filled": filled.len(), "not_filled": nf, "no_trades": nt, "graduated": grad, "same_slot_grad": ss, "entry_mcap_sol_median": mcm});
        for k in ["5m", "30m", "1h", "6h", "last", "best"] {
            let vals: Vec<f64> = filled.iter().filter_map(|r| r[format!("r_{k}")].as_f64()).collect();
            let st = stats(&vals);
            println!("{g}\t{}\t{}\t{nf}\t{nt}\t{grad}\t{ss}\t{mcm:.0}\t{k}\t{}\t{}\t{}\t{}\t{}\t{}", v.len(), filled.len(), filled.len() - vals.len(),
                st["n"], st["win"], st["mean"], st["median"], st["no_top5"]);
            gj[k] = st;
        }
        out[*g] = gj;
    }
    fs::write(format!("{dir}/fwd_result.json"), out.to_string()).map_err(|e| e.to_string())?;
    fs::write(format!("{dir}/fwd_rows.jsonl"), rows.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("\n")).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_is_after_create_slot_and_latency() {
        let tr = vec![Tr { slot: 10, bt: 100, price: 1.0, amm: false }, Tr { slot: 11, bt: 101, price: 2.0, amm: false }, Tr { slot: 18, bt: 103, price: 3.0, amm: true }];
        assert_eq!(entry_idx(&tr, 10, 100, 3), Some(2));
        assert_eq!(entry_idx(&tr, 10, 100, 9), None);
    }
    #[test]
    fn flat_price_loses_costs() {
        let r = ret_at(1e-7, 1e-7);
        assert!(r < -0.03 && r > -0.05, "{r}");
    }
}
