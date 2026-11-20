//! ML v3 (owner 18 Nov): "we buy a new Pump.fun coin about 1 s after birth: will it die on the curve, pump and die,
//! graduate, or moonshot - and when do we sell?"  Honest by construction:
//!  - population = every SOL-quoted, non-mayhem coin created 14-15 Nov that has curve trade data (tr_all = all-coins
//!    pull, tr_rand = random pull). Nothing is chosen by outcome. Representativeness is printed (graduation rate vs all creates).
//!  - entry = honest::entry: first trade with blockTime >= create + LAT_S and slot > create slot, we land AFTER it
//!    (its post-state); curve while on the curve, PumpSwap pool if the curve was already finished (same-slot bundle
//!    graduation); NOT_FILLED / no data are counted, never silently dropped.
//!  - features = create tx + curve trades with blockTime < create + LAT_S (what we could see before deciding) + as-of
//!    history (creator / payer earlier coins and graduations that already happened, market graduation rate last 6 h) +
//!    out-of-sample graduation score of the create-tx model trained on 7-12 Nov (data/gml). No funding data (leaked before).
//!    The entry price itself is NOT a feature (we do not know the state we land on before sending).
//!  - exits = grid of take-profit / stop-loss / max-hold. The trigger is seen on a trade; our sell fills on the market
//!    state LAT_EXIT seconds later (honest::value_at). Fees on both legs (in the curve/pool math), Jito tip + tx cost on
//!    both legs. Data that ends before the fill = CENSORED (counted, excluded from means).
//!  - class label (used only as a training target, never as an exit): MATI (no graduation, best < +30%),
//!    SCALP (no graduation, best >= +30%), LULUS (graduated, best < 10x), MOON (graduated, best >= 10x).
//!  - split by time: train 14 Nov 00-18 h, embargo 18-20 h, valid 14 Nov 20-24 h, test 15 Nov (looked at once).
//!
//! Usage: jaringan ml3 build <out_dir> <lat_s>     -> rows.tsv, train.tsv, valid.tsv, test.tsv, pool.cd
//!        jaringan ml3 eval  <out_dir> [family_mints.txt]  (needs pred_valid.tsv, pred_test.tsv from catboost calc)
use crate::graph::read_lines;
use crate::honest::{entry, parse_p, pool_sell, value_at, Coin, Venue, P, POOL_FEE_BPS, TIP, TXC, X};
use crate::outcomes::{parse_t, sell_out, T};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;

const D14: i64 = 1_794_614_400; // 14 Nov 00:00 UTC
const TRAIN_END: i64 = D14 + 18 * 3600;
const VALID_START: i64 = D14 + 20 * 3600;
const D15: i64 = D14 + 86_400;
const D16: i64 = D15 + 86_400;
const LAT_EXIT: i64 = 1;
const CURVE_HORIZON: i64 = 1800; // curve trades were pulled for 30 min after create
const BIRTH_PRICE: f64 = 30e9 / 1.073e15; // lamports per raw token
pub const TPS: [f64; 8] = [0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 5.0, 9.0];
pub const SLS: [f64; 4] = [0.15, 0.3, 0.5, 0.99];
pub const HOLDS: [i64; 4] = [15, 60, 300, 1200];
const CLASSES: [&str; 4] = ["MATI", "SCALP", "LULUS", "MOON"];

const FEATS: [&str; 34] = [
    "own_buy_sol", "tx_buy_sol", "tx_buys", "tx_sells", "tx_buyers", "after_vsol", "fee_bps", "cfee_bps", "cashback",
    "creator_is_payer", "n_signers", "token2022", "name_len", "sym_len", "sym_upper", "name_digit", "hour", "dow",
    "cr_prev", "cr_prev_grad", "payer_prev", "univ_grad_6h", "p_grad_prior",
    "seen_trades", "seen_buys", "seen_sells", "seen_buyers", "seen_buy_sol", "seen_sell_sol", "creator_seen_sold",
    "seen_vsol", "seen_price_x_birth", "seen_max_buy_sol", "curve_done_seen",
];

fn cost_sol() -> f64 {
    X as f64 / 1e9 + TIP + TXC
}

