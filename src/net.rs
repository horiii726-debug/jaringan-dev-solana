//! Moonshot networks (data/moonall): coins that SURVIVED as moonshots (now mcap >= $100k, liquidity >= $20k) ->
//! creator -> funder chain (4 levels) -> wallets funded by those funders (crawl) -> other creators in the network.
//! Clusters = moonshot coins that share any non-exchange funder wallet. For each cluster: how many coins its creators
//! made in total (6-18 Nov) and how many of those became moonshots (hit rate) -> is it a repeatable team?
use crate::graph::read_lines;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

struct Uf(Vec<usize>);
impl Uf {
    fn f(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut y = x;
        while self.0[y] != r {
            let n = self.0[y];
            self.0[y] = r;
            y = n;
        }
        r
    }
    fn u(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.f(a), self.f(b));
        if ra != rb {
            self.0[ra] = rb;
        }
    }
}

pub fn run(d: &str) -> Result<(), String> {
    let mut exch: HashSet<String> = fs::read_to_string("data/p3/exchange_list.txt").unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
    // behavioural labels (src/svc.rs): only OPERATOR wallets may link creators
    exch.extend(read_lines(&format!("{d}/svc.jsonl")).into_iter().filter(|v| v["label"] != "OPERATOR" && v["label"] != "UNKNOWN").filter_map(|v| v["w"].as_str().map(String::from)));
    let moon: HashMap<String, Value> = read_lines(&format!("{d}/moon.jsonl")).into_iter().map(|v| (v["mint"].as_str().unwrap_or("").to_string(), v)).collect();
    // all creates 6-18 Nov: mint, creator, user, bt, slot
    let mut coins_of: HashMap<String, Vec<String>> = HashMap::new();
    let mut creator_of: HashMap<String, (String, i64)> = HashMap::new();
    for l in fs::read_to_string(format!("{d}/all_creates.tsv")).unwrap_or_default().lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 4 {
            continue;
        }
        if creator_of.insert(f[0].to_string(), (f[1].to_string(), f[3].parse().unwrap_or(0))).is_none() {
            coins_of.entry(f[1].to_string()).or_default().push(f[0].to_string());
        }
    }
    // Owner rule 18 Nov: a random transfer is NOT family. A link counts only when it is the wallet's FIRST funding
    // (oldest tx), a direct System transfer (not the "largest balance drop" fallback) and material (>= MIN_LINK_LAM).
    // Outgoing transfers seen in crawls are never used as links.
    const MIN_LINK_LAM: u64 = 50_000_000; // 0.05 SOL
    let mut funder: HashMap<String, (String, u64, String)> = HashMap::new(); // w -> (funder, n_sigs of w, kind)
    let mut weak = 0usize;
    for l in 1..=4 {
        for v in read_lines(&format!("{d}/fund_l{l}.jsonl")) {
            if let (Some(w), Some(f)) = (v["w"].as_str(), v["funder"].as_str()) {
                if v["kind"] != "transfer" || v["lam"].as_u64().unwrap_or(0) < MIN_LINK_LAM {
                    weak += 1;
                    continue;
                }
                funder.insert(w.to_string(), (f.to_string(), v["n_sigs"].as_u64().unwrap_or(0), v["kind"].as_str().unwrap_or("").to_string()));
            }
        }
    }
    println!("first-funding links kept={} dropped as weak (balance-fallback or < 0.05 SOL)={weak}", funder.len());
    let mut kids: HashMap<String, HashSet<String>> = HashMap::new();
    for (w, (f, _, _)) in &funder {
        kids.entry(f.clone()).or_default().insert(w.clone());
    }
    let watch: HashMap<String, String> = ["data/fwd/watch.tsv", "data/fwd/watch_clean.tsv"].iter()
        .flat_map(|p| fs::read_to_string(p).unwrap_or_default().lines().filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); Some((f.first()?.to_string(), f.get(1)?.to_string())) }).collect::<Vec<_>>())
        .collect();
    // chain per moon coin
    let mints: Vec<String> = moon.keys().filter(|m| creator_of.contains_key(*m)).cloned().collect();
    let mut chain_of: HashMap<String, Vec<String>> = HashMap::new();
    for m in &mints {
        let c = creator_of[m].0.clone();
        let mut ch = Vec::new();
        let mut cur = c.clone();
        for _ in 0..4 {
            let Some((f, _, _)) = funder.get(&cur) else { break };
            if exch.contains(f) || ch.contains(f) || *f == c {
                break;
            }
            ch.push(f.clone());
            cur = f.clone();
        }
        chain_of.insert(m.clone(), ch);
    }
    // clusters: share creator or any chain funder (busy funders with > 300 funded wallets = service, not linked)
    let idx: HashMap<&String, usize> = mints.iter().enumerate().map(|(i, m)| (m, i)).collect();
    let mut uf = Uf((0..mints.len()).collect());
    let mut by_wallet: HashMap<String, usize> = HashMap::new();
    for m in &mints {
        let i = idx[m];
        let mut ws = vec![creator_of[m].0.clone()];
        ws.extend(chain_of[m].iter().filter(|f| kids.get(*f).map(|k| k.len() <= 300).unwrap_or(true)).cloned());
        for w in ws {
            match by_wallet.get(&w) {
                Some(&j) => uf.u(i, j),
                None => {
                    by_wallet.insert(w, i);
                }
            }
        }
    }
    let mut cl: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for m in &mints {
        let r = uf.f(idx[m]);
        cl.entry(r).or_default().push(m.clone());
    }
    // network of a cluster: creators + chain funders + wallets they funded -> creators among them
    let mut out_cl: Vec<Value> = Vec::new();
    for ms in cl.values() {
        let mut net: HashSet<String> = HashSet::new();
        for m in ms {
            net.insert(creator_of[m].0.clone());
            for f in &chain_of[m] {
                net.insert(f.clone());
                if let Some(k) = kids.get(f).filter(|k| k.len() <= 300) {
                    net.extend(k.iter().cloned());
                }
            }
        }
        let creators: Vec<&String> = net.iter().filter(|w| coins_of.contains_key(*w)).collect();
        let coins: Vec<&String> = creators.iter().flat_map(|w| coins_of[*w].iter()).collect();
        let moons_net = coins.iter().filter(|m| moon.contains_key(**m)).count();
        let classes: HashSet<String> = net.iter().filter_map(|w| watch.get(w).cloned()).collect();
        let max_mcap = ms.iter().map(|m| moon[m]["mcap"].as_f64().unwrap_or(0.0)).fold(0.0, f64::max);
        let roots: HashSet<String> = ms.iter().filter_map(|m| chain_of[m].last().cloned()).collect();
        let mut first = ms.iter().map(|m| creator_of[m].1).collect::<Vec<_>>();
        first.sort();
        out_cl.push(json!({"moon_coins": ms.len(), "symbols": ms.iter().map(|m| moon[m]["sym"].clone()).collect::<Vec<_>>(),
            "max_mcap_now": max_mcap.round(), "network_wallets": net.len(), "network_creators": creators.len(), "network_coins": coins.len(),
            "network_moons": moons_net, "hit_rate": if coins.is_empty() { 0.0 } else { moons_net as f64 / coins.len() as f64 },
            "classes": classes.into_iter().collect::<Vec<_>>(), "roots": roots.into_iter().collect::<Vec<_>>(),
            "creators": ms.iter().map(|m| creator_of[m].0.clone()).collect::<HashSet<_>>().into_iter().collect::<Vec<_>>(),
            "first_moon_bt": first.first(), "last_moon_bt": first.last()}));
    }
    out_cl.sort_by(|a, b| b["moon_coins"].as_u64().cmp(&a["moon_coins"].as_u64()).then(b["max_mcap_now"].as_f64().unwrap_or(0.0).total_cmp(&a["max_mcap_now"].as_f64().unwrap_or(0.0))));
    let n = mints.len();
    let solo = out_cl.iter().filter(|c| c["moon_coins"] == 1).count();
    let multi: Vec<&Value> = out_cl.iter().filter(|c| c["moon_coins"].as_u64().unwrap_or(0) >= 2).collect();
    let fresh_creator = mints.iter().filter(|m| coins_of[&creator_of[*m].0].len() == 1).count();
    let exch_funded = mints.iter().filter(|m| funder.get(&creator_of[*m].0).map(|f| exch.contains(&f.0)).unwrap_or(false)).count();
    let tiers: Vec<(f64, usize)> = [1e5, 1e6, 1e7, 1e8].iter().map(|t| (*t, mints.iter().filter(|m| moon[*m]["mcap"].as_f64().unwrap_or(0.0) >= *t).count())).collect();
    fs::write(format!("{d}/net_report.json"), serde_json::to_string_pretty(&json!({"moon_coins": n, "clusters": out_cl})).unwrap_or_default()).map_err(|e| e.to_string())?;
    println!("moonshot coins (now >= $100k mcap, >= $20k liquidity, created 6-18 Nov): {n}");
    for (t, c) in tiers {
        println!("  now >= ${t:.0}: {c}");
    }
    println!("creator made only this 1 coin: {fresh_creator} | creator funded straight from an exchange: {exch_funded}");
    println!("clusters: {} (single-moon {solo}, with >=2 moons {})", out_cl.len(), multi.len());
    println!("moons\tmax_mcap_now\tnet_wallets\tnet_creators\tnet_coins\tnet_moons\thit_rate\tclasses\tsymbols");
    for c in multi.iter().take(30) {
        println!("{}\t{}\t{}\t{}\t{}\t{}\t{:.1}%\t{}\t{}", c["moon_coins"], c["max_mcap_now"], c["network_wallets"], c["network_creators"], c["network_coins"],
            c["network_moons"], 100.0 * c["hit_rate"].as_f64().unwrap_or(0.0), c["classes"], c["symbols"].to_string().chars().take(80).collect::<String>());
    }
    Ok(())
}
