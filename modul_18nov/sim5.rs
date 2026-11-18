//! Owner 18 Nov: simulate buying the 5 relay-funded coins (COGE, GTA6, BLACKX, SPAX10R, AODT) as early as
//! possible and holding; $100 bankroll, 1/4 Kelly, compounding in time order. Candles already on disk only.
//! Also `instant`: base rate of all coins that graduated in the create second (dev buys the whole curve).
use crate::graph::read_lines;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

const FEE: f64 = 0.02; // buy+sell fees and slippage per round trip (PumpSwap 0.25%+, small size)

fn candles(path: &str) -> Vec<(i64, f64, f64)> {
    let Ok(s) = fs::read_to_string(path) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<Value>(&s) else { return Vec::new() };
    let mut c: Vec<(i64, f64, f64)> = v["data"]["attributes"]["ohlcv_list"].as_array().into_iter().flatten()
        .filter_map(|x| Some((x[0].as_i64()?, x[1].as_f64()?, x[4].as_f64()?))).collect();
    c.sort_by_key(|x| x.0);
    c
}

/// rows: (symbol, mint, create_bt) from a tsv: sym mint curve bt slot creator
pub fn run(list: &str) -> Result<(), String> {
    let mut rows: Vec<(String, String, i64)> = fs::read_to_string(list).map_err(|e| e.to_string())?.lines()
        .filter_map(|l| { let p: Vec<&str> = l.split('\t').collect(); Some((p.first()?.to_string(), p.get(1)?.to_string(), p.get(3)?.parse().ok()?)) })
        .collect();
    rows.sort_by_key(|r| r.2);
    // scenario -> per coin multiple (exit/entry, after fees)
    let names = ["E0 harga lulus (slot sama, mustahil)", "E1 tutup menit ke-1", "E5 tutup menit ke-5", "E60 tutup jam ke-1"];
    let mut res: Vec<Vec<(String, f64, f64)>> = vec![Vec::new(); names.len()]; // (sym, x_now, x_ath)
    println!("koin | lahir (UTC detik) | harga lulus | menit1 | menit5 | jam1 | ATH | sekarang   (SOL/token)");
    for (sym, mint, _bt) in &rows {
        let m = candles(&format!("data/roots/gt/{mint}.min.json"));
        let h = candles(&format!("data/roots/gt/{mint}.hour.json"));
        if m.len() < 6 || h.is_empty() {
            println!("{sym}: candle tidak lengkap");
            continue;
        }
        let e = [m[0].1, m[0].2, m[4].2, m.get(59).map(|x| x.2).unwrap_or(h[0].2)];
        let now = h.last().map(|x| x.2).unwrap_or(0.0);
        let ath = h.iter().map(|x| x.2).chain(m.iter().map(|x| x.2)).fold(0.0, f64::max);
        println!("{sym} | {} | {:.2e} | {:.2e} | {:.2e} | {:.2e} | {:.2e} | {:.2e}", m[0].0, e[0], e[1], e[2], e[3], ath, now);
        for (i, p) in e.iter().enumerate() {
            res[i].push((sym.clone(), now / p * (1.0 - FEE), ath / p * (1.0 - FEE)));
        }
    }
    for (i, n) in names.iter().enumerate() {
        println!("\n== {n}");
        for (exit_i, exit) in ["tahan s/d sekarang", "jual tepat di ATH (batas atas, tak realistis)"].iter().enumerate() {
            let xs: Vec<f64> = res[i].iter().map(|r| if exit_i == 0 { r.1 } else { r.2 }).collect();
            let wins: Vec<f64> = xs.iter().filter(|x| **x > 1.0).map(|x| x - 1.0).collect();
            let p = wins.len() as f64 / xs.len().max(1) as f64;
            let b = if wins.is_empty() { 0.0 } else { wins.iter().sum::<f64>() / wins.len() as f64 };
            let kelly = if b > 0.0 { (p - (1.0 - p) / b).max(0.0) } else { 0.0 };
            let f = (kelly / 4.0).min(0.25); // 1/4 Kelly, never more than 25% per trade
            let mut bank = 100.0;
            let mut trail = Vec::new();
            for (r, x) in res[i].iter().zip(&xs) {
                let stake = bank * f;
                bank += stake * (x - 1.0);
                trail.push(format!("{} {:.2}x -> ${:.0}", r.0, x, bank));
            }
            println!("  {exit}: menang {}/{} | Kelly penuh {:.2} -> 1/4 Kelly {:.1}% per trade | $100 -> ${:.0}\n     {}",
                wins.len(), xs.len(), kelly, f * 100.0, bank, trail.join(" | "));
        }
    }
    Ok(())
}

