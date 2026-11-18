//! Owner 18 Nov: coins of known families that died on Pump.fun (never graduated): did they pump first,
//! did the dev/family rug, or was it just quiet?
//!   famdead <dir>                      -> <dir>/dead.tsv (mint curve bt slot creator root) + dead.csv for `trades`
//!   deadcls <dir> <dead_trades.jsonl>  -> per coin class + per family summary
//! Classes (on curve trades; family = creator + every wallet found for that induk):
//!   RUG          family sold >= 50% of the tokens it bought and the price ended >= 50% under the peak
//!   NAIK_LALU_MATI  peak >= 3x the price at create slot + 2 (what we could buy), not RUG
//!   SEPI         < 10 outside buyers and peak < 3x
//!   LEMAH        the rest (some trading, no real pump)
use crate::graph::read_lines;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

pub fn famdead(dir: &str) -> Result<(), String> {
    let creates = read_lines(&format!("{dir}/creates.jsonl"));
    let mut grad: HashSet<String> = HashSet::new();
    for v in read_lines("data/roots/pool_creators.jsonl") {
        if let Some(m) = v["mint"].as_str() { grad.insert(m.to_string()); }
    }
    let mut want: HashMap<String, (String, String)> = HashMap::new(); // mint -> (creator, root)
    for c in &creates {
        let m = c["mint"].as_str().unwrap_or("").to_string();
        if !grad.contains(&m) {
            want.entry(m).or_insert((c["w"].as_str().unwrap_or("").to_string(), c["root"].as_str().unwrap_or("").to_string()));
        }
    }
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !(n.starts_with("launches_2026-") && n.ends_with(".jsonl")) { continue; }
            for v in read_lines(&format!("{d}/{n}")) {
                if v["kind"] != "create" { continue; }
                let m = v["mint"].as_str().unwrap_or("");
                if let Some((cr, root)) = want.get(m) {
                    if seen.insert(m.to_string()) {
                        out.push((m.to_string(), v["curve"].as_str().unwrap_or("").to_string(), v["bt"].as_i64().unwrap_or(0), v["slot"].as_u64().unwrap_or(0), cr.clone(), root.clone()));
                    }
                }
            }
        }
    }
    let tsv: Vec<String> = out.iter().map(|x| format!("{}\t{}\t{}\t{}\t{}\t{}", x.0, x.1, x.2, x.3, x.4, x.5)).collect();
    let csv: Vec<String> = out.iter().map(|x| format!("{},{},{}", x.0, x.1, x.2)).collect();
    fs::write(format!("{dir}/dead.tsv"), tsv.join("\n")).map_err(|e| e.to_string())?;
    fs::write(format!("{dir}/dead.csv"), csv.join("\n")).map_err(|e| e.to_string())?;
    println!("koin family mati di Pump.fun: {} | ada data peluncuran (6-18 Nov): {}", want.len(), out.len());
    Ok(())
}

