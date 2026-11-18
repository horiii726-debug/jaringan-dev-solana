//! Feature table for EVERY create (not only coins with trade data), label = coin later held a real market cap
//! at least $50k (min50k_fix2: at least 20 PumpSwap trades at at least $50k with at least 20 SOL and at least 5% supply in the pool).
//! Coins that never graduated cannot reach $50k (curve tops out near $29k), so their label is 0 by construction.
//! All features are known at the create tx: create-tx facts, universe history before the coin, dev / funder
//! (team) history of earlier coins whose label was settled (created at least 24 h before this coin).
use crate::graph::{load_fund, read_lines};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;

pub fn run(launch_files: &[String], fund_files: &[String], pos_file: &str, out: &str) -> Result<(), String> {
    let pos: HashSet<String> = fs::read_to_string(pos_file).map_err(|e| e.to_string())?.lines()
        .filter_map(|l| l.split('\t').next().map(String::from)).collect();
    let mut ls: Vec<serde_json::Value> = Vec::new();
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] == "create" && v["mayhem"] != true && v["quote"].as_str().unwrap_or("").starts_with("1111") {
                ls.push(v);
            }
        }
    }
    ls.sort_by_key(|v| (v["slot"].as_u64().unwrap_or(0), v["txi"].as_u64().unwrap_or(0)));
    let fm = load_fund(fund_files);
    let funder = |w: &str| fm.get(w).and_then(|f| f.funder.clone());
    let mut o = fs::File::create(out).map_err(|e| e.to_string())?;
    let names = ["label", "day", "bt", "mint", "own_buy_sol", "tx_buys", "tx_buy_sol", "tx_buyers", "user_ne_creator", "cashback", "name_len", "sym_len",
        "hour", "prev_launch_creator", "secs_since_prev", "launch_24h_creator", "heat_5m", "same_sym_24h", "has_fund", "fund_lag_s",
        "dev_prev_settled", "dev_prev_pos", "team_prev_settled", "team_prev_pos", "team_devs_seen"];
    writeln!(o, "{}", names.join("\t")).map_err(|e| e.to_string())?;
    let mut last_c: HashMap<String, (u64, i64)> = HashMap::new();
    let mut c24: HashMap<String, VecDeque<i64>> = HashMap::new();
    let mut sym24: HashMap<String, VecDeque<i64>> = HashMap::new();
    let mut heat: VecDeque<i64> = VecDeque::new();
    // settled history: queue of (settle_time, creator, team, positive)
    let mut pending: VecDeque<(i64, String, Option<String>, bool)> = VecDeque::new();
    let mut dev_h: HashMap<String, (u64, u64)> = HashMap::new();
    let mut team_h: HashMap<String, (u64, u64)> = HashMap::new();
    let mut team_devs: HashMap<String, HashSet<String>> = HashMap::new();
    let mut n = 0u64;
    for v in &ls {
        let bt = v["bt"].as_i64().unwrap_or(0);
        let mint = v["mint"].as_str().unwrap_or("");
        let c = v["creator"].as_str().unwrap_or("").to_string();
        let u = v["user"].as_str().unwrap_or("");
        let sym = v["symbol"].as_str().unwrap_or("").to_lowercase();
        while pending.front().map(|p| p.0 <= bt).unwrap_or(false) {
            let (_, pc, pt, pp) = pending.pop_front().expect("front");
            let e = dev_h.entry(pc).or_default();
            e.0 += 1;
            e.1 += pp as u64;
            if let Some(t) = pt {
                let e = team_h.entry(t).or_default();
                e.0 += 1;
                e.1 += pp as u64;
            }
        }
        let (pc, pbt) = last_c.get(&c).copied().unwrap_or((0, 0));
        let q = c24.entry(c.clone()).or_default();
        while q.front().map(|&t| t < bt - 86_400).unwrap_or(false) {
            q.pop_front();
        }
        let s = sym24.entry(sym.clone()).or_default();
        while s.front().map(|&t| t < bt - 86_400).unwrap_or(false) {
            s.pop_front();
        }
        while heat.front().map(|&t| t < bt - 300).unwrap_or(false) {
            heat.pop_front();
        }
        let f = fm.get(&c);
        let (has, lag) = match f {
            Some(f) if f.funder.is_some() && f.first_bt > 0 && f.first_bt <= bt => (1.0, (bt - f.first_bt) as f64),
            _ => (0.0, -1.0),
        };
        let team = funder(&c).filter(|_| has > 0.0);
        let (ds, dp) = dev_h.get(&c).copied().unwrap_or((0, 0));
        let (ts, tp) = team.as_ref().and_then(|t| team_h.get(t).copied()).unwrap_or((0, 0));
        let td = team.as_ref().and_then(|t| team_devs.get(t).map(|s| s.len())).unwrap_or(0);
        let label = pos.contains(mint) as u8;
        let day = crate::outcomes::day_of(bt);
        let vals = [
            label.to_string(), day, bt.to_string(), mint.to_string(),
            format!("{}", v["own_buy_sol"].as_f64().unwrap_or(0.0) / 1e9), format!("{}", v["tx_buys"].as_f64().unwrap_or(0.0)),
            format!("{}", v["tx_buy_sol"].as_f64().unwrap_or(0.0) / 1e9), format!("{}", v["tx_buyers"].as_array().map(|a| a.len()).unwrap_or(0)),
            format!("{}", (u != c) as u8), format!("{}", (v["cashback"] == true) as u8),
            format!("{}", v["name"].as_str().map(|s| s.chars().count()).unwrap_or(0)), format!("{}", sym.chars().count()),
            format!("{}", (bt % 86_400) / 3600), pc.to_string(), format!("{}", if pc > 0 { bt - pbt } else { -1 }), q.len().to_string(),
            heat.len().to_string(), s.len().to_string(), format!("{has}"), format!("{lag}"),
            ds.to_string(), dp.to_string(), ts.to_string(), tp.to_string(), td.to_string(),
        ];
        writeln!(o, "{}", vals.join("\t")).map_err(|e| e.to_string())?;
        n += 1;
        q.push_back(bt);
        s.push_back(bt);
        heat.push_back(bt);
        last_c.insert(c.clone(), (pc + 1, bt));
        if let Some(t) = &team {
            team_devs.entry(t.clone()).or_default().insert(c.clone());
        }
        pending.push_back((bt + 86_400, c, team, label == 1));
    }
    let cd = "0\tLabel\n1\tAuxiliary\n2\tAuxiliary\n3\tAuxiliary\n";
    fs::write(format!("{out}.cd"), cd).map_err(|e| e.to_string())?;
    println!("rows={n} positives={}", pos.len());
    Ok(())
}
