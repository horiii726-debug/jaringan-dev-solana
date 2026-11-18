//! P2.3: buyer-side outcomes per coin and entry delay, from curve trades (labels v1, config/labels.toml).
//! Entry = curve state after all trades up to create_slot + d (d=0: same block, after every trade in the create
//! slot; "0o": right after the create tx). Never the creation price. Exit = first creator/create-signer sell
//! (+react slots), time stop, graduation (censored) or data end (censored).
use crate::graph::read_lines;
use crate::rpc::cfg_u64;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;

#[derive(Clone)]
pub struct T {
    pub slot: u64,
    pub txi: u64,
    pub bt: i64,
    pub buy: bool,
    #[allow(dead_code)]
    pub sol: u64,
    pub user: String,
    pub vsol: u64,
    pub vtok: u64,
    pub fee_bps: u64,
}

pub fn parse_t(a: &Value) -> Option<T> {
    Some(T {
        slot: a[0].as_u64()?,
        txi: a[1].as_u64()?,
        bt: a[3].as_i64()?,
        buy: a[4].as_u64()? == 1,
        sol: a[5].as_u64()?,
        user: a[7].as_str()?.to_string(),
        vsol: a[8].as_u64()?,
        vtok: a[9].as_u64()?,
        fee_bps: a[10].as_u64()?,
    })
}

/// Tokens out for `x` lamports in, constant product after fee.
pub fn buy_out(vsol: u64, vtok: u64, x: u64, fee_bps: u64) -> u64 {
    let eff = x as u128 * (10_000 - fee_bps.min(10_000)) as u128 / 10_000;
    let k = vsol as u128 * vtok as u128;
    let nv = vsol as u128 + eff;
    (vtok as u128 - k.div_ceil(nv)) as u64
}

/// Lamports out (after fee) for selling `tok` tokens.
pub fn sell_out(vsol: u64, vtok: u64, tok: u64, fee_bps: u64) -> u64 {
    let k = vsol as u128 * vtok as u128;
    let gross = vsol as u128 - k.div_ceil(vtok as u128 + tok as u128);
    (gross * (10_000 - fee_bps.min(10_000)) as u128 / 10_000) as u64
}

pub struct Coin<'a> {
    pub slot: u64,
    pub txi: u64,
    pub bt: i64,
    pub vsol: u64,
    pub vtok: u64,
    pub creator: &'a str,
    pub user: &'a str,
}

pub struct Out {
    pub filled: bool,
    pub reason: &'static str,
    pub ret: f64,
    pub ret2: f64,
    pub peak: f64,
    pub entry_mult: f64,
    pub hold_s: i64,
}

pub struct Cfg {
    pub x: u64,
    pub txc: u64,
    pub react: u64,
    pub tstop: i64,
}

