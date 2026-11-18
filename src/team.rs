//! Teams: team = one funder + every dev wallet whose first SOL came from that funder.
//! A team becomes QUALIFIED (as-of) when one of its devs' coins closes as a moonshot
//! (curve price >= moon_creation_mult x creation price within the horizon, or graduated).
//! Every later coin by a dev of a qualified team is a signal; its buyer-side result is recorded.
use crate::graph::{load_fund, read_lines};
use crate::rpc::cfg_u64;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};

type Item = (String, String, String, i8, i8, i8, f64);

#[derive(Default)]
struct Team {
    devs: HashSet<String>,
    coins: u64,
    with_trades: u64,
    moons: u64,
    qualified_at: Option<i64>,
    sig_n: u64,
    sig_ret: Vec<f64>,
    sig_moon: u64,
    // closed coins with trade data (as-of): count, moonshots, rugs (creator sold <= 60 s), quiet (< 5 trades)
    c_n: u64,
    c_moon: u64,
    c_rug: u64,
    c_quiet: u64,
    // (dev, mint, symbol, moon -1/0/1, rug -1/0/1, quiet -1/0/1, our net return or NaN)
    items: Vec<(String, String, String, i8, i8, i8, f64)>,
}

pub struct Rule {
    pub min_moons: u64,
    pub min_closed: u64,
    pub max_rug: f64,
    pub max_quiet: f64,
}

impl Team {
    /// P4 state: UNKNOWN (no closed coin with data), TOO_FEW, GOOD (quality rule), BAD.
    fn state(&self, r: &Rule) -> &'static str {
        if self.c_n == 0 {
            "UNKNOWN"
        } else if self.c_n < r.min_closed {
            "TOO_FEW"
        } else if self.quality(r) {
            "GOOD"
        } else {
            "BAD"
        }
    }
    fn quality(&self, r: &Rule) -> bool {
        self.c_moon >= r.min_moons
            && self.c_n >= r.min_closed
            && (self.c_rug as f64) <= r.max_rug * self.c_n as f64
            && (self.c_quiet as f64) <= r.max_quiet * self.c_n as f64
    }
}

