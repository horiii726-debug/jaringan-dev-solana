#![allow(dead_code)] // fields used by later steps (outcomes, scores)
//! P3.4: funding graph, infrastructure removal, networks (union-find), H1 measurement.
//! Inputs: launches jsonl files + fund jsonl files (any depth). All evidence keeps its slot for as-of use.
use crate::rpc::cfg_u64;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};

pub struct Launch {
    pub mint: String,
    pub creator: String,
    pub user: String,
    pub payer: String,
    pub slot: u64,
    pub bt: i64,
    pub mayhem: bool,
    pub sol_quote: bool,
}

pub struct Fund {
    pub funder: Option<String>,
    pub n_sigs: u64,
    pub deep: bool,
    pub first_slot: u64,
    pub first_bt: i64,
}

pub fn read_lines(path: &str) -> Vec<Value> {
    let Ok(f) = fs::File::open(path) else { return vec![] };
    BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str(&l).ok()).collect()
}

pub fn load_launches(files: &[String]) -> Vec<Launch> {
    let mut out = Vec::new();
    for p in files {
        for v in read_lines(p) {
            if v["kind"] != "create" {
                continue;
            }
            let q = v["quote"].as_str().unwrap_or(crate::decode::SYSTEM);
            out.push(Launch {
                mint: v["mint"].as_str().unwrap_or("").into(),
                creator: v["creator"].as_str().unwrap_or("").into(),
                user: v["user"].as_str().unwrap_or("").into(),
                payer: v["payer"].as_str().unwrap_or("").into(),
                slot: v["slot"].as_u64().unwrap_or(0),
                bt: v["bt"].as_i64().unwrap_or(0),
                mayhem: v["mayhem"] == true,
                sol_quote: q == crate::decode::SYSTEM || q == "So11111111111111111111111111111111111111112",
            });
        }
    }
    out.sort_by_key(|l| l.slot);
    out
}

pub fn load_fund(files: &[String]) -> HashMap<String, Fund> {
    let mut m = HashMap::new();
    for p in files {
        for v in read_lines(p) {
            let w = v["w"].as_str().unwrap_or("").to_string();
            if w.is_empty() || v["kind"] == "rpc_error" {
                continue;
            }
            m.insert(w, Fund {
                funder: v["funder"].as_str().map(String::from),
                n_sigs: v["n_sigs"].as_u64().unwrap_or(0),
                deep: v["deep"] == true,
                first_slot: v["first_slot"].as_u64().unwrap_or(0),
                first_bt: v["first_bt"].as_i64().unwrap_or(0),
            });
        }
    }
    m
}

struct Uf {
    p: Vec<usize>,
}
impl Uf {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.p[r] != r {
            r = self.p[r];
        }
        let mut c = x;
        while self.p[c] != r {
            let n = self.p[c];
            self.p[c] = r;
            c = n;
        }
        r
    }
    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.p[a.max(b)] = a.min(b);
        }
    }
}

fn quant(v: &mut [u64], q: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[((v.len() - 1) as f64 * q).round() as usize]
}

/// Infrastructure rule (config/graph.toml): deep history OR fan-out at/above the cut.
pub struct Infra {
    pub min_sigs: u64,
    pub min_fanout: u64,
}

