//! Dev reputation (owner 21 Nov 2026): per dev, its launch history from devtrace — how many coins, how many rugged,
//! how many rose (coinrise rise from the reachable entry), and how the dev typically exits.
//! Usage: jaringan reput <universe_dir> <dev_dir> <rise_dir> <families.jsonl> <ath_dir> <out_dir>
//! Scope (config/reput.toml only_dexscreener = 1, owner 21 Nov): creators with >= 1 coin seen on DexScreener.
//! Output: <out>/dev_reputation.tsv (one row per dev) + summary json on stdout.
//! Exit type (descriptive, fixed before seeing the table). Base = coins the dev itself bought. "Total exit" = the dev
//! sold >= 90% of what it bought (qdev::DUMP_SHARE). The first type reached by >= 50% of the base wins, in order:
//! total exit with first sell <= 1 s, <= 60 s, <= 1 h, total exit later, partial sell; otherwise holds.
//! Limits: only sells from the dev wallet itself are seen (tokens moved to another wallet first are not), and a
//! truncated history (riwayat_lengkap = false) gives lower bounds.

use crate::qdev::{role_times, DUMP_SHARE, RUG_WINDOW_S};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

#[derive(Debug, Default, PartialEq)]
pub struct R {
    pub coins: usize,
    pub bought: usize,
    pub measured: usize,
    pub ge100: usize,
    pub moons: usize,
    pub rugs: usize,
    pub rugs_any: usize,
    pub median_rise_pct: f64,
    pub max_rise_pct: f64,
    pub sold_any: usize,
    pub total: usize,
    pub total_le1: usize,
    pub total_le60: usize,
    pub total_le3600: usize,
    pub median_first_sell_s: Option<i64>,
    pub median_sold_pct: f64,
    pub last_bt: i64,
}

fn median_f(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() { 0.0 } else { v[v.len() / 2] }
}

pub fn profile(coins: &[Value], rise: &HashMap<String, Value>) -> R {
    let mut r = R { coins: coins.len(), ..Default::default() };
    let (mut xs, mut sold, mut first) = (Vec::new(), Vec::new(), Vec::new());
    for c in coins {
        r.last_bt = r.last_bt.max(c["bt"].as_i64().unwrap_or(0));
        let fs_ = c["dev_first_sell_after_s"].as_i64();
        if c["dev_buy_sol"].as_f64().unwrap_or(0.0) > 0.0 {
            r.bought += 1;
            let share = c["dev_sold_share"].as_f64().unwrap_or(0.0);
            sold.push(share * 100.0);
            if let Some(s) = fs_ {
                r.sold_any += 1;
                first.push(s);
                if share >= DUMP_SHARE {
                    r.total += 1;
                    r.total_le1 += (s <= 1) as usize;
                    r.total_le60 += (s <= 60) as usize;
                    r.total_le3600 += (s <= 3600) as usize;
                }
            }
        }
        let Some(x) = c["mint"].as_str().and_then(|m| rise.get(m)).and_then(|v| v["rise_x"].as_f64().map(|x| (x, v["rug"] == true))) else { continue };
        r.measured += 1;
        r.ge100 += (x.0 >= 2.0) as usize;
        r.moons += (x.0 >= 5.0) as usize;
        if x.1 {
            r.rugs_any += 1;
            // owner rule v2 (qdev): a rug counts only when the dev was already selling within the first hour
            r.rugs += fs_.is_some_and(|s| s <= RUG_WINDOW_S) as usize;
        }
        xs.push((x.0 - 1.0) * 100.0);
    }
    r.median_rise_pct = median_f(&mut xs);
    r.max_rise_pct = xs.last().copied().unwrap_or(0.0);
    r.median_sold_pct = median_f(&mut sold);
    first.sort();
    r.median_first_sell_s = first.get(first.len() / 2).copied().filter(|_| !first.is_empty());
    r
}

pub fn exit_type(r: &R) -> &'static str {
    if r.bought == 0 {
        return "TANPA_BELI_DEV";
    }
    let half = |n: usize| 2 * n >= r.bought;
    if half(r.total_le1) {
        "EXIT_TOTAL_DETIK_PERTAMA"
    } else if half(r.total_le60) {
        "EXIT_TOTAL_LE60S"
    } else if half(r.total_le3600) {
        "EXIT_TOTAL_LE1JAM"
    } else if half(r.total) {
        "EXIT_TOTAL_LAMBAT"
    } else if half(r.sold_any) {
        "JUAL_SEBAGIAN"
    } else {
        "TAHAN"
    }
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 { 0.0 } else { 100.0 * n as f64 / d as f64 }
}

/// Pump.fun start mcap in SOL (30 virtual SOL / 1.073B virtual tokens x 1B supply).
pub const LAUNCH_SOL: f64 = 27.96;

