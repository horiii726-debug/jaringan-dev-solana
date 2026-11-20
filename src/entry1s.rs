//! Owner 18 Nov: buy < 1 s after the coin is born (land in create slot + 2, ~0.8 s), hold, sell:
//!   LULUS    = at graduation price (the moment it reaches PumpSwap / DexScreener); dead on curve = last curve price
//!   PUNCAK   = at the highest curve price seen (upper bound, needs perfect timing)
//!   SEKARANG = DexScreener snapshot 18 Nov (not listed = 0)
//! If the curve was already finished before our slot (dev bought it all at birth), the entry is on PumpSwap
//! (amm trades file), else on the curve (constant product on virtual reserves).
//! $100 bankroll, 1/4 Kelly from this same sample (optimistic), compounding in time order,
//! stake <= $100, exit <= 10% of pool liquidity for SEKARANG.
//! Usage: jaringan entry1s <list.tsv mint curve bt slot> <curve_trades.jsonl> <amm_trades.jsonl|->
use crate::graph::read_lines;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

const SOL_USD: f64 = 72.0;
const GRAD_PX: f64 = 4.16e-7;
const BUY_SOL: f64 = 0.5;
const FEE: f64 = 0.02;

fn px_curve(t: &Value) -> Option<(f64, f64)> {
    let (s, k) = (t[8].as_f64()?, t[9].as_f64()?);
    if k <= 0.0 { None } else { Some((s, k)) }
}

pub fn run(list: &str, curve_f: &str, amm_f: &str) -> Result<(), String> {
    let mut ds: HashMap<String, (f64, f64)> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), (v["mcap"].as_f64().unwrap_or(0.0), v["liq"].as_f64().unwrap_or(0.0)));
    }
    let curve: HashMap<String, Value> = read_lines(curve_f).into_iter().filter_map(|v| Some((v["mint"].as_str()?.to_string(), v))).collect();
    let amm: HashMap<String, Value> = if amm_f == "-" { HashMap::new() } else {
        read_lines(amm_f).into_iter().filter_map(|v| Some((v["mint"].as_str()?.to_string(), v))).collect()
    };
    // (bt, sym-ish mint, x_lulus, x_puncak, x_now, liq, where)
    let mut rows: Vec<(i64, String, f64, f64, f64, f64, &'static str)> = Vec::new();
    let (mut no_data, mut need_amm) = (0, Vec::new());
    for l in fs::read_to_string(list).map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() < 4 { continue; }
        let (mint, bt, cslot): (&str, i64, u64) = (p[0], p[2].parse().unwrap_or(0), p[3].parse().unwrap_or(0));
        let Some(c) = curve.get(mint) else { no_data += 1; continue };
        let tr: Vec<&Value> = c["t"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        if tr.is_empty() { no_data += 1; continue; }
        let cslot = if cslot > 0 { cslot } else { tr[0][0].as_u64().unwrap_or(0) };
        let entry_slot = cslot + 2;
        let gslot = c["graduated_slot"].as_u64();
        let (mcap, liq) = ds.get(mint).copied().unwrap_or((0.0, 0.0));
        let px_now = mcap / SOL_USD / 1e9;
        let last_curve = tr.iter().rev().find_map(|t| px_curve(t)).map(|(s, k)| s / k / 1000.0).unwrap_or(0.0);
        let peak_curve = tr.iter().filter_map(|t| px_curve(t)).map(|(s, k)| s / k / 1000.0).fold(0.0, f64::max);
        let (entry, place) = if gslot.map(|g| g < entry_slot).unwrap_or(false) {
            // curve finished before we could land -> PumpSwap
            let Some(a) = amm.get(mint) else { need_amm.push(mint.to_string()); continue };
            let at: Vec<&Value> = a["t"].as_array().map(|x| x.iter().collect()).unwrap_or_default();
            let before = at.iter().rfind(|t| t[0].as_u64().unwrap_or(0) < entry_slot).or(at.first());
            let Some(t) = before else { no_data += 1; continue };
            let (b, q) = (t[5].as_f64().unwrap_or(0.0), t[6].as_f64().unwrap_or(0.0));
            if b <= 0.0 { no_data += 1; continue; }
            let qs = q / 1e9; let bs = b / 1e6;
            let tok = bs - qs * bs / (qs + BUY_SOL * 0.99);
            (BUY_SOL / tok, "PumpSwap")
        } else {
            let (mut s, mut k) = (30e9, 1_073_000_000e6);
            for t in tr.iter().filter(|t| t[0].as_u64().unwrap_or(0) < entry_slot) {
                if let Some(x) = px_curve(t) { (s, k) = x; }
            }
            let (ss, ks) = (s / 1e9, k / 1e6);
            let tok = ks - ss * ks / (ss + BUY_SOL * 0.99);
            (BUY_SOL / tok, "curve")
        };
        let x_lulus = if gslot.is_some() { GRAD_PX / entry } else { last_curve / entry };
        let x_peak = peak_curve.max(if gslot.is_some() { GRAD_PX } else { 0.0 }) / entry;
        let x_now = if mcap > 0.0 { px_now / entry } else if gslot.is_some() { 0.0 } else { last_curve / entry };
        rows.push((bt, mint.to_string(), x_lulus * (1.0 - FEE), x_peak.max(x_lulus) * (1.0 - FEE), x_now * (1.0 - FEE), liq, place));
    }
    if !need_amm.is_empty() {
        fs::write("data/family_a/need_amm.txt", need_amm.join("\n")).map_err(|e| e.to_string())?;
    }
    rows.sort_by_key(|r| r.0);
    let n = rows.len();
    let on_curve = rows.iter().filter(|r| r.6 == "curve").count();
    println!("koin dihitung {n} (masuk di curve {on_curve}, di PumpSwap {}) | tanpa data {no_data} | perlu data PumpSwap {}", n - on_curve, need_amm.len());
    for (ei, name) in ["JUAL SAAT LULUS (masuk DexScreener)", "JUAL DI PUNCAK CURVE (batas atas)", "TAHAN SAMPAI 18 NOV"].iter().enumerate() {
        let xs: Vec<f64> = rows.iter().map(|r| [r.2, r.3, r.4][ei]).collect();
        let mut v = xs.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let c = |t: f64| v.iter().filter(|x| **x >= t).count();
        let wins: Vec<f64> = xs.iter().filter(|x| **x > 1.0).map(|x| x - 1.0).collect();
        let loss: Vec<f64> = xs.iter().filter(|x| **x <= 1.0).map(|x| 1.0 - x).collect();
        let p = wins.len() as f64 / n.max(1) as f64;
        let b = wins.iter().sum::<f64>() / wins.len().max(1) as f64;
        let a = (loss.iter().sum::<f64>() / loss.len().max(1) as f64).max(0.01);
        let kelly = (p / a - (1.0 - p) / b.max(1e-9)).max(0.0);
        let f = (kelly / 4.0).min(0.25);
        let mut bank = 100.0f64;
        let (mut peak, mut dd) = (100.0f64, 0.0f64);
        for (r, x) in rows.iter().zip(&xs) {
            let stake = (bank * f).min(100.0);
            let mut out = stake * x;
            if ei == 2 { out = out.min(0.10 * r.5.max(0.0)).max(if r.5 > 0.0 { 0.0 } else { stake * x }); }
            bank += out - stake;
            peak = peak.max(bank);
            dd = dd.max(1.0 - bank / peak);
        }
        let med = if n > 0 { v[n / 2] } else { 0.0 };
        println!("\n== {name}\n  median {:.2}x | naik (>1x) {} dari {n} ({:.0}%) | >=2x {} | >=10x {} | >=100x {}\n  1/4 Kelly = {:.1}% per koin | $100 -> ${:.0} | drawdown maks {:.0}%",
            med, wins.len(), p * 100.0, c(2.0), c(10.0), c(100.0), f * 100.0, bank, dd * 100.0);
    }
    Ok(())
}

