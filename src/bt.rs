//! P4+P5: as-of network score (time sweep, union-find with merged stats) and per-day backtest.
//! Events at equal time: edge, then outcome close, then score. A coin is scored only from outcomes whose
//! window closed before its creation, over the network as linked at that moment (no future edges).
use crate::graph::{load_fund, read_lines};
use crate::rpc::cfg_u64;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};

fn cfg_f(path: &str, name: &str) -> Option<f64> {
    let s = fs::read_to_string(path).ok()?;
    s.lines().find_map(|l| {
        let l = l.split('#').next()?.trim();
        let (k, v) = l.split_once('=')?;
        if k.trim() == name { v.trim().parse().ok() } else { None }
    })
}
fn cfg_s(path: &str, name: &str) -> Option<String> {
    let s = fs::read_to_string(path).ok()?;
    s.lines().find_map(|l| {
        let l = l.split('#').next()?.trim();
        let (k, v) = l.split_once('=')?;
        if k.trim() == name { Some(v.trim().trim_matches('"').to_string()) } else { None }
    })
}

#[derive(Default, Clone, Copy)]
struct St {
    n: u64,
    k: u64,
    moon: u64,
    nodes: u64,
}

struct Uf {
    p: Vec<usize>,
    s: Vec<St>,
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
        if a == b {
            return;
        }
        let (r, o) = (a.min(b), a.max(b));
        self.p[o] = r;
        let so = self.s[o];
        let sr = &mut self.s[r];
        sr.n += so.n;
        sr.k += so.k;
        sr.moon += so.moon;
        sr.nodes += so.nodes;
    }
}

type Groups<'a> = BTreeMap<&'a str, Vec<(f64, f64)>>;

struct Row {
    mint: String,
    creator: String,
    day: String,
    bt: i64,
    mayhem: bool,
    ret: f64,
    ret2: f64,
    peak: f64,
}

pub fn wilson(k: u64, n: u64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 0.0);
    }
    let (z, nf, p) = (1.96f64, n as f64, k as f64 / n as f64);
    let den = 1.0 + z * z / nf;
    let c = (p + z * z / (2.0 * nf)) / den;
    let h = z * ((p * (1.0 - p) + z * z / (4.0 * nf)) / nf).sqrt() / den;
    (c - h, c + h)
}

fn stats(v: &[(f64, f64)]) -> String {
    let n = v.len();
    if n == 0 {
        return "| 0 | - | - | - | - | - | - | - | - |".into();
    }
    let w: Vec<f64> = v.iter().map(|x| x.0).filter(|&r| r > 0.0).collect();
    let l: Vec<f64> = v.iter().map(|x| x.0).filter(|&r| r <= 0.0).collect();
    let aw = if w.is_empty() { 0.0 } else { w.iter().sum::<f64>() / w.len() as f64 };
    let al = if l.is_empty() { 0.0 } else { -l.iter().sum::<f64>() / l.len() as f64 };
    let pf = if al == 0.0 { f64::INFINITY } else { w.iter().sum::<f64>() / -l.iter().sum::<f64>() };
    let sum: f64 = v.iter().map(|x| x.0).sum();
    let mut s: Vec<f64> = v.iter().map(|x| x.0).collect();
    s.sort_by(|a, b| b.total_cmp(a));
    let no5: f64 = s.iter().skip(5).sum();
    let sum2: f64 = v.iter().map(|x| x.1).sum();
    let be = if aw + al > 0.0 { al / (aw + al) } else { 0.0 };
    let (lo, hi) = wilson(w.len() as u64, n as u64);
    format!("| {n} | {:.1}% [{:.1},{:.1}] | {:.3} | {:.3} | {:.1}% | {:.2} | {:+.3} | {:+.3} | {:+.3} |",
        100.0 * w.len() as f64 / n as f64, 100.0 * lo, 100.0 * hi, aw, al, 100.0 * be, pf, sum / n as f64, no5 / n as f64, sum2 / n as f64)
}