/// DEX ATH of one coin (athgt formula 2). complete = false: athgt hit its page cap, the ATH is a lower bound.
/// thin = whole pool < thin_pool_txs txs or < thin_traders traders (21 Nov: USEF "ATH $97M" from 58 txs = supply
/// bought out of a thin pool); fake = sells < sell_min_pct of trades (21 Nov: EXEN 4,078 buys vs 42 sells).
/// Thin and fake coins are listed but never counted as a success.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct A {
    pub usd: f64,
    pub sol: f64,
    pub complete: bool,
    pub thin: bool,
    pub fake: bool,
}
pub type Ath = HashMap<String, A>;

/// Rise (x) from the Pump.fun launch price to the coin's peak: graduated = pool ATH (SOL) / launch mcap; otherwise
/// the bonding-curve peak (coinrise peak_mcap_sol_curve) / launch mcap. None = not measured.
pub fn launch_x(m: &str, ath: &Ath, curve: &HashMap<String, f64>) -> Option<f64> {
    if let Some(t) = ath.get(m) {
        return Some(t.sol / LAUNCH_SOL);
    }
    curve.get(m).map(|p| p / LAUNCH_SOL)
}

/// ATH counts of one creator's coins (athgt rows by mint): coins with a DEX ATH, >= $50k / $100k / $1M, max, top 3.
pub fn ath_counts(coins: &[Value], ath: &Ath) -> (usize, usize, usize, usize, f64, String) {
    let mut xs: Vec<(f64, String)> = coins.iter().filter_map(|c| {
        let m = c["mint"].as_str()?;
        let a = ath.get(m).filter(|t| !t.thin && !t.fake)?.usd;
        let sym = c["symbol"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| m.chars().take(6).collect());
        Some((a, sym))
    }).collect();
    xs.sort_by(|a, b| b.0.total_cmp(&a.0));
    let ge = |t: f64| xs.iter().filter(|x| x.0 >= t).count();
    let top: Vec<String> = xs.iter().take(3).map(|(a, s)| format!("{}:${:.0}k", s.replace(['\t', ' '], "_"), a / 1000.0)).collect();
    (xs.len(), ge(50_000.0), ge(100_000.0), ge(1_000_000.0), xs.first().map(|x| x.0).unwrap_or(0.0), top.join(","))
}

/// Elimination screen (owner 21 Nov: "eliminasi creator yang sering rug, koinnya sepi, dan jual di detik-detik
/// pertama"). Thresholds in config/reput.toml (provisional, set before seeing the screened list).
pub struct Screen {
    pub rug_max: f64,
    pub rug_min_meas: usize,
    pub thin_pool_txs: u64,
    pub thin_traders: u64,
    pub sell_min: f64,
    pub ragu_min_chk: u64,
    pub ragu_max: f64,
    pub thin_curve: u64,
    pub thin_max: f64,
    pub fast_s: i64,
    pub fast_max: f64,
}

/// Status, elimination reasons and the checks that had data. thin / thin_known = sepi coins / coins with activity
/// data; fast = coins the dev bought and first sold within fast_s; ok = successful coins (ATH >= $50k, not thin).
pub fn screen(r: &R, thin: usize, thin_known: usize, fast: usize, ok: usize, s: &Screen) -> (&'static str, String, String) {
    let (mut why, mut cek) = (Vec::new(), Vec::new());
    if r.measured >= s.rug_min_meas {
        cek.push("rug");
        if r.rugs as f64 > s.rug_max * r.measured as f64 {
            why.push("SERING_RUG");
        }
    }
    if thin_known > 0 {
        cek.push("sepi");
        if thin as f64 >= s.thin_max * thin_known as f64 {
            why.push("KOIN_SEPI");
        }
    }
    if r.bought > 0 {
        cek.push("jual");
        if fast as f64 >= s.fast_max * r.bought as f64 {
            why.push("JUAL_DETIK_AWAL");
        }
    }
    let st = if !why.is_empty() {
        "ELIMINASI"
    } else if ok > 0 && cek.len() == 3 {
        "LOLOS_BERKUALITAS"
    } else if ok > 0 {
        "LOLOS_DATA_KURANG"
    } else if cek.is_empty() {
        "BELUM_CUKUP_DATA"
    } else {
        "LOLOS_BELUM_TERBUKTI"
    };
    (st, if why.is_empty() { "-".into() } else { why.join("+") }, if cek.is_empty() { "-".into() } else { cek.join(",") })
}

