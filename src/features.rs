//! Data quality of trade files (dq) and as-of feature table for CatBoost (feat).
//! Every feature uses only data with time <= the decision time; "s0_*" features use trades of the create slot
//! only and are allowed only when the entry delay is >= 1 slot (we have seen slot 0 before we send).
use crate::graph::{load_fund, read_lines};
use crate::outcomes::{parse_t, T};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Write};

/// Curve continuity: each trade must move virtual SOL reserves by exactly its own amount.
/// A break means a missing or misordered trade between two rows.
pub fn dq(launch_files: &[String], trade_files: &[String]) -> Result<(), String> {
    let mut init: HashMap<String, (u64, i64)> = HashMap::new();
    let mut names: HashMap<String, u64> = HashMap::new();
    let mut uris: HashMap<String, u64> = HashMap::new();
    let mut n_launch = 0u64;
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] != "create" {
                continue;
            }
            n_launch += 1;
            init.insert(v["mint"].as_str().unwrap_or("").into(), (v["vsol"].as_u64().unwrap_or(0), v["bt"].as_i64().unwrap_or(0)));
            *names.entry(format!("{}|{}", v["name"].as_str().unwrap_or(""), v["symbol"].as_str().unwrap_or("")).to_lowercase()).or_default() += 1;
            *uris.entry(v["uri"].as_str().unwrap_or("").into()).or_default() += 1;
        }
    }
    let (mut coins, mut err_rows, mut dup_coins, mut break_coins, mut breaks, mut trades, mut no_launch, mut censored, mut empty) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut seen_mint: HashMap<String, u64> = HashMap::new();
    for p in trade_files {
        for v in read_lines(p) {
            let m = v["mint"].as_str().unwrap_or("").to_string();
            *seen_mint.entry(m.clone()).or_default() += 1;
            if v.get("error").is_some() {
                err_rows += 1;
                continue;
            }
            coins += 1;
            if v["complete_to_horizon"] != true {
                censored += 1;
            }
            let Some(&(vs0, _)) = init.get(&m) else { no_launch += 1; continue };
            let mut tr: Vec<(u64, u64, u64, T)> = v["t"].as_array().map(|a| a.iter().filter_map(|x| Some((x[0].as_u64()?, x[1].as_u64()?, x[2].as_u64()?, parse_t(x)?))).collect()).unwrap_or_default();
            if tr.is_empty() {
                empty += 1;
                continue;
            }
            tr.sort_by_key(|t| (t.0, t.1, t.2));
            let before = tr.len();
            tr.dedup_by_key(|t| (t.0, t.1, t.2));
            if tr.len() != before {
                dup_coins += 1;
            }
            trades += tr.len() as u64;
            let mut prev = vs0;
            let mut b = 0u64;
            for (_, _, _, t) in &tr {
                let exp = if t.buy { prev.checked_add(t.sol) } else { prev.checked_sub(t.sol) };
                if exp != Some(t.vsol) {
                    b += 1;
                }
                prev = t.vsol;
            }
            breaks += b;
            if b > 0 {
                break_coins += 1;
            }
        }
    }
    let dup_mint_rows: u64 = seen_mint.values().filter(|&&c| c > 1).map(|c| c - 1).sum();
    let tmpl = names.values().filter(|&&c| c > 1).sum::<u64>();
    let same_uri = uris.iter().filter(|(u, &c)| c > 1 && !u.is_empty()).map(|(_, c)| c).sum::<u64>();
    let mut top: Vec<(&String, &u64)> = names.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    println!("trade files: coins={coins} error_rows={err_rows} duplicate_coin_rows={dup_mint_rows} coins_without_launch={no_launch} empty={empty}");
    println!("  censored_before_horizon={censored} ({:.2}%) trades={trades} coins_with_dup_trades={dup_coins}", 100.0 * censored as f64 / coins.max(1) as f64);
    println!("  curve continuity breaks={breaks} in {break_coins} coins ({:.2}% of coins)", 100.0 * break_coins as f64 / coins.max(1) as f64);
    println!("launches={n_launch}: coins sharing name+symbol with another coin={tmpl} ({:.1}%), sharing same metadata uri={same_uri}", 100.0 * tmpl as f64 / n_launch.max(1) as f64);
    println!("  most repeated name|symbol: {:?}", top.iter().take(5).map(|(k, c)| format!("{k}={c}")).collect::<Vec<_>>());
    Ok(())
}