pub fn run(outcomes_csv: &str, delay: &str, launch_files: &[String], fund_files: &[String], tag: &str) -> Result<(), String> {
    let sc = "config/score.toml";
    let a = cfg_f(sc, "shrink_a").ok_or("score.toml shrink_a")?;
    let min_n = cfg_u64(sc, "min_n").ok_or("score.toml min_n")?;
    let lift = cfg_f(sc, "good_lift").ok_or("score.toml good_lift")?;
    let moon_mult = cfg_f("config/labels.toml", "moon_mult").ok_or("moon_mult")?;
    let horizon = cfg_u64("config/labels.toml", "horizon_s").ok_or("horizon_s")? as i64;
    let train: HashSet<String> = cfg_s(sc, "train_days").ok_or("score.toml train_days")?.split(',').map(|s| s.trim().to_string()).collect();
    let infra_sigs = cfg_u64("config/graph.toml", "infra_min_sigs").ok_or("infra_min_sigs")?;
    let infra_fo = cfg_u64("config/graph.toml", "infra_min_fanout").ok_or("infra_min_fanout")?;
    let with_mayhem = cfg_u64(sc, "include_mayhem").unwrap_or(0) == 1;
    // outcomes for the chosen delay
    let mut rows: Vec<Row> = Vec::new();
    let mut not_filled = 0u64;
    let f = fs::File::open(outcomes_csv).map_err(|e| e.to_string())?;
    for l in BufReader::new(f).lines().map_while(Result::ok).skip(1) {
        let c: Vec<&str> = l.split(',').collect();
        if c.len() < 16 || c[8] != delay {
            continue;
        }
        if c[9] != "1" {
            not_filled += 1;
            continue;
        }
        let r = Row { mint: c[0].into(), creator: c[1].into(), day: c[3].into(), bt: c[5].parse().unwrap_or(0), mayhem: c[6] == "1",
            ret: c[11].parse().unwrap_or(0.0), ret2: c[12].parse().unwrap_or(0.0), peak: c[13].parse().unwrap_or(0.0) };
        if r.mayhem && !with_mayhem {
            continue;
        }
        rows.push(r);
    }
    // base rate p0 from training days only
    let tr: Vec<&Row> = rows.iter().filter(|r| train.contains(&r.day)).collect();
    if tr.is_empty() {
        return Err("no training rows".into());
    }
    let p0 = tr.iter().filter(|r| r.ret > 0.0).count() as f64 / tr.len() as f64;
    let good_cut = p0 * lift;
    // graph edges with time
    let fm = load_fund(fund_files);
    let mut fanout: HashMap<&str, u64> = HashMap::new();
    for fr in fm.values() {
        if let Some(fu) = &fr.funder {
            *fanout.entry(fu.as_str()).or_default() += 1;
        }
    }
    let infra = |w: &str| fm.get(w).map(|f| f.deep || f.n_sigs >= infra_sigs).unwrap_or(false) || fanout.get(w).copied().unwrap_or(0) >= infra_fo;
    let mut id: HashMap<String, usize> = HashMap::new();
    let get = |w: &str, id: &mut HashMap<String, usize>| -> usize {
        let n = id.len();
        *id.entry(w.to_string()).or_insert(n)
    };
    // events: (time, order, kind, a, b, row index)
    let mut ev: Vec<(i64, u8, usize, usize, usize)> = Vec::new();
    for (w, fr) in &fm {
        if let Some(fu) = &fr.funder {
            if fu != w && !infra(w) && !infra(fu) && fr.first_bt > 0 {
                let (x, y) = (get(w, &mut id), get(fu, &mut id));
                ev.push((fr.first_bt, 0, x, y, 0));
            }
        }
    }
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] != "create" {
                continue;
            }
            let (c, u) = (v["creator"].as_str().unwrap_or(""), v["user"].as_str().unwrap_or(""));
            if c != u && !infra(u) {
                let (x, y) = (get(c, &mut id), get(u, &mut id));
                ev.push((v["bt"].as_i64().unwrap_or(0), 0, x, y, 0));
            }
        }
    }
    for (i, r) in rows.iter().enumerate() {
        let x = get(&r.creator, &mut id);
        ev.push((r.bt + horizon, 1, x, 0, i));
        ev.push((r.bt, 2, x, 0, i));
    }
    ev.sort_by_key(|e| (e.0, e.1));
    let n = id.len();
    let mut uf = Uf { p: (0..n).collect(), s: vec![St { nodes: 1, ..Default::default() }; n] };
    let mut state: Vec<(&'static str, u64, f64, u64)> = vec![("", 0, 0.0, 0); rows.len()];
    for (_, kind, x, y, i) in ev {
        match kind {
            0 => uf.join(x, y),
            1 => {
                let r = uf.find(x);
                uf.s[r].n += 1;
                uf.s[r].k += (rows[i].ret > 0.0) as u64;
                uf.s[r].moon += (rows[i].peak >= moon_mult) as u64;
            }
            _ => {
                let r = uf.find(x);
                let s = uf.s[r];
                let ph = (s.k as f64 + a * p0) / (s.n as f64 + a);
                let st = if s.n == 0 { "UNKNOWN" } else if s.n < min_n { "TOO_FEW" } else if ph >= good_cut { "GOOD" } else { "BAD" };
                state[i] = (st, s.n, ph, s.nodes);
            }
        }
    }
    // output scored rows
    fs::create_dir_all("data/p5").map_err(|e| e.to_string())?;
    let mut o = fs::File::create(format!("data/p5/scored_{tag}_d{delay}.csv")).map_err(|e| e.to_string())?;
    writeln!(o, "mint,creator,day,state,hist_n,p_hat,net_nodes,ret,ret2,peak").map_err(|e| e.to_string())?;
    for (r, s) in rows.iter().zip(&state) {
        writeln!(o, "{},{},{},{},{},{:.4},{},{:.5},{:.5},{:.4}", r.mint, r.creator, r.day, s.0, s.1, s.2, s.3, r.ret, r.ret2, r.peak).map_err(|e| e.to_string())?;
    }
    // per-day backtest tables
    let mut days: BTreeMap<String, Groups> = BTreeMap::new();
    for (r, s) in rows.iter().zip(&state) {
        let g = days.entry(r.day.clone()).or_default();
        g.entry("ALL").or_default().push((r.ret, r.ret2));
        g.entry(s.0).or_default().push((r.ret, r.ret2));
        if s.0 == "GOOD" && s.3 >= 2 {
            g.entry("GOOD_NET").or_default().push((r.ret, r.ret2));
        }
    }
    let mut md = format!("# Backtest {tag}, entry delay {delay} slots\n\nLabels v1 (config/labels.toml). Score: shrink a={a}, min_n={min_n}, good = p_hat >= {lift} x p0, p0={:.4} from train days {:?}. Mayhem included: {with_mayhem}. Not filled: {not_filled}.\n\nColumns: n | win% [Wilson 95%] | avg win | avg loss | breakeven win% | profit factor | mean ret | mean ret without top 5 | mean ret, costs doubled\n\n", p0, train);
    let mut all_test: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
    for (d, g) in &days {
        md.push_str(&format!("## {d}{}\n\n| group | n | win% | avgW | avgL | BE win% | PF | mean | mean no top5 | mean x2 cost |\n|---|---|---|---|---|---|---|---|---|---|\n", if train.contains(d) { " (train)" } else { " (test)" }));
        for k in ["ALL", "UNKNOWN", "TOO_FEW", "BAD", "GOOD", "GOOD_NET"] {
            let v = g.get(k).cloned().unwrap_or_default();
            md.push_str(&format!("| {k} {}\n", stats(&v)));
            if !train.contains(d) {
                all_test.entry(k).or_default().extend(v);
            }
        }
        md.push('\n');
    }
    md.push_str("## All test days\n\n| group | n | win% | avgW | avgL | BE win% | PF | mean | mean no top5 | mean x2 cost |\n|---|---|---|---|---|---|---|---|---|---|\n");
    for (k, v) in &all_test {
        md.push_str(&format!("| {k} {}\n", stats(v)));
    }
    // quarter Kelly from training GOOD trades
    let tg: Vec<f64> = rows.iter().zip(&state).filter(|(r, s)| train.contains(&r.day) && s.0 == "GOOD").map(|(r, _)| r.ret).collect();
    let w = tg.iter().filter(|&&x| x > 0.0).count() as f64 / tg.len().max(1) as f64;
    let aw = tg.iter().filter(|&&x| x > 0.0).sum::<f64>() / tg.iter().filter(|&&x| x > 0.0).count().max(1) as f64;
    let al = -tg.iter().filter(|&&x| x <= 0.0).sum::<f64>() / tg.iter().filter(|&&x| x <= 0.0).count().max(1) as f64;
    let kelly = if al > 0.0 && aw > 0.0 { (w - (1.0 - w) / (aw / al)) / 4.0 } else { 0.0 };
    md.push_str(&format!("\nQuarter Kelly from train GOOD trades (n={}): w={:.3} b={:.3} f={:.4} -> {}\n", tg.len(), w, if al > 0.0 { aw / al } else { 0.0 }, kelly, if kelly <= 0.0 { "DO NOT TRADE" } else { "positive" }));
    fs::create_dir_all("reports/p5").map_err(|e| e.to_string())?;
    let path = format!("reports/p5/backtest_{tag}_d{delay}.md");
    fs::write(&path, &md).map_err(|e| e.to_string())?;
    println!("rows={} p0={:.4} -> {path}", rows.len(), p0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wilson_bounds() {
        let (lo, hi) = wilson(5, 10);
        assert!(lo < 0.5 && hi > 0.5 && lo > 0.2 && hi < 0.8);
        let _ = crate::outcomes::day_of(0);
    }
}
