//! Model evaluation: AUC, logloss, Brier, calibration, and trading result of the top fraction (threshold set on
//! the validation fold, applied unchanged to the test fold).
use std::fs;

type Rows = Vec<(f64, f64, f64, f64, String)>;

fn read(feat: &str, pred: &str) -> Result<Rows, String> {
    // feat: label ret ret2 day bt ... (header) ; pred: catboost calc output (header, last column = probability)
    let f = fs::read_to_string(feat).map_err(|e| format!("{feat}: {e}"))?;
    let p = fs::read_to_string(pred).map_err(|e| format!("{pred}: {e}"))?;
    let fl: Vec<&str> = f.lines().skip(1).collect();
    let pl: Vec<&str> = p.lines().skip(1).collect();
    if fl.len() != pl.len() {
        return Err(format!("row mismatch {} vs {}", fl.len(), pl.len()));
    }
    Ok(fl.iter().zip(pl).map(|(a, b)| {
        let c: Vec<&str> = a.split('\t').collect();
        let pr: f64 = b.split('\t').next_back().and_then(|x| x.parse().ok()).unwrap_or(0.5);
        // optional Jito tip as a fraction of cost, taken from every trade (doubled in the costs-x2 column)
        let tip: f64 = std::env::var("JAR_TIP_FRAC").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        (c[0].parse().unwrap_or(0.0), pr, c[1].parse::<f64>().unwrap_or(0.0) - tip, c[2].parse::<f64>().unwrap_or(0.0) - 2.0 * tip, c[3].to_string())
    }).collect())
}

pub fn auc(v: &[(f64, f64)]) -> f64 {
    let mut s: Vec<(f64, f64)> = v.to_vec();
    s.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (mut rank_sum, mut i) = (0.0, 0usize);
    while i < s.len() {
        let mut j = i;
        while j + 1 < s.len() && s[j + 1].1 == s[i].1 {
            j += 1;
        }
        let r = (i + j) as f64 / 2.0 + 1.0;
        for x in &s[i..=j] {
            if x.0 > 0.5 {
                rank_sum += r;
            }
        }
        i = j + 1;
    }
    let np = s.iter().filter(|x| x.0 > 0.5).count() as f64;
    let nn = s.len() as f64 - np;
    if np == 0.0 || nn == 0.0 {
        return f64::NAN;
    }
    (rank_sum - np * (np + 1.0) / 2.0) / (np * nn)
}

fn trade_stats(r: &[(f64, f64)]) -> String {
    if r.is_empty() {
        return "n=0".into();
    }
    let n = r.len() as f64;
    let w = r.iter().filter(|x| x.0 > 0.0).count() as f64;
    let mut s: Vec<f64> = r.iter().map(|x| x.0).collect();
    s.sort_by(|a, b| b.total_cmp(a));
    let no5 = if s.len() > 5 { s[5..].iter().sum::<f64>() / (s.len() - 5) as f64 } else { f64::NAN };
    format!("n={} menang={:.1}% rata={:+.2}% tanpa_top5={:+.2}% biaya_x2={:+.2}%", r.len(), 100.0 * w / n, 100.0 * s.iter().sum::<f64>() / n, 100.0 * no5, 100.0 * r.iter().map(|x| x.1).sum::<f64>() / n)
}

pub fn run(feat_valid: &str, pred_valid: &str, feat_test: &str, pred_test: &str) -> Result<(), String> {
    let va = read(feat_valid, pred_valid)?;
    let te = read(feat_test, pred_test)?;
    for (name, d) in [("valid", &va), ("test", &te)] {
        let lp: Vec<(f64, f64)> = d.iter().map(|x| (x.0, x.1)).collect();
        let n = lp.len() as f64;
        let ll = -lp.iter().map(|(y, p)| { let p = p.clamp(1e-12, 1.0 - 1e-12); y * p.ln() + (1.0 - y) * (1.0 - p).ln() }).sum::<f64>() / n;
        let base = lp.iter().map(|x| x.0).sum::<f64>() / n;
        let ll0 = -(base * base.max(1e-12).ln() + (1.0 - base) * (1.0 - base).max(1e-12).ln());
        let brier = lp.iter().map(|(y, p)| (p - y).powi(2)).sum::<f64>() / n;
        println!("{name}: n={} base_rate={:.3} AUC={:.4} logloss={:.4} (constant model {:.4}) brier={:.4}", lp.len(), base, auc(&lp), ll, ll0, brier);
    }
    // calibration on test (deciles of prediction)
    let mut t: Vec<&(f64, f64, f64, f64, String)> = te.iter().collect();
    t.sort_by(|a, b| a.1.total_cmp(&b.1));
    let k = t.len() / 10;
    if k > 0 {
        let mut cal = String::from("test calibration (pred -> actual win): ");
        for b in 0..10 {
            let s = &t[b * k..if b == 9 { t.len() } else { (b + 1) * k }];
            let m = s.len() as f64;
            cal.push_str(&format!("{:.2}->{:.2} ", s.iter().map(|x| x.1).sum::<f64>() / m, s.iter().map(|x| x.0).sum::<f64>() / m));
        }
        println!("{cal}");
    }
    println!("test ALL coins: {}", trade_stats(&te.iter().map(|x| (x.2, x.3)).collect::<Vec<_>>()));
    for q in [0.5, 0.8, 0.9, 0.95, 0.99] {
        let mut ps: Vec<f64> = va.iter().map(|x| x.1).collect();
        ps.sort_by(|a, b| a.total_cmp(b));
        let thr = ps[((ps.len() - 1) as f64 * q) as usize];
        let sel: Vec<(f64, f64)> = te.iter().filter(|x| x.1 >= thr).map(|x| (x.2, x.3)).collect();
        println!("test top {:.0}% (thr {:.3} from valid): {}", 100.0 * (1.0 - q), thr, trade_stats(&sel));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn auc_perfect_and_random() {
        assert_eq!(super::auc(&[(0.0, 0.1), (1.0, 0.9)]), 1.0);
        assert_eq!(super::auc(&[(0.0, 0.5), (1.0, 0.5)]), 0.5);
    }
}
