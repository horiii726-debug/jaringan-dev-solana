//! Creator / team reputation from the BUYER's view (owner rules 18 Nov):
//!   every coin a wallet made is judged as if we bought immediately (first trade after the create slot):
//!   RUG (category RUGPULL or -50% within 5 min), QUIET (no trade after create / category SEPI), DOWN (return <= -20%),
//!   UP (could reach 2x from our entry), plus mean return. A wallet / team is dropped from the watchlist when it
//!   rugs often, its coins mostly fall or stay quiet, or our entries lose on average. As-of: only coins created
//!   before `cutoff` count, so the list can be tested on later coins.
use crate::graph::read_lines;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;

#[derive(Default, Clone, Debug)]
pub struct Rep {
    pub coins: u32,
    pub known: u32,
    pub up: u32,
    pub down: u32,
    pub rug: u32,
    pub quiet: u32,
    pub sum_ret: f64,
}

impl Rep {
    pub fn add(&mut self, o: &Outcome) {
        self.known += 1;
        self.up += o.up as u32;
        self.down += o.down as u32;
        self.rug += o.rug as u32;
        self.quiet += o.quiet as u32;
        self.sum_ret += o.ret;
    }
    pub fn mean(&self) -> f64 {
        if self.known == 0 { 0.0 } else { self.sum_ret / self.known as f64 }
    }
    /// owner rules: enough history, rarely rugs, mostly not falling / quiet, entries win on average, many coins rise
    pub fn verdict(&self) -> &'static str {
        if self.known < 3 {
            return "BELUM_CUKUP_DATA";
        }
        let k = self.known as f64;
        if self.rug as f64 / k > 0.2 {
            return "KELUAR_RUGPULL";
        }
        if (self.down + self.quiet) as f64 / k > 0.5 {
            return "KELUAR_TURUN_SEPI";
        }
        if self.mean() <= 0.0 {
            return "KELUAR_ENTRY_MINUS";
        }
        if (self.up as f64 / k) < 0.3 {
            return "KELUAR_JARANG_NAIK";
        }
        "BERKUALITAS"
    }
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub ret: f64,
    pub up: bool,
    pub down: bool,
    pub rug: bool,
    pub quiet: bool,
}

pub fn outcome(row: &Value, cat: Option<&str>) -> Option<Outcome> {
    let st = row["status"].as_str().unwrap_or("");
    let quiet = st == "NotFilled" || st == "NOT_FILLED" || st == "NO_TRADES" || cat == Some("SEPI");
    if quiet {
        return Some(Outcome { ret: 0.0, up: false, down: false, rug: cat == Some("RUGPULL"), quiet: true });
    }
    if st != "FILLED" {
        return None;
    }
    let ret = row["r_30m"].as_f64().or(row["r_last"].as_f64())?;
    let r5 = row["r_5m"].as_f64();
    let best = row["r_best"].as_f64().unwrap_or(ret);
    let rug = cat == Some("RUGPULL") || r5.map(|x| x <= -0.5).unwrap_or(false);
    Some(Outcome { ret, up: best >= 1.0, down: ret <= -0.2, rug, quiet: false })
}

