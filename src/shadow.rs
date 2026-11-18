//! P7 monitor + executor (PAPER mode only: no signing, no sending, no private keys).
//! Every second: new Pump.fun creates -> creator/signer looked up in the class-A watchlist (in memory) -> the
//! signal goes straight to the executor in this process (no web in the path). The executor:
//!   * sizes the buy with quarter Kelly of the paper bankroll (cap 1 SOL), Jito tip as a cost,
//!   * fills at the REAL curve state read when we act (our real latency, not the creation price),
//!   * exits when any dev-family wallet (creator, signer, create-tx bundle, same-team wallets) sells,
//!     or at graduation (sell value from the PumpSwap pool, not the dead curve), or after 24 h
//!     (independent of state: EXIT_UNKNOWN_STATE = counted as a total loss).
//!
//! Fixes 18 Nov review (F2): curve reserves were swapped; first exit poll skipped trades; polls jumped over gaps;
//! graduated exits sold on the curve; positions with an unreadable curve never closed; reachability not logged.
//! Files: reports/shadow/{creates,signals,orders,gaps}.jsonl, reports/web/signals.json (status only).
use crate::amm::{amm_events, PAMM_CREATE_POOL};
use crate::decode::{self, Ev, MINT_AUTH};
use crate::outcomes::{buy_out, sell_out};
use crate::rpc::{cfg_u64, Rpc};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_PAGES: usize = 10;
const TIMEOUT_S: f64 = 86_400.0;
const POOL_FEE_BPS: u64 = 125; // [U] PumpSwap fee tier; real bps are in the events (amm.rs stores them from F4 on)

struct Pos {
    mint: String,
    curve: String,
    symbol: String,
    team: String,
    family: HashSet<String>,
    lamports: u64,
    tokens: u64,
    entry_t: f64,
    last_sig: Option<String>,
    create_slot: u64,
    detect_slot: Option<u64>,
}

/// Bonding curve state (layout from idl/pump.json BondingCurve).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve {
    pub vtok: u64,
    pub vsol: u64,
    pub complete: bool,
}

/// disc 8 | virtual_token_reserves @8 | virtual_quote_reserves @16 | real_token @24 | real_quote @32 | supply @40 | complete @48
pub fn parse_curve(d: &[u8]) -> Option<Curve> {
    let rd = |o: usize| d.get(o..o + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap_or([0; 8])));
    Some(Curve { vtok: rd(8)?, vsol: rd(16)?, complete: *d.get(48)? != 0 })
}

/// PumpSwap Pool account (idl/pump_amm.json Pool): disc 8, bump 1, index 2, creator 32, base_mint 32, quote_mint 32,
/// lp_mint 32, pool_base_token_account @139, pool_quote_token_account @171.
pub fn parse_pool_accounts(d: &[u8]) -> Option<(String, String)> {
    let pk = |o: usize| d.get(o..o + 32).map(|s| bs58::encode(s).into_string());
    Some((pk(139)?, pk(171)?))
}

/// Constant-product sell into a pool (base in, quote out), fee on the output.
pub fn pool_sell(pb: u64, pq: u64, tok: u64, fee_bps: u64) -> u64 {
    if pb == 0 {
        return 0;
    }
    let k = pb as u128 * pq as u128;
    let out = pq as u128 - k.div_ceil(pb as u128 + tok as u128);
    (out * (10_000 - fee_bps.min(10_000) as u128) / 10_000) as u64
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn account_data(rpc: &mut Rpc, acc: &str) -> Option<Vec<u8>> {
    let r = rpc.call("getAccountInfo", json!([acc, {"encoding": "base64", "commitment": "processed"}]), 1).ok()?;
    base64::engine::general_purpose::STANDARD.decode(r["value"]["data"][0].as_str()?).ok()
}

fn curve_state(rpc: &mut Rpc, curve: &str) -> Option<Curve> {
    parse_curve(&account_data(rpc, curve)?)
}

fn gap(f: &mut File, what: &str, addr: &str, detail: Value) {
    let _ = writeln!(f, "{}", json!({"t": now(), "gap": what, "addr": addr, "detail": detail}));
}

/// All successful signatures newer than `until` (oldest first), paginating with `before` so nothing is skipped.
/// If `until` is not reached within MAX_PAGES, or a page fails, a gap row (slot range) is written.
fn new_sigs(rpc: &mut Rpc, addr: &str, until: Option<&str>, limit: u64, gaps: &mut File) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut before: Option<String> = None;
    for page in 0..MAX_PAGES {
        let mut o = json!({"limit": limit});
        if let Some(u) = until {
            o["until"] = json!(u);
        }
        if let Some(b) = &before {
            o["before"] = json!(b);
        }
        let a = match rpc.call("getSignaturesForAddress", json!([addr, o]), 1) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => {
                gap(gaps, "sigs_error", addr, json!({"err": e, "page": page, "newest_seen_slot": out.first().map(|s| s["slot"].clone())}));
                break;
            }
        };
        let n = a.len() as u64;
        before = a.last().and_then(|s| s["signature"].as_str()).map(String::from);
        out.extend(a);
        // `until` given: a short page means we reached it. No `until` (first poll): one page is the reference point.
        if n < limit || until.is_none() {
            out.reverse();
            return out;
        }
        if page + 1 == MAX_PAGES {
            gap(gaps, "page_cap", addr, json!({"oldest_slot_fetched": out.last().map(|s| s["slot"].clone()), "newest_slot": out.first().map(|s| s["slot"].clone())}));
        }
    }
    out.reverse();
    out
}

