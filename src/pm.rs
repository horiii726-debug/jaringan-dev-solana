//! Loss post-mortem (brief section 11): one primary reason code per losing filled trade, by rule, in table order.
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader};

fn lines(p: &str) -> Vec<String> {
    fs::File::open(p).map(|f| BufReader::new(f).lines().map_while(Result::ok).skip(1).collect()).unwrap_or_default()
}

/// outcomes csv: mint,creator,user,day,slot,bt,mayhem,n_trades,d,filled,reason,ret,ret2,peak,entry_mult,hold_s
pub fn code(reason: &str, n_trades: u64, entry_mult: f64, peak: f64, ret: f64, ret_nocost: f64) -> &'static str {
    if reason == "CREATOR_SELL" {
        "CREATOR_DUMP"
    } else if n_trades < 5 {
        "NO_FOLLOW"
    } else if entry_mult >= 3.0 {
        "LATE_ENTRY"
    } else if peak >= 1.0 {
        "EXIT_LATE"
    } else if ret < 0.0 && ret_nocost > 0.0 {
        "COSTS"
    } else {
        "OTHER"
    }
}

type Codes = BTreeMap<&'static str, (u64, f64)>;

pub fn run(outcomes_csv: &str, scored_csv: &str, delay: &str, tag: &str) -> Result<(), String> {
    let lam = crate::rpc::cfg_u64("config/labels.toml", "buy_lamports").ok_or("buy_lamports")? as f64 / 1e9;
    let txc = crate::rpc::cfg_u64("config/labels.toml", "tx_cost_lamports").ok_or("tx_cost")? as f64 / 1e9;
    let mut state: HashMap<String, String> = HashMap::new();
    for l in lines(scored_csv) {
        let c: Vec<&str> = l.split(',').collect();
        if c.len() > 3 {
            state.insert(c[0].into(), c[3].into());
        }
    }
    // (day, group) -> code -> (count, sol lost)
    let mut t: BTreeMap<(String, String), Codes> = BTreeMap::new();
    for l in lines(outcomes_csv) {
        let c: Vec<&str> = l.split(',').collect();
        if c.len() < 16 || c[8] != delay || c[9] != "1" {
            continue;
        }
        let Some(st) = state.get(c[0]) else { continue };
        let ret: f64 = c[11].parse().unwrap_or(0.0);
        if ret >= 0.0 {
            continue;
        }
        // ret without tx costs (fees still in): add back both tx costs relative to cost
        let ret_nc = ret + 2.0 * txc / (lam + txc);
        let k = code(c[10], c[7].parse().unwrap_or(0), c[14].parse().unwrap_or(0.0), c[13].parse().unwrap_or(0.0), ret, ret_nc);
        for g in ["ALL", st.as_str()] {
            let e = t.entry((c[3].to_string(), g.to_string())).or_default().entry(k).or_default();
            e.0 += 1;
            e.1 += -ret * (lam + txc);
        }
    }
    let mut md = format!("# Loss post-mortem {tag}, delay {delay}\n\nCount and SOL lost (at {lam} SOL per trade) per primary code.\n\n| day | group | code | losing trades | SOL lost |\n|---|---|---|---|---|\n");
    for ((d, g), m) in &t {
        let mut v: Vec<_> = m.iter().collect();
        v.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
        for (k, (n, s)) in v {
            md.push_str(&format!("| {d} | {g} | {k} | {n} | {s:.3} |\n"));
        }
    }
    fs::create_dir_all("reports/postmortem").map_err(|e| e.to_string())?;
    let p = format!("reports/postmortem/{tag}_d{delay}.md");
    fs::write(&p, md).map_err(|e| e.to_string())?;
    println!("-> {p}");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn order_of_codes() {
        assert_eq!(super::code("CREATOR_SELL", 1, 9.0, 3.0, -0.5, -0.4), "CREATOR_DUMP");
        assert_eq!(super::code("TIME_STOP", 2, 9.0, 3.0, -0.5, -0.4), "NO_FOLLOW");
        assert_eq!(super::code("TIME_STOP", 50, 1.0, 1.2, -0.5, -0.4), "EXIT_LATE");
        assert_eq!(super::code("TIME_STOP", 50, 1.0, 0.9, -0.01, 0.01), "COSTS");
    }
}