/// Net return of selling for `lamports` (exit leg pays tip + tx too).
fn net_ret(lamports: u64) -> f64 {
    (lamports as f64 / 1e9 - TIP - TXC) / cost_sol() - 1.0
}

/// Market states strictly after our block: (bt, sell value of `tok` in lamports).
fn path(c: &Coin, tok: u64, after_slot: u64) -> Vec<(i64, u64)> {
    let mut v: Vec<(u64, i64, u64)> = Vec::new();
    if let Some(tr) = c.curve {
        for t in tr.iter().filter(|t| t.slot > after_slot && c.mig.map(|m| t.slot < m.0).unwrap_or(true)) {
            v.push((t.slot, t.bt, sell_out(t.vsol, t.vtok, tok, t.fee_bps.max(1))));
        }
    }
    if c.mig.is_some() {
        if let Some(p) = c.pool {
            for t in p.iter().filter(|t| t.slot > after_slot) {
                let (pb, pq) = t.post();
                v.push((t.slot, t.bt, pool_sell(pb, pq, tok, POOL_FEE_BPS)));
            }
        }
    }
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|x| (x.1, x.2)).collect()
}

/// One exit policy. None = censored.
fn exit_ret(c: &Coin, tok: u64, st: &[(i64, u64)], entry_bt: i64, tp: f64, sl: f64, hold: i64) -> Option<f64> {
    let deadline = entry_bt + hold;
    let mut trig = deadline;
    for (bt, v) in st {
        if *bt > deadline {
            break;
        }
        let r = net_ret(*v);
        if r >= tp || r <= -sl {
            trig = *bt;
            break;
        }
    }
    value_at(c, tok, trig + LAT_EXIT).map(net_ret)
}

fn read_preds(test_tsv: &str, pred_tsv: &str) -> HashMap<String, f64> {
    let mints: Vec<String> = fs::read_to_string(test_tsv).unwrap_or_default().lines().skip(1)
        .filter_map(|l| l.split('\t').nth(1).map(String::from)).collect();
    let probs: Vec<f64> = fs::read_to_string(pred_tsv).unwrap_or_default().lines().skip(1)
        .filter_map(|l| l.split('\t').next_back()?.parse().ok()).collect();
    if mints.len() != probs.len() {
        eprintln!("p_grad_prior: rows differ ({} vs {}) -> feature left empty", mints.len(), probs.len());
        return HashMap::new();
    }
    mints.into_iter().zip(probs).collect()
}