pub fn run(launch_files: &[String], fund_files: &[String], tag: &str) -> Result<(), String> {
    let ls = load_launches(launch_files);
    let fm = load_fund(fund_files);
    let infra = Infra {
        min_sigs: cfg_u64("config/graph.toml", "infra_min_sigs").ok_or("config/graph.toml infra_min_sigs missing")?,
        min_fanout: cfg_u64("config/graph.toml", "infra_min_fanout").ok_or("config/graph.toml infra_min_fanout missing")?,
    };
    // Launch wallets: creator, and the create signer when it differs.
    let mut launches_of: HashMap<&str, u64> = HashMap::new();
    let mut launch_wallets: HashSet<&str> = HashSet::new();
    for l in &ls {
        *launches_of.entry(&l.creator).or_default() += 1;
        launch_wallets.insert(&l.creator);
        launch_wallets.insert(&l.user);
    }
    // Fan-out: distinct wallets each funder funded (within everything we fetched).
    let mut fanout: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (w, f) in &fm {
        if let Some(fu) = &f.funder {
            fanout.entry(fu.as_str()).or_default().insert(w.as_str());
        }
    }
    let is_infra = |w: &str| -> bool {
        let deep = fm.get(w).map(|f| f.deep || f.n_sigs >= infra.min_sigs).unwrap_or(false);
        deep || fanout.get(w).map(|s| s.len() as u64 >= infra.min_fanout).unwrap_or(false)
    };
    // Node ids
    let mut id: HashMap<String, usize> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let nid = |w: &str, id: &mut HashMap<String, usize>, names: &mut Vec<String>| -> usize {
        if let Some(&i) = id.get(w) {
            return i;
        }
        names.push(w.to_string());
        id.insert(w.to_string(), names.len() - 1);
        names.len() - 1
    };
    let mut edges: Vec<(usize, usize, &'static str)> = Vec::new();
    // strong: wallet <- first funder (walk up the fetched chain, stop at infra or unknown)
    for (w, f) in &fm {
        let Some(fu) = &f.funder else { continue };
        if is_infra(w) || is_infra(fu) || fu == w {
            continue;
        }
        let a = nid(w, &mut id, &mut names);
        let b = nid(fu, &mut id, &mut names);
        edges.push((a, b, "funding"));
    }
    // medium: create signer differs from creator
    for l in &ls {
        if l.user != l.creator && !is_infra(&l.user) {
            let a = nid(&l.creator, &mut id, &mut names);
            let b = nid(&l.user, &mut id, &mut names);
            edges.push((a, b, "signer"));
        }
    }
    let n = names.len();
    let mut uf = Uf { p: (0..n).collect() };
    for &(a, b, t) in &edges {
        if t == "funding" {
            uf.join(a, b);
        }
    }
    let mut uf2 = Uf { p: uf.p.clone() };
    for &(a, b, t) in &edges {
        if t == "signer" {
            uf2.join(a, b);
        }
    }
    // Network stats over funding components (strong only) and funding+signer components.
    let mut comp_creators: HashMap<usize, HashSet<&str>> = HashMap::new();
    let mut comp2_creators: HashMap<usize, HashSet<&str>> = HashMap::new();
    for l in &ls {
        if let Some(&i) = id.get(&l.creator) {
            comp_creators.entry(uf.find(i)).or_default().insert(&l.creator);
            comp2_creators.entry(uf2.find(i)).or_default().insert(&l.creator);
        }
        if l.user != l.creator {
            if let Some(&i) = id.get(&l.user) {
                comp2_creators.entry(uf2.find(i)).or_default().insert(&l.creator);
            }
        }
    }
    // H1: single-launch creators (by creator field) whose wallet sits in a component with >= 2 creators.
    let mut single = 0u64;
    let mut single_with_fund = 0u64;
    let (mut linked, mut linked2) = (0u64, 0u64);
    let mut fund_status: BTreeMap<&str, u64> = BTreeMap::new();
    for (c, &k) in &launches_of {
        let st = match fm.get(*c) {
            None => "not_fetched",
            Some(f) if f.deep || f.n_sigs >= infra.min_sigs => "creator_deep",
            Some(f) if f.funder.is_none() => "no_funder",
            Some(f) if is_infra(f.funder.as_deref().unwrap_or("")) => "funder_infra",
            Some(_) => "funder_ok",
        };
        *fund_status.entry(st).or_default() += 1;
        if k != 1 {
            continue;
        }
        single += 1;
        if st == "funder_ok" {
            single_with_fund += 1;
        }
        if let Some(&i) = id.get(*c) {
            if comp_creators.get(&uf.find(i)).map(|s| s.len() >= 2).unwrap_or(false) {
                linked += 1;
            }
            if comp2_creators.get(&uf2.find(i)).map(|s| s.len() >= 2).unwrap_or(false) {
                linked2 += 1;
            }
        }
    }
    // Launch share by network size
    let mut launches_in_net = 0u64;
    let mut net_rows: Vec<(usize, usize, u64)> = Vec::new(); // root, creators, launches
    let mut launches_per_comp: HashMap<usize, u64> = HashMap::new();
    for l in &ls {
        if let Some(&i) = id.get(&l.creator) {
            *launches_per_comp.entry(uf.find(i)).or_default() += 1;
        }
    }
    for (r, cs) in &comp_creators {
        let nl = launches_per_comp.get(r).copied().unwrap_or(0);
        if cs.len() >= 2 {
            launches_in_net += nl;
            net_rows.push((*r, cs.len(), nl));
        }
    }
    net_rows.sort_by_key(|r| std::cmp::Reverse(r.2));
    let mut fo: Vec<u64> = fanout.values().map(|s| s.len() as u64).collect();
    let fo_q: Vec<u64> = [0.5, 0.9, 0.99, 1.0].iter().map(|&q| quant(&mut fo, q)).collect();
    let infra_n = fanout.keys().filter(|w| is_infra(w)).count();
    // Funding lag for creators with an ok funder (first funding -> first create), seconds
    let mut first_create: HashMap<&str, i64> = HashMap::new();
    for l in &ls {
        first_create.entry(&l.creator).or_insert(l.bt);
    }
    let mut lags: Vec<u64> = first_create
        .iter()
        .filter_map(|(c, &bt)| fm.get(*c).filter(|f| f.funder.is_some() && f.first_bt > 0).map(|f| (bt - f.first_bt).max(0) as u64))
        .collect();
    let lag_q: Vec<u64> = [0.1, 0.25, 0.5, 0.75, 0.9].iter().map(|&q| quant(&mut lags, q)).collect();
    // Write networks file
    fs::create_dir_all("data/p3").map_err(|e| e.to_string())?;
    let mut f = fs::File::create(format!("data/p3/networks_{tag}.csv")).map_err(|e| e.to_string())?;
    writeln!(f, "wallet,network,role,launches").map_err(|e| e.to_string())?;
    for (i, w) in names.iter().enumerate() {
        let r = uf.find(i);
        if comp_creators.get(&r).map(|s| s.len() >= 2).unwrap_or(false) {
            let role = if launches_of.contains_key(w.as_str()) { "creator" } else if launch_wallets.contains(w.as_str()) { "signer" } else { "funder" };
            writeln!(f, "{w},{},{role},{}", names[r], launches_of.get(w.as_str()).copied().unwrap_or(0)).map_err(|e| e.to_string())?;
        }
    }
    let top: Vec<Value> = net_rows.iter().take(15).map(|(r, c, l)| json!({"root": names[*r], "creators": c, "launches": l})).collect();
    let out = json!({
        "launches": ls.len(), "creators": launches_of.len(), "single_launch_creators": single,
        "fund_status_creators": fund_status, "single_with_ok_funder": single_with_fund,
        "H1_single_linked_funding": linked, "H1_single_linked_funding_or_signer": linked2,
        "networks_ge2_creators": net_rows.len(), "launches_in_networks": launches_in_net,
        "funder_fanout_p50_p90_p99_max": fo_q, "infra_wallets": infra_n,
        "infra_rule": {"min_sigs": infra.min_sigs, "min_fanout": infra.min_fanout},
        "funding_lag_s_p10_p25_p50_p75_p90": lag_q, "top_networks": top,
        "edges": {"funding": edges.iter().filter(|e| e.2 == "funding").count(), "signer": edges.iter().filter(|e| e.2 == "signer").count()}
    });
    let s = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    fs::create_dir_all("reports/p3").map_err(|e| e.to_string())?;
    fs::write(format!("reports/p3/graph_{tag}.json"), &s).map_err(|e| e.to_string())?;
    println!("{s}");
    Ok(())
}