/// `d` = None for "0o", Some(n) for create_slot + n.
#[allow(clippy::too_many_arguments)]
pub fn simulate(c: &Coin, tr: &[T], d: Option<u64>, grad_slot: Option<u64>, complete: bool, last_bt: i64, cfg: &Cfg, extra: usize) -> Out {
    let in_entry = |t: &T| match d {
        None => t.slot < c.slot || (t.slot == c.slot && t.txi <= c.txi),
        Some(n) => t.slot <= c.slot + n,
    };
    let mut ne = tr.iter().take_while(|t| in_entry(t)).count();
    // "Jito" variants: we land after `extra` more trades of the create slot
    let mut add = 0;
    while add < extra && ne < tr.len() && tr[ne].slot == c.slot {
        ne += 1;
        add += 1;
    }
    let (mut vs, mut vt) = (c.vsol, c.vtok);
    let mut fee = tr.first().map(|t| t.fee_bps).unwrap_or(125);
    if ne > 0 {
        vs = tr[ne - 1].vsol;
        vt = tr[ne - 1].vtok;
        fee = tr[ne - 1].fee_bps;
    }
    let entry_slot = match d {
        None => c.slot,
        Some(n) => c.slot + n,
    };
    let none = Out { filled: false, reason: "NOT_FILLED", ret: 0.0, ret2: 0.0, peak: 0.0, entry_mult: 0.0, hold_s: 0 };
    if grad_slot.map(|g| g <= entry_slot).unwrap_or(false) || vt == 0 {
        return none;
    }
    let entry_bt = tr.get(ne.saturating_sub(1)).map(|t| t.bt).unwrap_or(c.bt).max(c.bt);
    let tok = buy_out(vs, vt, cfg.x, fee);
    let cost = (cfg.x + cfg.txc) as f64;
    let cost2 = (cfg.x + 2 * cfg.txc) as f64;
    let entry_mult = (vs as f64 / vt as f64) / (c.vsol as f64 / c.vtok as f64);
    let after = &tr[ne..];
    // exit trigger: first sell by creator or create signer (any time from create on)
    let trig = tr.iter().find(|t| !t.buy && (t.user == c.creator || t.user == c.user)).map(|t| t.slot.max(entry_slot) + cfg.react);
    let stop_bt = entry_bt + cfg.tstop;
    let mut exit_state = (vs, vt, fee, entry_bt);
    let mut reason = "TIME_STOP";
    let mut peak = 0u64;
    let mut decided = false;
    for t in after {
        if grad_slot.map(|g| t.slot >= g).unwrap_or(false) {
            reason = "GRADUATED_CENSORED";
            decided = true;
            break;
        }
        if trig.map(|s| t.slot > s).unwrap_or(false) {
            reason = "CREATOR_SELL";
            decided = true;
            break;
        }
        if t.bt > stop_bt {
            decided = true;
            break;
        }
        exit_state = (t.vsol, t.vtok, t.fee_bps, t.bt);
        peak = peak.max(sell_out(t.vsol, t.vtok, tok, t.fee_bps));
    }
    if !decided {
        if grad_slot.is_some() {
            reason = "GRADUATED_CENSORED";
        } else if trig.is_some() && !complete {
            reason = "DATA_CAP_CENSORED";
        } else if trig.is_some() {
            reason = "CREATOR_SELL";
        } else if !complete && last_bt < stop_bt {
            reason = "DATA_CAP_CENSORED";
        }
    }
    // peak over the full horizon (label), not only until exit
    for t in after {
        if grad_slot.map(|g| t.slot >= g).unwrap_or(false) {
            break;
        }
        peak = peak.max(sell_out(t.vsol, t.vtok, tok, t.fee_bps));
    }
    let (es, et, ef, ebt) = exit_state;
    let out = sell_out(es, et, tok, ef);
    // costs doubled: both fees and both tx costs x2
    let entry_tok2 = buy_out(vs, vt, cfg.x, (fee * 2).min(10_000));
    let out2 = sell_out(es, et, entry_tok2, (ef * 2).min(10_000));
    Out {
        filled: true,
        reason,
        ret: (out as f64 - cfg.txc as f64 - cost) / cost,
        ret2: (out2 as f64 - 2.0 * cfg.txc as f64 - cost2) / cost2,
        peak: (peak as f64 - cfg.txc as f64) / cost,
        entry_mult,
        hold_s: ebt - entry_bt,
    }
}