/// Base rate: coins that graduated within 2 s of their create (dev bought the whole curve at birth).
pub fn instant() -> Result<(), String> {
    let mut create: HashMap<String, i64> = HashMap::new();
    let mut files: Vec<String> = Vec::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("launches_2026-") && n.ends_with(".jsonl") {
                files.push(format!("{d}/{n}"));
            }
        }
    }
    for f in &files {
        for v in read_lines(f) {
            if v["kind"] == "create" {
                if let (Some(m), Some(bt)) = (v["mint"].as_str(), v["bt"].as_i64()) {
                    create.entry(m.to_string()).or_insert(bt);
                }
            }
        }
    }
    let mut ds: HashMap<String, f64> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), v["mcap"].as_f64().unwrap_or(0.0));
    }
    let (mut n_grad, mut n_inst, mut inst_ds, mut i100, mut i1m, mut n100, mut n1m) = (0, 0, 0, 0, 0, 0, 0);
    for l in fs::read_to_string("data/roots/grad.tsv").map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        let (Some(m), Some(gbt)) = (p.first(), p.get(2).and_then(|x| x.parse::<i64>().ok())) else { continue };
        let Some(cbt) = create.get(*m) else { continue };
        n_grad += 1;
        let mc = ds.get(*m).copied().unwrap_or(0.0);
        if gbt - cbt <= 2 {
            n_inst += 1;
            if ds.contains_key(*m) { inst_ds += 1; }
            if mc >= 100_000.0 { i100 += 1; }
            if mc >= 1_000_000.0 { i1m += 1; }
        } else {
            if mc >= 100_000.0 { n100 += 1; }
            if mc >= 1_000_000.0 { n1m += 1; }
        }
    }
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!("koin lulus dgn data create: {n_grad} (file launches: {})", files.len());
    println!("LULUS INSTAN (<=2 dtk dari lahir): {n_inst} | masih di DexScreener {inst_ds} | mcap>=$100k {i100} ({:.1}%) | >=$1jt {i1m} ({:.1}%)", pct(i100, n_inst), pct(i1m, n_inst));
    println!("lulus biasa: {} | mcap>=$100k {n100} ({:.1}%) | >=$1jt {n1m} ({:.1}%)", n_grad - n_inst, pct(n100, n_grad - n_inst), pct(n1m, n_grad - n_inst));
    Ok(())
}

/// "If we were the first buyer": buy every instant-graduation coin at the graduation price (before the team's
/// AMM buy), hold to the DexScreener snapshot (18 Nov). Not on DexScreener / no mcap = total loss.
/// $100, 1/4 Kelly (from this same sample = optimistic), compounding in time order.
pub fn first_buyer() -> Result<(), String> {
    let mut create: HashMap<String, i64> = HashMap::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("launches_2026-") && n.ends_with(".jsonl") {
                for v in read_lines(&format!("{d}/{n}")) {
                    if v["kind"] == "create" {
                        if let (Some(m), Some(bt)) = (v["mint"].as_str(), v["bt"].as_i64()) {
                            create.entry(m.to_string()).or_insert(bt);
                        }
                    }
                }
            }
        }
    }
    let mut ds: HashMap<String, f64> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), v["mcap"].as_f64().unwrap_or(0.0));
    }
    let grad_mc = 4.16e-7 * 1e9 * 72.0; // ~ $30k at SOL $72 (median implied on 18 Nov)
    let mut xs: Vec<(i64, f64)> = Vec::new();
    for l in fs::read_to_string("data/roots/grad.tsv").map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        let (Some(m), Some(gbt)) = (p.first(), p.get(2).and_then(|x| x.parse::<i64>().ok())) else { continue };
        let Some(cbt) = create.get(*m) else { continue };
        if gbt - cbt <= 2 {
            let mc = ds.get(*m).copied().unwrap_or(0.0);
            xs.push((*cbt, mc / grad_mc * (1.0 - FEE)));
        }
    }
    xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let n = xs.len();
    let mut v: Vec<f64> = xs.iter().map(|x| x.1).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cnt = |t: f64| v.iter().filter(|x| **x >= t).count();
    let mean = v.iter().sum::<f64>() / n.max(1) as f64;
    println!("koin lulus instan: {n} | median hasil {:.3}x | rata-rata {:.2}x | >=1x {} | >=10x {} | >=100x {}", v[n / 2], mean, cnt(1.0), cnt(10.0), cnt(100.0));
    let wins: Vec<f64> = v.iter().filter(|x| **x > 1.0).map(|x| x - 1.0).collect();
    let losses: Vec<f64> = v.iter().filter(|x| **x <= 1.0).map(|x| 1.0 - x).collect();
    let p = wins.len() as f64 / n as f64;
    let b = wins.iter().sum::<f64>() / wins.len().max(1) as f64;
    let a = losses.iter().sum::<f64>() / losses.len().max(1) as f64;
    let kelly = (p / a - (1.0 - p) / b).max(0.0);
    for (name, f) in [("1/4 Kelly", (kelly / 4.0).min(0.25)), ("tetap 1% per koin", 0.01)] {
        let mut bank = 100.0f64;
        let mut peak = 100.0f64;
        let mut dd = 0.0f64;
        for (_, x) in &xs {
            bank += bank * f * (x - 1.0);
            peak = peak.max(bank);
            dd = dd.max(1.0 - bank / peak);
        }
        println!("{name}: menang {:.1}% | rata untung {:.1}x | rata rugi {:.0}% | f={:.2}% per koin | $100 -> ${:.0} | drawdown maks {:.0}%",
            p * 100.0, b, a * 100.0, f * 100.0, bank, dd * 100.0);
    }
    Ok(())
}