pub fn build(out: &str, lat_s: i64) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    // migrations (graduation): slot + time
    let mut mig: HashMap<String, (u64, i64)> = HashMap::new();
    for l in fs::read_to_string("data/roots/mig_all.tsv").map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if let (Some(m), Some(b), Some(s)) = (p.first(), p.get(2).and_then(|x| x.parse::<i64>().ok()), p.get(3).and_then(|x| x.parse::<u64>().ok())) {
            mig.entry(m.to_string()).or_insert((s, b));
        }
    }
    let mut grads: Vec<i64> = mig.values().map(|x| x.1).collect();
    grads.sort();
    // curve trades (14-15 Nov pulls)
    let mut curves: HashMap<String, (Vec<T>, i64)> = HashMap::new();
    for f in ["data/raw/tr_all_a.jsonl", "data/raw/tr_all_b.part_00.jsonl", "data/raw/tr_all_b.part_01.jsonl", "data/raw/tr_all_b.part_02.jsonl",
        "data/raw/tr_all_b.part_03.jsonl", "data/raw/tr_rand_a.jsonl", "data/raw/tr_rand_b.jsonl"] {
        let before = curves.len();
        for v in read_lines(f) {
            let m = v["mint"].as_str().unwrap_or("").to_string();
            let mut tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).collect()).unwrap_or_default();
            tr.sort_by_key(|t| (t.slot, t.txi));
            let cbt = v["cbt"].as_i64().unwrap_or(0);
            let end = if v["complete_to_horizon"] == true { cbt + CURVE_HORIZON } else { tr.last().map(|t| t.bt).unwrap_or(cbt) };
            if curves.get(&m).map(|e| e.0.len() >= tr.len()).unwrap_or(false) {
                continue;
            }
            curves.insert(m, (tr, end));
        }
        println!("{f}: +{} coins", curves.len() - before);
    }
    // PumpSwap pools (first pages after graduation)
    let mut pools: HashMap<String, (Vec<P>, i64)> = HashMap::new();
    for f in ["data/raw/pumpswap_amm_all.jsonl", "data/amm_new.jsonl"] {
        for v in read_lines(f) {
            let Some(t) = v["t"].as_array() else { continue };
            let mut p: Vec<P> = t.iter().filter_map(parse_p).collect();
            p.sort_by_key(|x| x.slot);
            let gbt = v["gbt"].as_i64().unwrap_or(0);
            let end = if v["complete"] == true { gbt + 86_400 } else { p.last().map(|x| x.bt).unwrap_or(gbt) };
            let m = v["mint"].as_str().unwrap_or("").to_string();
            if pools.get(&m).map(|e| e.0.len() >= p.len()).unwrap_or(false) {
                continue;
            }
            pools.insert(m, (p, end));
        }
    }
    let pgrad = read_preds("data/gml/test.tsv", "data/gml/pred.tsv");
    // launches: history features need every create from 6 Nov (warm-up), rows only for 14-15 Nov coins with curve data
    let mut ls: Vec<serde_json::Value> = Vec::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("launches_2026-") && n.ends_with(".jsonl") {
                for v in read_lines(&format!("{d}/{n}")) {
                    if v["kind"] == "create" {
                        ls.push(v);
                    }
                }
            }
        }
    }
    ls.sort_by_key(|v| (v["slot"].as_u64().unwrap_or(0), v["ci"].as_u64().unwrap_or(0)));
    ls.dedup_by(|a, b| a["mint"] == b["mint"]);
    let mut cr_prev: HashMap<String, Vec<String>> = HashMap::new();
    let mut payer_prev: HashMap<String, u32> = HashMap::new();
    let mut win: VecDeque<i64> = VecDeque::new();
    let (mut gi, mut gwin): (usize, VecDeque<i64>) = (0, VecDeque::new());
    let mut rows = String::new();
    let head = format!("class\tmint\tbt\tset\tvenue\tentry_s\tentry_blocks\tentry_mcap_sol\tbest_r\tgrad_in_data\t{}\t{}\n", FEATS.join("\t"),
        policy_names().join("\t"));
    rows.push_str(&head);
    let (mut pop_all, mut pop_all_grad, mut pop_cov, mut pop_cov_grad) = (0u64, 0u64, 0u64, 0u64);
    let mut status: HashMap<String, u64> = HashMap::new();
    for v in &ls {
        let bt = v["bt"].as_i64().unwrap_or(0);
        let mint = v["mint"].as_str().unwrap_or("").to_string();
        let creator = v["creator"].as_str().unwrap_or("").to_string();
        let payer = v["payer"].as_str().unwrap_or("").to_string();
        let t_dec = bt + lat_s; // decision time: we see everything with blockTime < t_dec
        while win.front().map(|t| *t < t_dec - 21_600).unwrap_or(false) {
            win.pop_front();
        }
        while gi < grads.len() && grads[gi] < t_dec {
            gwin.push_back(grads[gi]);
            gi += 1;
        }
        while gwin.front().map(|t| *t < t_dec - 21_600).unwrap_or(false) {
            gwin.pop_front();
        }
        let univ = gwin.len() as f64 / win.len().max(1) as f64;
        let (prev_len, prev_grad) = cr_prev.get(&creator)
            .map(|p| (p.len(), p.iter().filter(|m| mig.get(*m).map(|g| g.1 < t_dec).unwrap_or(false)).count()))
            .unwrap_or((0, 0));
        let pp = payer_prev.get(&payer).copied().unwrap_or(0);
        cr_prev.entry(creator.clone()).or_default().push(mint.clone());
        *payer_prev.entry(payer.clone()).or_default() += 1;
        win.push_back(bt);
        let sol_quote = v["quote"].as_str().unwrap_or("").starts_with("1111") && v["mayhem"] != true;
        if !(D14..D16).contains(&bt) || !sol_quote {
            continue;
        }
        pop_all += 1;
        let graduated = mig.contains_key(&mint);
        pop_all_grad += graduated as u64;
        let Some((tr, cend)) = curves.get(&mint) else { continue };
        pop_cov += 1;
        pop_cov_grad += graduated as u64;
        let cs = v["slot"].as_u64().unwrap_or(0);
        let c = Coin { cs, cbt: bt, curve: Some(tr.as_slice()), curve_end: *cend, mig: mig.get(&mint).copied(),
            pool: pools.get(&mint).map(|x| x.0.as_slice()), pool_end: pools.get(&mint).map(|x| x.1).unwrap_or(bt) };
        let en = match entry(&c, lat_s, X) {
            Ok(e) => e,
            Err(e) => {
                *status.entry(format!("{e:?}")).or_default() += 1;
                continue;
            }
        };
        *status.entry("FILLED".into()).or_default() += 1;
        // ---- features: create tx + trades seen before t_dec ----
        let seen: Vec<&T> = tr.iter().filter(|t| t.bt < t_dec).collect();
        let buyers: HashSet<&str> = seen.iter().filter(|t| t.buy && t.user != creator).map(|t| t.user.as_str()).collect();
        let buy_sol: f64 = seen.iter().filter(|t| t.buy).map(|t| t.sol as f64 / 1e9).sum();
        let sell_sol: f64 = seen.iter().filter(|t| !t.buy).map(|t| t.sol as f64 / 1e9).sum();
        let creator_sold = seen.iter().any(|t| !t.buy && t.user == creator) as u8 as f64;
        let (svsol, svtok) = seen.last().map(|t| (t.vsol as f64, t.vtok as f64)).unwrap_or((30e9, 1.073e15));
        let max_buy = seen.iter().filter(|t| t.buy).map(|t| t.sol as f64 / 1e9).fold(0.0, f64::max);
        let done_seen = c.mig.map(|m| m.1 < t_dec).unwrap_or(false) as u8 as f64;
        let name = v["name"].as_str().unwrap_or("");
        let sym = v["symbol"].as_str().unwrap_or("");
        let up = sym.chars().filter(|ch| ch.is_uppercase()).count() as f64 / sym.chars().count().max(1) as f64;
        let f: Vec<f64> = vec![
            v["own_buy_sol"].as_f64().unwrap_or(0.0) / 1e9, v["tx_buy_sol"].as_f64().unwrap_or(0.0) / 1e9,
            v["tx_buys"].as_f64().unwrap_or(0.0), v["tx_sells"].as_f64().unwrap_or(0.0),
            v["tx_buyers"].as_array().map(|a| a.len()).unwrap_or(0) as f64, v["after_vsol"].as_f64().unwrap_or(30e9) / 1e9,
            v["fee_bps"].as_f64().unwrap_or(0.0), v["cfee_bps"].as_f64().unwrap_or(0.0), (v["cashback"] == true) as u8 as f64,
            (creator == payer) as u8 as f64, v["signers"].as_array().map(|a| a.len()).unwrap_or(0) as f64,
            v["tprog"].as_str().unwrap_or("").starts_with("Tokenz") as u8 as f64,
            name.chars().count() as f64, sym.chars().count() as f64, up, name.chars().any(|ch| ch.is_ascii_digit()) as u8 as f64,
            ((bt % 86_400) / 3600) as f64, (((bt / 86_400) + 4) % 7) as f64,
            prev_len as f64, prev_grad as f64, pp as f64, univ, pgrad.get(&mint).copied().unwrap_or(f64::NAN),
            seen.len() as f64, seen.iter().filter(|t| t.buy).count() as f64, seen.iter().filter(|t| !t.buy).count() as f64,
            buyers.len() as f64, buy_sol, sell_sol, creator_sold, svsol / 1e9, (svsol / svtok) / BIRTH_PRICE, max_buy, done_seen,
        ];
        // ---- outcome path after our block ----
        let st = path(&c, en.tokens, en.slot);
        let best = st.iter().map(|x| net_ret(x.1)).fold(f64::NEG_INFINITY, f64::max);
        let best = if best.is_finite() { best } else { -1.0 };
        // graduated = migrated within 24 h of birth (from the migration list, not limited by our trade window);
        // "best" is measured on the data we have (curve 30 min + first pool pages) - label only, never an exit
        let grad_in_data = c.mig.map(|m| m.1 - bt <= 86_400).unwrap_or(false);
        let class = if grad_in_data { if best >= 9.0 { 3 } else { 2 } } else if best >= 0.3 { 1 } else { 0 };
        let mut rets: Vec<String> = Vec::new();
        for tp in TPS {
            for sl in SLS {
                for h in HOLDS {
                    rets.push(exit_ret(&c, en.tokens, &st, en.bt, tp, sl, h).map(|r| format!("{r:.5}")).unwrap_or_else(|| "NA".into()));
                }
            }
        }
        let set = if bt < TRAIN_END { "train" } else if bt < VALID_START { "embargo" } else if bt < D15 { "valid" } else { "test" };
        let venue = if en.venue == Venue::Curve { "curve" } else { "pool" };
        rows.push_str(&format!("{class}\t{mint}\t{bt}\t{set}\t{venue}\t{}\t{}\t{:.1}\t{best:.4}\t{}\t{}\t{}\n", en.bt - bt, en.slot.saturating_sub(cs),
            en.mcap_sol, grad_in_data as u8, f.iter().map(|x| if x.is_nan() { "nan".to_string() } else { format!("{x}") }).collect::<Vec<_>>().join("\t"),
            rets.join("\t")));
    }
    fs::write(format!("{out}/rows.tsv"), &rows).map_err(|e| e.to_string())?;
    // CatBoost files: label + features only (everything else auxiliary)
    let lines: Vec<&str> = rows.lines().collect();
    let hcols: Vec<&str> = lines[0].split('\t').collect();
    let feat_idx: Vec<usize> = FEATS.iter().map(|n| hcols.iter().position(|h| h == n).unwrap()).collect();
    let set_idx = 3;
    for set in ["train", "valid", "test"] {
        let mut o = fs::File::create(format!("{out}/{set}.tsv")).map_err(|e| e.to_string())?;
        writeln!(o, "class\t{}", FEATS.join("\t")).map_err(|e| e.to_string())?;
        for l in &lines[1..] {
            let c: Vec<&str> = l.split('\t').collect();
            if c[set_idx] != set {
                continue;
            }
            let fv: Vec<&str> = feat_idx.iter().map(|i| c[*i]).collect();
            writeln!(o, "{}\t{}", c[0], fv.join("\t")).map_err(|e| e.to_string())?;
        }
    }
    let mut cd = String::from("0\tLabel\n");
    for (i, n) in FEATS.iter().enumerate() {
        cd.push_str(&format!("{}\tNum\t{n}\n", i + 1));
    }
    fs::write(format!("{out}/pool.cd"), cd).map_err(|e| e.to_string())?;
    let pct = |a: u64, b: u64| 100.0 * a as f64 / b.max(1) as f64;
    println!("REPRESENTATIF? semua koin SOL 14-15 Nov {pop_all} (lulus {:.2}%) | punya data trade {pop_cov} (lulus {:.2}%)",
        pct(pop_all_grad, pop_all), pct(pop_cov_grad, pop_cov));
    let mut st: Vec<_> = status.into_iter().collect();
    st.sort();
    println!("status entry: {st:?}");
    let mut cnt = [[0u64; 4]; 3];
    for l in &lines[1..] {
        let c: Vec<&str> = l.split('\t').collect();
        let s = match c[3] { "train" => 0, "valid" => 1, "test" => 2, _ => continue };
        if let Ok(k) = c[0].parse::<usize>() {
            cnt[s][k] += 1;
        }
    }
    for (s, n) in ["train", "valid", "test"].iter().enumerate() {
        println!("{n}: MATI {} | SCALP {} | LULUS {} | MOON {}", cnt[s][0], cnt[s][1], cnt[s][2], cnt[s][3]);
    }
    Ok(())
}

