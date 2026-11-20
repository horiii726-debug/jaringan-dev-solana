//! Helius RPC client: rate limit, retry with backoff, credit ledger, hard budget cap.
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn cfg_u64(path: &str, name: &str) -> Option<u64> {
    let s = fs::read_to_string(path).ok()?;
    s.lines().find_map(|l| {
        let l = l.split('#').next()?.trim();
        let (k, v) = l.split_once('=')?;
        if k.trim() == name { v.trim().replace('_', "").parse().ok() } else { None }
    })
}

/// Append one data-quality issue (never silent).
pub fn issue(kind: &str, detail: Value) {
    let _ = fs::create_dir_all("reports");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open("reports/data_quality_issues.jsonl") {
        let _ = writeln!(f, "{}", json!({"kind": kind, "detail": detail}));
    }
}

fn ledger_others(job: &str) -> u64 {
    let mut t = 0;
    if let Ok(rd) = fs::read_dir("checkpoints/credits") {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy() == format!("{job}.txt") {
                continue;
            }
            t += fs::read_to_string(e.path()).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0);
        }
    }
    t
}

pub struct Rpc {
    url: String,
    key: String,
    agent: ureq::Agent,
    last: Instant,
    gap: Duration,
    pub credits: u64,
    pub calls: u64,
    pub retries: u64,
    cap: u64,
    prior: u64,
    job: String,
    free: bool,
    hdr: Option<(String, String)>,
}

/// Secondary providers for light, standard methods (owner 21 Nov: use Tatum + public RPC so Helius quota is kept for
/// full-transaction and transfer history). Tatum key goes ONLY to the Tatum gateway (header), never elsewhere.
pub struct Aux {
    pub v: Vec<Rpc>,
    i: usize,
}