fn get_tx(rpc: &mut Rpc, sig: &str, gaps: &mut File, addr: &str, slot: &Value) -> Option<Value> {
    match rpc.call("getTransaction", json!([sig, {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 1, "commitment": "confirmed"}]), 1) {
        Ok(t) if !t.is_null() => Some(t),
        other => {
            gap(gaps, "tx_missing", addr, json!({"sig": sig, "slot": slot, "err": other.err()}));
            None
        }
    }
}

/// After graduation: find the migrate tx on the curve (CreatePoolEvent), read the pool's token accounts, and return
/// (pool, pool_base, pool_quote, migrate_slot). None if anything cannot be read (the caller keeps the position open).
fn pool_state(rpc: &mut Rpc, curve: &str, gaps: &mut File) -> Option<(String, u64, u64, u64)> {
    let sigs = rpc.call("getSignaturesForAddress", json!([curve, {"limit": 20}]), 1).ok()?;
    for s in sigs.as_array().into_iter().flatten().filter(|s| s["err"].is_null()) {
        let Some(tx) = get_tx(rpc, s["signature"].as_str()?, gaps, curve, &s["slot"]) else { continue };
        for (disc, b) in amm_events(&tx) {
            if disc != PAMM_CREATE_POOL {
                continue;
            }
            let pool = b.get(165..197).map(|x| bs58::encode(x).into_string())?;
            let (ba, qa) = parse_pool_accounts(&account_data(rpc, &pool)?)?;
            let r = rpc.call("getMultipleAccounts", json!([[ba, qa], {"encoding": "jsonParsed", "commitment": "processed"}]), 1).ok()?;
            let amt = |i: usize| r["value"][i]["data"]["parsed"]["info"]["tokenAmount"]["amount"].as_str().and_then(|x| x.parse::<u64>().ok());
            return Some((pool, amt(0)?, amt(1)?, tx["slot"].as_u64().unwrap_or(0)));
        }
    }
    None
}

fn load_watch(nj: &Value, watch: &mut HashMap<String, String>, team_wallets: &mut HashMap<String, Vec<String>>, class_of: &mut HashMap<String, String>) {
    watch.clear();
    team_wallets.clear();
    class_of.clear();
    for t in nj["teams"].as_array().into_iter().flatten() {
        let root = t["root"].as_str().unwrap_or("").to_string();
        class_of.insert(root.clone(), t["class"].as_str().unwrap_or("A_TIM").to_string());
        let mut ws: Vec<String> = t["devs"].as_array().into_iter().flatten().filter_map(|d| d["w"].as_str().map(String::from)).collect();
        ws.extend(t["watch"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(String::from)));
        ws.push(root.clone());
        for w in &ws {
            watch.insert(w.clone(), root.clone());
        }
        team_wallets.insert(root, ws);
    }
}