pub fn run(launch_files: &[String], trade_files: &[String], fund_files: &[String], outcomes_csv: &str, delay: &str, tip_frac: f64) -> Result<(), String> {
    let mult = cfg_u64("config/labels.toml", "moon_creation_mult").ok_or("labels.toml moon_creation_mult missing")? as f64;
    let horizon = cfg_u64("config/labels.toml", "horizon_s").ok_or("horizon_s")? as i64;
    // moonshot per coin from curve trades
    let mut moon: HashMap<String, bool> = HashMap::new();
    let mut peakm: HashMap<String, (f64, bool)> = HashMap::new(); // peak multiple of creation price, graduated
    for p in trade_files {
        for v in read_lines(p) {
            let Some(t) = v["t"].as_array() else { continue };
            let peak = t.iter().filter_map(|x| Some(x[8].as_f64()? / x[9].as_f64()?)).fold(0.0f64, f64::max);
            let m = peak / (30e9 / 1.073e15) >= mult || !v["graduated_slot"].is_null();
            moon.insert(v["mint"].as_str().unwrap_or("").into(), m);
            peakm.insert(v["mint"].as_str().unwrap_or("").into(), (peak / (30e9 / 1.073e15), !v["graduated_slot"].is_null()));
        }
    }
    let rule = Rule {
        min_moons: cfg_u64("config/score.toml", "team_min_moons").unwrap_or(2),
        min_closed: cfg_u64("config/score.toml", "team_min_closed").unwrap_or(3),
        max_rug: cfg_u64("config/score.toml", "team_max_rug_pct").unwrap_or(50) as f64 / 100.0,
        max_quiet: cfg_u64("config/score.toml", "team_max_quiet_pct").unwrap_or(50) as f64 / 100.0,
    };
    let busy_tph = cfg_u64("config/graph.toml", "busy_tx_per_hour").unwrap_or(150) as f64;
    // busy funders (services/exchanges/bot hubs) from data/p3/activity.jsonl
    let mut busy: HashSet<String> = HashSet::new();
    for v in read_lines("data/p3/activity.jsonl") {
        let (n, span) = (v["n"].as_f64().unwrap_or(0.0), v["span_s"].as_f64().unwrap_or(-1.0));
        if n >= 1000.0 && span >= 0.0 && n / (span.max(1.0) / 3600.0) >= busy_tph {
            busy.insert(v["w"].as_str().unwrap_or("").into());
        }
    }
    // wallets profiled as exchange/service (behaviour) are infrastructure too
    for l in fs::read_to_string("data/p3/exchange_list.txt").unwrap_or_default().lines() {
        busy.insert(l.trim().to_string());
    }
    // buyer-side result per coin at the chosen delay, plus rug / quiet flags
    let mut ret: HashMap<String, f64> = HashMap::new();
    let mut opeak: HashMap<String, f64> = HashMap::new(); // peak sellable value / our cost
    let mut flags: HashMap<String, (bool, bool)> = HashMap::new();
    if let Ok(f) = fs::File::open(outcomes_csv) {
        for l in BufReader::new(f).lines().map_while(Result::ok).skip(1) {
            let c: Vec<&str> = l.split(',').collect();
            if c.len() >= 16 && c[8] == delay && c[9] == "1" {
                ret.insert(c[0].into(), c[11].parse::<f64>().unwrap_or(0.0) - tip_frac);
                opeak.insert(c[0].into(), c[13].parse::<f64>().unwrap_or(0.0));
                let rug = c[10] == "CREATOR_SELL" && c[15].parse::<i64>().unwrap_or(9999) <= 60;
                let quiet = c[7].parse::<u64>().unwrap_or(0) < 5;
                flags.insert(c[0].into(), (rug, quiet));
            }
        }
    }
    let fm = load_fund(fund_files);
    let mut teams: HashMap<String, Team> = HashMap::new();
    for (w, f) in &fm {
        if let Some(fu) = &f.funder {
            if !f.deep && !busy.contains(fu) {
                teams.entry(fu.clone()).or_default().devs.insert(w.clone());
            }
        }
    }
    // coins in time order (non-Mayhem, SOL quote)
    let mut coins: Vec<(i64, String, String)> = Vec::new();
    let mut sym: HashMap<String, String> = HashMap::new();
    for p in launch_files {
        for v in read_lines(p) {
            let q = v["quote"].as_str().unwrap_or("");
            if v["kind"] == "create" && v["mayhem"] != true && q.starts_with("1111") {
                sym.insert(v["mint"].as_str().unwrap_or("").into(), v["symbol"].as_str().unwrap_or("").chars().take(16).collect());
                coins.push((v["bt"].as_i64().unwrap_or(0), v["mint"].as_str().unwrap_or("").into(), v["creator"].as_str().unwrap_or("").into()));
            }
        }
    }
    coins.sort();
    let mut closes: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, c) in coins.iter().enumerate() {
        closes.entry(c.0 + horizon).or_default().push(i);
    }
    let team_of = |c: &str| fm.get(c).filter(|f| !f.deep).and_then(|f| f.funder.clone()).filter(|fu| !busy.contains(fu));
    let mut ci = closes.iter().peekable();
    let (mut all_n, mut all_ret, mut all_moon) = (0u64, Vec::new(), 0u64);
    for (bt, mint, creator) in &coins {
        while let Some((&t, _)) = ci.peek() {
            if t > *bt {
                break;
            }
            let (_, idx) = ci.next().expect("peeked");
            for &i in idx {
                let (cbt, m, c) = &coins[i];
                let (Some(&mo), Some(&(rug, quiet))) = (moon.get(m), flags.get(m)) else { continue };
                if let Some(tf) = team_of(c) {
                    let t = teams.entry(tf).or_default();
                    t.c_n += 1;
                    t.c_moon += mo as u64;
                    t.c_rug += rug as u64;
                    t.c_quiet += quiet as u64;
                    if t.quality(&rule) {
                        t.qualified_at.get_or_insert(*cbt + horizon);
                    }
                }
            }
        }
        if let Some(r) = ret.get(mint) {
            all_n += 1;
            all_ret.push(*r);
            all_moon += moon.get(mint).copied().unwrap_or(false) as u64;
        }
        let Some(tf) = team_of(creator) else { continue };
        let t = teams.entry(tf).or_default();
        t.coins += 1;
        let fl = flags.get(mint).copied();
        let tri = |b: Option<bool>| b.map(|x| x as i8).unwrap_or(-1);
        t.items.push((creator.clone(), mint.clone(), sym.get(mint).cloned().unwrap_or_default(), tri(moon.get(mint).copied()),
            tri(fl.map(|f| f.0)), tri(fl.map(|f| f.1)), ret.get(mint).copied().unwrap_or(f64::NAN)));
        if moon.contains_key(mint) {
            t.with_trades += 1;
            t.moons += moon[mint] as u64;
        }
        // signal only while the team currently passes the quality rule (it can lose it after rugs / quiet coins)
        if t.qualified_at.map(|q| q <= *bt).unwrap_or(false) && t.quality(&rule) {
            t.sig_n += 1;
            if let Some(r) = ret.get(mint) {
                t.sig_ret.push(*r);
                t.sig_moon += moon.get(mint).copied().unwrap_or(false) as u64;
            }
        }
    }
    let stat = |v: &[f64]| -> String {
        if v.is_empty() {
            return "n=0".into();
        }
        let w = v.iter().filter(|&&x| x > 0.0).count();
        format!("n={} menang={:.0}% rata={:+.1}%", v.len(), 100.0 * w as f64 / v.len() as f64, 100.0 * v.iter().sum::<f64>() / v.len() as f64)
    };
    let mut list: Vec<(&String, &Team)> = teams.iter().filter(|(_, t)| t.moons > 0).collect();
    list.sort_by(|a, b| b.1.moons.cmp(&a.1.moons).then(b.1.devs.len().cmp(&a.1.devs.len())));
    let sig: Vec<f64> = teams.values().flat_map(|t| t.sig_ret.clone()).collect();
    let sig_moon: u64 = teams.values().map(|t| t.sig_moon).sum();
    let mut md = format!("# Teams (funder + funded devs), delay {delay}, tip {tip_frac}\n\nMoonshot = curve price >= {mult}x creation price within {horizon}s, or graduated. Team qualified from the close of its first moonshot (as-of).\n\n");
    md.push_str(&format!("- busy funders excluded (>= {busy_tph} tx/h): {} ; quality rule: >= {} moonshots, >= {} closed coins, rug <= {:.0}%, quiet <= {:.0}% ; teams passing now: {}
", busy.len(), rule.min_moons, rule.min_closed, 100.0 * rule.max_rug, 100.0 * rule.max_quiet, teams.values().filter(|t| t.quality(&rule)).count()));
    md.push_str(&format!("- teams with >=1 dev: {} ; with >=2 devs: {} ; teams with a moonshot: {}\n", teams.len(), teams.values().filter(|t| t.devs.len() >= 2).count(), list.len()));
    md.push_str(&format!("- BASE all coins with result: {} ; moonshot rate {:.2}%\n", stat(&all_ret), 100.0 * all_moon as f64 / all_n.max(1) as f64));
    md.push_str(&format!("- SIGNALS (coins of qualified teams after qualification): {} ; moonshot among signals with trades: {}\n\n", stat(&sig), sig_moon));
    md.push_str("| tim | funder | dev | koin | selesai+trade | moonshot | ragpul<=60s | sepi | BERKUALITAS | sinyal | hasil sinyal |\n|---|---|---|---|---|---|---|---|---|---|---|\n");
    for (i, (f, t)) in list.iter().enumerate().take(40) {
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n", i + 1, f, t.devs.len(), t.coins, t.c_n, t.c_moon, t.c_rug, t.c_quiet,
            if t.quality(&rule) { "YA" } else { "-" }, t.sig_n, stat(&t.sig_ret)));
    }
    fs::create_dir_all("reports/p4").map_err(|e| e.to_string())?;
    let p = format!("reports/p4/teams_d{delay}.md");
    fs::write(&p, &md).map_err(|e| e.to_string())?;
    let mut csv = fs::File::create(format!("reports/p4/teams_d{delay}.csv")).map_err(|e| e.to_string())?;
    writeln!(csv, "funder,devs,coins,with_trades,moons,qualified_at,signals").map_err(|e| e.to_string())?;
    for (f, t) in &list {
        writeln!(csv, "{f},{},{},{},{},{},{}", t.devs.len(), t.coins, t.with_trades, t.moons, t.qualified_at.unwrap_or(0), t.sig_n).map_err(|e| e.to_string())?;
    }
    // Web export: teams with >= 2 devs or a moonshot, BAD teams removed, bad devs removed
    // (dev whose known coins are all rug or quiet and none was a moonshot).
    let order = |s: &str| match s { "GOOD" => 0, "TOO_FEW" => 1, "UNKNOWN" => 2, _ => 3 };
    let mut web: Vec<(&String, &Team)> = teams.iter().filter(|(_, t)| (t.devs.len() >= 2 || t.moons > 0) && t.state(&rule) != "BAD").collect();
    web.sort_by(|a, b| order(a.1.state(&rule)).cmp(&order(b.1.state(&rule))).then(b.1.moons.cmp(&a.1.moons)).then(b.1.devs.len().cmp(&a.1.devs.len())));
    let bad_teams = teams.values().filter(|t| t.state(&rule) == "BAD").count();
    let mut bad_devs = 0usize;
    let mut out_teams = Vec::new();
    for (f, t) in web.iter().take(120) {
        let mut by_dev: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
        for it in &t.items {
            by_dev.entry(it.0.as_str()).or_default().push(it);
        }
        let mut devs = Vec::new();
        for (d, its) in by_dev {
            let known: Vec<_> = its.iter().filter(|x| x.4 >= 0).collect();
            let bad = !known.is_empty() && known.iter().all(|x| x.4 == 1 || x.5 == 1) && !its.iter().any(|x| x.3 == 1);
            if bad {
                bad_devs += 1;
                continue;
            }
            let coins: Vec<serde_json::Value> = its.iter().map(|x| serde_json::json!({"mint": x.1, "sym": x.2, "moon": x.3, "rug": x.4, "quiet": x.5,
                "ret": if x.6.is_nan() { serde_json::Value::Null } else { serde_json::json!((x.6 * 1000.0).round() / 1000.0) }})).collect();
            devs.push(serde_json::json!({"dev": d, "coins": coins}));
        }
        if devs.is_empty() {
            continue;
        }
        out_teams.push(serde_json::json!({"funder": f, "state": t.state(&rule), "devs_total": t.devs.len(), "coins": t.coins, "closed": t.c_n,
            "moons": t.c_moon, "rugs": t.c_rug, "quiet": t.c_quiet, "signals": t.sig_n, "sig_ret": if t.sig_ret.is_empty() { serde_json::Value::Null } else {
            serde_json::json!(t.sig_ret.iter().sum::<f64>() / t.sig_ret.len() as f64) }, "devs": devs}));
    }
    // rising coins: moonshot or >= 2x from our entry, with dev, funder and team status
    let mut rising: Vec<(f64, serde_json::Value)> = Vec::new();
    for (bt, m, c) in &coins {
        let (pc, gr) = peakm.get(m).copied().unwrap_or((0.0, false));
        let po = opeak.get(m).copied().unwrap_or(0.0);
        if !(moon.get(m).copied().unwrap_or(false) || po >= 2.0) {
            continue;
        }
        let fu = fm.get(c.as_str()).and_then(|f| f.funder.clone());
        let st = fu.as_ref().and_then(|f| teams.get(f)).map(|t| t.state(&rule)).unwrap_or(if fu.as_ref().map(|f| busy.contains(f)).unwrap_or(false) { "LAYANAN" } else { "-" });
        rising.push((pc, serde_json::json!({"mint": m, "sym": sym.get(m).cloned().unwrap_or_default(), "bt": bt, "peak_creation": (pc * 100.0).round() / 100.0,
            "graduated": gr, "peak_ours": (po * 100.0).round() / 100.0, "ret": ret.get(m).map(|r| (r * 1000.0).round() / 1000.0), "creator": c, "funder": fu, "team": st})));
    }
    rising.sort_by(|a, b| b.0.total_cmp(&a.0));
    let rising: Vec<serde_json::Value> = rising.into_iter().take(400).map(|x| x.1).collect();
    let web_json = serde_json::json!({"rising": rising, "updated": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        "rule": {"min_moons": rule.min_moons, "min_closed": rule.min_closed, "max_rug": rule.max_rug, "max_quiet": rule.max_quiet},
        "removed": {"busy_funders": busy.len(), "bad_teams": bad_teams, "bad_devs": bad_devs},
        "base": {"coins": all_n, "moon_rate": all_moon as f64 / all_n.max(1) as f64, "mean_ret": all_ret.iter().sum::<f64>() / all_ret.len().max(1) as f64},
        "states": {"GOOD": teams.values().filter(|t| t.state(&rule) == "GOOD").count(), "TOO_FEW": teams.values().filter(|t| t.state(&rule) == "TOO_FEW").count(),
            "UNKNOWN": teams.values().filter(|t| t.state(&rule) == "UNKNOWN").count(), "BAD": bad_teams},
        "teams": out_teams});
    fs::create_dir_all("reports/web").map_err(|e| e.to_string())?;
    fs::write("reports/web/teams.json", web_json.to_string()).map_err(|e| e.to_string())?;
    println!("{}", md.lines().take(6).collect::<Vec<_>>().join("\n"));
    println!("-> {p} + reports/web/teams.json");
    Ok(())
}

/// Minimal local web server (std only) for reports/web: http://127.0.0.1:<port>/ (reach it through an SSH tunnel).
pub fn serve(port: &str) -> Result<(), String> {
    use std::io::Read;
    let l = std::net::TcpListener::bind(format!("127.0.0.1:{port}")).map_err(|e| e.to_string())?;
    println!("serving reports/web on 127.0.0.1:{port}");
    for s in l.incoming().flatten() {
        let mut s = s;
        let mut buf = [0u8; 2048];
        let n = s.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req.split_whitespace().nth(1).unwrap_or("/");
        let p0 = path.split('?').next().unwrap_or("/");
        let safe = p0.len() > 1 && p0[1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') && !p0.contains("..");
        let (file, ctype) = if safe && p0.ends_with(".json") {
            (format!("reports/web{p0}"), "application/json")
        } else {
            ("reports/web/index.html".to_string(), "text/html; charset=utf-8")
        };
        let body = fs::read(&file).unwrap_or_else(|_| b"not ready".to_vec());
        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len()).as_bytes());
        let _ = s.write_all(&body);
    }
    Ok(())
}
