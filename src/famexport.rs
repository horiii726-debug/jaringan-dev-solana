//! Owner 18 Nov: export EVERY wallet of every known-origin family, with its role, for GitHub.
//!   famexport <dir>  -> <dir>/export/family_ringkas.csv  (one row per family)
//!                       <dir>/export/wallet_family.csv   (one row per wallet)
//!                       <dir>/export/koin_family.csv     (one row per coin)
//! Roles: INDUK (core wallet), PERANTARA (relay on the money path dev <- ... <- induk), DEV (created a coin),
//! RAMAI (busy wallet, >= 300 tx read, not expanded: bot/trader/service-like), KELUARGA (other wallet that
//! received >= 0.05 SOL from the family).
//! Rug counts come from <dir>/rugs.tsv (written by deadcls).
use crate::graph::read_lines;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

const ACTIVE_SINCE: i64 = 1_794_700_800; // 15 Nov 00:00 UTC

pub fn run(dir: &str) -> Result<(), String> {
    let out = format!("{dir}/export");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let ups = read_lines(&format!("{dir}/up.jsonl"));
    let wallets = read_lines(&format!("{dir}/wallets.jsonl"));
    let creates = read_lines(&format!("{dir}/creates.jsonl"));
    let rugs: HashMap<String, usize> = fs::read_to_string(format!("{dir}/rugs.tsv")).unwrap_or_default().lines()
        .filter_map(|l| { let p: Vec<&str> = l.split('\t').collect(); Some((p.first()?.to_string(), p.get(1)?.parse().ok()?)) }).collect();
    let mut ds: HashMap<String, (f64, String, i64)> = HashMap::new();
    for v in read_lines("data/roots/ds.jsonl") {
        ds.insert(v["mint"].as_str().unwrap_or("").to_string(),
            (v["mcap"].as_f64().unwrap_or(0.0), v["sym"].as_str().unwrap_or("").to_string(), v["pair_created"].as_i64().unwrap_or(0) / 1000));
    }
    let mut grad: HashSet<String> = HashSet::new();
    let mut dev_coins: HashMap<String, Vec<String>> = HashMap::new();
    for v in read_lines("data/roots/pool_creators.jsonl") {
        let m = v["mint"].as_str().unwrap_or("").to_string();
        dev_coins.entry(v["coin_creator"].as_str().unwrap_or("").to_string()).or_default().push(m.clone());
        grad.insert(m);
    }
    // known-origin families
    let mut fam_devs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut relay: HashMap<String, HashSet<String>> = HashMap::new();
    let mut end_of: HashMap<String, String> = HashMap::new();
    for u in &ups {
        if u["induk"] == u["dev"] || u["end"] == "wallet_ramai" { continue; }
        let r = u["induk"].as_str().unwrap_or("").to_string();
        let d = u["dev"].as_str().unwrap_or("").to_string();
        fam_devs.entry(r.clone()).or_default().push(d.clone());
        end_of.entry(r.clone()).or_insert(u["end"].as_str().unwrap_or("").to_string());
        for c in u["chain"].as_array().into_iter().flatten() {
            let w = c["w"].as_str().unwrap_or("");
            if w != d && w != r { relay.entry(r.clone()).or_default().insert(w.to_string()); }
        }
    }
    // coins per family: creates found while crawling + coins of the seed devs
    let mut coins: HashMap<String, BTreeMap<String, (String, i64)>> = HashMap::new(); // root -> mint -> (creator, bt)
    let mut creator_of: HashMap<String, HashSet<String>> = HashMap::new(); // root -> wallets that created
    for c in &creates {
        let r = c["root"].as_str().unwrap_or("").to_string();
        if !fam_devs.contains_key(&r) { continue; }
        let m = c["mint"].as_str().unwrap_or("").to_string();
        let w = c["w"].as_str().unwrap_or("").to_string();
        coins.entry(r.clone()).or_default().entry(m).or_insert((w.clone(), c["bt"].as_i64().unwrap_or(0)));
        creator_of.entry(r).or_default().insert(w);
    }
    for (r, ds_) in &fam_devs {
        for d in ds_ {
            for m in dev_coins.get(d).into_iter().flatten() {
                let bt = ds.get(m).map(|x| x.2).unwrap_or(0);
                coins.entry(r.clone()).or_default().entry(m.clone()).or_insert((d.clone(), bt));
            }
            creator_of.entry(r.clone()).or_default().insert(d.clone());
        }
    }
    let mut fam_rows = vec!["induk,status,aktif_sejak_15nov,ujung_atas,jumlah_wallet,jumlah_dev,jumlah_perantara,koin,mati_pumpfun,lulus,di_dexscreener,mcap_40k,mcap_50k,mcap_100k,mcap_1jt,rug,koin_terbaik,mcap_terbaik".to_string()];
    let mut w_rows = vec!["induk,wallet,peran,kedalaman,jumlah_tx_dibaca,koin_dibuat".to_string()];
    let mut c_rows = vec!["induk,mint,simbol,dibuat_oleh,waktu_unix,status,mcap_usd".to_string()];
    let mut summary: Vec<(usize, String)> = Vec::new();
    for (r, devs) in &fam_devs {
        let cs = coins.get(r).cloned().unwrap_or_default();
        let n = cs.len();
        let cnt = |t: f64| cs.keys().filter(|m| ds.get(*m).map(|x| x.0).unwrap_or(0.0) >= t).count();
        let (k40, k50, k100, m1) = (cnt(40_000.0), cnt(50_000.0), cnt(100_000.0), cnt(1_000_000.0));
        let on_ds = cs.keys().filter(|m| ds.contains_key(*m)).count();
        let n_grad = cs.keys().filter(|m| grad.contains(*m)).count();
        let rg = rugs.get(r).copied().unwrap_or(0);
        let often_rug = rg >= 2 && rg * 3 >= n;
        let status = if often_rug && k100 < 2 { "KELUARKAN" } else { "PAKAI" };
        let recent = cs.values().filter(|x| x.1 >= ACTIVE_SINCE).count();
        let best = cs.keys().max_by(|a, b| ds.get(*a).map(|x| x.0).unwrap_or(0.0).partial_cmp(&ds.get(*b).map(|x| x.0).unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
        let (best_sym, best_mc) = best.map(|m| (ds.get(m).map(|x| x.1.clone()).unwrap_or_default(), ds.get(m).map(|x| x.0).unwrap_or(0.0))).unwrap_or_default();
        let fw: Vec<&serde_json::Value> = wallets.iter().filter(|w| w["root"] == r.as_str()).collect();
        let rel = relay.get(r).cloned().unwrap_or_default();
        let crt = creator_of.get(r).cloned().unwrap_or_default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut emit = |w: &str, depth: i64, ntx: i64, busy: bool| {
            if !seen.insert(w.to_string()) { return; }
            let made: Vec<String> = cs.iter().filter(|(_, x)| x.0 == w).map(|(m, _)| m.clone()).collect();
            let peran = if w == r { "INDUK" } else if crt.contains(w) || !made.is_empty() { "DEV" } else if rel.contains(w) { "PERANTARA" } else if busy { "RAMAI" } else { "KELUARGA" };
            w_rows.push(format!("{r},{w},{peran},{depth},{ntx},{}", made.join(";")));
        };
        emit(r, 0, -1, false);
        for d in devs { emit(d, -1, -1, false); }
        for x in &rel { emit(x, -1, -1, false); }
        for w in &fw {
            emit(w["w"].as_str().unwrap_or(""), w["depth"].as_i64().unwrap_or(-1), w["ntx"].as_i64().unwrap_or(-1), w["busy"] == true);
        }
        let n_w = seen.len();
        let n_dev = w_rows.iter().filter(|l| l.starts_with(r.as_str()) && l.contains(",DEV,")).count();
        let n_rel = w_rows.iter().filter(|l| l.starts_with(r.as_str()) && l.contains(",PERANTARA,")).count();
        for (m, (w, bt)) in &cs {
            let st = if let Some(x) = ds.get(m) { if x.0 >= 1_000_000.0 { "MOONSHOT_1JT" } else if x.0 >= 100_000.0 { "DS_100K" } else if x.0 >= 40_000.0 { "DS_40K" } else { "DS_KECIL" } }
                else if grad.contains(m) { "LULUS_HILANG" } else { "MATI_PUMPFUN" };
            c_rows.push(format!("{r},{m},{},{w},{bt},{st},{:.0}", ds.get(m).map(|x| x.1.replace(',', " ")).unwrap_or_default(), ds.get(m).map(|x| x.0).unwrap_or(0.0)));
        }
        fam_rows.push(format!("{r},{status},{},{},{n_w},{n_dev},{n_rel},{n},{},{n_grad},{on_ds},{k40},{k50},{k100},{m1},{rg},{},{best_mc:.0}",
            if recent > 0 { format!("YA ({recent})") } else { "TIDAK".into() }, end_of.get(r).cloned().unwrap_or_default().replace(',', " "),
            n - n_grad, best_sym.replace(',', " ")));
        summary.push((m1 * 1_000_000 + k100 * 1000 + on_ds, r.clone()));
    }
    fs::write(format!("{out}/family_ringkas.csv"), fam_rows.join("\n")).map_err(|e| e.to_string())?;
    fs::write(format!("{out}/wallet_family.csv"), w_rows.join("\n")).map_err(|e| e.to_string())?;
    fs::write(format!("{out}/koin_family.csv"), c_rows.join("\n")).map_err(|e| e.to_string())?;
    println!("family {} | wallet {} | koin {} -> {out}/", fam_rows.len() - 1, w_rows.len() - 1, c_rows.len() - 1);
    Ok(())
}