impl Aux {
    pub fn new(job: &str) -> Result<Self, String> {
        let mut v = Vec::new();
        if let Some(k) = fs::read_to_string(".tatum.env").ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("TATUM_API_KEY=").map(|k| k.trim().to_string()))) {
            v.push(Rpc::endpoint(&format!("{job}_tatum"), "https://solana-mainnet.gateway.tatum.io/", Some(("x-api-key".into(), k)), cfg_u64("config/budget.toml", "tatum_rps").unwrap_or(3))?);
        }
        // 21 Nov: Shyft RPC (owner key in .env SHYFT_API_KEY; the key only goes to rpc.shyft.to, in its URL)
        if let Some(k) = fs::read_to_string(".env").ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("SHYFT_API_KEY=").map(|k| k.trim().to_string()))).filter(|k| !k.is_empty()) {
            v.push(Rpc::endpoint(&format!("{job}_shyft"), &format!("https://rpc.shyft.to?api_key={k}"), Some(("x-api-key".into(), k.clone())), cfg_u64("config/budget.toml", "shyft_rps").unwrap_or(2))?);
        }
        v.push(Rpc::endpoint(&format!("{job}_public"), "https://api.mainnet-beta.solana.com", None, cfg_u64("config/budget.toml", "public_rps").unwrap_or(2))?);
        Ok(Aux { v, i: 0 })
    }
    /// Round robin; on failure try the next provider. Err only when every provider failed.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let mut last = String::new();
        for _ in 0..self.v.len() {
            self.i = (self.i + 1) % self.v.len();
            match self.v[self.i].call(method, params.clone(), 0) {
                Ok(r) => return Ok(r),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

/// Credits for a successful response (Helius billing page, checked 2026-11-16).
pub fn cost(method: &str, params: &Value, result: &Value) -> u64 {
    if method == "getTransactionsForAddress" {
        if params[1]["transactionDetails"] == "full" {
            let n = result["data"].as_array().map(|a| a.len()).unwrap_or(0) as u64;
            return 10 * n.div_ceil(100).max(1);
        }
        return 10;
    }
    if method == "getTransfersByAddress" {
        return 10; // price not confirmed from Helius docs: counted like getTransactionsForAddress (conservative)
    }
    1
}

impl Rpc {
    pub fn new(job: &str) -> Result<Self, String> {
        let env = fs::read_to_string(".env").map_err(|e| format!(".env: {e}"))?;
        let key = env
            .lines()
            .find_map(|l| l.strip_prefix("HELIUS_API_KEY="))
            .ok_or("HELIUS_API_KEY missing")?
            .trim()
            .to_string();
        let cap = cfg_u64("config/budget.toml", "max_provider_credits").ok_or("budget cap missing")?;
        let rps = std::env::var("JAR_RPS").ok().and_then(|v| v.parse().ok()).or_else(|| cfg_u64("config/budget.toml", "max_rps")).unwrap_or(7).max(1);
        fs::create_dir_all("checkpoints/credits").map_err(|e| e.to_string())?;
        let own = fs::read_to_string(format!("checkpoints/credits/{job}.txt"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        // another provider via JAR_RPC_URL: its api_key is what must be masked in logs
        let key = match std::env::var("JAR_RPC_URL").ok().and_then(|u| u.split("api_key=").nth(1).map(String::from)) {
            Some(k) => k,
            None => key,
        };
        Ok(Rpc {
            url: std::env::var("JAR_RPC_URL").unwrap_or(format!("https://mainnet.helius-rpc.com/?api-key={key}")),
            free: std::env::var("JAR_RPC_URL").is_ok(),
            key,
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(90)).build(),
            last: Instant::now() - Duration::from_secs(1),
            gap: Duration::from_millis(1000 / rps),
            credits: own,
            calls: 0,
            retries: 0,
            cap,
            prior: ledger_others(job),
            job: job.into(),
            hdr: None,
        })
    }

    /// A non-Helius endpoint (no Helius credits counted, own rate limit, optional auth header).
    pub fn endpoint(job: &str, url: &str, hdr: Option<(String, String)>, rps: u64) -> Result<Self, String> {
        let key = hdr.as_ref().map(|h| h.1.clone()).unwrap_or_else(|| "\u{0}".into());
        Ok(Rpc {
            url: url.into(),
            free: true,
            key,
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build(),
            last: Instant::now() - Duration::from_secs(1),
            gap: Duration::from_millis(1000 / rps.max(1)),
            credits: 0,
            calls: 0,
            retries: 0,
            cap: u64::MAX,
            prior: 0,
            job: job.into(),
            hdr,
        })
    }

    pub fn used(&self) -> u64 {
        self.prior + self.credits
    }

    pub fn save(&self) {
        let _ = fs::write(format!("checkpoints/credits/{}.txt", self.job), self.credits.to_string());
    }

    fn clean(&self, s: &str) -> String {
        let s = s.replace(&self.key, "***");
        s.chars().take(300).collect()
    }

    /// `worst` = most credits this call can cost; refused before sending if it would cross the cap.
    pub fn call(&mut self, method: &str, params: Value, worst: u64) -> Result<Value, String> {
        if self.used() + worst > self.cap {
            self.save();
            return Err(format!("BUDGET_CAP used={} worst={} cap={}", self.used(), worst, self.cap));
        }
        let body = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
        let mut wait = 500u64;
        for attempt in 0..12 {
            let target = self.last + self.gap;
            let now = Instant::now();
            if target > now {
                sleep(target - now);
            }
            self.last = Instant::now();
            self.calls += 1;
            if self.calls.is_multiple_of(200) {
                self.prior = ledger_others(&self.job);
            }
            let mut req = self.agent.post(&self.url).set("content-type", "application/json");
            if let Some((h, v)) = &self.hdr {
                req = req.set(h, v);
            }
            let err = match req.send_string(&body) {
                Ok(resp) => {
                    let mut s = String::new();
                    match resp.into_reader().read_to_string(&mut s) {
                        Err(e) => format!("read {e}"),
                        Ok(_) => match serde_json::from_str::<Value>(&s) {
                            Err(e) => format!("json {e}"),
                            Ok(mut v) => {
                                if let Some(e) = v.get("error") {
                                    let code = e["code"].as_i64().unwrap_or(0);
                                    let msg = e["message"].as_str().unwrap_or("").to_string();
                                    let low = msg.to_lowercase();
                                    if low.contains("max usage") || low.contains("credits") {
                                        self.save();
                                        return Err(format!("QUOTA {code} {msg}"));
                                    }
                                    if code == 429 || code == -32429 || code == -32603 || code == -32005 || low.contains("rate") || low.contains("timeout") {
                                        format!("rpc {code} {msg}")
                                    } else {
                                        issue("rpc_error", json!({"method": method, "code": code, "msg": self.clean(&msg)}));
                                        return Err(format!("RPC {code} {}", self.clean(&msg)));
                                    }
                                } else {
                                    let r = v["result"].take();
                                    if !self.free {
                                        self.credits += cost(method, &params, &r);
                                    }
                                    if self.calls.is_multiple_of(20) {
                                        self.save();
                                    }
                                    return Ok(r);
                                }
                            }
                        },
                    }
                }
                Err(ureq::Error::Status(c, resp)) => {
                    let t = resp.into_string().unwrap_or_default();
                    if t.to_lowercase().contains("max usage") {
                        self.save();
                        return Err(format!("QUOTA http {c}"));
                    }
                    format!("http {c} {}", self.clean(&t))
                }
                Err(e) => format!("transport {}", self.clean(&e.to_string())),
            };
            self.retries += 1;
            issue("rpc_retry", json!({"method": method, "attempt": attempt, "err": err}));
            sleep(Duration::from_millis(wait));
            wait = (wait * 2).min(30_000);
        }
        self.save();
        Err(format!("GAVE_UP {method}"))
    }
}

impl Rpc {
    /// JSON-RPC batch (Helius: historical methods up to 10 items, getTransaction up to 100). 1 credit per item.
    /// Returns one Value per request: the result, or Null when that item failed (logged).
    pub fn call_batch(&mut self, reqs: &[(&str, Value)]) -> Result<Vec<Value>, String> {
        let n = reqs.len() as u64;
        if self.used() + n > self.cap {
            self.save();
            return Err(format!("BUDGET_CAP used={} worst={} cap={}", self.used(), n, self.cap));
        }
        let body: Vec<Value> = reqs.iter().enumerate().map(|(i, (m, p))| json!({"jsonrpc":"2.0","id":i,"method":m,"params":p})).collect();
        let body = Value::Array(body).to_string();
        let mut wait = 500u64;
        for attempt in 0..30 {
            let target = self.last + self.gap;
            let now = Instant::now();
            if target > now {
                sleep(target - now);
            }
            self.last = Instant::now();
            self.last += self.gap * (reqs.len() as u32).saturating_sub(1);
            self.calls += 1;
            let mut req = self.agent.post(&self.url).set("content-type", "application/json");
            if let Some((h, v)) = &self.hdr {
                req = req.set(h, v);
            }
            let err = match req.send_string(&body) {
                Ok(resp) => {
                    let mut s = String::new();
                    match resp.into_reader().read_to_string(&mut s).map_err(|e| e.to_string()).and_then(|_| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string())) {
                        Ok(Value::Array(items)) => {
                            let mut out = vec![Value::Null; reqs.len()];
                            let mut rate = false;
                            for mut it in items {
                                let Some(i) = it["id"].as_u64().map(|i| i as usize).filter(|&i| i < out.len()) else { continue };
                                if let Some(e) = it.get("error") {
                                    let msg = e["message"].as_str().unwrap_or("").to_lowercase();
                                    if msg.contains("max usage") {
                                        self.save();
                                        return Err("QUOTA batch".into());
                                    }
                                    rate |= msg.contains("rate") || e["code"].as_i64() == Some(429);
                                    issue("rpc_batch_item_error", json!({"method": reqs[i].0, "msg": self.clean(&msg)}));
                                } else {
                                    self.credits += (!self.free) as u64;
                                    out[i] = it["result"].take();
                                }
                            }
                            if self.calls.is_multiple_of(20) {
                                self.save();
                            }
                            if !rate || attempt > 3 {
                                return Ok(out);
                            }
                            "batch items rate limited".to_string()
                        }
                        Ok(v) => format!("batch non-array {}", self.clean(&v.to_string())),
                        Err(e) => format!("batch parse {e}"),
                    }
                }
                Err(ureq::Error::Status(c, resp)) => {
                    let t = resp.into_string().unwrap_or_default();
                    if t.to_lowercase().contains("max usage") {
                        self.save();
                        return Err(format!("QUOTA http {c}"));
                    }
                    format!("http {c} {}", self.clean(&t))
                }
                Err(e) => format!("transport {}", self.clean(&e.to_string())),
            };
            self.retries += 1;
            issue("rpc_retry", json!({"method": "batch", "attempt": attempt, "err": err}));
            sleep(Duration::from_millis(wait));
            wait = (wait * 2).min(30_000);
        }
        self.save();
        Err("GAVE_UP batch".into())
    }
}