/// Owner 18 Nov: history of each dev: all coins it created (devhist), how many reached PumpSwap/DexScreener
/// and how many died on Pump.fun. Only devs with a known origin (family) are counted separately.
pub fn devsum(devhist: &str, up: &str) -> Result<(), String> {
    let mut grad: std::collections::HashSet<String> = std::collections::HashSet::new();
    for v in read_lines("data/roots/pool_creators.jsonl") {
        if let Some(m) = v["mint"].as_str() { grad.insert(m.to_string()); }
    }
    let mut ds: HashMap<String, f64> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), v["mcap"].as_f64().unwrap_or(0.0));
    }
    let known: std::collections::HashSet<String> = read_lines(up).into_iter()
        .filter(|u| u["induk"] != u["dev"] && u["end"] != "wallet_ramai")
        .filter_map(|u| u["dev"].as_str().map(String::from)).collect();
    for (label, only_known) in [("SEMUA DEV", false), ("DEV ASAL JELAS (family)", true)] {
        let (mut devs, mut coins, mut g, mut d, mut big, mut incomplete) = (0, 0, 0, 0, 0, 0);
        let (mut k40, mut k50, mut m1) = (0usize, 0usize, 0usize);
        let mut buckets = [0usize; 4]; // dev success rate: 0%, <25%, 25-50%, >=50%
        for v in read_lines(devhist) {
            let w = v["w"].as_str().unwrap_or("");
            if only_known && !known.contains(w) { continue; }
            let cs: Vec<&str> = v["creates"].as_array().into_iter().flatten().filter_map(|c| c["mint"].as_str()).collect();
            if cs.is_empty() { continue; }
            devs += 1;
            if v["complete"] != true { incomplete += 1; }
            let ng = cs.iter().filter(|m| grad.contains(**m)).count();
            let nd = cs.iter().filter(|m| ds.contains_key(**m)).count();
            let nb = cs.iter().filter(|m| ds.get(**m).copied().unwrap_or(0.0) >= 100_000.0).count();
            coins += cs.len(); g += ng; d += nd; big += nb;
            let mc = |t: f64| cs.iter().filter(|m| ds.get(**m).copied().unwrap_or(0.0) >= t).count();
            k40 += mc(40_000.0); k50 += mc(50_000.0); m1 += mc(1_000_000.0);
            let r = nd as f64 / cs.len() as f64;
            buckets[if nd == 0 { 0 } else if r < 0.25 { 1 } else if r < 0.5 { 2 } else { 3 }] += 1;
        }
        let pct = |a: usize| 100.0 * a as f64 / coins.max(1) as f64;
        println!("\n== {label}: dev {devs} (riwayat terpotong {incomplete}) | koin dibuat {coins}\n  mati di Pump.fun (tak lulus) {} ({:.0}%) | lulus {g} ({:.0}%) | masih di DexScreener {d} ({:.0}%) | mcap>=$40k {k40} ({:.1}%) | >=$50k {k50} ({:.1}%) | >=$100k {big} ({:.1}%) | >=$1jt {m1} ({:.1}%)\n  dev menurut % koin yang masih di DexScreener: 0% = {} | <25% = {} | 25-50% = {} | >=50% = {}",
            coins - g, pct(coins - g), pct(g), pct(d), pct(k40), pct(k50), pct(big), pct(m1), buckets[0], buckets[1], buckets[2], buckets[3]);
    }
    Ok(())
}