pub fn policy_names() -> Vec<String> {
    let mut v = Vec::new();
    for tp in TPS {
        for sl in SLS {
            for h in HOLDS {
                v.push(format!("r_tp{tp}_sl{sl}_h{h}"));
            }
        }
    }
    v
}

struct Row {
    class: usize,
    mint: String,
    bt: i64,
    set: String,
    blocks: u64,
    venue: String,
    rets: Vec<Option<f64>>,
}

fn load_rows(out: &str) -> Result<Vec<Row>, String> {
    let s = fs::read_to_string(format!("{out}/rows.tsv")).map_err(|e| e.to_string())?;
    let mut lines = s.lines();
    let h: Vec<&str> = lines.next().ok_or("empty rows")?.split('\t').collect();
    let r0 = h.iter().position(|x| x.starts_with("r_tp")).ok_or("no policy cols")?;
    Ok(lines.map(|l| {
        let c: Vec<&str> = l.split('\t').collect();
        Row { class: c[0].parse().unwrap_or(0), mint: c[1].to_string(), bt: c[2].parse().unwrap_or(0), set: c[3].to_string(),
            venue: c[4].to_string(), blocks: c[6].parse().unwrap_or(0), rets: c[r0..].iter().map(|x| x.parse().ok()).collect() }
    }).collect())
}