pub fn run(watch_file: &str, out_dir: &str) -> Result<(), String> {
    let wj: Value = serde_json::from_str(&fs::read_to_string(watch_file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let (mut watch, mut team_wallets, mut class_of) = (HashMap::new(), HashMap::new(), HashMap::new());
    load_watch(&wj, &mut watch, &mut team_wallets, &mut class_of);
    let mode = std::env::var("JAR_EXEC_MODE").unwrap_or_else(|_| "paper".into());
    if mode != "paper" {
        return Err("only JAR_EXEC_MODE=paper exists; live trading is not implemented".into());
    }
    let mut bank_usd = cfg_u64("config/budget.toml", "paper_bank_usd_cents").unwrap_or(1400) as f64 / 100.0;
    let sol_usd = cfg_u64("config/budget.toml", "paper_sol_usd_cents").unwrap_or(7108) as f64 / 100.0;
    let kelly_q = cfg_u64("config/score.toml", "quarter_kelly_bp").unwrap_or(2250) as f64 / 10_000.0;
    let tip_sol = 0.00146;
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let open_f = |n: &str| OpenOptions::new().create(true).append(true).open(format!("{out_dir}/{n}"));
    let mut sig_f = open_f("signals.jsonl").map_err(|e| e.to_string())?;
    let mut all_f = open_f("creates.jsonl").map_err(|e| e.to_string())?;
    let mut ord_f = open_f("orders.jsonl").map_err(|e| e.to_string())?;
    let mut gap_f = open_f("gaps.jsonl").map_err(|e| e.to_string())?;
    let mut rpc = Rpc::new("shadow")?;
    println!("mode=paper watchlist wallets={} teams={} bank=${bank_usd}", watch.len(), team_wallets.len());
    let mut recent: Vec<Value> = Vec::new();
    let (mut seen_n, mut sig_n, mut closed_n, mut wins) = (0u64, 0u64, 0u64, 0u64);
    let mut open: Vec<Pos> = Vec::new();
    let watch_mtime = |p: &str| fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut wm = watch_mtime(watch_file);
    let mut last: Option<String> = None;
    loop {
        let t0 = SystemTime::now();
        if watch_mtime(watch_file) != wm {
            if let Ok(nj) = fs::read_to_string(watch_file).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string())) {
                load_watch(&nj, &mut watch, &mut team_wallets, &mut class_of);
                println!("watchlist reloaded: {} wallets", watch.len());
            }
            wm = watch_mtime(watch_file);
        }
        let first_round = last.is_none();
        let sigs = new_sigs(&mut rpc, MINT_AUTH, last.as_deref(), 100, &mut gap_f);
        if let Some(s) = sigs.last().and_then(|s| s["signature"].as_str()) {
            last = Some(s.to_string());
        }
        if !first_round {
            for s in sigs.iter().filter(|s| s["err"].is_null()) {
                let sig = s["signature"].as_str().unwrap_or("");
                let Some(tx) = get_tx(&mut rpc, sig, &mut gap_f, MINT_AUTH, &s["slot"]) else { continue };
                let bt = tx["blockTime"].as_i64().unwrap_or(0);
                let create_slot = tx["slot"].as_u64().unwrap_or(0);
                let evs = decode::events(&tx).0;
                for e in &evs {
                    let Ev::Create(c) = e else { continue };
                    let creator = c.creator.clone().unwrap_or(c.user.clone());
                    let team = watch.get(&creator).or_else(|| watch.get(&c.user)).cloned();
                    let row = json!({"t_seen": now(), "block_time": bt, "latency_s": now() - bt as f64, "slot": create_slot, "sig": sig, "mint": c.mint,
                        "symbol": c.symbol, "creator": creator, "user": c.user, "team": team});
                    writeln!(all_f, "{row}").map_err(|e| e.to_string())?;
                    seen_n += 1;
                    let Some(team) = team else { continue };
                    let detect_slot = rpc.call("getSlot", json!([{"commitment": "processed"}]), 1).ok().and_then(|v| v.as_u64());
                    let mut row = row;
                    row["detect_slot"] = json!(detect_slot);
                    row["create_slot"] = json!(create_slot);
                    writeln!(sig_f, "{row}").map_err(|e| e.to_string())?;
                    sig_n += 1;
                    recent.push(row.clone());
                    if recent.len() > 100 {
                        recent.remove(0);
                    }
                    let cls = class_of.get(&team).cloned().unwrap_or_default();
                    if !cls.starts_with('A') {
                        writeln!(ord_f, "{}", json!({"t": now(), "action": "SIGNAL_ONLY", "class": cls, "mint": c.mint, "symbol": c.symbol, "team": team,
                            "create_slot": create_slot, "detect_slot": detect_slot})).map_err(|e| e.to_string())?;
                        continue;
                    }
                    let size_usd = (bank_usd * kelly_q).min(sol_usd);
                    if size_usd < 0.5 {
                        writeln!(ord_f, "{}", json!({"t": now(), "action": "SKIP_SIZE", "mint": c.mint, "bank_usd": bank_usd})).map_err(|e| e.to_string())?;
                        continue;
                    }
                    // reference point BEFORE the buy: every curve trade after this signature is scanned for family sells
                    let ref_sig = rpc.call("getSignaturesForAddress", json!([c.curve, {"limit": 1}]), 1).ok()
                        .and_then(|v| v[0]["signature"].as_str().map(String::from));
                    let lamports = (size_usd / sol_usd * 1e9) as u64;
                    let Some(cv) = curve_state(&mut rpc, &c.curve) else {
                        writeln!(ord_f, "{}", json!({"t": now(), "action": "SKIP_NO_STATE", "mint": c.mint})).map_err(|e| e.to_string())?;
                        continue;
                    };
                    if cv.complete {
                        // graduated before we could act (same-slot bundle graduation): measured, not bought on the dead curve
                        writeln!(ord_f, "{}", json!({"t": now(), "action": "MISSED_GRADUATED", "mint": c.mint, "create_slot": create_slot, "detect_slot": detect_slot})).map_err(|e| e.to_string())?;
                        continue;
                    }
                    let tokens = buy_out(cv.vsol, cv.vtok, lamports, 125);
                    let entry_mcap = cv.vsol as f64 / cv.vtok as f64 * 1e6 * sol_usd;
                    let mut family: HashSet<String> = team_wallets.get(&team).cloned().unwrap_or_default().into_iter().collect();
                    family.insert(creator.clone());
                    family.insert(c.user.clone());
                    for ev in &evs {
                        if let Ev::Trade(t) = ev {
                            if t.mint == c.mint {
                                family.insert(t.user.clone());
                            }
                        }
                    }
                    bank_usd -= size_usd + tip_sol * sol_usd;
                    let ord = json!({"t": now(), "action": "BUY_PAPER", "mint": c.mint, "symbol": c.symbol, "team": team, "size_usd": size_usd,
                        "tip_sol": tip_sol, "entry_mcap_usd": entry_mcap, "vsol": cv.vsol, "vtok": cv.vtok, "latency_s": now() - bt as f64,
                        "create_slot": create_slot, "detect_slot": detect_slot, "family": family.len(), "bank_usd": bank_usd});
                    writeln!(ord_f, "{ord}").map_err(|e| e.to_string())?;
                    println!("BUY_PAPER {} ${:.2} mcap ${:.0} latency {:.1}s", c.symbol, size_usd, entry_mcap, now() - bt as f64);
                    open.push(Pos { mint: c.mint.clone(), curve: c.curve.clone(), symbol: c.symbol.clone(), team, family, lamports, tokens,
                        entry_t: now(), last_sig: ref_sig, create_slot, detect_slot });
                }
            }
        }
        // ---- exits: dev-family sell, graduation (pool value), or 24 h (any state) ----
        let mut keep: Vec<Pos> = Vec::new();
        for mut p in open.drain(..) {
            let mut reason: Option<&str> = None;
            let a = new_sigs(&mut rpc, &p.curve, p.last_sig.as_deref(), 50, &mut gap_f);
            for s in a.iter() {
                p.last_sig = s["signature"].as_str().map(String::from).or(p.last_sig.take());
                if !s["err"].is_null() {
                    continue;
                }
                let Some(tx) = get_tx(&mut rpc, s["signature"].as_str().unwrap_or(""), &mut gap_f, &p.curve, &s["slot"]) else { continue };
                if decode::events(&tx).0.iter().any(|e| matches!(e, Ev::Trade(t) if !t.buy && t.mint == p.mint && p.family.contains(&t.user))) {
                    reason = Some("DEV_FAMILY_SELL");
                    break;
                }
            }
            let state = curve_state(&mut rpc, &p.curve);
            let graduated = state.map(|s| s.complete).unwrap_or(false);
            let cost_usd = p.lamports as f64 / 1e9 * sol_usd + tip_sol * sol_usd;
            // value: curve while trading, PumpSwap pool after graduation
            let mut migrate_slot: Option<u64> = None;
            let value_l: Option<u64> = if graduated {
                pool_state(&mut rpc, &p.curve, &mut gap_f).map(|(_, pb, pq, ms)| {
                    migrate_slot = Some(ms);
                    pool_sell(pb, pq, p.tokens, POOL_FEE_BPS)
                })
            } else {
                state.map(|s| sell_out(s.vsol, s.vtok, p.tokens, 125))
            };
            if reason.is_none() && graduated {
                reason = Some("GRADUATED");
            }
            let timed_out = now() - p.entry_t > TIMEOUT_S;
            let (why, got_l) = match (reason, value_l) {
                (Some(w), Some(v)) => (w, v),
                (_, None) if timed_out => ("EXIT_UNKNOWN_STATE", 0),
                (None, Some(v)) if timed_out => ("TIME_24H", v),
                _ => {
                    keep.push(p);
                    continue;
                }
            };
            let got_usd = got_l as f64 / 1e9 * sol_usd;
            bank_usd += got_usd;
            closed_n += 1;
            wins += (got_usd > cost_usd) as u64;
            let ord = json!({"t": now(), "action": "SELL_PAPER", "reason": why, "mint": p.mint, "symbol": p.symbol, "team": p.team,
                "ret": got_usd / cost_usd - 1.0, "held_s": now() - p.entry_t, "bank_usd": bank_usd,
                "create_slot": p.create_slot, "detect_slot": p.detect_slot, "migrate_slot": migrate_slot});
            writeln!(ord_f, "{ord}").map_err(|e| e.to_string())?;
            println!("SELL_PAPER {} {} ret {:+.1}% bank ${:.2}", p.symbol, why, 100.0 * (got_usd / cost_usd - 1.0), bank_usd);
        }
        open = keep;
        let status = json!({"updated": now() as u64, "mode": "paper", "teams": team_wallets.len(), "watch_wallets": watch.len(), "creates_seen": seen_n,
            "signals_total": sig_n, "open_positions": open.len(), "closed": closed_n, "wins": wins, "bank_usd": bank_usd, "signals": recent});
        let _ = fs::write("reports/web/signals.tmp", status.to_string()).and_then(|_| fs::rename("reports/web/signals.tmp", "reports/web/signals.json"));
        rpc.save();
        let el = t0.elapsed().unwrap_or(Duration::ZERO);
        if el < Duration::from_secs(1) {
            sleep(Duration::from_secs(1) - el);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve_bytes(vtok: u64, vsol: u64, complete: bool) -> Vec<u8> {
        let mut d = vec![0u8; 8];
        for v in [vtok, vsol, 793_100_000_000_000u64, 0, 1_000_000_000_000_000] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.push(complete as u8);
        d.extend_from_slice(&[0u8; 32]);
        d
    }

    #[test]
    fn curve_fields_not_swapped() {
        // fresh coin: virtual token 1.073e15, virtual SOL 30 SOL
        let c = parse_curve(&curve_bytes(1_073_000_000_000_000, 30_000_000_000, false)).unwrap();
        assert_eq!(c.vtok, 1_073_000_000_000_000);
        assert_eq!(c.vsol, 30_000_000_000);
        assert!(!c.complete);
        let mcap = c.vsol as f64 / c.vtok as f64 * 1e6 * 71.08;
        assert!((1_000.0..100_000.0).contains(&mcap), "mcap {mcap}");
        // swapped inputs would give an absurd market cap
        let bad = c.vtok as f64 / c.vsol as f64 * 1e6 * 71.08;
        assert!(bad > 1e9);
    }

    #[test]
    fn golden_real_account_if_present() {
        // tests/data/curve_*.b64 = base64 of a real BondingCurve account (getAccountInfo); saved when RPC is available
        let Ok(rd) = fs::read_dir("tests/data") else { return };
        for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("curve_")) {
            let s = fs::read_to_string(e.path()).unwrap();
            let d = base64::engine::general_purpose::STANDARD.decode(s.trim()).unwrap();
            let c = parse_curve(&d).unwrap();
            assert!(c.vtok > 1e14 as u64 && c.vtok <= 1_073_000_000_000_000, "vtok {}", c.vtok);
            assert!(c.vsol >= 30_000_000_000 && c.vsol < 1_000_000_000_000, "vsol {}", c.vsol);
        }
    }

    #[test]
    fn pool_sell_drained_pool_is_near_zero() {
        let tok = 3_000_000_000_000; // 0.3% of supply
        let healthy = pool_sell(200_000_000_000_000, 85_000_000_000, tok, 125);
        let drained = pool_sell(900_000_000_000_000, 500_000_000, tok, 125);
        assert!(healthy > 1_000_000_000);
        assert!(drained < 2_000_000);
    }
}
