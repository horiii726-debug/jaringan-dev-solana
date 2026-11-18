//! Owner 18 Nov: ML "when we buy (< 1 s after birth), will this coin die on Pump.fun or graduate to the DEX?"
//! Every feature is known in the create transaction (no future data). Label = migrated to PumpSwap within 24 h.
//! Coins whose curve was finished inside the create tx (bought out at birth) are excluded: we cannot buy them
//! on the curve. SOL-quoted, non-mayhem coins only.
//!   gradml build <out_dir>                -> train.tsv (6-12 Nov), test.tsv (13-16 Nov), pool.cd
//!   gradml eval <test.tsv> <pred.tsv>     -> logloss vs baseline, AUC, precision at the top
use crate::graph::read_lines;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Write;

const CUT: i64 = 1_794_528_000; // 13 Nov 00:00 UTC
const END: i64 = 1_794_873_600; // 17 Nov 00:00 UTC (labels need 24 h)
const START: i64 = 1_794_009_600; // 7 Nov 00:00 UTC (6 Nov is warm-up for history features)

const NAMES: [&str; 22] = ["own_buy_sol", "tx_buy_sol", "tx_buys", "tx_sells", "tx_buyers", "after_vsol", "fee_bps", "cfee_bps",
    "cashback", "creator_is_payer", "n_signers", "token2022", "name_len", "sym_len", "sym_upper", "name_digit", "hour", "dow",
    "cr_prev", "cr_prev_grad", "payer_prev", "univ_grad_6h"];

pub fn build(out: &str) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut mig: HashMap<String, i64> = HashMap::new();
    for l in fs::read_to_string("data/roots/mig_all.tsv").map_err(|e| e.to_string())?.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if let (Some(m), Some(b)) = (p.first(), p.get(2).and_then(|x| x.parse::<i64>().ok())) {
            mig.insert(m.to_string(), b);
        }
    }
    let mut ls: Vec<serde_json::Value> = Vec::new();
    for d in ["data/raw", "data"] {
        for e in fs::read_dir(d).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("launches_2026-") && n.ends_with(".jsonl") {
                for v in read_lines(&format!("{d}/{n}")) {
                    if v["kind"] == "create" && v["mayhem"] != true && v["quote"].as_str().unwrap_or("").starts_with("1111") {
                        ls.push(v);
                    }
                }
            }
        }
    }
    ls.sort_by_key(|v| (v["slot"].as_u64().unwrap_or(0), v["ci"].as_u64().unwrap_or(0)));
    ls.dedup_by(|a, b| a["mint"] == b["mint"]);
    // graduation events in time order for "universe grad rate in the last 6 h" (only grads already happened)
    let mut grads: Vec<i64> = mig.values().copied().collect();
    grads.sort();
    let mut cr_prev: HashMap<String, Vec<String>> = HashMap::new();
    let mut payer_prev: HashMap<String, u32> = HashMap::new();
    let mut win: VecDeque<i64> = VecDeque::new(); // create times in last 6 h
    let (mut gi, mut gwin): (usize, VecDeque<i64>) = (0, VecDeque::new());
    let mut tr = fs::File::create(format!("{out}/train.tsv")).map_err(|e| e.to_string())?;
    let mut te = fs::File::create(format!("{out}/test.tsv")).map_err(|e| e.to_string())?;
    let head = format!("label\tmint\tbt\t{}\n", NAMES.join("\t"));
    tr.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    te.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    let (mut ntr, mut nte, mut ptr, mut pte, mut inst) = (0, 0, 0, 0, 0);
    for v in &ls {
        let bt = v["bt"].as_i64().unwrap_or(0);
        let mint = v["mint"].as_str().unwrap_or("").to_string();
        let creator = v["creator"].as_str().unwrap_or("").to_string();
        let payer = v["payer"].as_str().unwrap_or("").to_string();
        while win.front().map(|t| *t < bt - 21_600).unwrap_or(false) { win.pop_front(); }
        while gi < grads.len() && grads[gi] < bt { gwin.push_back(grads[gi]); gi += 1; }
        while gwin.front().map(|t| *t < bt - 21_600).unwrap_or(false) { gwin.pop_front(); }
        let univ = gwin.len() as f64 / win.len().max(1) as f64;
        let prev = cr_prev.get(&creator).cloned().unwrap_or_default();
        let prev_grad = prev.iter().filter(|m| mig.get(*m).map(|g| *g < bt).unwrap_or(false)).count();
        let pp = payer_prev.get(&payer).copied().unwrap_or(0);
        cr_prev.entry(creator.clone()).or_default().push(mint.clone());
        *payer_prev.entry(payer.clone()).or_default() += 1;
        win.push_back(bt);
        let after_vsol = v["after_vsol"].as_f64().unwrap_or(30e9) / 1e9;
        if bt < START || bt >= END { continue; }
        if after_vsol >= 114.0 { inst += 1; continue; } // bought out at birth -> not buyable on curve
        let label = mig.get(&mint).map(|g| *g - bt <= 86_400).unwrap_or(false) as u8;
        let name = v["name"].as_str().unwrap_or("");
        let sym = v["symbol"].as_str().unwrap_or("");
        let up = sym.chars().filter(|c| c.is_uppercase()).count() as f64 / sym.chars().count().max(1) as f64;
        let f = [
            v["own_buy_sol"].as_f64().unwrap_or(0.0) / 1e9, v["tx_buy_sol"].as_f64().unwrap_or(0.0) / 1e9,
            v["tx_buys"].as_f64().unwrap_or(0.0), v["tx_sells"].as_f64().unwrap_or(0.0),
            v["tx_buyers"].as_array().map(|a| a.len()).unwrap_or(0) as f64, after_vsol,
            v["fee_bps"].as_f64().unwrap_or(0.0), v["cfee_bps"].as_f64().unwrap_or(0.0),
            (v["cashback"] == true) as u8 as f64, (creator == payer) as u8 as f64,
            v["signers"].as_array().map(|a| a.len()).unwrap_or(0) as f64,
            (v["tprog"].as_str().unwrap_or("").starts_with("Tokenz")) as u8 as f64,
            name.chars().count() as f64, sym.chars().count() as f64, up,
            name.chars().any(|c| c.is_ascii_digit()) as u8 as f64,
            ((bt % 86_400) / 3600) as f64, (((bt / 86_400) + 4) % 7) as f64,
            prev.len() as f64, prev_grad as f64, pp as f64, univ,
        ];
        let row = format!("{label}\t{mint}\t{bt}\t{}\n", f.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join("\t"));
        if bt < CUT {
            tr.write_all(row.as_bytes()).map_err(|e| e.to_string())?;
            ntr += 1; ptr += label as usize;
        } else {
            te.write_all(row.as_bytes()).map_err(|e| e.to_string())?;
            nte += 1; pte += label as usize;
        }
    }
    let mut cd = String::from("0\tLabel\n1\tAuxiliary\tmint\n2\tAuxiliary\tbt\n");
    for (i, n) in NAMES.iter().enumerate() {
        cd.push_str(&format!("{}\tNum\t{n}\n", i + 3));
    }
    fs::write(format!("{out}/pool.cd"), cd).map_err(|e| e.to_string())?;
    println!("train 7-12 Nov: {ntr} koin, lulus {ptr} ({:.2}%) | test 13-16 Nov: {nte} koin, lulus {pte} ({:.2}%) | dibuang (diborong saat lahir) {inst}",
        100.0 * ptr as f64 / ntr.max(1) as f64, 100.0 * pte as f64 / nte.max(1) as f64);
    Ok(())
}