/// Realistic "first buyer": same sample, but position and exit are limited by liquidity.
///  - harga lulus: max buy $300 (pool ~85 SOL at that moment), harga lahir: max buy $50 (curve ~30 SOL virtual)
///  - exit: at most 10% of today's pool liquidity (DexScreener liq), the rest is unsellable without crashing price
pub fn first_buyer_real() -> Result<(), String> {
    let mut create: HashMap<String, i64> = HashMap::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("launches_2026-") && n.ends_with(".jsonl") {
                for v in read_lines(&format!("{d}/{n}")) {
                    if v["kind"] == "create" {
                        if let (Some(m), Some(bt)) = (v["mint"].as_str(), v["bt"].as_i64()) {
                            create.entry(m.to_string()).or_insert(bt);
                        }
                    }
                }
            }
        }
    }
    let mut ds: HashMap<String, (f64, f64)> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), (v["mcap"].as_f64().unwrap_or(0.0), v["liq"].as_f64().unwrap_or(0.0)));
    }
    let grad_mc = 4.16e-7 * 1e9 * 72.0;
    let mut xs: Vec<(i64, f64, f64)> = Vec::new(); // bt, x from grad price, liq now
    for l in fs::read_to_string("data/roots/grad.tsv").map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        let (Some(m), Some(gbt)) = (p.first(), p.get(2).and_then(|x| x.parse::<i64>().ok())) else { continue };
        let Some(cbt) = create.get(*m) else { continue };
        if gbt - cbt <= 2 {
            let (mc, liq) = ds.get(*m).copied().unwrap_or((0.0, 0.0));
            xs.push((*cbt, mc / grad_mc, liq));
        }
    }
    xs.sort_by_key(|x| x.0);
    for (name, mult) in [("LULUS", 1.0), ("LAHIR", 4.16e-7 / 2.8e-8)] {
        let (mut cost, mut back, mut w, mut first_win) = (0.0f64, 0.0f64, 0, 0usize);
        for (i, (_, x, liq)) in xs.iter().enumerate() {
            let out = (x * mult * (1.0 - FEE)).min(0.10 * liq);
            cost += 1.0;
            back += out;
            if out > 1.0 {
                w += 1;
                if first_win == 0 {
                    first_win = i + 1;
                }
            }
        }
        println!("FLAT $1 per koin, harga {name}: modal ${cost:.0} -> kembali ${back:.0} ({:+.0}%) | menang {w} | menang pertama di koin ke-{first_win}", 100.0 * (back / cost - 1.0));
    }
    for (name, mult, cap) in [("beli di HARGA LULUS (sebelum tim beli 400-774 SOL)", 1.0, 300.0), ("beli di HARGA LAHIR (sebelum dev borong curve)", 4.16e-7 / 2.8e-8, 50.0)] {
        for frac in [0.0075, 0.02] {
            let mut bank = 100.0f64;
            let (mut w, mut best) = (0, 0.0f64);
            for (_, x, liq) in &xs {
                let stake = (bank * frac).min(cap);
                let out = (stake * x * mult * (1.0 - FEE)).min(0.10 * liq).max(0.0);
                if out > stake { w += 1; }
                best = best.max(out - stake);
                bank += out - stake;
                if bank < 1.0 { break; }
            }
            println!("{name} | {:.2}% bank/koin, maks ${cap:.0} | menang {w}/{} | untung terbesar 1 koin ${best:.0} | $100 -> ${bank:.0}", frac * 100.0, xs.len());
        }
    }
    Ok(())
}
