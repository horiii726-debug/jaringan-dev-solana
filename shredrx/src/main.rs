//! shredrx (21 Nov 2026): ShredStream gRPC receiver -> Pump.fun create detector -> watchlist hits (PAPER only: no
//! keys, no signing, no sending). Entries come from the AllenHark shared gRPC (Jito shredstream proto:
//! /shredstream.ShredstreamProxy/SubscribeEntries, header x-api-key); each message = one slot's bincode Vec<Entry>.
//! Transactions are decoded by hand (bincode + short_vec layout of VersionedTransaction), no solana crates.
//! Per transaction:
//!   * Pump.fun create / create_v2 -> data/shred/creates.jsonl (recv time ms, slot, sig, mint, user, creator arg, name)
//!   * creator or user in the watchlist (data/v3/reput/watchlist.json, reloaded every 60 s) or in the dynamic watch
//!     set -> data/shred/hits.jsonl
//!   * System transfer >= 0.05 SOL from a watched wallet to a new wallet -> that wallet joins the dynamic watch set
//!     (data/shred/dynamic_watch.jsonl, same entity): fresh wallets the funder pays usually launch the next coin.
//! Stats line every 10 s on stdout. Config: .allenhark.env (ALLENHARK_GRPC, ALLENHARK_KEY).

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, PartialEq, prost::Message)]
pub struct SubscribeEntriesRequest {}

#[derive(Clone, PartialEq, prost::Message)]
pub struct EntryMsg {
    #[prost(uint64, tag = "1")]
    pub slot: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub entries: Vec<u8>,
}

const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const SYSTEM: &str = "11111111111111111111111111111111";
const CREATE: [u8; 8] = [24, 30, 200, 40, 5, 28, 7, 119];
const CREATE_V2: [u8; 8] = [214, 144, 76, 236, 95, 139, 49, 180];
const MIN_FUND_LAMPORTS: u64 = 50_000_000;

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
    }
    /// compact-u16 (short_vec length)
    fn short(&mut self) -> Option<usize> {
        let (mut v, mut shift) = (0usize, 0);
        for _ in 0..3 {
            let x = self.u8()?;
            v |= ((x & 0x7f) as usize) << shift;
            if x & 0x80 == 0 {
                return Some(v);
            }
            shift += 7;
        }
        None
    }
}

pub struct Ix {
    pub prog: usize,
    pub accts: Vec<usize>,
    pub data: Vec<u8>,
}

pub struct Tx {
    pub sig: String,
    pub keys: Vec<[u8; 32]>,
    pub ixs: Vec<Ix>,
}

/// One bincode VersionedTransaction.
pub fn read_tx(r: &mut Rd) -> Option<Tx> {
    let ns = r.short()?;
    let mut sig = String::new();
    for i in 0..ns {
        let s = r.take(64)?;
        if i == 0 {
            sig = bs58::encode(s).into_string();
        }
    }
    let first = *r.b.get(r.p)?;
    let v0 = first & 0x80 != 0;
    if v0 {
        r.u8()?; // version prefix
    }
    r.take(3)?; // header
    let nk = r.short()?;
    let mut keys = Vec::with_capacity(nk);
    for _ in 0..nk {
        keys.push(r.take(32)?.try_into().ok()?);
    }
    r.take(32)?; // recent blockhash
    let ni = r.short()?;
    let mut ixs = Vec::with_capacity(ni);
    for _ in 0..ni {
        let prog = r.u8()? as usize;
        let na = r.short()?;
        let accts = r.take(na)?.iter().map(|x| *x as usize).collect();
        let nd = r.short()?;
        let data = r.take(nd)?.to_vec();
        ixs.push(Ix { prog, accts, data });
    }
    if v0 {
        let nl = r.short()?;
        for _ in 0..nl {
            r.take(32)?;
            let w = r.short()?;
            r.take(w)?;
            let ro = r.short()?;
            r.take(ro)?;
        }
    }
    Some(Tx { sig, keys, ixs })
}