/// Per-coin screen inputs of a coin set: (sepi coins, coins with activity data, coins the dev bought and first sold
/// within fast_s, smallest and largest rise from launch). Sepi = thin or buy-only DEX pool, or a fully read bonding
/// curve with < thin_curve trades.
pub fn coin_flags(coins: &[Value], ath: &Ath, rise: &HashMap<String, Value>, curve: &HashMap<String, f64>, sc: &Screen)
    -> (usize, usize, usize, Option<f64>, Option<f64>) {
    let (mut n_thin, mut known, mut n_fast) = (0usize, 0usize, 0usize);
    let mut xs: Vec<f64> = Vec::new();
    for c in coins {
        let m = c["mint"].as_str().unwrap_or("");
        if let Some(x) = launch_x(m, ath, curve) {
            xs.push(x);
        }
        let a = ath.get(m);
        let rz = rise.get(m);
        let curve_thin = rz.is_some_and(|v| v["history_complete"] == true && v["n_trades"].as_u64().unwrap_or(0) < sc.thin_curve);
        if a.is_some() || rz.is_some() {
            known += 1;
            n_thin += (a.is_some_and(|t| t.thin || t.fake) || curve_thin) as usize;
        }
        n_fast += (c["dev_buy_sol"].as_f64().unwrap_or(0.0) > 0.0 && c["dev_first_sell_after_s"].as_i64().is_some_and(|s| s <= sc.fast_s)) as usize;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    (n_thin, known, n_fast, xs.first().copied(), xs.last().copied())
}

/// Scope list for other Helius jobs (fam2, coinrise): JAR_SCOPE_FILE = one wallet per line (reput writes
/// <out>/ds_creators.txt = creators of DexScreener coins, owner 21 Nov). None = no scope set, every dev.
pub fn scope_set() -> Option<std::collections::HashSet<String>> {
    let p = std::env::var("JAR_SCOPE_FILE").ok()?;
    let s = fs::read_to_string(p).ok()?;
    Some(s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

/// One coin of the project archive: mint, symbol, date, graduated, ATH $, ATH complete, rise % from launch, status,
/// rug, dev buy SOL, dev first sell s, dev sold %.
pub fn coin_line(c: &Value, ath: &Ath, curve: &HashMap<String, f64>, rise: &HashMap<String, Value>) -> String {
    let m = c["mint"].as_str().unwrap_or("");
    let x = launch_x(m, ath, curve);
    let t = ath.get(m);
    let a = t.map(|t| t.usd);
    let lengkap = t.map(|t| if t.complete { "ya" } else { "batas_bawah" }).unwrap_or("-");
    let status = match (a, x) {
        (Some(a), _) if a >= 50_000.0 && t.is_some_and(|t| t.fake) => "BERHASIL_PALSU",
        (Some(a), _) if a >= 50_000.0 && t.is_some_and(|t| t.thin) => "BERHASIL_SEPI",
        (Some(a), _) if a >= 50_000.0 => "BERHASIL",
        (None, None) => "BELUM_DIUKUR",
        _ => "GAGAL",
    };
    let rz = rise.get(m);
    format!("{m}\t{}\t{}\t{}\t{}\t{lengkap}\t{}\t{status}\t{}\t{}\t{}\t{}",
        c["symbol"].as_str().unwrap_or("").replace(['\t', ' '], "_"),
        c["bt"].as_i64().map(crate::outcomes::day_of).unwrap_or("-".into()),
        t.is_some() || rz.is_some_and(|v| v["graduated"] == true),
        a.map(|a| format!("{a:.0}")).unwrap_or("-".into()),
        x.map(|x| format!("{:.0}", (x - 1.0) * 100.0)).unwrap_or("-".into()),
        rz.map(|v| (v["rug"] == true).to_string()).unwrap_or("-".into()),
        c["dev_buy_sol"].as_f64().map(|s| format!("{s:.2}")).unwrap_or("-".into()),
        c["dev_first_sell_after_s"].as_i64().map(|s| s.to_string()).unwrap_or("-".into()),
        c["dev_sold_share"].as_f64().map(|s| format!("{:.0}", s * 100.0)).unwrap_or("-".into()))
}

const COIN_HEAD: &str = "mint\tsimbol\ttanggal\tlulus\tATH_usd\tATH_lengkap\tnaik_dari_launch_pct\tstatus\trug\tdev_beli_sol\tdev_jual_pertama_detik\tdev_porsi_jual_pct";

fn status_rank(s: &str) -> u8 {
    match s {
        "LOLOS_BERKUALITAS" => 0,
        "LOLOS_DATA_KURANG" => 1,
        "LOLOS_BELUM_TERBUKTI" => 2,
        "BELUM_CUKUP_DATA" | "LOLOS_TIDAK_AKTIF" => 3,
        _ => 4,
    }
}

pub fn run(uni: &str, dev_dir: &str, rise_dir: &str, fam_file: &str, ath_dir: &str, out: &str) -> Result<(), String> {
    let k = |key: &str| crate::rpc::cfg_u64("config/reput.toml", key).ok_or(format!("config/reput.toml: {key} missing"));
    let sc = Screen { rug_max: k("rug_max_pct")? as f64 / 100.0, rug_min_meas: k("rug_min_measured")? as usize, thin_pool_txs: k("thin_pool_txs")?,
        thin_traders: k("thin_traders")?, sell_min: k("sell_min_pct")? as f64 / 100.0, ragu_min_chk: k("ragu_min_checks")?,
        ragu_max: k("ragu_max_pct")? as f64 / 100.0,
        thin_curve: k("thin_curve_trades")?, thin_max: k("thin_max_pct")? as f64 / 100.0, fast_s: k("fast_sell_s")? as i64,
        fast_max: k("fast_sell_max_pct")? as f64 / 100.0 };
    // athgt shards (<ath_dir>/ath_<i>.jsonl), formula 2 rows only (spot = real + virtual quote reserves, validated
    // against execution prices). A pool whose small trades often execute > 15% away from the spot price is DATA_RAGU
    // and left out. A pool that starts empty (first trade < 100 SOL mcap; execution confirms it, e.g. AJoP7w5p
    // 0.15 SOL in the pool) keeps its real tiny ATH, so the coin counts as GAGAL.
    let (mut ath, mut ragu, mut kosong): (Ath, usize, usize) = (HashMap::new(), 0, 0);
    let mut files: Vec<String> = fs::read_dir(ath_dir).map_err(|e| format!("{ath_dir}: {e}"))?.filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.ends_with(".jsonl") && p.contains("/ath_")).collect();
    files.sort();
    for f in files {
        for v in read_jsonl(&f) {
            if v["formula"] != 3 {
                continue;
            }
            let (Some(m), Some(usd), Some(sol)) = (v["mint"].as_str(), v["ath_mcap_usd"].as_f64(), v["ath_mcap_sol"].as_f64()) else { continue };
            let n = |k: &str| v[k].as_u64().unwrap_or(0);
            if n("chk_n") >= sc.ragu_min_chk && n("chk_bad") as f64 > sc.ragu_max * n("chk_n") as f64 {
                ragu += 1;
                continue;
            }
            kosong += (v["first_mcap_sol"].as_f64().unwrap_or(0.0) < 100.0) as usize;
            let complete = v["complete"] == true;
            let trades = n("n_buy") + n("n_sell");
            let thin = (complete && n("n_tx") < sc.thin_pool_txs) || (n("traders") as u64) < sc.thin_traders;
            let fake = trades >= 100 && (n("n_sell") as f64) < sc.sell_min * trades as f64;
            ath.insert(m.to_string(), A { usd, sol, complete, thin, fake });
        }
    }
    let mut curve: HashMap<String, f64> = HashMap::new();
    for v in read_jsonl(&format!("{rise_dir}/coins.jsonl")) {
        if let (Some(m), Some(p)) = (v["mint"].as_str(), v["peak_mcap_sol_curve"].as_f64()) {
            curve.insert(m.to_string(), p);
        }
    }
    let mut coin_rows: Vec<String> = Vec::new();
    // coins known from the create tx (roles) per creator, and the payer when another wallet launched it
    let mut by_creator: HashMap<String, Vec<Value>> = HashMap::new();
    let mut payer: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/roles.jsonl")) {
        if let (Some(c), Some(m)) = (v["creator"].as_str(), v["mint"].as_str()) {
            by_creator.entry(c.to_string()).or_default().push(json!({"mint": m, "bt": v["bt"], "symbol": v["symbol"]}));
            if let Some(p) = v["fee_payer"].as_str().filter(|p| *p != c) {
                *payer.entry(c.to_string()).or_default().entry(p.to_string()).or_insert(0) += 1;
            }
        }
    }
    let mut rise: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{rise_dir}/coins.jsonl")) {
        if let Some(m) = v["mint"].as_str() {
            rise.insert(m.to_string(), v);
        }
    }
    // newest row per dev is the valid one (devtrace redoes truncated histories)
    let mut devs: HashMap<String, Value> = HashMap::new();
    for v in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
        if let Some(d) = v["dev"].as_str() {
            devs.insert(d.to_string(), v);
        }
    }
    // families: fam2 format ("devs") or famview format (members with a role; DEV = launched coins)
    let mut fam: HashMap<String, String> = HashMap::new();
    let mut fam_rows: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for f in read_jsonl(fam_file) {
        let root = f["root"].as_str().or(f["family_id"].as_str()).unwrap_or("").to_string();
        let members: Vec<(String, String)> = f["members"].as_array().into_iter().flatten()
            .filter_map(|x| Some((x["role"].as_str()?.to_string(), x["w"].as_str()?.to_string()))).collect();
        let mut dws: Vec<String> = f["devs"].as_array().into_iter().flatten().filter_map(|d| d["dev"].as_str().or(d.as_str()).map(String::from)).collect();
        dws.extend(members.iter().filter(|(r, _)| r == "DEV").map(|(_, w)| w.clone()));
        for d in dws {
            fam.insert(d, root.clone());
        }
        fam_rows.push((root, members));
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let role_bt = role_times(dev_dir);
    let mut rows: Vec<(u8, usize, f64, usize, String)> = Vec::new();
    let (mut status_n, mut why_n): (HashMap<&str, usize>, HashMap<String, usize>) = (HashMap::new(), HashMap::new());
    let (mut types, mut types3): (HashMap<&str, usize>, HashMap<&str, usize>) = (HashMap::new(), HashMap::new());
    let (mut n_coins, mut n_meas, mut n_rug, mut n_ge100, mut n_moon) = (0, 0, 0, 0, 0);
    let (mut c_ath, mut c50, mut c100, mut c1m, mut dev50, mut not_traced) = (0, 0, 0, 0, 0, 0);
    let none = Value::Null;
    // creators with >= 1 coin seen on DexScreener (snap / qualified rows, or a devtrace coin with on_dex)
    let ds_mints: std::collections::HashSet<String> = ["snap", "qualified"].iter()
        .flat_map(|f| read_jsonl(&format!("{uni}/{f}.jsonl")))
        .filter_map(|v| v["mint"].as_str().map(String::from)).collect();
    let is_ds = |d: &String| {
        by_creator.get(d).is_some_and(|cs| cs.iter().any(|c| c["mint"].as_str().is_some_and(|m| ds_mints.contains(m))))
            || devs.get(d).is_some_and(|v| v["coins"].as_array().into_iter().flatten()
                .any(|c| c["on_dex"] == true || c["mint"].as_str().is_some_and(|m| ds_mints.contains(m))))
    };
    let only_ds = k("only_dexscreener").unwrap_or(0) == 1;
    let mut all: Vec<&String> = devs.keys().chain(by_creator.keys()).collect();
    all.sort();
    all.dedup();
    let n_all = all.len();
    if only_ds {
        all.retain(|&d| is_ds(d));
    }
    let non_ds = n_all - all.len();
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let scope_txt: Vec<&str> = all.iter().map(|d| d.as_str()).collect();
    fs::write(format!("{out}/ds_creators.txt"), format!("{}\n", scope_txt.join("\n"))).map_err(|e| e.to_string())?;
    let mut creator_coins: HashMap<String, Vec<Value>> = HashMap::new();
    for d in all {
        let v = devs.get(d).unwrap_or(&none);
        let mut coins = v["coins"].as_array().cloned().unwrap_or_default();
        let have: std::collections::HashSet<String> = coins.iter().filter_map(|c| c["mint"].as_str().map(String::from)).collect();
        for c in by_creator.get(d).into_iter().flatten() {
            if !have.contains(c["mint"].as_str().unwrap_or("")) {
                coins.push(c.clone());
            }
        }
        let riwayat = if v.is_null() { not_traced += 1; "belum" } else if v["history_complete"] == true { "lengkap" } else { "terpotong" };
        let (na, a50, a100, a1m, amax, top) = ath_counts(&coins, &ath);
        c_ath += na;
        c50 += a50;
        c100 += a100;
        c1m += a1m;
        dev50 += (a50 > 0) as usize;
        let pay = payer.get(d).and_then(|m| m.iter().max_by_key(|x| (*x.1, x.0.clone())).map(|x| format!("{}({})", x.0, x.1))).unwrap_or("-".into());
        // per-coin history and the creator's smallest / largest rise from the launch price
        let (n_thin, thin_known, n_fast, xmin, xmax) = coin_flags(&coins, &ath, &rise, &curve, &sc);
        let mut cs = coins.clone();
        cs.sort_by_key(|c| -c["bt"].as_i64().unwrap_or(0));
        for c in &cs {
            coin_rows.push(format!("{d}\t{}", coin_line(c, &ath, &curve, &rise)));
        }
        let pc = |x: Option<f64>| x.map(|x| format!("{:.0}", (x - 1.0) * 100.0)).unwrap_or("-".into());
        let (nmin, nmax) = (pc(xmin), pc(xmax));
        creator_coins.insert(d.clone(), coins.clone());
        let mut r = profile(&coins, &rise);
        r.last_bt = role_bt.get(d).copied().unwrap_or(0).max(r.last_bt);
        let t = exit_type(&r);
        let (st, why, cek) = screen(&r, n_thin, thin_known, n_fast, a50, &sc);
        *status_n.entry(st).or_insert(0) += 1;
        for w in why.split('+').filter(|w| *w != "-") {
            *why_n.entry(w.to_string()).or_insert(0) += 1;
        }
        *types.entry(t).or_insert(0) += 1;
        if r.coins >= 3 {
            *types3.entry(t).or_insert(0) += 1;
        }
        n_coins += r.coins;
        n_meas += r.measured;
        n_rug += r.rugs;
        n_ge100 += r.ge100;
        n_moon += r.moons;
        let line = format!(
            "{d}\t{st}\t{why}\t{cek}\t{n_thin}/{thin_known}\t{n_fast}/{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{}\t{:.0}\t{:.0}\t{}\t{}",
            r.bought, fam.get(d).map(String::as_str).unwrap_or("-"), pay, riwayat, r.coins, a50, r.coins - a50, nmin, nmax, na, a100, a1m, amax,
            if top.is_empty() { "-" } else { &top },
            r.measured, r.ge100, r.moons, r.rugs,
            r.rugs_any, pct(r.rugs, r.measured), r.median_rise_pct, r.max_rise_pct, r.bought, pct(r.sold_any, r.bought),
            pct(r.total, r.bought), pct(r.total_le1, r.bought), pct(r.total_le60, r.bought), pct(r.total_le3600, r.bought),
            r.median_first_sell_s.map(|s| s.to_string()).unwrap_or("-".into()), r.median_sold_pct, pct(r.bought - r.sold_any, r.bought),
            t, if r.last_bt > 0 { ((now - r.last_bt) / 3600).to_string() } else { "-".into() });
        rows.push((status_rank(st), a50, amax, r.rugs, line));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.total_cmp(&a.2)).then(a.3.cmp(&b.3)).then(a.4.cmp(&b.4)));
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    fs::write(format!("{out}/creator_coins.tsv"), format!("creator\t{COIN_HEAD}\n{}\n", coin_rows.join("\n"))).map_err(|e| e.to_string())?;
    let head = "creator\tstatus\talasan_eliminasi\tcek_berdata\tkoin_sepi\tjual_le_fast_s\tfamily\tpembayar_lain\triwayat\tkoin\tberhasil_ATH_ge50k\tgagal\tnaik_min_pct\tnaik_max_pct\tkoin_ada_ATH\t\
ATH_ge100k\tATH_ge1M\tATH_max_usd\tATH_top3\tkoin_terukur_entry\tnaik_entry_ge100pct\tmoonshot_entry_ge400pct\trug\trug_semua\trug_pct\tmedian_naik_entry_pct\t\
max_naik_pct\tkoin_dev_beli\tdev_jual_pct\texit_total_pct\texit_total_le1s_pct\texit_total_le60s_pct\texit_total_le1jam_pct\t\
median_detik_jual_pertama\tmedian_porsi_jual_pct\ttahan_pct\ttipe_exit\tjam_sejak_koin_terakhir";
    let body: Vec<String> = rows.iter().map(|r| r.4.clone()).collect();
    fs::write(format!("{out}/dev_reputation.tsv"), format!("{head}\n{}\n", body.join("\n"))).map_err(|e| e.to_string())?;
    let lolos: Vec<String> = rows.iter().filter(|r| r.0 <= 1).map(|r| r.4.clone()).collect();
    fs::write(format!("{out}/creator_lolos.tsv"), format!("{head}\n{}\n", lolos.join("\n"))).map_err(|e| e.to_string())?;
    // entities (owner 21 Nov: "kalau emang ada dev/family-nya jadikan 1"): a family = one operator, judged on the coins
    // of all its DEV wallets together with the same screen; a DexScreener creator outside every family is its own
    // entity. Watchlist = passed entities with >= 1 successful coin that launched within active_days. Wallets to watch:
    // DEV + INDUK (funder) + PERANTARA (78% of traced DexScreener creators made 1 coin = fresh wallet per coin, so the
    // next launch comes from a new wallet the funder pays; PENERIMA only receives profits and is not watched).
    let active_s = k("active_days")? as i64 * 86_400;
    let mut ents: Vec<(String, &str, Vec<String>, Vec<(String, String)>)> = Vec::new();
    let mut in_fam: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (root, members) in &fam_rows {
        let dws: Vec<String> = members.iter().filter(|(r, w)| r == "DEV" && creator_coins.contains_key(w)).map(|(_, w)| w.clone()).collect();
        if dws.is_empty() {
            continue;
        }
        in_fam.extend(dws.iter().cloned());
        let wws: Vec<(String, String)> = members.iter().filter(|(r, _)| r == "DEV" || r == "INDUK" || r == "PERANTARA").cloned().collect();
        ents.push((root.clone(), "FAMILY", dws, wws));
    }
    let mut solo: Vec<&String> = creator_coins.keys().filter(|d| !in_fam.contains(*d)).collect();
    solo.sort();
    for d in solo {
        ents.push((d.clone(), "INDEPENDEN", vec![d.clone()], vec![("DEV".to_string(), d.clone())]));
    }
    let (mut ent_rows, mut watch): (Vec<(u8, usize, f64, String)>, Vec<Value>) = (Vec::new(), Vec::new());
    let mut ent_n: HashMap<String, usize> = HashMap::new();
    // project archive of watchlist entities: every coin of every DEV wallet (status, ATH, rise, rug, dev exit)
    let mut wl_coins: Vec<String> = Vec::new();
    let (mut wl_fam, mut wl_solo, mut wl_moon, mut wl_coin_n) = (0usize, 0usize, 0usize, 0usize);
    for (id, kind, dws, wws) in &ents {
        let mut seen = std::collections::HashSet::new();
        let coins: Vec<Value> = dws.iter().flat_map(|d| creator_coins.get(d).cloned().unwrap_or_default())
            .filter(|c| seen.insert(c["mint"].as_str().unwrap_or("").to_string())).collect();
        let (n_thin, known, n_fast, xmin, xmax) = coin_flags(&coins, &ath, &rise, &curve, &sc);
        let (_, a50, _, _, amax, top) = ath_counts(&coins, &ath);
        let mut r = profile(&coins, &rise);
        r.last_bt = dws.iter().filter_map(|d| role_bt.get(d).copied()).fold(r.last_bt, i64::max);
        let (mut st, why, cek) = screen(&r, n_thin, known, n_fast, a50, &sc);
        if status_rank(st) <= 1 && now - r.last_bt > active_s {
            st = "LOLOS_TIDAK_AKTIF";
        }
        *ent_n.entry(format!("{kind}:{st}")).or_insert(0) += 1;
        let pc = |x: Option<f64>| x.map(|x| format!("{:.0}", (x - 1.0) * 100.0)).unwrap_or("-".into());
        let hours = if r.last_bt > 0 { ((now - r.last_bt) / 3600).to_string() } else { "-".into() };
        let wl: Vec<String> = wws.iter().map(|(ro, w)| format!("{ro}:{w}")).collect();
        ent_rows.push((status_rank(st), a50, amax, format!(
            "{id}\t{kind}\t{st}\t{why}\t{cek}\t{}\t{}\t{}\t{a50}\t{}\t{}\t{:.0}\t{n_thin}/{known}\t{n_fast}/{}\t{}\t{}\t{amax:.0}\t{}\t{hours}\t{}",
            dws.len(), wws.len(), r.coins, r.coins - a50, r.rugs, pct(r.rugs, r.measured), r.bought, pc(xmin), pc(xmax),
            if top.is_empty() { "-" } else { &top }, wl.join(","))));
        if status_rank(st) <= 1 {
            if *kind == "FAMILY" { wl_fam += 1 } else { wl_solo += 1 }
            wl_moon += a50;
            wl_coin_n += r.coins;
            for d in dws {
                let mut cs = creator_coins.get(d).cloned().unwrap_or_default();
                cs.sort_by_key(|c| -c["bt"].as_i64().unwrap_or(0));
                for c in &cs {
                    wl_coins.push(format!("{id}\t{kind}\t{st}\t{d}\t{}", coin_line(c, &ath, &curve, &rise)));
                }
            }
            watch.push(json!({"entitas": id, "tipe": kind, "status": st, "koin": r.coins, "berhasil_ath_ge50k": a50, "ath_max_usd": amax.round(),
                "top3": top, "rug": r.rugs, "jam_sejak_launch": hours,
                "wallets": wws.iter().map(|(ro, w)| json!({"role": ro, "w": w})).collect::<Vec<_>>()}));
        }
    }
    ent_rows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.total_cmp(&a.2)).then(a.3.cmp(&b.3)));
    let ehead = "entitas\ttipe\tstatus\talasan_eliminasi\tcek_berdata\twallet_dev\twallet_pantau\tkoin\tberhasil_ATH_ge50k\tgagal\trug\trug_pct\t\
koin_sepi\tjual_le_fast_s\tnaik_min_pct\tnaik_max_pct\tATH_max_usd\tATH_top3\tjam_sejak_launch\tdaftar_wallet_pantau";
    let eb: Vec<String> = ent_rows.iter().map(|r| r.3.clone()).collect();
    fs::write(format!("{out}/entitas.tsv"), format!("{ehead}\n{}\n", eb.join("\n"))).map_err(|e| e.to_string())?;
    let wb: Vec<String> = ent_rows.iter().filter(|r| r.0 <= 1).map(|r| r.3.clone()).collect();
    fs::write(format!("{out}/watchlist.tsv"), format!("{ehead}\n{}\n", wb.join("\n"))).map_err(|e| e.to_string())?;
    let n_wallets: usize = watch.iter().map(|w| w["wallets"].as_array().map_or(0, |a| a.len())).sum();
    fs::write(format!("{out}/watchlist_koin.tsv"), format!("entitas\ttipe\tstatus_entitas\tcreator\t{COIN_HEAD}\n{}\n", wl_coins.join("\n")))
        .map_err(|e| e.to_string())?;
    fs::write(format!("{out}/watchlist.json"), serde_json::to_string_pretty(&watch).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("{}", json!({"entitas": ent_n, "watchlist_entitas": watch.len(), "watchlist_family": wl_fam, "watchlist_independen": wl_solo,
        "watchlist_wallet": n_wallets, "watchlist_koin": wl_coin_n, "watchlist_koin_berhasil_ge50k": wl_moon, "status": status_n, "eliminasi_alasan": why_n, "creators": rows.len(), "creators_non_dexscreener_dropped": non_ds, "creators_not_traced": not_traced, "coins": n_coins, "coins_with_ath": c_ath, "ath_data_ragu": ragu, "pool_kosong": kosong,
        "ath_ge50k": c50, "ath_ge100k": c100, "ath_ge1m": c1m, "creators_with_ath_ge50k": dev50,
        "coins_measured": n_meas, "rugs": n_rug, "ge100": n_ge100, "moons": n_moon, "exit_types": types, "exit_types_devs_ge3_coins": types3}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(m: &str, buy: f64, sell: Option<i64>, share: f64) -> Value {
        json!({"mint": m, "bt": 100, "dev_buy_sol": buy, "dev_first_sell_after_s": sell, "dev_sold_share": share})
    }
    #[test]
    fn exit_profile() {
        let mut rise = HashMap::new();
        rise.insert("a".to_string(), json!({"rise_x": 6.0, "rug": false}));
        rise.insert("b".to_string(), json!({"rise_x": 1.2, "rug": true}));
        let coins = vec![c("a", 1.0, Some(0), 1.0), c("b", 1.0, Some(30), 0.95), c("c", 1.0, None, 0.0), c("d", 0.0, None, 0.0)];
        let r = profile(&coins, &rise);
        assert_eq!((r.coins, r.bought, r.sold_any, r.total, r.total_le1, r.total_le60), (4, 3, 2, 2, 1, 2));
        assert_eq!((r.measured, r.ge100, r.moons, r.rugs, r.rugs_any), (2, 1, 1, 1, 1));
        assert_eq!(exit_type(&r), "EXIT_TOTAL_LE60S");
        let hold = profile(&[c("x", 1.0, None, 0.0), c("y", 1.0, Some(9000), 0.2)], &rise);
        assert_eq!(exit_type(&hold), "JUAL_SEBAGIAN");
        assert_eq!(exit_type(&profile(&[c("x", 1.0, Some(1), 0.99)], &rise)), "EXIT_TOTAL_DETIK_PERTAMA");
        assert_eq!(exit_type(&profile(&[c("x", 0.0, None, 0.0)], &rise)), "TANPA_BELI_DEV");
    }
    #[test]
    fn screen_rules() {
        let s = Screen { rug_max: 0.4, rug_min_meas: 2, thin_pool_txs: 200, thin_traders: 50, sell_min: 0.05, ragu_min_chk: 10, ragu_max: 0.2,
            thin_curve: 50, thin_max: 0.5, fast_s: 10, fast_max: 0.5 };
        let r = R { measured: 4, rugs: 1, bought: 4, ..Default::default() };
        assert_eq!(screen(&r, 1, 4, 1, 1, &s).0, "LOLOS_BERKUALITAS");
        // 2 of 4 rugged = 50% > 40%
        let rug = R { measured: 4, rugs: 2, bought: 4, ..Default::default() };
        assert_eq!(screen(&rug, 0, 4, 0, 1, &s).1, "SERING_RUG");
        let (st, why, _) = screen(&r, 2, 4, 2, 1, &s);
        assert_eq!((st, why.as_str()), ("ELIMINASI", "KOIN_SEPI+JUAL_DETIK_AWAL"));
        let nobuy = R { measured: 4, bought: 0, ..Default::default() };
        assert_eq!(screen(&nobuy, 0, 4, 0, 1, &s).0, "LOLOS_DATA_KURANG");
        assert_eq!(screen(&R::default(), 0, 0, 0, 0, &s).0, "BELUM_CUKUP_DATA");
        assert_eq!(screen(&r, 0, 4, 0, 0, &s).0, "LOLOS_BELUM_TERBUKTI");
    }
    #[test]
    fn ath_count() {
        let mut ath = HashMap::new();
        let a = |usd: f64, sol: f64, complete: bool, thin: bool, fake: bool| A { usd, sol, complete, thin, fake };
        ath.insert("a".to_string(), a(1_500_000.0, 22_368.0, true, false, false));
        ath.insert("b".to_string(), a(60_000.0, 900.0, false, false, false));
        ath.insert("c".to_string(), a(20_000.0, 300.0, true, false, false));
        ath.insert("t".to_string(), a(90_000_000.0, 1_400_000.0, true, true, false));
        ath.insert("f".to_string(), a(170_000_000.0, 2_400_000.0, false, false, true));
        let mut curve = HashMap::new();
        curve.insert("d".to_string(), 55.92);
        // pool ATH 22,368 SOL / 27.96 = 800x from launch; curve peak 55.92 SOL / 27.96 = 2x
        assert!((launch_x("a", &ath, &curve).unwrap() - 800.0).abs() < 0.01);
        assert_eq!(launch_x("d", &ath, &curve), Some(2.0));
        assert_eq!(launch_x("e", &ath, &curve), None);
        let coins = vec![json!({"mint": "a", "symbol": "AAA"}), json!({"mint": "b"}), json!({"mint": "c"}), json!({"mint": "d"})];
        let (n, a50, a100, a1m, mx, top) = ath_counts(&coins, &ath);
        assert_eq!((n, a50, a100, a1m), (3, 2, 1, 1));
        assert_eq!(mx, 1_500_000.0);
        assert_eq!(top, "AAA:$1500k,b:$60k,c:$20k");
        // a thin pool (bought out, few txs) is never a success
        assert_eq!(ath_counts(&[json!({"mint": "t"})], &ath).1, 0);
        // a buy-only pool (no real sellers) is never a success either
        assert_eq!(ath_counts(&[json!({"mint": "f"})], &ath).1, 0);
    }
}
