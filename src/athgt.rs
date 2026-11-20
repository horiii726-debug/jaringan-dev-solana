//! ATH market cap per graduated Pump.fun coin from its PumpSwap pool trades (Helius getTransactionsForAddress on the
//! pool, oldest first from graduation, JAR_ATH_PAGES x 100 txs, 10 credits per page). mcap (SOL) = pool quote / pool
//! base reserves of each Buy/Sell event x 1e9 supply (amm.rs layout). USD = x SOL/USD close of that day (GeckoTerminal
//! SOL/USDC daily candles, 1 call per pass). The bonding curve ends at the graduation cap (410.9 SOL, ~$28k at SOL $67),
//! so the pool high is the coin's ATH. complete = false: the page cap was hit, the ATH is a lower bound.
//! (GeckoTerminal per-coin candles were tried first on 21 Nov: the public API answers ~1 call/min, too slow.)
//! Usage: JAR_RPS=4 jaringan athgt <universe_dir> <dev_dir> <out_dir>   (loops; resumes from <out>/ath.jsonl)
//! Scope: graduated coins of creators with >= 1 coin on DexScreener (owner 21 Nov); DexScreener coins first, newest first.

use crate::amm::{amm_events, pk_at, u64_at, BUY, SELL};
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

const WSOL: &str = "So11111111111111111111111111111111111111112";
const SOL_USDC: &str = "https://api.geckoterminal.com/api/v2/networks/solana/pools/58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2/ohlcv/day?aggregate=1&limit=200&currency=usd";

fn read_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn append(path: &str, v: &Value) -> Result<(), String> {
    let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    writeln!(f, "{v}").map_err(|e| e.to_string())
}