/// bincode Vec<Entry>: u64 count; Entry = num_hashes u64, hash [32], Vec<VersionedTransaction> (u64 count).
pub fn read_entries(b: &[u8]) -> Option<Vec<Tx>> {
    let mut r = Rd { b, p: 0 };
    let ne = r.u64()?;
    let mut out = Vec::new();
    for _ in 0..ne {
        r.u64()?;
        r.take(32)?;
        let nt = r.u64()?;
        for _ in 0..nt {
            out.push(read_tx(&mut r)?);
        }
    }
    (r.p == b.len()).then_some(out)
}

fn borsh_str(r: &mut Rd) -> Option<String> {
    let n = u32::from_le_bytes(r.take(4)?.try_into().ok()?) as usize;
    if n > 512 {
        return None;
    }
    Some(String::from_utf8_lossy(r.take(n)?).to_string())
}

fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn append(path: &str, v: &Value) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(format!("{v}\n").as_bytes());
    }
}

/// wallet -> (entity, role) from the watchlist and the persisted dynamic watch set.
fn load_watch() -> HashMap<String, (String, String)> {
    let mut w = HashMap::new();
    if let Ok(s) = fs::read_to_string("data/v3/reput/watchlist.json") {
        if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&s) {
            for e in a {
                let ent = e["entitas"].as_str().unwrap_or("").to_string();
                for x in e["wallets"].as_array().into_iter().flatten() {
                    if let Some(addr) = x["w"].as_str() {
                        w.insert(addr.to_string(), (ent.clone(), x["role"].as_str().unwrap_or("").to_string()));
                    }
                }
            }
        }
    }
    for l in fs::read_to_string("data/shred/dynamic_watch.jsonl").unwrap_or_default().lines() {
        if let Ok(v) = serde_json::from_str::<Value>(l) {
            if let (Some(a), Some(e)) = (v["wallet"].as_str(), v["entitas"].as_str()) {
                w.entry(a.to_string()).or_insert((e.to_string(), "BARU_DIDANAI".to_string()));
            }
        }
    }
    w
}

/// Debug: where decoding of a dumped message stops (entry / tx index, byte offset).
fn debug_file(path: &str) {
    let b = fs::read(path).unwrap_or_default();
    let mut r = Rd { b: &b, p: 0 };
    let ne = r.u64().unwrap_or(0);
    println!("len={} entries={ne} head={:02x?}", b.len(), &b[..b.len().min(48)]);
    for e in 0..ne {
        let (nh, p0) = (r.u64(), r.p);
        r.take(32);
        let nt = r.u64().unwrap_or(0);
        println!("entry {e}: num_hashes={nh:?} at={p0} txs={nt}");
        for t in 0..nt {
            let p = r.p;
            match read_tx(&mut r) {
                Some(tx) => println!("  tx {t} ok at={p} keys={} ixs={}", tx.keys.len(), tx.ixs.len()),
                None => {
                    println!("  tx {t} FAIL at={p} next={:02x?}", &b[p..b.len().min(p + 40)]);
                    return;
                }
            }
        }
    }
    println!("end at={} of {}", r.p, b.len());
}