pub fn deadcls(dir: &str, trades_f: &str) -> Result<(), String> {
    let mut fam: HashMap<String, HashSet<String>> = HashMap::new();
    for w in read_lines(&format!("{dir}/wallets.jsonl")) {
        fam.entry(w["root"].as_str().unwrap_or("").to_string()).or_default().insert(w["w"].as_str().unwrap_or("").to_string());
    }
    let tr: HashMap<String, serde_json::Value> = read_lines(trades_f).into_iter().filter_map(|v| Some((v["mint"].as_str()?.to_string(), v))).collect();
    let mut per_root: BTreeMap<String, [usize; 5]> = BTreeMap::new(); // RUG, NAIK, SEPI, LEMAH, no data
    let mut rugs: HashMap<String, usize> = HashMap::new();
    let mut total = [0usize; 5];
    let mut lines = Vec::new();
    for l in fs::read_to_string(format!("{dir}/dead.tsv")).map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() < 6 { continue; }
        let (mint, cslot, creator, root) = (p[0], p[3].parse::<u64>().unwrap_or(0), p[4], p[5]);
        let empty = HashSet::new();
        let fw = fam.get(root).unwrap_or(&empty);
        let is_fam = |u: &str| u == creator || fw.contains(u);
        let slot_of = |t: &serde_json::Value| t[0].as_u64().unwrap_or(0);
        let Some(v) = tr.get(mint) else {
            per_root.entry(root.to_string()).or_default()[4] += 1;
            total[4] += 1;
            continue;
        };
        let t: Vec<&serde_json::Value> = v["t"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        if t.is_empty() {
            per_root.entry(root.to_string()).or_default()[4] += 1;
            total[4] += 1;
            continue;
        }
        let px = |x: &serde_json::Value| x[8].as_f64().unwrap_or(0.0) / x[9].as_f64().unwrap_or(1.0).max(1.0);
        let entry = t.iter().filter(|x| slot_of(x) < cslot + 2).last().map(|x| px(x)).unwrap_or(30e9 / 1.073e15);
        let peak = t.iter().map(|x| px(x)).fold(0.0, f64::max);
        let last = px(t[t.len() - 1]);
        let (mut fb, mut fs_, mut buyers) = (0.0f64, 0.0f64, HashSet::new());
        for x in &t {
            let u = x[7].as_str().unwrap_or("");
            let tok = x[6].as_f64().unwrap_or(0.0);
            let buy = x[4].as_u64().unwrap_or(0) == 1;
            if is_fam(u) {
                if buy { fb += tok } else { fs_ += tok }
            } else if buy {
                buyers.insert(u.to_string());
            }
        }
        // non-SOL quote coins: the decoder has no SOL reserves (price 0) -> judge only by who sold to whom
        let priced = peak > 0.0 && entry > 0.0;
        let peak_x = if priced { peak / entry } else { 0.0 };
        let drop = if priced { 1.0 - last / peak } else { 0.0 };
        let fam_dump = fb > 0.0 && fs_ >= 0.5 * fb;
        let k = if priced {
            if fam_dump && drop >= 0.5 { 0 } else if peak_x >= 3.0 { 1 } else if buyers.len() < 10 { 2 } else { 3 }
        } else if fam_dump && buyers.len() >= 10 { 0 } else if buyers.len() < 10 { 2 } else { 3 };
        *rugs.entry(root.to_string()).or_default() += (k == 0) as usize;
        per_root.entry(root.to_string()).or_default()[k] += 1;
        total[k] += 1;
        lines.push(format!("{mint} | {} | puncak {:.1}x dari harga beli kita | turun dari puncak {:.0}% | pembeli luar {} | family jual {:.0}% dari belinya",
            ["RUG", "NAIK_LALU_MATI", "SEPI", "LEMAH"][k], peak_x, drop * 100.0, buyers.len(), if fb > 0.0 { 100.0 * fs_ / fb } else { 0.0 }));
    }
    let n: usize = total[..4].iter().sum();
    let pct = |a: usize| 100.0 * a as f64 / n.max(1) as f64;
    println!("koin mati dianalisis {n} (tanpa data transaksi {})", total[4]);
    println!("RUG (family buang) {} ({:.0}%) | NAIK LALU MATI {} ({:.0}%) | SEPI {} ({:.0}%) | LEMAH {} ({:.0}%)",
        total[0], pct(total[0]), total[1], pct(total[1]), total[2], pct(total[2]), total[3], pct(total[3]));
    println!("\nper induk (RUG | NAIK_LALU_MATI | SEPI | LEMAH):");
    let mut v: Vec<(&String, &[usize; 5])> = per_root.iter().collect();
    v.sort_by(|a, b| b.1[..4].iter().sum::<usize>().cmp(&a.1[..4].iter().sum::<usize>()));
    for (r, c) in v.iter().take(15) {
        println!("  {} | {} | {} | {} | {}", &r[..8.min(r.len())], c[0], c[1], c[2], c[3]);
    }
    fs::write(format!("{dir}/dead_cls.txt"), lines.join("\n")).map_err(|e| e.to_string())?;
    let rt: Vec<String> = rugs.iter().map(|(r, n)| format!("{r}\t{n}")).collect();
    fs::write(format!("{dir}/rugs.tsv"), rt.join("\n")).map_err(|e| e.to_string())?;

    // Owner rule: a family that rugs often is dropped, unless it also makes many big DexScreener coins.
    let mut ds: HashMap<String, f64> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), v["mcap"].as_f64().unwrap_or(0.0));
    }
    let mut coins: HashMap<String, HashSet<String>> = HashMap::new();
    let mut cbt: HashMap<String, i64> = HashMap::new();
    for c in read_lines(&format!("{dir}/creates.jsonl")) {
        coins.entry(c["root"].as_str().unwrap_or("").to_string()).or_default().insert(c["mint"].as_str().unwrap_or("").to_string());
        cbt.insert(c["mint"].as_str().unwrap_or("").to_string(), c["bt"].as_i64().unwrap_or(0));
    }
    for v in read_lines("data/roots/ds.jsonl") {
        let m = v["mint"].as_str().unwrap_or("").to_string();
        let b = v["pair_created"].as_i64().unwrap_or(0) / 1000;
        cbt.entry(m).or_insert(b);
    }
    const ACTIVE_SINCE: i64 = 1_794_700_800; // 15 Nov 00:00 UTC (data ends 18 Nov)
    let mut dev_root: HashMap<String, String> = HashMap::new();
    for u in read_lines(&format!("{dir}/up.jsonl")) {
        if u["induk"] != u["dev"] && u["end"] != "wallet_ramai" {
            dev_root.insert(u["dev"].as_str().unwrap_or("").to_string(), u["induk"].as_str().unwrap_or("").to_string());
        }
    }
    for v in read_lines("data/roots/pool_creators.jsonl") {
        if let Some(r) = dev_root.get(v["coin_creator"].as_str().unwrap_or("")) {
            coins.entry(r.clone()).or_default().insert(v["mint"].as_str().unwrap_or("").to_string());
        }
    }
    let mut rows: Vec<(usize, String)> = Vec::new();
    let (mut keep, mut drop_n) = (0, 0);
    let (mut n_moon_fam, mut n_moon, mut n_moon_active) = (0, 0, 0);
    for (r, ms) in &coins {
        let n = ms.len();
        let big = ms.iter().filter(|m| ds.get(*m).copied().unwrap_or(0.0) >= 100_000.0).count();
        let mil = ms.iter().filter(|m| ds.get(*m).copied().unwrap_or(0.0) >= 1_000_000.0).count();
        let k40 = ms.iter().filter(|m| ds.get(*m).copied().unwrap_or(0.0) >= 40_000.0).count();
        let k50 = ms.iter().filter(|m| ds.get(*m).copied().unwrap_or(0.0) >= 50_000.0).count();
        let on_ds = ms.iter().filter(|m| ds.contains_key(*m)).count();
        let rg = rugs.get(r).copied().unwrap_or(0);
        let often_rug = rg >= 2 && rg * 3 >= n; // >= 2 rugs and >= 1/3 of its coins
        let status = if often_rug && big < 2 { drop_n += 1; "KELUARKAN (sering rug)" } else if often_rug { keep += 1; "PAKAI (rug, tapi banyak koin besar)" } else { keep += 1; "PAKAI" };
        let last = ms.iter().filter_map(|m| cbt.get(m)).copied().max().unwrap_or(0);
        let recent = ms.iter().filter(|m| cbt.get(*m).copied().unwrap_or(0) >= ACTIVE_SINCE).count();
        let akt = if recent > 0 { format!("AKTIF ({recent} koin sejak 15 Nov)") } else { "tidak aktif sejak 15 Nov".to_string() };
        if mil > 0 { n_moon_fam += 1; n_moon += mil; if recent > 0 { n_moon_active += 1; } }
        rows.push((mil * 100_000 + big * 1000 + on_ds, format!("{} | {status} | koin {n} | rug {rg} | di DexScreener {on_ds} | >=$40k {k40} | >=$50k {k50} | >=$100k {big} | MOONSHOT >=$1jt {mil} | {akt} | terakhir bikin {last}", &r[..8.min(r.len())])));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    println!("\nfamily dgn moonshot (>=$1jt): {n_moon_fam} | total koin moonshot: {n_moon} | family moonshot yg masih AKTIF sejak 15 Nov: {n_moon_active}");
    println!("\nATURAN PEMILIK (sering rug = >=2 rug dan >=1/3 koinnya; kecuali >=2 koin >=$100k): pakai {keep} | keluarkan {drop_n}");
    for r in rows.iter().take(20) {
        println!("  {}", r.1);
    }
    Ok(())
}