/// CatBoost MultiClass probabilities (columns Probability:Class=0..3), rows in the same order as <set>.tsv.
fn load_probs(path: &str) -> Result<Vec<[f64; 4]>, String> {
    let s = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(s.lines().skip(1).map(|l| {
        let c: Vec<f64> = l.split('\t').skip(1).filter_map(|x| x.parse().ok()).collect();
        let mut p = [0.0; 4];
        for (i, v) in c.iter().take(4).enumerate() {
            p[i] = *v;
        }
        p
    }).collect())
}

#[derive(Clone)]
struct St {
    n: usize,
    cens: usize,
    win: f64,
    mean: f64,
    median: f64,
    no_top3: f64,
    avg_win: f64,
    avg_loss: f64,
}

fn stats(v: &[Option<f64>]) -> St {
    let mut x: Vec<f64> = v.iter().flatten().copied().collect();
    let cens = v.len() - x.len();
    if x.is_empty() {
        return St { n: 0, cens, win: 0.0, mean: 0.0, median: 0.0, no_top3: 0.0, avg_win: 0.0, avg_loss: 0.0 };
    }
    x.sort_by(|a, b| a.total_cmp(b));
    let n = x.len();
    let mean = x.iter().sum::<f64>() / n as f64;
    let wins: Vec<f64> = x.iter().copied().filter(|r| *r > 0.0).collect();
    let losses: Vec<f64> = x.iter().copied().filter(|r| *r <= 0.0).collect();
    let no_top3 = if n > 3 { x[..n - 3].iter().sum::<f64>() / (n - 3) as f64 } else { f64::NAN };
    St { n, cens, win: wins.len() as f64 / n as f64, mean, median: x[n / 2], no_top3,
        avg_win: if wins.is_empty() { 0.0 } else { wins.iter().sum::<f64>() / wins.len() as f64 },
        avg_loss: if losses.is_empty() { 0.0 } else { -losses.iter().sum::<f64>() / losses.len() as f64 } }
}

