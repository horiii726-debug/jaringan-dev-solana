//! Coin categories (labels v1.2, frozen 16 Nov before results) and their profile:
//! MOONSHOT  peak >= moon_creation_mult x creation price, or graduated
//! SEPI      fewer than 5 trades in the horizon
//! RUGPULL   creator / create signer sells within 60 s and the coin never reached 2x creation price
//! NAIK      peak 2x..moon_mult x creation price
//! TURUN     everything else (never doubled)
//! Profile = medians of facts known at creation (create tx) and in the first 10 s, per category.
use crate::graph::read_lines;
use crate::outcomes::{parse_t, T};
use crate::rpc::cfg_u64;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;

fn med(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

pub fn run(launch_files: &[String], trade_files: &[String], out_labels: &str) -> Result<(), String> {
    let mult = cfg_u64("config/labels.toml", "moon_creation_mult").ok_or("moon_creation_mult")? as f64;
    let mut ls: HashMap<String, Value> = HashMap::new();
    for p in launch_files {
        for v in read_lines(p) {
            if v["kind"] == "create" {
                ls.insert(v["mint"].as_str().unwrap_or("").into(), v);
            }
        }
    }
    let names = ["dev_beli_sol", "bundle_sol", "bundle_wallet", "pembeli_10dtk", "beli_10dtk", "jual_10dtk", "sol_masuk_10dtk", "dev_jual_detik", "trade_30mnt", "pembeli_30mnt", "puncak_x"];
    let mut prof: BTreeMap<&str, Vec<Vec<f64>>> = BTreeMap::new();
    let mut f = fs::File::create(out_labels).map_err(|e| e.to_string())?;
    writeln!(f, "mint\tcat").map_err(|e| e.to_string())?;
    for p in trade_files {
        for v in read_lines(p) {
            let m = v["mint"].as_str().unwrap_or("");
            let Some(l) = ls.get(m) else { continue };
            if l["mayhem"] == true || !l["quote"].as_str().unwrap_or("").starts_with("1111") || v.get("error").is_some() {
                continue;
            }
            let mut tr: Vec<T> = v["t"].as_array().map(|a| a.iter().filter_map(parse_t).collect()).unwrap_or_default();
            tr.sort_by_key(|t| (t.slot, t.txi));
            let cbt = l["bt"].as_i64().unwrap_or(0);
            let (c, u) = (l["creator"].as_str().unwrap_or(""), l["user"].as_str().unwrap_or(""));
            let peak = tr.iter().map(|t| t.vsol as f64 / t.vtok as f64).fold(0.0, f64::max) / (30e9 / 1.073e15);
            let grad = !v["graduated_slot"].is_null();
            let dev_sell = tr.iter().find(|t| !t.buy && (t.user == c || t.user == u)).map(|t| (t.bt - cbt) as f64);
            let cat = if peak >= mult || grad {
                "MOONSHOT"
            } else if tr.len() < 5 {
                "SEPI"
            } else if dev_sell.map(|s| s <= 60.0).unwrap_or(false) && peak < 2.0 {
                "RUGPULL"
            } else if peak >= 2.0 {
                "NAIK"
            } else {
                "TURUN"
            };
            writeln!(f, "{m}\t{cat}").map_err(|e| e.to_string())?;
            let early: Vec<&T> = tr.iter().filter(|t| t.bt - cbt <= 10).collect();
            let mut eb: Vec<&str> = early.iter().filter(|t| t.buy).map(|t| t.user.as_str()).collect();
            eb.sort_unstable();
            eb.dedup();
            let mut all_b: Vec<&str> = tr.iter().filter(|t| t.buy).map(|t| t.user.as_str()).collect();
            all_b.sort_unstable();
            all_b.dedup();
            let row = vec![
                l["own_buy_sol"].as_f64().unwrap_or(0.0) / 1e9,
                l["tx_buy_sol"].as_f64().unwrap_or(0.0) / 1e9,
                l["tx_buyers"].as_array().map(|a| a.len()).unwrap_or(0) as f64,
                eb.len() as f64,
                early.iter().filter(|t| t.buy).count() as f64,
                early.iter().filter(|t| !t.buy).count() as f64,
                early.iter().map(|t| if t.buy { t.sol as f64 } else { -(t.sol as f64) }).sum::<f64>() / 1e9,
                dev_sell.unwrap_or(f64::NAN),
                tr.len() as f64,
                all_b.len() as f64,
                peak,
            ];
            prof.entry(cat).or_default().push(row);
        }
    }
    let total: usize = prof.values().map(|v| v.len()).sum();
    let mut cats = Vec::new();
    for (cat, rows) in &prof {
        let mut med_row = serde_json::Map::new();
        for (j, n) in names.iter().enumerate() {
            let mut col: Vec<f64> = rows.iter().map(|r| r[j]).filter(|x| !x.is_nan()).collect();
            let m = med(&mut col);
            med_row.insert(n.to_string(), if m.is_nan() { Value::Null } else { json!((m * 100.0).round() / 100.0) });
        }
        // share of coins where the dev sold at all
        let sold = rows.iter().filter(|r| !r[7].is_nan()).count() as f64 / rows.len() as f64;
        med_row.insert("dev_pernah_jual_pct".into(), json!((sold * 1000.0).round() / 10.0));
        cats.push(json!({"cat": cat, "n": rows.len(), "pct": (rows.len() as f64 * 1000.0 / total.max(1) as f64).round() / 10.0, "median": med_row}));
    }
    let out = json!({"total": total, "fields": names, "cats": cats});
    fs::write("reports/web/cats.json", out.to_string()).map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string(&out["cats"].as_array().map(|a| a.iter().map(|c| format!("{}={} ({}%)", c["cat"], c["n"], c["pct"])).collect::<Vec<_>>()).unwrap_or_default()).unwrap_or_default());
    Ok(())
}