#[tokio::main]
async fn main() {
    if let Some(p) = std::env::args().nth(2).filter(|_| std::env::args().nth(1).as_deref() == Some("debug")) {
        debug_file(&p);
        return;
    }
    let env = fs::read_to_string(".allenhark.env").unwrap_or_default();
    let get = |k: &str| env.lines().find_map(|l| l.strip_prefix(&format!("{k}=")).map(|v| v.trim().to_string())).unwrap_or_default();
    let (addr, key) = (get("ALLENHARK_GRPC"), get("ALLENHARK_KEY"));
    fs::create_dir_all("data/shred").ok();
    let mut watch = load_watch();
    let mut last_reload = Instant::now();
    println!("{}", json!({"start": now_ms(), "server": addr, "watch_wallets": watch.len()}));
    let mut backoff = 1u64;
    loop {
        let res: Result<(), String> = async {
            let ch = tonic::transport::Channel::from_shared(format!("http://{addr}")).map_err(|e| e.to_string())?
                .connect_timeout(Duration::from_secs(10)).connect().await.map_err(|e| format!("connect: {e}"))?;
            let mut grpc = tonic::client::Grpc::new(ch);
            grpc.ready().await.map_err(|e| format!("ready: {e}"))?;
            let mut req = tonic::Request::new(SubscribeEntriesRequest {});
            req.metadata_mut().insert("x-api-key", key.parse().map_err(|_| "bad key".to_string())?);
            let path = http::uri::PathAndQuery::from_static("/shredstream.ShredstreamProxy/SubscribeEntries");
            let codec = tonic::codec::ProstCodec::<SubscribeEntriesRequest, EntryMsg>::default();
            let mut st = grpc.server_streaming(req, path, codec).await.map_err(|e| format!("subscribe: {e}"))?.into_inner();
            println!("{}", json!({"connected": now_ms()}));
            let (mut n_msg, mut n_tx, mut n_create, mut n_hit, mut n_bad, mut bytes) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let mut last_slot = 0u64;
            let mut tick = Instant::now();
            while let Some(m) = st.message().await.map_err(|e| format!("stream: {e}"))? {
                let t = now_ms();
                n_msg += 1;
                bytes += m.entries.len() as u64;
                last_slot = last_slot.max(m.slot);
                let Some(txs) = read_entries(&m.entries) else {
                    n_bad += 1;
                    // fallback (21 Nov: part of the stream uses the new 0x81 transaction format the parser does not
                    // know): find Pump.fun create instruction data directly (discriminator + name, symbol, uri,
                    // creator); the mint is resolved later
                    for (v2, disc) in [(false, CREATE), (true, CREATE_V2)] {
                        let b = &m.entries;
                        let mut i = 0;
                        while i + 8 <= b.len() {
                            if b[i..i + 8] == disc {
                                let mut r = Rd { b, p: i + 8 };
                                if let (Some(name), Some(symbol), Some(_uri), Some(cr)) =
                                    (borsh_str(&mut r), borsh_str(&mut r), borsh_str(&mut r), r.take(32))
                                {
                                    n_create += 1;
                                    let creator = bs58::encode(cr).into_string();
                                    let hit = watch.get(&creator).map(|(e, ro)| json!({"wallet": creator, "entitas": e, "role": ro}));
                                    let v = json!({"t_ms": t, "slot": m.slot, "ix": if v2 { "create_v2" } else { "create" }, "parse": "scan",
                                        "mint": null, "creator": creator, "name": name, "symbol": symbol, "hit": hit});
                                    append("data/shred/creates.jsonl", &v);
                                    if v["hit"].is_object() {
                                        n_hit += 1;
                                        append("data/shred/hits.jsonl", &v);
                                    }
                                    i = r.p;
                                    continue;
                                }
                            }
                            i += 1;
                        }
                    }
                    // SHREDRX_DUMP=n: keep the first n undecodable messages for offline debugging
                    if n_bad <= std::env::var("SHREDRX_DUMP").ok().and_then(|v| v.parse().ok()).unwrap_or(0) {
                        fs::write(format!("data/shred/raw_{n_bad}.bin"), &m.entries).ok();
                    }
                    continue;
                };
                for tx in &txs {
                    n_tx += 1;
                    let key58 = |i: usize| tx.keys.get(i).map(|k| bs58::encode(k).into_string());
                    for ix in &tx.ixs {
                        let Some(prog) = key58(ix.prog) else { continue };
                        if prog == SYSTEM && ix.data.len() == 12 && ix.data[..4] == [2, 0, 0, 0] {
                            let lamports = u64::from_le_bytes(ix.data[4..12].try_into().unwrap());
                            if let (Some(from), Some(to)) = (ix.accts.first().and_then(|i| key58(*i)), ix.accts.get(1).and_then(|i| key58(*i))) {
                                if lamports >= MIN_FUND_LAMPORTS && !watch.contains_key(&to) {
                                    if let Some((ent, role)) = watch.get(&from).cloned() {
                                        let v = json!({"t_ms": t, "slot": m.slot, "sig": tx.sig, "entitas": ent, "from": from, "from_role": role,
                                            "wallet": to, "sol": lamports as f64 / 1e9});
                                        append("data/shred/dynamic_watch.jsonl", &v);
                                        watch.insert(to, (ent, "BARU_DIDANAI".into()));
                                    }
                                }
                            }
                            continue;
                        }
                        if prog != PUMP || ix.data.len() < 8 {
                            continue;
                        }
                        let v2 = ix.data[..8] == CREATE_V2;
                        if !v2 && ix.data[..8] != CREATE {
                            continue;
                        }
                        n_create += 1;
                        let mint = ix.accts.first().and_then(|i| key58(*i)).unwrap_or_default();
                        let user = ix.accts.get(if v2 { 5 } else { 7 }).and_then(|i| key58(*i)).unwrap_or_default();
                        let mut r = Rd { b: &ix.data, p: 8 };
                        let (name, symbol) = (borsh_str(&mut r).unwrap_or_default(), borsh_str(&mut r).unwrap_or_default());
                        let _uri = borsh_str(&mut r);
                        let creator = r.take(32).map(|k| bs58::encode(k).into_string()).unwrap_or_default();
                        let payer = key58(0).unwrap_or_default();
                        let hit = [&creator, &user, &payer].iter().find_map(|w| watch.get(*w).map(|(e, ro)| (w.to_string(), e.clone(), ro.clone())));
                        let v = json!({"t_ms": t, "slot": m.slot, "sig": tx.sig, "ix": if v2 { "create_v2" } else { "create" }, "mint": mint,
                            "user": user, "creator": creator, "payer": payer, "name": name, "symbol": symbol,
                            "hit": hit.as_ref().map(|(w, e, ro)| json!({"wallet": w, "entitas": e, "role": ro}))});
                        append("data/shred/creates.jsonl", &v);
                        if hit.is_some() {
                            n_hit += 1;
                            append("data/shred/hits.jsonl", &v);
                        }
                    }
                }
                if tick.elapsed() >= Duration::from_secs(10) {
                    tick = Instant::now();
                    println!("{}", json!({"t_ms": t, "slot": last_slot, "msgs": n_msg, "txs": n_tx, "creates": n_create, "hits": n_hit,
                        "decode_fail": n_bad, "mb": bytes / 1_000_000, "watch": watch.len()}));
                }
                if last_reload.elapsed() >= Duration::from_secs(60) {
                    last_reload = Instant::now();
                    let fresh = load_watch();
                    for (k, v) in fresh {
                        watch.entry(k).or_insert(v);
                    }
                }
            }
            Err("stream ended".into())
        }
        .await;
        if let Err(e) = res {
            println!("{}", json!({"t_ms": now_ms(), "error": e, "retry_s": backoff}));
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(60);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_vec_and_legacy_tx() {
        // legacy tx: 1 sig, header, 2 keys, blockhash, 1 ix (prog 1, accts [0], data [9,9])
        let mut b = vec![1u8];
        b.extend([7u8; 64]);
        b.extend([1, 0, 1]);
        b.push(2);
        b.extend([1u8; 32]);
        b.extend([2u8; 32]);
        b.extend([3u8; 32]);
        b.push(1);
        b.extend([1, 1, 0, 2, 9, 9]);
        let mut r = Rd { b: &b, p: 0 };
        let tx = read_tx(&mut r).unwrap();
        assert_eq!((tx.keys.len(), tx.ixs.len(), tx.ixs[0].prog, tx.ixs[0].data.clone()), (2, 1, 1, vec![9, 9]));
        assert_eq!(r.p, b.len());
        let mut r = Rd { b: &[0x80, 0x01], p: 0 };
        assert_eq!(r.short(), Some(128));
    }
}
