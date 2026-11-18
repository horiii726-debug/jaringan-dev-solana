#![allow(dead_code)] // used by later pipeline steps
//! Pump.fun event decoder (layouts from the official IDL, idl/pump.json, fetched 2026-11-16).
//! Events come from self-CPI inner instructions (preferred, never truncated) or "Program data:" logs.
use base64::Engine;
use serde_json::Value;

pub const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const MINT_AUTH: &str = "TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM";
pub const SYSTEM: &str = "11111111111111111111111111111111";
const CREATE_DISC: [u8; 8] = [27, 114, 169, 77, 222, 235, 99, 118];
const TRADE_DISC: [u8; 8] = [189, 219, 127, 211, 78, 230, 97, 238];
const COMPLETE_DISC: [u8; 8] = [95, 114, 97, 156, 212, 46, 152, 8];
const EVENT_TAG: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

struct R<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.p..self.p + n)?;
        self.p += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|s| u64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
    }
    fn i64(&mut self) -> Option<i64> {
        self.u64().map(|v| v as i64)
    }
    fn b(&mut self) -> Option<bool> {
        self.u8().map(|v| v != 0)
    }
    fn pk(&mut self) -> Option<String> {
        self.take(32).map(|s| bs58::encode(s).into_string())
    }
    fn s(&mut self) -> Option<String> {
        let n = u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize;
        if n > 4096 {
            return None;
        }
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Create {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint: String,
    pub curve: String,
    pub user: String,
    pub creator: Option<String>,
    pub ts: Option<i64>,
    pub vtok: Option<u64>,
    pub vsol: Option<u64>,
    pub rtok: Option<u64>,
    pub supply: Option<u64>,
    pub tprog: Option<String>,
    pub mayhem: Option<bool>,
    pub cashback: Option<bool>,
    pub quote: Option<String>,
    pub vquote: Option<u64>,
    pub cfee_bps: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct Trade {
    pub mint: String,
    pub sol: u64,
    pub tok: u64,
    pub buy: bool,
    pub user: String,
    pub ts: i64,
    pub vsol: u64,
    pub vtok: u64,
    pub rsol: u64,
    pub rtok: u64,
    pub fee_bps: Option<u64>,
    pub fee: Option<u64>,
    pub creator: Option<String>,
    pub cfee_bps: Option<u64>,
    pub cfee: Option<u64>,
    pub ix: Option<String>,
    pub mayhem: Option<bool>,
}

#[derive(Debug, Clone)]
pub enum Ev {
    Create(Create),
    Trade(Trade),
    Complete(String),
}

pub fn parse_create(d: &[u8]) -> Option<Create> {
    let mut r = R { b: d, p: 0 };
    let mut c = Create { name: r.s()?, symbol: r.s()?, uri: r.s()?, mint: r.pk()?, curve: r.pk()?, user: r.pk()?, ..Default::default() };
    c.creator = r.pk();
    c.ts = r.i64();
    c.vtok = r.u64();
    c.vsol = r.u64();
    c.rtok = r.u64();
    c.supply = r.u64();
    c.tprog = r.pk();
    c.mayhem = r.b();
    c.cashback = r.b();
    c.quote = r.pk();
    c.vquote = r.u64();
    c.cfee_bps = r.u64();
    Some(c)
}

pub fn parse_trade(d: &[u8]) -> Option<Trade> {
    let mut r = R { b: d, p: 0 };
    let mut t = Trade { mint: r.pk()?, sol: r.u64()?, tok: r.u64()?, buy: r.b()?, user: r.pk()?, ts: r.i64()?, vsol: r.u64()?, vtok: r.u64()?, rsol: r.u64()?, rtok: r.u64()?, ..Default::default() };
    let _fee_recipient = r.pk();
    t.fee_bps = r.u64();
    t.fee = r.u64();
    t.creator = r.pk();
    t.cfee_bps = r.u64();
    t.cfee = r.u64();
    let _ = (r.b(), r.u64(), r.u64(), r.u64(), r.i64());
    t.ix = r.s();
    t.mayhem = r.b();
    Some(t)
}

fn parse_ev(d: &[u8]) -> Option<Ev> {
    let (disc, body) = (d.get(..8)?, d.get(8..)?);
    if disc == CREATE_DISC {
        parse_create(body).map(Ev::Create)
    } else if disc == TRADE_DISC {
        parse_trade(body).map(Ev::Trade)
    } else if disc == COMPLETE_DISC {
        // CompleteEvent: user, mint, bonding_curve, timestamp
        let mut r = R { b: body, p: 0 };
        let _user = r.pk()?;
        r.pk().map(Ev::Complete)
    } else {
        None
    }
}

/// Events of one transaction in execution order. Returns (events, source) with source "cpi" | "log" | "none".
pub fn events(tx: &Value) -> (Vec<Ev>, &'static str) {
    let mut out = Vec::new();
    let mut inner: Vec<&Value> = tx["meta"]["innerInstructions"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    inner.sort_by_key(|g| g["index"].as_u64().unwrap_or(0));
    for g in inner {
        for ix in g["instructions"].as_array().into_iter().flatten() {
            if ix["programId"] != PUMP {
                continue;
            }
            let Some(d) = ix["data"].as_str().and_then(|s| bs58::decode(s).into_vec().ok()) else { continue };
            if d.len() >= 16 && d[..8] == EVENT_TAG {
                if let Some(e) = parse_ev(&d[8..]) {
                    out.push(e);
                }
            }
        }
    }
    if !out.is_empty() {
        return (out, "cpi");
    }
    // Fallback: logs emitted while PUMP is the executing program.
    let mut stack: Vec<String> = Vec::new();
    for l in tx["meta"]["logMessages"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
        if let Some(rest) = l.strip_prefix("Program ") {
            if let Some(p) = rest.split(' ').next().filter(|p| p.len() >= 32 && !p.ends_with(':')) {
                if rest.contains(" invoke [") {
                    stack.push(p.to_string());
                    continue;
                }
                if rest.ends_with(" success") || rest.contains(" failed") {
                    stack.pop();
                    continue;
                }
            }
        }
        if let Some(b) = l.strip_prefix("Program data: ") {
            if stack.last().map(|s| s == PUMP).unwrap_or(false) {
                if let Ok(d) = base64::engine::general_purpose::STANDARD.decode(b) {
                    if let Some(e) = parse_ev(&d) {
                        out.push(e);
                    }
                }
            }
        }
    }
    let src = if out.is_empty() { "none" } else { "log" };
    (out, src)
}

/// All native SOL movements in a parsed tx: (source, destination, lamports) from System transfer / createAccount
/// in outer and inner instructions.
pub fn sol_moves(tx: &Value) -> Vec<(String, String, u64)> {
    let mut v = Vec::new();
    let mut push = |ix: &Value| {
        if ix["programId"] != SYSTEM {
            return;
        }
        let p = &ix["parsed"];
        let i = &p["info"];
        let lam = i["lamports"].as_u64().unwrap_or(0);
        match p["type"].as_str().unwrap_or("") {
            "transfer" | "transferWithSeed" => {
                if let (Some(s), Some(d)) = (i["source"].as_str(), i["destination"].as_str()) {
                    v.push((s.to_string(), d.to_string(), lam));
                }
            }
            "createAccount" | "createAccountWithSeed" => {
                if let (Some(s), Some(d)) = (i["source"].as_str(), i["newAccount"].as_str()) {
                    v.push((s.to_string(), d.to_string(), lam));
                }
            }
            _ => {}
        }
    };
    for ix in tx["transaction"]["message"]["instructions"].as_array().into_iter().flatten() {
        push(ix);
    }
    for g in tx["meta"]["innerInstructions"].as_array().into_iter().flatten() {
        for ix in g["instructions"].as_array().into_iter().flatten() {
            push(ix);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    fn le(v: u64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }
    #[test]
    fn create_roundtrip_and_truncated() {
        let mut d = Vec::new();
        for s in ["N", "S", "u"] {
            d.extend((s.len() as u32).to_le_bytes());
            d.extend(s.as_bytes());
        }
        for k in 1..=4u8 {
            d.extend([k; 32]);
        }
        let full = parse_create(&d).expect("prefix parses");
        assert_eq!(full.name, "N");
        assert_eq!(full.creator.as_deref(), Some(bs58::encode([4u8; 32]).into_string().as_str()));
        assert!(full.ts.is_none());
        d.extend(le(7));
        assert_eq!(parse_create(&d).and_then(|c| c.ts), Some(7));
        assert!(parse_create(&d[..10]).is_none());
    }
    #[test]
    fn trade_parses_core() {
        let mut d = vec![1u8; 32];
        d.extend(le(5));
        d.extend(le(9));
        d.push(1);
        d.extend([2u8; 32]);
        for v in [100u64, 30, 1000, 20, 900] {
            d.extend(le(v));
        }
        let t = parse_trade(&d).expect("trade");
        assert!(t.buy && t.sol == 5 && t.tok == 9 && t.vtok == 1000 && t.fee_bps.is_none());
    }
}