fn fmt(s: &St) -> String {
    format!("n {} (censored {}) | win {:.1}% | mean {:+.1}% | median {:+.1}% | mean tanpa 3 terbaik {:+.1}% | rata menang {:+.1}% | rata kalah -{:.1}%",
        s.n, s.cens, 100.0 * s.win, 100.0 * s.mean, 100.0 * s.median, 100.0 * s.no_top3, 100.0 * s.avg_win, 100.0 * s.avg_loss)
}

fn kelly(s: &St) -> f64 {
    if s.n == 0 || s.avg_loss <= 0.0 || s.avg_win <= 0.0 {
        return 0.0;
    }
    (s.win / s.avg_loss - (1.0 - s.win) / s.avg_win).max(0.0)
}

/// Bankroll in USD, trades in time order, stake = f x bank (cap: 0.2 SOL, returns were simulated with 0.1 SOL).
fn compound(trades: &[(i64, f64)], f: f64) -> (f64, f64) {
    let sol_usd = 72.0;
    let mut bank = 100.0f64;
    let (mut peak, mut dd) = (100.0f64, 0.0f64);
    for (_, r) in trades {
        let stake = (bank * f).min(0.2 * sol_usd);
        bank += stake * r;
        peak = peak.max(bank);
        dd = dd.max(1.0 - bank / peak);
    }
    (bank, dd)
}