/// SOL/USD daily close by UTC day number (unix / 86400).
fn sol_days(agent: &ureq::Agent) -> Result<BTreeMap<i64, f64>, String> {
    for attempt in 0..8u64 {
        match agent.get(SOL_USDC).call() {
            Ok(r) => {
                let v: Value = serde_json::from_str(&r.into_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                let m: BTreeMap<i64, f64> = v["data"]["attributes"]["ohlcv_list"].as_array().into_iter().flatten()
                    .filter_map(|c| Some((c.get(0)?.as_i64()? / 86_400, c.get(4)?.as_f64()?))).collect();
                if !m.is_empty() {
                    return Ok(m);
                }
            }
            Err(_) => sleep(Duration::from_secs(15 * (attempt + 1))),
        }
    }
    Err("sol price: GeckoTerminal unavailable".into())
}

/// Close of the day of `bt`, else the nearest earlier day, else the first known day.
pub fn sol_at(m: &BTreeMap<i64, f64>, bt: i64) -> Option<f64> {
    m.range(..=bt / 86_400).next_back().or_else(|| m.iter().next()).map(|x| *x.1)
}

fn i128_at(d: &[u8], o: usize) -> Option<i128> {
    d.get(o..o + 16).map(|b| i128::from_le_bytes(b.try_into().unwrap()))
}

/// Tail fields of the current pump_amm Buy/SellEvent (idl/pump_amm.json, 21 Nov): (virtual_quote_reserves, base_supply).
/// BuyEvent has a variable ix_name string at 393 (u32 length + bytes); SellEvent has a fixed layout. None = older event.
pub fn ev_tail(ev: &[u8], buy: bool) -> Option<(i128, u64)> {
    let o = if buy {
        let l = u32::from_le_bytes(ev.get(393..397)?.try_into().ok()?) as usize;
        397 + l + 32
    } else {
        384
    };
    Some((i128_at(ev, o)?, u64_at(ev, o + 17)?))
}

/// Debug (21 Nov validation): first events of a pool with every price input.
pub fn dbg(pool: &str, gbt: i64, n: usize) -> Result<(), String> {
    let mut rpc = Rpc::new("athdbg")?;
    let o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
        "sortOrder": "asc", "filters": {"status": "succeeded", "blockTime": {"gte": gbt - 5}}});
    let r = rpc.call("getTransactionsForAddress", json!([pool, o]), 10)?;
    let mut k = 0;
    for tx in r["data"].as_array().into_iter().flatten() {
        for (disc, ev) in amm_events(tx) {
            let buy = disc == BUY;
            if (!buy && disc != SELL) || pk_at(&ev, 112).as_deref() != Some(pool) {
                continue;
            }
            let (base, pb, pq, q) = (u64_at(&ev, 8).unwrap_or(0), u64_at(&ev, 40).unwrap_or(0), u64_at(&ev, 48).unwrap_or(0), u64_at(&ev, 56).unwrap_or(0));
            let tail = ev_tail(&ev, buy);
            println!("{}", json!({"bt": tx["blockTime"], "buy": buy, "len": ev.len(), "base_amt": base, "quote_amt": q, "pool_base": pb, "pool_quote": pq,
                "vq": tail.map(|t| t.0.to_string()), "base_supply": tail.map(|t| t.1),
                "mcap_sol_reserve_1e9": mcap_sol(pb, pq), "mcap_sol_exec_1e9": (base > 0).then(|| q as f64 / base as f64 * 1e6),
                "mcap_sol_reserve_vq_supply": tail.and_then(|(vq, s)| (pb > 0).then(|| (pq as f64 + vq as f64) / pb as f64 * s as f64 / 1e9))}));
            k += 1;
            if k >= n {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// mcap in SOL from pool reserves (quote lamports, base units with 6 decimals) for a 1e9 supply (no virtual reserves).
pub fn mcap_sol(pool_base: u64, pool_quote: u64) -> Option<f64> {
    (pool_base > 0).then(|| pool_quote as f64 / pool_base as f64 * 1e6)
}

/// Spot mcap in SOL of one Buy/SellEvent: (pool quote + virtual quote) / pool base x base supply (current pump_amm;
/// older events without the tail: vq 0, supply 1e9 tokens). Validated 21 Nov: 4Bvy6z6G first trade 410.8 SOL by this
/// formula vs 416.7 SOL execution price (fees + impact); without vq it read 325.8 SOL (26% low).
/// Also returns the execution mcap of the trade (quote / base x supply) when the trade is small (< 0.5% of the pool),
/// for the per-pool cross-check.
pub fn ev_mcap(ev: &[u8], buy: bool) -> Option<(f64, Option<f64>)> {
    let (base, pb, pq, q) = (u64_at(ev, 8)?, u64_at(ev, 40)?, u64_at(ev, 48)?, u64_at(ev, 56)?);
    if pb == 0 {
        return None;
    }
    let (vq, supply) = match ev_tail(ev, buy).filter(|t| t.1 > 0) {
        // sanity: supply 0.1B-10B tokens, |virtual quote| < 100k SOL; anything else is a layout the parser does not know
        Some((vq, s)) if !(100_000_000_000_000..=10_000_000_000_000_000).contains(&s) || vq.abs() > 100_000_000_000_000 => return None,
        Some(t) => t,
        None => (0, 1_000_000_000_000_000),
    };
    let spot = (pq as f64 + vq as f64) / pb as f64 * supply as f64 / 1e9;
    let exec = (base > 0 && (base as f64) < 0.005 * pb as f64 && q > 1_000_000).then(|| q as f64 / base as f64 * supply as f64 / 1e9);
    Some((spot, exec))
}

pub fn run(uni: &str, dev_dir: &str, out: &str) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    // JAR_SHARD=i/n: this process takes the mints whose byte sum % n == i and writes <out>/ath_<i>.jsonl
    let (si, sn) = std::env::var("JAR_SHARD").ok().and_then(|s| {
        let (a, b) = s.split_once('/')?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
    }).unwrap_or((0, 1));
    let path = format!("{out}/ath_{si}.jsonl");
    let cap: u64 = std::env::var("JAR_ATH_PAGES").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let mut rpc = Rpc::new(&format!("athgt{si}"))?;
    loop {
        let sol = sol_days(&agent)?;
        // done = final rows in any shard file (ath_<digits>.jsonl), so the shard count can change without redoing coins
        let shard_files: Vec<String> = fs::read_dir(out).map_err(|e| e.to_string())?.filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.strip_prefix("ath_").and_then(|r| r.strip_suffix(".jsonl")).is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())))
            .map(|n| format!("{out}/{n}")).collect();
        let done: HashSet<String> = shard_files.iter().flat_map(|f| read_jsonl(f))
            .filter(|v| v["ath_mcap_sol"].is_number() || v["error"] == "404")
            .filter_map(|v| v["mint"].as_str().map(String::from)).collect();
        let mine = |m: &str| m.bytes().map(|b| b as usize).sum::<usize>() % sn == si;
        let mut pool: HashMap<String, (String, i64)> = HashMap::new();
        // only pools quoted in SOL: 14% of graduations pair the coin with USDC or another pump token, whose reserves
        // are not lamports (21 Nov: SPONGEBOB read "$9.5M" from a SPONGEBOB/8mnJ..pump pool)
        for v in read_jsonl(&format!("{uni}/migrations.jsonl")) {
            if let (Some(m), Some(p)) = (v["mint"].as_str(), v["pool"].as_str()) {
                if v["quote"].as_str() == Some(WSOL) {
                    pool.entry(m.to_string()).or_insert((p.to_string(), v["bt"].as_i64().unwrap_or(0)));
                }
            }
        }
        // scope (owner 21 Nov: "gua mau creator coinnya ada di DexScreener"): creators with >= 1 coin seen on
        // DexScreener (snap / qualified, or devtrace on_dex); all their graduated coins are measured so their failures
        // count too. Priority: the DexScreener coins first, then the creators' other coins. JAR_ATH_SCOPE=all = every graduation.
        let ds_mints: HashSet<String> = ["snap", "qualified"].iter()
            .flat_map(|f| read_jsonl(&format!("{uni}/{f}.jsonl")))
            .filter_map(|v| v["mint"].as_str().map(String::from)).collect();
        let roles = read_jsonl(&format!("{dev_dir}/roles.jsonl"));
        let mut ds_creators: HashSet<String> = roles.iter().filter(|v| v["mint"].as_str().is_some_and(|m| ds_mints.contains(m)))
            .filter_map(|v| v["creator"].as_str().map(String::from)).collect();
        let mut scope: HashSet<String> = HashSet::new();
        for d in read_jsonl(&format!("{dev_dir}/devs.jsonl")) {
            let coins = d["coins"].as_array().cloned().unwrap_or_default();
            let dev = d["dev"].as_str().unwrap_or("").to_string();
            if ds_creators.contains(&dev) || coins.iter().any(|c| c["on_dex"] == true || c["mint"].as_str().is_some_and(|m| ds_mints.contains(m))) {
                ds_creators.insert(dev);
                scope.extend(coins.iter().filter_map(|c| c["mint"].as_str().map(String::from)));
            }
        }
        scope.extend(roles.iter().filter(|v| v["creator"].as_str().is_some_and(|c| ds_creators.contains(c)))
            .filter_map(|v| v["mint"].as_str().map(String::from)));
        let all_scope = std::env::var("JAR_ATH_SCOPE").is_ok_and(|s| s == "all");
        // JAR_ATH_ONLY=mint1,mint2: validation run on a fixed list
        let only: Option<HashSet<String>> = std::env::var("JAR_ATH_ONLY").ok().map(|s| s.split(',').map(String::from).collect());
        let mut todo: Vec<(u8, i64, String, String)> = pool.iter()
            .filter(|(m, _)| !done.contains(*m) && mine(m) && (all_scope || only.is_some() || scope.contains(*m)) && only.as_ref().is_none_or(|o| o.contains(*m)))
            .map(|(m, (p, bt))| (if ds_mints.contains(m) { 0 } else if scope.contains(m) { 1 } else { 2 }, -bt, m.clone(), p.clone())).collect();
        todo.sort();
        let (mut ok, mut miss) = (0usize, 0usize);
        for (pri, nbt, m, p) in &todo {
            let gbt = -nbt;
            let (mut token, mut pages, mut n_tx, mut complete) = (None::<String>, 0u64, 0u64, false);
            let (mut hi, mut hi_bt, mut first) = (0.0f64, 0i64, None::<f64>);
            let (mut n_buy, mut n_sell, mut chk_n, mut chk_bad) = (0u64, 0u64, 0u64, 0u64);
            // confirmed ATH: the best level the price held for 3 consecutive trades (one odd trade cannot make an ATH)
            let (mut win, mut hi_conf, mut hi_conf_bt) = (std::collections::VecDeque::<f64>::new(), 0.0f64, 0i64);
            let mut users: HashSet<String> = HashSet::new();
            let mut err = None;
            loop {
                let mut o = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed", "maxSupportedTransactionVersion": 1,
                    "sortOrder": "asc", "filters": {"status": "succeeded", "blockTime": {"gte": gbt - 5}}});
                if let Some(t) = &token {
                    o["paginationToken"] = json!(t);
                }
                let mut r = match rpc.call("getTransactionsForAddress", json!([p, o]), 10) {
                    Ok(r) => r,
                    Err(e) if e.starts_with("BUDGET") || e.starts_with("QUOTA") => return Err(e),
                    Err(e) => {
                        err = Some(e.chars().take(120).collect::<String>());
                        break;
                    }
                };
                pages += 1;
                let data = r["data"].take();
                let data = data.as_array().map(|x| x.as_slice()).unwrap_or(&[]);
                for tx in data {
                    n_tx += 1;
                    let bt = tx["blockTime"].as_i64().unwrap_or(0);
                    for (disc, ev) in amm_events(tx) {
                        if (disc != BUY && disc != SELL) || pk_at(&ev, 112).as_deref() != Some(p.as_str()) {
                            continue;
                        }
                        let buy = disc == BUY;
                        let Some((mc, exec)) = ev_mcap(&ev, buy) else { continue };
                        if buy { n_buy += 1 } else { n_sell += 1 }
                        if let Some(u) = pk_at(&ev, 144) {
                            users.insert(u);
                        }
                        // cross-check: a small trade executes within 15% of the spot price (fees ~1-2%)
                        if let Some(x) = exec {
                            chk_n += 1;
                            chk_bad += ((x / mc - 1.0).abs() > 0.15) as u64;
                        }
                        win.push_back(mc);
                        if win.len() > 3 {
                            win.pop_front();
                        }
                        if win.len() == 3 {
                            let lo = win.iter().copied().fold(f64::MAX, f64::min);
                            if lo > hi_conf {
                                (hi_conf, hi_conf_bt) = (lo, bt);
                            }
                        }
                        first.get_or_insert(mc);
                        if mc > hi {
                            (hi, hi_bt) = (mc, bt);
                        }
                    }
                }
                token = r["paginationToken"].as_str().map(String::from);
                if data.is_empty() || token.is_none() {
                    complete = true;
                    break;
                }
                if pages >= cap {
                    break;
                }
                // 21 Nov: ~half of the registry pools start at 0-3 SOL mcap (normal graduation pools start ~325 SOL);
                // one page is enough to tell, the rest of such a pool is not scanned
                if first.is_some_and(|f| f < 100.0) {
                    break;
                }
            }
            if let Some(e) = err {
                append(&path, &json!({"mint": m, "pool": p, "error": e}))?;
                continue;
            }
            let Some(first) = first else {
                append(&path, &json!({"mint": m, "pool": p, "error": "404", "note": "no pool trades", "pages": pages}))?;
                miss += 1;
                continue;
            };
            let px = sol_at(&sol, hi_bt).unwrap_or(0.0);
            let pxc = sol_at(&sol, hi_conf_bt).unwrap_or(px);
            append(&path, &json!({"mint": m, "pool": p, "bt": gbt, "pri": pri, "pages": pages, "n_tx": n_tx, "complete": complete, "formula": 3,
                "ath_spike_sol": hi, "ath_spike_bt": hi_bt, "ath_spike_usd": hi * px,
                "ath_mcap_sol": hi_conf, "ath_bt": hi_conf_bt, "ath_mcap_usd": hi_conf * pxc, "sol_usd": pxc,
                "first_mcap_sol": first, "first_open_mcap_usd": first * px,
                "n_buy": n_buy, "n_sell": n_sell, "traders": users.len(), "chk_n": chk_n, "chk_bad": chk_bad}))?;
            ok += 1;
            if (ok + miss) % 100 == 0 {
                rpc.save();
                println!("{}", json!({"todo": todo.len(), "ok": ok, "miss": miss, "credits_total": rpc.used()}));
            }
        }
        rpc.save();
        println!("{}", json!({"pass_done": true, "todo": todo.len(), "ok": ok, "miss": miss, "credits_total": rpc.used()}));
        sleep(Duration::from_secs(600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcap_and_sol_day() {
        // graduation: 85 SOL / 206.9M tokens -> ~410.8 SOL mcap
        let mc = mcap_sol(206_900_000_000_000, 85_000_000_000).unwrap();
        assert!((mc - 410.8).abs() < 0.1);
        assert_eq!(mcap_sol(0, 5), None);
        // SellEvent (fixed layout): first trade of 4Bvy6z6G (pool 67.4 SOL + virtual 17.58 SOL / 206.9M tokens, 1B supply)
        let mut ev = vec![0u8; 433];
        let put = |ev: &mut Vec<u8>, o: usize, b: &[u8]| ev[o..o + b.len()].copy_from_slice(b);
        put(&mut ev, 8, &1_000_000u64.to_le_bytes());
        put(&mut ev, 40, &206_900_000_000_000u64.to_le_bytes());
        put(&mut ev, 48, &67_405_855_195u64.to_le_bytes());
        put(&mut ev, 56, &400u64.to_le_bytes());
        put(&mut ev, 384, &17_584_505_661i128.to_le_bytes());
        put(&mut ev, 401, &1_000_000_000_000_000u64.to_le_bytes());
        let (spot, _) = ev_mcap(&ev, false).unwrap();
        assert!((spot - 410.78).abs() < 0.01, "{spot}");
        // an older event without the tail falls back to real reserves and a 1B supply
        assert!((ev_mcap(&ev[..120], false).unwrap().0 - 325.79).abs() < 0.01);
        let m: BTreeMap<i64, f64> = [(10, 100.0), (12, 120.0)].into_iter().collect();
        assert_eq!(sol_at(&m, 11 * 86_400 + 5), Some(100.0));
        assert_eq!(sol_at(&m, 12 * 86_400), Some(120.0));
        assert_eq!(sol_at(&m, 5), Some(100.0));
    }
}