struct Row {
    mint: String,
    creator: String,
    day: String,
    bt: i64,
    ret: f64,
    ret2: f64,
    peak: f64,
}

/// Closed-coin history of one entity (funder or bundle wallet): count, wins (net > 0), moons (peak >= 2x our cost).
#[derive(Default, Clone, Copy)]
struct Hist {
    n: u64,
    win: u64,
    moon: u64,
    ret: f64,
}
impl Hist {
    fn add(&mut self, r: &Row) {
        self.n += 1;
        self.win += (r.ret > 0.0) as u64;
        self.moon += (r.peak >= 2.0) as u64;
        self.ret += r.ret;
    }
    fn rate(&self, k: u64) -> f64 {
        if self.n == 0 { f64::NAN } else { k as f64 / self.n as f64 }
    }
    fn mean_ret(&self) -> f64 {
        if self.n == 0 { f64::NAN } else { self.ret / self.n as f64 }
    }
}

struct L {
    bt: i64,
    slot: u64,
    txi: u64,
    creator: String,
    user: String,
    sym: String,
    v: Value,
}

pub fn feat(launch_files: &[String], trade_files: &[String], fund_files: &[String], outcomes_csv: &str, delay: &str, out: &str) -> Result<(), String> {
    // slot-0 trades are visible only when we enter at slot >= 1 (numeric delay); never for "0o"/"jK" (same block)
    let use_s0 = delay.parse::<u64>().map(|d| d >= 1).unwrap_or(false);
    // all launches (all downloaded days) for universe history
    let mut ls: Vec<L> = Vec::new();
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] != "create" {
                continue;
            }
            ls.push(L { bt: v["bt"].as_i64().unwrap_or(0), slot: v["slot"].as_u64().unwrap_or(0), txi: v["txi"].as_u64().unwrap_or(0),
                creator: v["creator"].as_str().unwrap_or("").into(), user: v["user"].as_str().unwrap_or("").into(),
                sym: v["symbol"].as_str().unwrap_or("").to_lowercase(), v });
        }
    }
    ls.sort_by_key(|l| (l.slot, l.txi));
    // as-of universe counters, computed in one sweep
    let mut hist: HashMap<String, [f64; 7]> = HashMap::new();
    {
        let mut last_c: HashMap<&str, (u64, i64)> = HashMap::new();
        let mut last_u: HashMap<&str, u64> = HashMap::new();
        let mut c24: HashMap<&str, VecDeque<i64>> = HashMap::new();
        let mut sym24: HashMap<&str, VecDeque<i64>> = HashMap::new();
        let mut heat: VecDeque<i64> = VecDeque::new();
        for l in &ls {
            let (pc, pbt) = last_c.get(l.creator.as_str()).copied().unwrap_or((0, 0));
            let q = c24.entry(&l.creator).or_default();
            while q.front().map(|&t| t < l.bt - 86_400).unwrap_or(false) {
                q.pop_front();
            }
            let s = sym24.entry(&l.sym).or_default();
            while s.front().map(|&t| t < l.bt - 86_400).unwrap_or(false) {
                s.pop_front();
            }
            while heat.front().map(|&t| t < l.bt - 300).unwrap_or(false) {
                heat.pop_front();
            }
            hist.insert(l.v["mint"].as_str().unwrap_or("").into(), [pc as f64, if pc > 0 { (l.bt - pbt) as f64 } else { -1.0 }, q.len() as f64,
                last_u.get(l.user.as_str()).copied().unwrap_or(0) as f64, heat.len() as f64, s.len() as f64, 0.0]);
            q.push_back(l.bt);
            s.push_back(l.bt);
            heat.push_back(l.bt);
            last_c.insert(&l.creator, (pc + 1, l.bt));
            *last_u.entry(&l.user).or_default() += 1;
        }
    }
    let by_mint: HashMap<&str, &L> = ls.iter().map(|l| (l.v["mint"].as_str().unwrap_or(""), l)).collect();
    // funding, with as-of funder fan-out
    let fm = load_fund(fund_files);
    let mut funded_times: HashMap<&str, Vec<i64>> = HashMap::new();
    for f in fm.values() {
        if let Some(fu) = &f.funder {
            funded_times.entry(fu.as_str()).or_default().push(f.first_bt);
        }
    }
    for v in funded_times.values_mut() {
        v.sort_unstable();
    }
    // slot-0 trades per mint
    let mut s0: HashMap<String, Vec<T>> = HashMap::new();
    if use_s0 {
        for p in trade_files {
            for v in read_lines(p) {
                let Some(m) = v["mint"].as_str() else { continue };
                let Some(l) = by_mint.get(m) else { continue };
                let tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).filter(|t| t.slot == l.slot).collect()).unwrap_or_default();
                s0.insert(m.to_string(), tr);
            }
        }
    }
    // labels from outcomes, plus as-of creator outcome history from closed coins
    let mut rows: Vec<Row> = Vec::new();
    let f = fs::File::open(outcomes_csv).map_err(|e| e.to_string())?;
    for l in BufReader::new(f).lines().map_while(Result::ok).skip(1) {
        let c: Vec<&str> = l.split(',').collect();
        if c.len() < 16 || c[8] != delay || c[9] != "1" || c[6] == "1" {
            continue;
        }
        rows.push(Row { mint: c[0].into(), creator: c[1].into(), day: c[3].into(), bt: c[5].parse().unwrap_or(0), ret: c[11].parse().unwrap_or(0.0),
            ret2: c[12].parse().unwrap_or(0.0), peak: c[13].parse().unwrap_or(0.0) });
    }
    rows.sort_by_key(|r| r.bt);
    // closed outcome events (time = create + horizon) -> creator / funder / bundle-wallet history
    let mut closed: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        closed.entry(r.bt + 1800).or_default().push(i);
    }
    let funder_of = |c: &str| fm.get(c).and_then(|f| f.funder.clone());
    let bundlers = |m: &str| -> Vec<String> {
        by_mint.get(m).map(|l| l.v["tx_buyers"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).filter(|x| *x != l.creator && *x != l.user).map(String::from).collect()).unwrap_or_default()).unwrap_or_default()
    };
    let mut fh: HashMap<String, Hist> = HashMap::new();
    let mut bh: HashMap<String, Hist> = HashMap::new();
    let mut ch: HashMap<String, (u64, u64)> = HashMap::new();
    let mut ci = closed.iter().peekable();
    let mut o = fs::File::create(out).map_err(|e| e.to_string())?;
    let names = ["label", "ret", "ret2", "day", "bt", "own_buy_sol", "tx_buys", "tx_buy_sol", "tx_buyers", "user_ne_creator", "cashback", "cfee_bps",
        "name_len", "sym_len", "hour", "prev_launch_creator", "secs_since_prev", "launch_24h_creator", "prev_launch_user", "heat_5m", "same_sym_24h",
        "has_fund", "fund_lag_s", "fund_sol", "funder_prior_funded", "fund_kind_balance", "prev_closed_n", "prev_closed_win",
        "s0_buys", "s0_sells", "s0_sol_in", "s0_buyers", "s0_creator_sold", "s0_price_mult",
        "f_hist_n", "f_hist_win", "f_hist_moon", "f_hist_ret", "b_hist_n", "b_hist_win", "b_hist_moon", "b_hist_ret", "b_known", "mint"];
    writeln!(o, "{}", names.join("\t")).map_err(|e| e.to_string())?;
    let mut n = 0u64;
    for r in &rows {
        let (mint, creator, day, bt, ret, ret2) = (&r.mint, &r.creator, &r.day, &r.bt, &r.ret, &r.ret2);
        while let Some((&t, _)) = ci.peek() {
            if t > *bt {
                break;
            }
            let (_, idx) = ci.next().expect("peeked");
            for &i in idx {
                let q = &rows[i];
                let e = ch.entry(q.creator.clone()).or_default();
                e.0 += 1;
                e.1 += (q.ret > 0.0) as u64;
                if let Some(fw) = funder_of(&q.creator) {
                    fh.entry(fw).or_default().add(q);
                }
                for b in bundlers(&q.mint) {
                    bh.entry(b).or_default().add(q);
                }
            }
        }
        let fhv = funder_of(creator).and_then(|fw| fh.get(&fw).copied()).unwrap_or_default();
        let mut bagg = Hist::default();
        let mut known = 0.0;
        for b in bundlers(mint) {
            if let Some(h) = bh.get(&b) {
                bagg.n += h.n;
                bagg.win += h.win;
                bagg.moon += h.moon;
                bagg.ret += h.ret;
                known += 1.0;
            }
        }
        let Some(l) = by_mint.get(mint.as_str()) else { continue };
        let v = &l.v;
        let h = hist.get(mint).copied().unwrap_or([0.0; 7]);
        let fu = fm.get(creator.as_str());
        let (has, lag, fsol, prior, kind) = match fu {
            Some(f) if f.funder.is_some() && f.first_bt > 0 && f.first_bt <= *bt => {
                let fw = f.funder.as_deref().unwrap_or("");
                let prior = funded_times.get(fw).map(|t| t.partition_point(|&x| x < *bt) as f64).unwrap_or(0.0);
                (1.0, (*bt - f.first_bt) as f64, 0.0, prior, 0.0)
            }
            _ => (0.0, -1.0, -1.0, -1.0, -1.0),
        };
        let _ = (fsol, kind);
        let (pn, pw) = ch.get(creator).copied().unwrap_or((0, 0));
        let s0v: [f64; 6] = if use_s0 {
            let t = s0.get(mint).cloned().unwrap_or_default();
            let buys = t.iter().filter(|x| x.buy).count() as f64;
            let sells = t.iter().filter(|x| !x.buy).count() as f64;
            let sol_in: f64 = t.iter().map(|x| if x.buy { x.sol as f64 } else { -(x.sol as f64) }).sum::<f64>() / 1e9;
            let mut b: Vec<&str> = t.iter().filter(|x| x.buy).map(|x| x.user.as_str()).collect();
            b.sort_unstable();
            b.dedup();
            let csold = t.iter().any(|x| !x.buy && (x.user == l.creator || x.user == l.user)) as u8 as f64;
            let pm = t.last().map(|x| (x.vsol as f64 / x.vtok as f64) / (30e9 / 1.073e15)).unwrap_or(1.0);
            [buys, sells, sol_in, b.len() as f64, csold, pm]
        } else {
            [f64::NAN; 6]
        };
        let fmt = |x: f64| if x.is_nan() { "nan".to_string() } else { format!("{x}") };
        let vals: Vec<String> = vec![
            ((*ret > 0.0) as u8).to_string(), format!("{ret}"), format!("{ret2}"), day.clone(), bt.to_string(),
            fmt(v["own_buy_sol"].as_f64().unwrap_or(0.0) / 1e9), fmt(v["tx_buys"].as_f64().unwrap_or(0.0)), fmt(v["tx_buy_sol"].as_f64().unwrap_or(0.0) / 1e9),
            fmt(v["tx_buyers"].as_array().map(|a| a.len()).unwrap_or(0) as f64), fmt((l.user != l.creator) as u8 as f64),
            fmt((v["cashback"] == true) as u8 as f64), fmt(v["cfee_bps"].as_f64().unwrap_or(-1.0)),
            fmt(v["name"].as_str().map(|s| s.chars().count()).unwrap_or(0) as f64), fmt(l.sym.chars().count() as f64), fmt(((bt % 86_400) / 3600) as f64),
            fmt(h[0]), fmt(h[1]), fmt(h[2]), fmt(h[3]), fmt(h[4]), fmt(h[5]),
            fmt(has), fmt(lag), fmt(fsol), fmt(prior), fmt(kind), fmt(pn as f64), fmt(pw as f64),
            fmt(s0v[0]), fmt(s0v[1]), fmt(s0v[2]), fmt(s0v[3]), fmt(s0v[4]), fmt(s0v[5]),
            fmt(fhv.n as f64), fmt(fhv.rate(fhv.win)), fmt(fhv.rate(fhv.moon)), fmt(fhv.mean_ret()),
            fmt(bagg.n as f64), fmt(bagg.rate(bagg.win)), fmt(bagg.rate(bagg.moon)), fmt(bagg.mean_ret()), fmt(known), mint.clone(),
        ];
        writeln!(o, "{}", vals.join("\t")).map_err(|e| e.to_string())?;
        n += 1;
    }
    // CatBoost column description: label + auxiliary columns
    let cd = format!("0\tLabel\n1\tAuxiliary\n2\tAuxiliary\n3\tAuxiliary\n4\tAuxiliary\n{}\tAuxiliary\n", names.len() - 1);
    fs::write(format!("{out}.cd"), cd).map_err(|e| e.to_string())?;
    println!("rows={n} delay={delay} s0_features={use_s0} -> {out}");
    Ok(())
}