pub fn eval(out: &str, family: Option<&str>) -> Result<(), String> {
    let rows = load_rows(out)?;
    let names = policy_names();
    let pv = load_probs(&format!("{out}/pred_valid.tsv"))?;
    let pt = load_probs(&format!("{out}/pred_test.tsv"))?;
    let valid: Vec<&Row> = rows.iter().filter(|r| r.set == "valid").collect();
    let test: Vec<&Row> = rows.iter().filter(|r| r.set == "test").collect();
    if valid.len() != pv.len() || test.len() != pt.len() {
        return Err(format!("prediction rows differ: valid {}/{} test {}/{}", valid.len(), pv.len(), test.len(), pt.len()));
    }
    let mut rep = String::new();
    // ---- loss vs baseline (class priors from train) ----
    let train: Vec<&Row> = rows.iter().filter(|r| r.set == "train").collect();
    let mut prior = [1e-9f64; 4];
    for r in &train {
        prior[r.class] += 1.0;
    }
    let tot: f64 = prior.iter().sum();
    for p in prior.iter_mut() {
        *p /= tot;
    }
    let ll = |rs: &[&Row], ps: &[[f64; 4]]| -> (f64, f64) {
        let n = rs.len().max(1) as f64;
        let m: f64 = rs.iter().zip(ps).map(|(r, p)| -p[r.class].clamp(1e-9, 1.0).ln()).sum::<f64>() / n;
        let b: f64 = rs.iter().map(|r| -prior[r.class].ln()).sum::<f64>() / n;
        (m, b)
    };
    let (mt, bt_) = ll(&test, &pt);
    rep.push_str(&format!("TEST MultiClass logloss model {mt:.4} vs tebak-rata-rata {bt_:.4} ({:+.1}%)\n", 100.0 * (1.0 - mt / bt_)));
    let mut conf = [[0usize; 4]; 4];
    for (r, p) in test.iter().zip(&pt) {
        let k = (0..4).max_by(|a, b| p[*a].total_cmp(&p[*b])).unwrap();
        conf[r.class][k] += 1;
    }
    rep.push_str("confusion TEST (baris = asli, kolom = tebakan MATI/SCALP/LULUS/MOON):\n");
    for (i, c) in conf.iter().enumerate() {
        rep.push_str(&format!("  {:<5} {:?}\n", CLASSES[i], c));
    }
    // ---- choose rule on VALID only ----
    // score kinds: P(not MATI), P(LULUS)+P(MOON); thresholds = top fractions of the valid score
    let score = |p: &[f64; 4], k: usize| if k == 0 { 1.0 - p[0] } else { p[2] + p[3] };
    let fracs = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0];
    let mut best: Option<(f64, usize, f64, usize, St)> = None; // (thr, kind, frac, policy, stats)
    let mut all_best: Option<(usize, St)> = None;
    for pi in 0..names.len() {
        let s_all = stats(&valid.iter().map(|r| r.rets[pi]).collect::<Vec<_>>());
        if s_all.n >= 50 && all_best.as_ref().map(|b| s_all.mean > b.1.mean).unwrap_or(true) {
            all_best = Some((pi, s_all));
        }
    }
    for kind in 0..2 {
        let mut sc: Vec<f64> = pv.iter().map(|p| score(p, kind)).collect();
        sc.sort_by(|a, b| b.total_cmp(a));
        for frac in fracs {
            let k = ((sc.len() as f64 * frac).ceil() as usize).clamp(1, sc.len());
            let thr = sc[k - 1];
            let pick: Vec<&&Row> = valid.iter().zip(&pv).filter(|(_, p)| score(p, kind) >= thr).map(|(r, _)| r).collect();
            for pi in 0..names.len() {
                let s = stats(&pick.iter().map(|r| r.rets[pi]).collect::<Vec<_>>());
                if s.n < 30 {
                    continue;
                }
                if best.as_ref().map(|b| s.mean > b.4.mean).unwrap_or(true) {
                    best = Some((thr, kind, frac, pi, s));
                }
            }
        }
    }
    let (thr, kind, frac, pi, sv) = best.ok_or("no rule with >= 30 valid trades")?;
    let f = (kelly(&sv) / 4.0).min(0.25);
    rep.push_str(&format!("\nATURAN DIPILIH DI VALID: skor {} >= {thr:.4} (top {:.0}% valid) | exit {} | 1/4 Kelly {:.1}%\n  VALID: {}\n",
        if kind == 0 { "P(tidak MATI)" } else { "P(LULUS)+P(MOON)" }, 100.0 * frac, names[pi], 100.0 * f, fmt(&sv)));
    // ---- apply ONCE on TEST ----
    let picks: Vec<(&&Row, &[f64; 4])> = test.iter().zip(&pt).filter(|(_, p)| score(p, kind) >= thr).collect();
    let st = stats(&picks.iter().map(|(r, _)| r.rets[pi]).collect::<Vec<_>>());
    rep.push_str(&format!("  TEST : {}\n", fmt(&st)));
    let mut tr: Vec<(i64, f64)> = picks.iter().filter_map(|(r, _)| r.rets[pi].map(|x| (r.bt, x))).collect();
    tr.sort_by_key(|x| x.0);
    let (bank, dd) = compound(&tr, f);
    rep.push_str(&format!("  $100, 1/4 Kelly {:.1}%, compounding (maks 0,2 SOL per beli): $100 -> ${bank:.0} | drawdown maks {:.0}%\n", 100.0 * f, 100.0 * dd));
    // per true class on test for the chosen rule
    for (k, name) in CLASSES.iter().enumerate() {
        let s = stats(&picks.iter().filter(|(r, _)| r.class == k).map(|(r, _)| r.rets[pi]).collect::<Vec<_>>());
        rep.push_str(&format!("    koin yang ternyata {name}: {}\n", fmt(&s)));
    }
    // blocks after birth
    let mut bl: Vec<u64> = picks.iter().map(|(r, _)| r.blocks).collect();
    bl.sort();
    if !bl.is_empty() {
        rep.push_str(&format!("  blok masuk setelah lahir (dibeli): min {} | median {} | 90% {} | pool {} dari {}\n", bl[0], bl[bl.len() / 2], bl[bl.len() * 9 / 10],
            picks.iter().filter(|(r, _)| r.venue == "pool").count(), picks.len()));
    }
    // ---- baselines on TEST ----
    if let Some((api, sa)) = all_best {
        let s = stats(&test.iter().map(|r| r.rets[api]).collect::<Vec<_>>());
        rep.push_str(&format!("\nPEMBANDING tanpa ML (beli SEMUA koin, exit terbaik di valid {}):\n  VALID: {}\n  TEST : {}\n", names[api], fmt(&sa), fmt(&s)));
        let s2 = stats(&test.iter().map(|r| r.rets[pi]).collect::<Vec<_>>());
        rep.push_str(&format!("  semua koin TEST dgn exit aturan ML ({}): {}\n", names[pi], fmt(&s2)));
    }
    // best possible exit per class on TEST (diagnostic only, chosen on test = upper bound, not a strategy)
    rep.push_str("\nDIAGNOSTIK (bukan strategi): exit terbaik per kelas asli di TEST\n");
    for (k, name) in CLASSES.iter().enumerate() {
        let sub: Vec<&&Row> = test.iter().filter(|r| r.class == k).collect();
        let mut bb: Option<(usize, St)> = None;
        for pj in 0..names.len() {
            let s = stats(&sub.iter().map(|r| r.rets[pj]).collect::<Vec<_>>());
            if s.n >= 10 && bb.as_ref().map(|b| s.mean > b.1.mean).unwrap_or(true) {
                bb = Some((pj, s));
            }
        }
        if let Some((pj, s)) = bb {
            rep.push_str(&format!("  {name} ({} koin): {} -> {}\n", sub.len(), names[pj], fmt(&s)));
        }
    }
    // ---- family subset (mints given by file) ----
    if let Some(ff) = family {
        let fam: HashSet<String> = fs::read_to_string(ff).unwrap_or_default().lines().map(|l| l.trim().to_string()).collect();
        let sub: Vec<&&Row> = test.iter().filter(|r| fam.contains(&r.mint)).collect();
        let s_rule = stats(&sub.iter().map(|r| r.rets[pi]).collect::<Vec<_>>());
        rep.push_str(&format!("\nKOIN FAMILY di TEST ({} koin, daftar {}): exit {} -> {}\n", sub.len(), ff, names[pi], fmt(&s_rule)));
    }
    fs::write(format!("{out}/report.txt"), &rep).map_err(|e| e.to_string())?;
    print!("{rep}");
    Ok(())
}
