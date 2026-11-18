//! Latency probe (paper, no signing, no sending): how late do we see a Pump.fun create with the RPC we have?
//! Shyft WebSocket: slotSubscribe + logsSubscribe(mentions Pump program, processed). For every create:
//!   slot_lag = newest slot our node had announced when the create arrived - create slot,
//!   ms_after_slot = arrival time - time our node first announced the create slot,
//!   decide_us = decode CreateEvent from the logs + watchlist lookup (what the bot needs before sending).
//! In parallel, keep-alive HTTP round trips to Jito block engines (getTipAccounts) and Shyft RPC (getSlot).
//! Later: blockTime of a sample of slots (getBlockTime) -> arrival - blockTime (1 s resolution).
use crate::decode::{self, Ev, PUMP};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

pub fn run(secs: u64, out: &str) -> Result<(), String> {
    let env = fs::read_to_string(".env").map_err(|e| e.to_string())?;
    let key = env.lines().find_map(|l| l.strip_prefix("SHYFT_KEY=")).ok_or("SHYFT_KEY missing")?.trim().to_string();
    let watch: HashSet<String> = fs::read_to_string("data/fwd/watch.tsv").unwrap_or_default().lines().filter_map(|l| l.split('\t').next().map(String::from)).collect();
    // ---- keep-alive HTTP round trips in a side thread ----
    let rtts: Arc<Mutex<HashMap<String, Vec<f64>>>> = Arc::new(Mutex::new(HashMap::new()));
    let stop = Instant::now() + Duration::from_secs(secs);
    let r2 = rtts.clone();
    let k2 = key.clone();
    let th = std::thread::spawn(move || {
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
        let targets: Vec<(String, String, Value)> = ["tokyo", "ny", "frankfurt", "amsterdam", "slc"].iter()
            .map(|h| (format!("jito_{h}"), format!("https://{h}.mainnet.block-engine.jito.wtf/api/v1/getTipAccounts"), json!({"jsonrpc":"2.0","id":1,"method":"getTipAccounts","params":[]})))
            .chain(std::iter::once(("shyft_rpc".to_string(), format!("https://rpc.shyft.to?api_key={k2}"), json!({"jsonrpc":"2.0","id":1,"method":"getSlot"}))))
            .collect();
        while Instant::now() < stop {
            for (name, url, body) in &targets {
                let t = Instant::now();
                if agent.post(url).set("content-type", "application/json").send_string(&body.to_string()).is_ok() {
                    r2.lock().unwrap().entry(name.clone()).or_default().push(t.elapsed().as_secs_f64() * 1000.0);
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
    // ---- WebSocket ----
    let hkey = env.lines().find_map(|l| l.strip_prefix("HELIUS_API_KEY=")).unwrap_or("").trim().to_string();
    let ws_url = if hkey.len() > 10 { format!("wss://mainnet.helius-rpc.com/?api-key={hkey}") } else { format!("wss://rpc.shyft.to?api_key={key}") };
    let (mut ws, _) = tungstenite::connect(ws_url).map_err(|e| format!("ws connect: {}", e.to_string().replace(&key, "***").replace(&hkey, "***")))?;
    ws.send(tungstenite::Message::text(json!({"jsonrpc":"2.0","id":1,"method":"slotSubscribe"}).to_string())).map_err(|e| e.to_string())?;
    ws.send(tungstenite::Message::text(json!({"jsonrpc":"2.0","id":2,"method":"logsSubscribe","params":[{"mentions":[PUMP]},{"commitment":"processed"}]}).to_string())).map_err(|e| e.to_string())?;
    let mut slot_seen: HashMap<u64, f64> = HashMap::new();
    let mut max_slot = 0u64;
    let mut f = fs::File::create(out).map_err(|e| e.to_string())?;
    let (mut lag, mut after, mut dec): (Vec<f64>, Vec<f64>, Vec<f64>) = (vec![], vec![], vec![]);
    let (mut n, mut hits) = (0u64, 0u64);
    let mut slots_for_bt: Vec<(u64, f64)> = Vec::new();
    while Instant::now() < stop {
        let msg = match ws.read() {
            Ok(m) => m,
            Err(e) => return Err(format!("ws read: {e}")),
        };
        let t_recv = now_ms();
        let tungstenite::Message::Text(txt) = msg else { continue };
        let v: Value = serde_json::from_str(txt.as_str()).unwrap_or(Value::Null);
        match v["method"].as_str() {
            Some("slotNotification") => {
                if let Some(s) = v["params"]["result"]["slot"].as_u64() {
                    slot_seen.entry(s).or_insert(t_recv);
                    max_slot = max_slot.max(s);
                }
            }
            Some("logsNotification") => {
                let r = &v["params"]["result"];
                if !r["value"]["err"].is_null() {
                    continue;
                }
                let logs = &r["value"]["logs"];
                let is_create = logs.as_array().map(|a| a.iter().any(|l| l.as_str().map(|s| s.ends_with("Instruction: Create") || s.ends_with("Instruction: CreateV2")).unwrap_or(false))).unwrap_or(false);
                if !is_create {
                    continue;
                }
                let t0 = Instant::now();
                let tx = json!({"meta": {"logMessages": logs, "innerInstructions": []}});
                let evs = decode::events(&tx).0;
                let Some(c) = evs.iter().find_map(|e| if let Ev::Create(c) = e { Some(c) } else { None }) else { continue };
                let creator = c.creator.clone().unwrap_or(c.user.clone());
                let hit = watch.contains(&creator) || watch.contains(&c.user);
                let decide = t0.elapsed().as_secs_f64() * 1e6;
                let slot = r["context"]["slot"].as_u64().unwrap_or(0);
                let sl = max_slot.saturating_sub(slot) as f64;
                let ms = slot_seen.get(&slot).map(|s| t_recv - s);
                n += 1;
                hits += hit as u64;
                lag.push(sl);
                dec.push(decide);
                if let Some(m) = ms {
                    after.push(m);
                }
                if n % 20 == 1 {
                    slots_for_bt.push((slot, t_recv));
                }
                let _ = writeln!(f, "{}", json!({"t_recv_ms": t_recv, "slot": slot, "slot_lag": sl, "ms_after_slot": ms, "decide_us": decide, "mint": c.mint,
                    "creator": creator, "watch_hit": hit, "sig": r["value"]["signature"]}));
            }
            _ => {}
        }
    }
    let _ = th.join();
    // arrival vs blockTime on a sample (1 s resolution, cluster clock)
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    let mut vs_bt: Vec<f64> = Vec::new();
    for (s, t) in slots_for_bt.iter().take(60) {
        if let Ok(r) = agent.post(&format!("https://rpc.shyft.to?api_key={key}")).set("content-type", "application/json")
            .send_string(&json!({"jsonrpc":"2.0","id":1,"method":"getBlockTime","params":[s]}).to_string()) {
            if let Some(bt) = r.into_string().ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v["result"].as_i64()) {
                vs_bt.push(t / 1000.0 - bt as f64);
            }
        }
    }
    let mut rt = rtts.lock().unwrap().clone();
    let mut rt_s = String::new();
    let keys: Vec<String> = rt.keys().cloned().collect();
    for k in keys {
        let v = rt.get_mut(&k).unwrap();
        rt_s.push_str(&format!("  {k}: n={} p50={:.0}ms p90={:.0}ms\n", v.len(), pct(v, 0.5), pct(v, 0.9)));
    }
    println!("creates={n} watch_hits={hits} seconds={secs}");
    println!("slot_lag (slots behind newest when create arrived): p10={} p50={} p90={}", pct(&mut lag, 0.1), pct(&mut lag, 0.5), pct(&mut lag, 0.9));
    println!("ms after our node announced the slot: p10={:.0} p50={:.0} p90={:.0}", pct(&mut after, 0.1), pct(&mut after, 0.5), pct(&mut after, 0.9));
    println!("arrival - blockTime (s, 1 s resolution, n={}): p10={:.2} p50={:.2} p90={:.2}", vs_bt.len(), pct(&mut vs_bt, 0.1), pct(&mut vs_bt, 0.5), pct(&mut vs_bt, 0.9));
    println!("decide (decode + watchlist lookup): p50={:.1}us p99={:.1}us", pct(&mut dec, 0.5), pct(&mut dec, 0.99));
    print!("keep-alive round trips:\n{rt_s}");
    Ok(())
}
