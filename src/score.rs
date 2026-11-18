//! Dev and team scoreboard (descriptive, all data so far): how often each dev / team made a RISING coin
//! (moonshot >= moon_creation_mult x creation price or graduated, or >= 2x from our entry). Quiet coins are not
//! punished; a dev with >= 3 traded coins and no rising coin is "trash" and removed. Each dev's funding is traced:
//! team (funder funds >= 2 devs), exchange/service (busy funder = self-funded from an exchange), single link, unknown.
use crate::graph::{load_fund, read_lines};
use crate::rpc::cfg_u64;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};

#[derive(Default)]
struct D {
    coins: u64,
    traded: u64,
    rising: u64,
    moons: u64,
    best_peak: f64,
    best_sym: String,
}

pub fn run(launch_files: &[String], trade_files: &[String], fund_files: &[String], outcomes_csv: &str) -> Result<(), String> {
    let mult = cfg_u64("config/labels.toml", "moon_creation_mult").ok_or("moon_creation_mult")? as f64;
    let busy_tph = cfg_u64("config/graph.toml", "busy_tx_per_hour").unwrap_or(150) as f64;
    let mut peak: HashMap<String, (f64, bool)> = HashMap::new();
    for p in trade_files {
        for v in read_lines(p) {
            let Some(t) = v["t"].as_array() else { continue };
            let pk = t.iter().filter_map(|x| Some(x[8].as_f64()? / x[9].as_f64()?)).fold(0.0f64, f64::max) / (30e9 / 1.073e15);
            peak.insert(v["mint"].as_str().unwrap_or("").into(), (pk, !v["graduated_slot"].is_null()));
        }
    }
    let mut ours: HashMap<String, f64> = HashMap::new();
    if let Ok(f) = fs::File::open(outcomes_csv) {
        for l in BufReader::new(f).lines().map_while(Result::ok).skip(1) {
            let c: Vec<&str> = l.split(',').collect();
            if c.len() >= 16 && c[8] == "j1" && c[9] == "1" {
                ours.insert(c[0].into(), c[13].parse().unwrap_or(0.0));
            }
        }
    }
    let mut busy: HashSet<String> = HashSet::new();
    for v in read_lines("data/p3/activity.jsonl") {
        let (n, span) = (v["n"].as_f64().unwrap_or(0.0), v["span_s"].as_f64().unwrap_or(-1.0));
        if n >= 1000.0 && span >= 0.0 && n / (span.max(1.0) / 3600.0) >= busy_tph {
            busy.insert(v["w"].as_str().unwrap_or("").into());
        }
    }
    for l in fs::read_to_string("data/p3/exchange_list.txt").unwrap_or_default().lines() {
        busy.insert(l.trim().to_string());
    }
    let fm = load_fund(fund_files);
    let mut funded: HashMap<&str, u64> = HashMap::new();
    for f in fm.values() {
        if let Some(fu) = &f.funder {
            *funded.entry(fu.as_str()).or_default() += 1;
        }
    }
    let mut devs: HashMap<String, D> = HashMap::new();
    let mut base = (0u64, 0u64);
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] != "create" || v["mayhem"] == true || !v["quote"].as_str().unwrap_or("").starts_with("1111") {
                continue;
            }
            let m = v["mint"].as_str().unwrap_or("");
            let d = devs.entry(v["creator"].as_str().unwrap_or("").into()).or_default();
            d.coins += 1;
            if let Some(&(pk, gr)) = peak.get(m) {
                d.traded += 1;
                let po = ours.get(m).copied().unwrap_or(0.0);
                let moon = pk >= mult || gr;
                let rising = moon || po >= 2.0;
                d.rising += rising as u64;
                d.moons += moon as u64;
                base.0 += 1;
                base.1 += rising as u64;
                if pk > d.best_peak {
                    d.best_peak = pk;
                    d.best_sym = v["symbol"].as_str().unwrap_or("").chars().take(16).collect();
                }
            }
        }
    }
    let base_rate = base.1 as f64 / base.0.max(1) as f64;
    let class = |w: &str| -> (String, Option<String>) {
        let Some(f) = fm.get(w) else { return ("BELUM_DICEK".into(), None) };
        let Some(fu) = f.funder.clone() else { return ("TIDAK_DIKETAHUI".into(), None) };
        if busy.contains(&fu) {
            return ("EXCHANGE/LAYANAN".into(), Some(fu));
        }
        if funded.get(fu.as_str()).copied().unwrap_or(0) >= 2 {
            return ("TIM".into(), Some(fu));
        }
        if devs.contains_key(&fu) {
            return ("DIDANAI_DEV_LAIN".into(), Some(fu));
        }
        if let Some(ff) = fm.get(&fu).and_then(|x| x.funder.clone()) {
            if !busy.contains(&ff) && funded.get(ff.as_str()).copied().unwrap_or(0) >= 2 {
                return ("TIM_2_TINGKAT".into(), Some(ff));
            }
        }
        ("FUNDER_TUNGGAL".into(), Some(fu))
    };
    // dev rows (trash removed), team aggregation
    let mut out_devs = Vec::new();
    let mut teams: HashMap<String, (u64, u64, u64, u64, Vec<String>)> = HashMap::new(); // devs, traded, rising, moons, dev list
    let (mut trash, mut kept) = (0u64, 0u64);
    let mut cls_count: HashMap<String, u64> = HashMap::new();
    for (w, d) in &devs {
        if d.traded >= 3 && d.rising == 0 {
            trash += 1;
            continue;
        }
        if d.traded == 0 {
            continue;
        }
        kept += 1;
        let (c, fu) = class(w);
        if d.rising > 0 {
            *cls_count.entry(c.clone()).or_default() += 1;
        }
        if c == "TIM" || c == "TIM_2_TINGKAT" {
            if let Some(f) = &fu {
                let t = teams.entry(f.clone()).or_default();
                t.0 += 1;
                t.1 += d.traded;
                t.2 += d.rising;
                t.3 += d.moons;
                t.4.push(w.clone());
            }
        }
        out_devs.push(json!({"dev": w, "coins": d.coins, "traded": d.traded, "rising": d.rising, "moons": d.moons,
            "rate": d.rising as f64 / d.traded as f64, "best_peak": (d.best_peak * 10.0).round() / 10.0, "best_sym": d.best_sym, "class": c, "funder": fu}));
    }
    out_devs.sort_by(|a, b| b["rising"].as_u64().cmp(&a["rising"].as_u64()).then(b["rate"].as_f64().unwrap_or(0.0).total_cmp(&a["rate"].as_f64().unwrap_or(0.0))));
    let mut out_teams: Vec<Value> = teams.iter().filter(|(_, t)| t.2 > 0).map(|(f, t)| json!({"funder": f, "devs": t.0, "traded": t.1, "rising": t.2, "moons": t.3,
        "rate": t.2 as f64 / t.1.max(1) as f64, "dev_list": t.4, "good": t.2 >= 2 && (t.2 as f64 / t.1.max(1) as f64) >= 2.0 * base_rate})).collect();
    out_teams.sort_by(|a, b| b["rising"].as_u64().cmp(&a["rising"].as_u64()));
    let rising_devs = out_devs.iter().filter(|d| d["rising"].as_u64().unwrap_or(0) > 0).count();
    let good_devs = out_devs.iter().filter(|d| d["rising"].as_u64().unwrap_or(0) >= 2).count();
    let s = json!({"base": {"traded": base.0, "rising": base.1, "rate": base_rate}, "trash_devs_removed": trash, "devs_kept": kept,
        "rising_devs": rising_devs, "devs_ge2_rising": good_devs, "rising_dev_funding": cls_count,
        "teams_with_rising": out_teams.len(), "good_teams": out_teams.iter().filter(|t| t["good"] == true).count(),
        "devs": out_devs.into_iter().filter(|d| d["rising"].as_u64().unwrap_or(0) > 0 || d["traded"].as_u64().unwrap_or(0) >= 2).take(3000).collect::<Vec<_>>(),
        "teams": out_teams});
    fs::write("reports/web/score.json", s.to_string()).map_err(|e| e.to_string())?;
    println!("base traded={} rising={} ({:.1}%) | trash devs removed={trash} kept={kept} | rising devs={rising_devs}, >=2 rising={good_devs} | funding of rising devs={:?} | teams with rising={} good={}",
        base.0, base.1, 100.0 * base_rate, s["rising_dev_funding"], s["teams_with_rising"], s["good_teams"]);
    Ok(())
}