pub fn eval(test: &str, pred: &str) -> Result<(), String> {
    let labels: Vec<u8> = fs::read_to_string(test).map_err(|e| e.to_string())?.lines().skip(1)
        .filter_map(|l| l.split('\t').next()?.parse().ok()).collect();
    let probs: Vec<f64> = fs::read_to_string(pred).map_err(|e| e.to_string())?.lines().skip(1)
        .filter_map(|l| l.split('\t').last()?.parse().ok()).collect();
    if labels.len() != probs.len() || labels.is_empty() {
        return Err(format!("rows differ: labels {} preds {}", labels.len(), probs.len()));
    }
    let n = labels.len() as f64;
    let pos = labels.iter().filter(|l| **l == 1).count() as f64;
    let base = pos / n;
    let ll = |p: f64, y: u8| { let p = p.clamp(1e-9, 1.0 - 1e-9); if y == 1 { -p.ln() } else { -(1.0 - p).ln() } };
    let model_ll: f64 = labels.iter().zip(&probs).map(|(y, p)| ll(*p, *y)).sum::<f64>() / n;
    let base_ll: f64 = labels.iter().map(|y| ll(base, *y)).sum::<f64>() / n;
    let mut idx: Vec<usize> = (0..labels.len()).collect();
    idx.sort_by(|a, b| probs[*b].partial_cmp(&probs[*a]).unwrap_or(std::cmp::Ordering::Equal));
    // AUC by rank
    let (mut auc_num, mut neg_seen) = (0.0f64, 0.0f64);
    for i in idx.iter().rev() {
        if labels[*i] == 1 { auc_num += neg_seen; } else { neg_seen += 1.0; }
    }
    let auc = auc_num / (pos * (n - pos)).max(1.0);
    println!("TEST {n} koin, lulus {pos} ({:.2}%)", 100.0 * base);
    println!("Logloss model {model_ll:.4} | Logloss tebak-rata-rata {base_ll:.4} | lebih baik {:.1}% | AUC {auc:.3}", 100.0 * (1.0 - model_ll / base_ll));
    for top in [0.001, 0.005, 0.01, 0.02, 0.05] {
        let k = ((n * top).round() as usize).max(1);
        let hit = idx[..k].iter().filter(|i| labels[**i] == 1).count();
        println!("  skor tertinggi {:.1}% ({k} koin): lulus {hit} = presisi {:.1}% ({:.1}x rata-rata) | menangkap {:.1}% dari semua yang lulus",
            top * 100.0, 100.0 * hit as f64 / k as f64, hit as f64 / k as f64 / base, 100.0 * hit as f64 / pos);
    }
    Ok(())
}
