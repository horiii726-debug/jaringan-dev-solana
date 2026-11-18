//! Owner 18 Nov: rank the core wallets (induk) of known-origin families by their history, choose the good ones
//! using ONLY coins created before `cut`, and list their coins created after `cut` for an honest test.
//! Good induk (fixed before looking at the test): >= 2 coins before cut and >= 50% of them still on DexScreener,
//! or any coin before cut with mcap >= $100k.
//! Output in <dir>: rank.txt, test_good.tsv / test_all.tsv (mint curve bt slot) for `trades` + `entry1s`.
//! Usage: jaringan famsel <dir> <cut_unix>
use crate::graph::read_lines;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

pub fn run(dir: &str, cut: i64) -> Result<(), String> {
    let ups = read_lines(&format!("{dir}/up.jsonl"));
    let creates = read_lines(&format!("{dir}/creates.jsonl"));
    let mut dev_root: HashMap<String, String> = HashMap::new();
    for u in &ups {
        if u["induk"] != u["dev"] && u["end"] != "wallet_ramai" {
            dev_root.insert(u["dev"].as_str().unwrap_or("").to_string(), u["induk"].as_str().unwrap_or("").to_string());
        }
    }
    let roots: HashSet<String> = dev_root.values().cloned().collect();
    // coins per root: family creates + coins of the seed devs (graduated list)
    let mut coins: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for c in &creates {
        let r = c["root"].as_str().unwrap_or("");
        if roots.contains(r) {
            coins.entry(r.to_string()).or_default().insert(c["mint"].as_str().unwrap_or("").to_string());
        }
    }
    let mut grad: HashSet<String> = HashSet::new();
    for v in read_lines("data/roots/pool_creators.jsonl") {
        let m = v["mint"].as_str().unwrap_or("").to_string();
        if let Some(r) = dev_root.get(v["coin_creator"].as_str().unwrap_or("")) {
            coins.entry(r.clone()).or_default().insert(m.clone());
        }
        grad.insert(m);
    }
    let need: HashSet<String> = coins.values().flatten().cloned().collect();
    // create info (curve, bt, slot) from our launch files
    let mut info: HashMap<String, (String, i64, u64)> = HashMap::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !(n.starts_with("launches_2026-") && n.ends_with(".jsonl")) { continue; }
            for v in read_lines(&format!("{d}/{n}")) {
                if v["kind"] != "create" { continue; }
                let m = v["mint"].as_str().unwrap_or("");
                if need.contains(m) && !info.contains_key(m) {
                    info.insert(m.to_string(), (v["curve"].as_str().unwrap_or("").to_string(), v["bt"].as_i64().unwrap_or(0), v["slot"].as_u64().unwrap_or(0)));
                }
            }
        }
    }
    for f in ["data/roots/alive_roles.jsonl", "data/roots/roles.jsonl", "data/roots/roles2.jsonl"] {
        for v in read_lines(f) {
            let m = v["mint"].as_str().unwrap_or("");
            if need.contains(m) && !info.contains_key(m) {
                if let Some(c) = v["curve"].as_str() {
                    info.insert(m.to_string(), (c.to_string(), v["bt"].as_i64().unwrap_or(0), v["slot"].as_u64().unwrap_or(0)));
                }
            }
        }
    }
    let mut bt_creates: HashMap<String, i64> = HashMap::new();
    for c in &creates {
        bt_creates.insert(c["mint"].as_str().unwrap_or("").to_string(), c["bt"].as_i64().unwrap_or(0));
    }
    let mut ds: HashMap<String, (f64, i64)> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(), (v["mcap"].as_f64().unwrap_or(0.0), v["pair_created"].as_i64().unwrap_or(0) / 1000));
    }
    let bt_of = |m: &str| -> i64 {
        info.get(m).map(|x| x.1).filter(|b| *b > 0)
            .or_else(|| bt_creates.get(m).copied().filter(|b| *b > 0))
            .or_else(|| ds.get(m).map(|x| x.1).filter(|b| *b > 0))
            .unwrap_or(0)
    };
    let mut rank: Vec<(usize, usize, String)> = Vec::new();
    let (mut test_good, mut test_all): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let (mut n_good, mut n_test_good, mut n_test_all) = (0, 0, 0);
    for (r, ms) in &coins {
        let (mut tr, mut tr_ds, mut tr_big, mut te, mut te_ds, mut te_big, mut te_grad, mut tr_grad) = (0, 0, 0, 0, 0, 0, 0, 0);
        for m in ms {
            let b = bt_of(m);
            let mc = ds.get(m).map(|x| x.0).unwrap_or(0.0);
            let (on_ds, big, g) = (ds.contains_key(m), mc >= 100_000.0, grad.contains(m));
            if b > 0 && b < cut {
                tr += 1; tr_ds += on_ds as usize; tr_big += big as usize; tr_grad += g as usize;
            } else if b >= cut {
                te += 1; te_ds += on_ds as usize; te_big += big as usize; te_grad += g as usize;
            }
        }
        let good = (tr >= 2 && tr_ds * 2 >= tr) || tr_big >= 1;
        if good { n_good += 1; }
        for m in ms {
            let b = bt_of(m);
            if b >= cut {
                if let Some((c, bt, s)) = info.get(m) {
                    let line = format!("{m}\t{c}\t{bt}\t{s}");
                    test_all.push(line.clone());
                    n_test_all += 1;
                    if good { test_good.push(line); n_test_good += 1; }
                }
            }
        }
        rank.push((tr_ds * 1000 + tr_big, ms.len(), format!(
            "{r} | {} | sebelum cut: koin {tr} lulus {tr_grad} DS {tr_ds} >=$100k {tr_big} | sesudah cut: koin {te} lulus {te_grad} DS {te_ds} >=$100k {te_big}",
            if good { "BAGUS" } else { "-" })));
    }
    rank.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let head = format!("induk asal jelas: {} | induk BAGUS (dipilih dari data sebelum cut {cut}): {n_good}\nkoin uji sesudah cut: semua family {n_test_all} | family BAGUS {n_test_good}\n\n",
        coins.len());
    let body: String = rank.iter().map(|r| format!("{}\n", r.2)).collect();
    fs::write(format!("{dir}/rank.txt"), format!("{head}{body}")).map_err(|e| e.to_string())?;
    fs::write(format!("{dir}/test_good.tsv"), test_good.join("\n")).map_err(|e| e.to_string())?;
    fs::write(format!("{dir}/test_all.tsv"), test_all.join("\n")).map_err(|e| e.to_string())?;
    print!("{head}");
    for r in rank.iter().take(25) {
        println!("{}", r.2);
    }
    Ok(())
}