pub fn run(launch_files: &[String], trade_files: &[String], out_path: &str) -> Result<(), String> {
    let cfg = Cfg {
        x: cfg_u64("config/labels.toml", "buy_lamports").ok_or("buy_lamports")?,
        txc: cfg_u64("config/labels.toml", "tx_cost_lamports").ok_or("tx_cost_lamports")?,
        react: cfg_u64("config/labels.toml", "creator_sell_react_slots").ok_or("react")?,
        tstop: cfg_u64("config/labels.toml", "time_stop_s").ok_or("time_stop_s")? as i64,
    };
    let mut ls: HashMap<String, Value> = HashMap::new();
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] == "create" {
                ls.insert(v["mint"].as_str().unwrap_or("").to_string(), v);
            }
        }
    }
    let delays: [(Option<u64>, usize, &str); 11] = [(None, 0, "ub_0o"), (None, 1, "ub_j1"), (None, 2, "ub_j2"), (None, 3, "ub_j3"), (None, 5, "ub_j5"), (Some(0), 0, "ub_0"), (Some(1), 0, "1"), (Some(2), 0, "2"), (Some(3), 0, "3"), (Some(5), 0, "5"), (Some(13), 0, "13")];
    let mut f = fs::File::create(out_path).map_err(|e| e.to_string())?;
    writeln!(f, "mint,creator,user,day,slot,bt,mayhem,n_trades,d,filled,reason,ret,ret2,peak,entry_mult,hold_s").map_err(|e| e.to_string())?;
    let (mut coins, mut missing) = (0u64, 0u64);
    for p in trade_files {
        for v in read_lines(p) {
            if v.get("error").is_some() {
                continue;
            }
            let m = v["mint"].as_str().unwrap_or("");
            let Some(l) = ls.get(m) else { missing += 1; continue };
            let mut tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).collect()).unwrap_or_default();
            tr.sort_by_key(|t| (t.slot, t.txi));
            let c = Coin {
                slot: l["slot"].as_u64().unwrap_or(0),
                txi: l["txi"].as_u64().unwrap_or(0),
                bt: l["bt"].as_i64().unwrap_or(0),
                vsol: l["vsol"].as_u64().unwrap_or(30_000_000_000),
                vtok: l["vtok"].as_u64().unwrap_or(1_073_000_000_000_000),
                creator: l["creator"].as_str().unwrap_or(""),
                user: l["user"].as_str().unwrap_or(""),
            };
            let day = day_of(c.bt);
            coins += 1;
            for (d, extra, name) in delays {
                let o = simulate(&c, &tr, d, v["graduated_slot"].as_u64(), v["complete_to_horizon"] == true, v["last_bt"].as_i64().unwrap_or(0), &cfg, extra);
                writeln!(f, "{m},{},{},{day},{},{},{},{},{name},{},{},{:.5},{:.5},{:.5},{:.4},{}", c.creator, c.user, c.slot, c.bt,
                    (l["mayhem"] == true) as u8, tr.len(), o.filled as u8, o.reason, o.ret, o.ret2, o.peak, o.entry_mult, o.hold_s).map_err(|e| e.to_string())?;
            }
        }
    }
    println!("coins={coins} missing_launch={missing} -> {out_path}");
    Ok(())
}

pub fn day_of(bt: i64) -> String {
    // civil date from unix seconds (UTC)
    let z = bt.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(slot: u64, buy: bool, user: &str, vsol: u64, vtok: u64, bt: i64) -> T {
        T { slot, txi: 0, bt, buy, sol: 0, user: user.into(), vsol, vtok, fee_bps: 0 }
    }
    #[test]
    fn curve_math_roundtrip_loses_nothing_without_fee() {
        let tok = buy_out(30_000_000_000, 1_073_000_000_000_000, 1_000_000_000, 0);
        let back = sell_out(31_000_000_000, 1_073_000_000_000_000 - tok, tok, 0);
        assert!((1_000_000_000 - back as i64).abs() <= 2);
    }
    #[test]
    fn day_and_entry_never_creation_price() {
        assert_eq!(day_of(1794744000), "2026-11-15");
        let c = Coin { slot: 10, txi: 5, bt: 100, vsol: 30, vtok: 1000, creator: "C", user: "C" };
        let tr = vec![t(10, true, "X", 60, 500, 100), t(12, true, "Y", 120, 250, 101), t(20, false, "C", 30, 1000, 110)];
        let cfg = Cfg { x: 0, txc: 0, react: 1, tstop: 1800 };
        let o = simulate(&c, &tr, Some(0), None, true, 110, &cfg, 0);
        assert!((o.entry_mult - 4.0).abs() < 1e-9, "entry after same-slot buy, 4x creation price");
        assert_eq!(o.reason, "CREATOR_SELL");
    }
    #[test]
    fn truncation_does_not_change_entry() {
        // Entry state must depend only on trades up to the entry slot.
        let c = Coin { slot: 10, txi: 0, bt: 100, vsol: 30, vtok: 1000, creator: "C", user: "C" };
        let full = vec![t(10, true, "X", 60, 500, 100), t(11, true, "Y", 120, 250, 101), t(30, false, "Z", 30, 1000, 120)];
        let cfg = Cfg { x: 0, txc: 0, react: 1, tstop: 1800 };
        let a = simulate(&c, &full, Some(1), None, true, 120, &cfg, 0);
        let b = simulate(&c, &full[..2], Some(1), None, true, 120, &cfg, 0);
        assert_eq!(a.entry_mult, b.entry_mult);
    }
}