/// rep <launch files,..> <outcome rows files,..> <cutoff_unix> <out_dir>
pub fn run(launch_files: &[String], row_files: &[String], cutoff: i64, out_dir: &str) -> Result<(), String> {
    // mint -> (creator, bt)
    let mut who: HashMap<String, (String, i64)> = HashMap::new();
    for f in launch_files {
        for v in read_lines(f) {
            if v["kind"] == "create" || v["signer"].is_string() {
                let c = v["creator"].as_str().or(v["signer"].as_str()).unwrap_or("").to_string();
                who.insert(v["mint"].as_str().unwrap_or("").to_string(), (c, v["bt"].as_i64().unwrap_or(0)));
            }
        }
    }
    let cats: HashMap<String, String> = fs::read_to_string("data/raw/categories.tsv").unwrap_or_default().lines().skip(1)
        .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_string(), b.to_string()))).collect();
    let mut outs: HashMap<String, Outcome> = HashMap::new();
    for f in row_files {
        for r in read_lines(f) {
            let m = r["mint"].as_str().unwrap_or("").to_string();
            if let Some(o) = outcome(&r, cats.get(&m).map(|s| s.as_str())) {
                outs.insert(m, o);
            }
        }
    }
    // funder (team) of each creator: historical edges + window funders; exchanges and busy funders excluded
    let exch: HashSet<String> = fs::read_to_string("dataset/wallets/exchange_list.txt").unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
    let mut funder: HashMap<String, String> = HashMap::new();
    let mut fan: HashMap<String, HashSet<String>> = HashMap::new();
    for v in read_lines("data/raw/fund_edges.jsonl").into_iter().chain(read_lines("data/fwd/funders.jsonl")) {
        if let (Some(w), Some(f)) = (v["w"].as_str(), v["funder"].as_str()) {
            if exch.contains(f) || v["kind"] == "balance" {
                continue;
            }
            funder.entry(w.to_string()).or_insert_with(|| f.to_string());
            fan.entry(f.to_string()).or_default().insert(w.to_string());
        }
    }
    let team_of = |c: &str| funder.get(c).filter(|f| fan.get(*f).map(|s| s.len() <= 50).unwrap_or(false)).cloned();
    let (mut dev, mut team): (HashMap<String, Rep>, HashMap<String, Rep>) = (HashMap::new(), HashMap::new());
    for (m, (c, bt)) in &who {
        if *bt >= cutoff {
            continue;
        }
        dev.entry(c.clone()).or_default().coins += 1;
        let t = team_of(c);
        if let Some(t) = &t {
            team.entry(t.clone()).or_default().coins += 1;
        }
        if let Some(o) = outs.get(m) {
            dev.get_mut(c).unwrap().add(o);
            if let Some(t) = &t {
                team.get_mut(t).unwrap().add(o);
            }
        }
    }
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let watch: HashMap<String, String> = fs::read_to_string("data/fwd/watch.tsv").unwrap_or_default().lines()
        .filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); Some((f.first()?.to_string(), f.get(1)?.to_string())) }).collect();
    let mut tsv = String::from("jenis\twallet\tkelas_lama\tkoin_dibuat\tkoin_ada_hasil\tnaik_2x\tturun\trugpull\tsepi\trata_entry\tputusan\n");
    let mut verdicts: HashMap<(&str, &str, String), u32> = HashMap::new();
    let mut keep: Vec<String> = Vec::new();
    for (kind, map) in [("DEV", &dev), ("TIM", &team)] {
        let mut v: Vec<(&String, &Rep)> = map.iter().filter(|(_, r)| r.known >= 1).collect();
        v.sort_by(|a, b| b.1.mean().total_cmp(&a.1.mean()));
        for (w, r) in v {
            let cls = watch.get(w).cloned().unwrap_or_else(|| "-".into());
            let vd = r.verdict();
            *verdicts.entry((kind, vd, cls.chars().next().unwrap_or('-').to_string())).or_default() += 1;
            if vd == "BERKUALITAS" {
                keep.push(format!("{kind}\t{w}"));
            }
            tsv.push_str(&format!("{kind}\t{w}\t{cls}\t{}\t{}\t{}\t{}\t{}\t{}\t{:+.1}%\t{vd}\n", r.coins, r.known, r.up, r.down, r.rug, r.quiet, 100.0 * r.mean()));
        }
    }
    fs::write(format!("{out_dir}/reputasi.tsv"), &tsv).map_err(|e| e.to_string())?;
    fs::write(format!("{out_dir}/pantau_berkualitas.tsv"), keep.join("\n") + "\n").map_err(|e| e.to_string())?;
    let mut vs: Vec<_> = verdicts.into_iter().collect();
    vs.sort();
    println!("cutoff={cutoff} devs_with_outcome={} teams_with_outcome={} keep={}", dev.values().filter(|r| r.known > 0).count(), team.values().filter(|r| r.known > 0).count(), keep.len());
    for ((k, v, c), n) in vs {
        println!("{k}\tkelas_lama={c}\t{v}\t{n}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn o(ret: f64, up: bool, rug: bool, quiet: bool) -> Outcome {
        Outcome { ret, up, down: ret <= -0.2, rug, quiet }
    }
    #[test]
    fn rules() {
        let mut r = Rep::default();
        for x in [o(0.5, true, false, false), o(0.3, true, false, false)] {
            r.add(&x);
        }
        assert_eq!(r.verdict(), "BELUM_CUKUP_DATA");
        r.add(&o(-0.1, false, false, false));
        assert_eq!(r.verdict(), "BERKUALITAS");
        r.add(&o(-0.9, false, true, false));
        r.add(&o(-0.9, false, true, false));
        assert_eq!(r.verdict(), "KELUAR_RUGPULL");
        let mut q = Rep::default();
        for _ in 0..3 {
            q.add(&o(0.0, false, false, true));
        }
        assert_eq!(q.verdict(), "KELUAR_TURUN_SEPI");
    }
}
