//! Moonshot trace (fresh window): coins that reached >= $50k (>= 20 trades at >= $50k) -> how they started
//! (same-slot bundle graduation vs organic from a small cap), creator -> funder chain (3 levels, full history via RPC),
//! wallets connected through the same funders (window + historical edges), and links to watched classes A/B/C.
use crate::graph::read_lines;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;

fn tsv_map(path: &str, k: usize, v: usize, skip: usize) -> HashMap<String, String> {
    fs::read_to_string(path).unwrap_or_default().lines().skip(skip).filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((f.get(k)?.to_string(), f.get(v)?.to_string()))
    }).collect()
}

pub fn run(dir: &str) -> Result<(), String> {
    let watch = tsv_map("data/fwd/watch.tsv", 0, 1, 0);
    let kelas = tsv_map("dataset/teams/kelas_abc.tsv", 2, 0, 1);
    let team_of = tsv_map("dataset/teams/team_members.tsv", 1, 0, 0);
    let exch: HashSet<String> = fs::read_to_string("dataset/wallets/exchange_list.txt").unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
    let class_of = |w: &str| -> Option<String> {
        watch.get(w).cloned().or_else(|| kelas.get(w).map(|k| format!("{k}_hist"))).or_else(|| team_of.get(w).map(|_| "TEAM_hist".into()))
    };
    // funder chain from RPC (full history)
    let mut fund: HashMap<String, Value> = HashMap::new();
    for l in 1..=3 {
        for v in read_lines(&format!("{dir}/fund_l{l}.jsonl")) {
            fund.insert(v["w"].as_str().unwrap_or("").to_string(), v);
        }
    }
    // who funded whom: window (Bitquery) + historical (6-16 Nov) + RPC chain
    let mut kids: HashMap<String, HashSet<String>> = HashMap::new();
    for v in read_lines("data/fwd/funders.jsonl").into_iter().chain(read_lines("data/raw/fund_edges.jsonl")) {
        if let (Some(w), Some(f)) = (v["w"].as_str(), v["funder"].as_str()) {
            kids.entry(f.to_string()).or_default().insert(w.to_string());
        }
    }
    for (w, v) in &fund {
        if let Some(f) = v["funder"].as_str() {
            kids.entry(f.to_string()).or_default().insert(w.clone());
        }
    }
    let creates = read_lines("data/fwd/creates.jsonl");
    let mut coins_by: HashMap<String, Vec<String>> = HashMap::new();
    for c in &creates {
        coins_by.entry(c["signer"].as_str().unwrap_or("").to_string()).or_default().push(c["mint"].as_str().unwrap_or("").to_string());
    }
    let moon: HashMap<String, Value> = read_lines("data/fwd/moon.jsonl").into_iter().map(|v| (v["mint"].as_str().unwrap_or("").to_string(), v)).collect();
    let mut early: HashMap<String, Vec<Value>> = HashMap::new();
    for v in read_lines(&format!("{dir}/early.jsonl")) {
        early.entry(v["mint"].as_str().unwrap_or("").to_string()).or_default().push(v);
    }
    let mut rows: Vec<Value> = Vec::new();
    let mut funder_moons: HashMap<String, HashSet<String>> = HashMap::new();
    for c in read_lines(&format!("{dir}/creates.jsonl")) {
        let mint = c["mint"].as_str().unwrap_or("").to_string();
        let cs = c["slot"].as_u64().unwrap_or(0);
        let creator = c["signer"].as_str().unwrap_or("").to_string();
        let mut e = early.get(&mint).cloned().unwrap_or_default();
        e.sort_by_key(|x| (x["slot"].as_u64().unwrap_or(0), x["txi"].as_u64().unwrap_or(0)));
        let first_amm = e.iter().find(|x| x["dex"] == "pump_amm").and_then(|x| x["slot"].as_u64());
        let mc = |x: &Value| x["usd"].as_f64().unwrap_or(0.0) * 1e9;
        let start_mcap = e.iter().find(|x| x["slot"].as_u64() == Some(cs)).map(mc);
        let next_mcap = e.iter().find(|x| x["slot"].as_u64().unwrap_or(0) > cs && (x["dex"] == "pump" || x["dex"] == "pump_amm")).map(mc);
        let dev_bundle_sol: f64 = e.iter().filter(|x| x["slot"].as_u64() == Some(cs) && x["buy"] == true).map(|x| x["sol"].as_f64().unwrap_or(0.0)).sum();
        // chain
        let mut chain: Vec<Value> = Vec::new();
        let mut cur = creator.clone();
        let mut connected: HashSet<String> = HashSet::from([creator.clone()]);
        for _ in 0..3 {
            let Some(f) = fund.get(&cur).and_then(|v| v["funder"].as_str()).map(String::from) else { break };
            let v = &fund[&cur];
            let is_exch = exch.contains(&f);
            chain.push(json!({"funder": f, "kind": v["kind"], "class": class_of(&f), "exchange": is_exch,
                "funded_wallets_known": kids.get(&f).map(|k| k.len()).unwrap_or(0)}));
            if is_exch {
                break;
            }
            connected.insert(f.clone());
            if let Some(k) = kids.get(&f) {
                if k.len() <= 200 {
                    connected.extend(k.iter().cloned());
                }
            }
            funder_moons.entry(f.clone()).or_default().insert(mint.clone());
            cur = f;
        }
        let creators_conn: Vec<&String> = connected.iter().filter(|w| coins_by.contains_key(*w)).collect();
        let coins_conn: usize = creators_conn.iter().map(|w| coins_by[*w].len()).sum();
        let moons_conn: usize = creators_conn.iter().flat_map(|w| coins_by[*w].iter()).filter(|m| moon.contains_key(*m)).count();
        let classes: Vec<String> = {
            let mut s: Vec<String> = connected.iter().filter_map(|w| class_of(w)).collect::<HashSet<_>>().into_iter().collect();
            s.sort();
            s
        };
        let m = &moon[&mint];
        rows.push(json!({"mint": mint, "sym": m["sym"], "max_mcap_usd": (m["maxp"].as_f64().unwrap_or(0.0) * 1e9).round(), "trades_ge_50k": m["n50"],
            "start_mcap_usd": start_mcap.map(|x| x.round()), "first_reachable_mcap_usd": next_mcap.map(|x| x.round()), "create_tx_buys_sol": dev_bundle_sol,
            "same_slot_grad": first_amm == Some(cs), "graduated_slot_offset": first_amm.map(|a| a.saturating_sub(cs)),
            "creator": creator, "creator_class": class_of(&creator), "creator_coins_window": coins_by.get(&creator).map(|v| v.len()).unwrap_or(0),
            "creator_n_sigs": fund.get(&creator).map(|v| v["n_sigs"].clone()), "chain": chain,
            "connected_wallets": connected.len(), "connected_creators_window": creators_conn.len(), "connected_coins_window": coins_conn,
            "connected_moons_window": moons_conn, "classes_in_network": classes}));
    }
    rows.sort_by(|a, b| b["max_mcap_usd"].as_f64().unwrap_or(0.0).total_cmp(&a["max_mcap_usd"].as_f64().unwrap_or(0.0)));
    let n = rows.len();
    let same = rows.iter().filter(|r| r["same_slot_grad"] == true).count();
    let organic = rows.iter().filter(|r| r["same_slot_grad"] == false && r["first_reachable_mcap_usd"].as_f64().map(|x| x < 15_000.0).unwrap_or(false)).count();
    let direct_cls = rows.iter().filter(|r| !r["creator_class"].is_null()).count();
    let net_cls = rows.iter().filter(|r| r["classes_in_network"].as_array().map(|a| !a.is_empty()).unwrap_or(false)).count();
    let exch_funded = rows.iter().filter(|r| r["chain"][0]["exchange"] == true).count();
    let shared: Vec<Value> = {
        let mut v: Vec<(&String, &HashSet<String>)> = funder_moons.iter().filter(|(_, m)| m.len() >= 2).collect();
        v.sort_by_key(|(_, m)| std::cmp::Reverse(m.len()));
        v.into_iter().take(20).map(|(f, m)| json!({"funder": f, "moon_coins": m.len(), "class": class_of(f), "funded_wallets_known": kids.get(f).map(|k| k.len()).unwrap_or(0)})).collect()
    };
    let out = json!({"n": n, "same_slot_bundle_grad": same, "organic_from_small_cap": organic, "creator_in_watch_or_hist_class": direct_cls,
        "network_touches_class": net_cls, "first_funder_exchange": exch_funded, "shared_funders": shared, "coins": rows});
    fs::write(format!("{dir}/moon_report.json"), serde_json::to_string_pretty(&out).unwrap_or_default()).map_err(|e| e.to_string())?;
    println!("moon coins={n} same_slot_bundle={same} organic_small_cap_start={organic} creator_class={direct_cls} network_touches_class={net_cls} first_funder_exchange={exch_funded}");
    println!("funders shared by >=2 moon coins: {}", shared.len());
    for s in shared.iter().take(10) {
        println!("  {s}");
    }
    println!("sym\tmax_mcap\tstart\treach\tsame_slot\tcreator_coins\tchain_len\tconn_wallets\tconn_coins\tconn_moons\tclasses");
    for r in rows.iter().take(25) {
        println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", r["sym"].as_str().unwrap_or(""), r["max_mcap_usd"], r["start_mcap_usd"], r["first_reachable_mcap_usd"],
            r["same_slot_grad"], r["creator_coins_window"], r["chain"].as_array().map(|a| a.len()).unwrap_or(0), r["connected_wallets"],
            r["connected_coins_window"], r["connected_moons_window"], r["classes_in_network"]);
    }
    Ok(())
}
