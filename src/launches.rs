//! P1 launch universe: every tx touching the Pump.fun mint authority (all create/create_v2), no activity filter.
use crate::decode::{self, Ev, MINT_AUTH};
use crate::rpc::Rpc;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

pub fn rows(tx: &Value) -> Vec<Value> {
    let sig = tx["transaction"]["signatures"][0].as_str().unwrap_or("");
    let keys = tx["transaction"]["message"]["accountKeys"].as_array().cloned().unwrap_or_default();
    let payer = keys.first().and_then(|k| k["pubkey"].as_str()).unwrap_or("");
    let signers: Vec<&str> = keys.iter().filter(|k| k["signer"] == true).filter_map(|k| k["pubkey"].as_str()).collect();
    let mut base = json!({"sig": sig, "slot": tx["slot"], "txi": tx["transactionIndex"], "bt": tx["blockTime"],
        "payer": payer, "signers": signers, "fee": tx["meta"]["fee"]});
    if !tx["meta"]["err"].is_null() {
        base["kind"] = json!("failed");
        base["err"] = tx["meta"]["err"].clone();
        return vec![base];
    }
    let (evs, src) = decode::events(tx);
    base["src"] = json!(src);
    let creates: Vec<&decode::Create> = evs.iter().filter_map(|e| if let Ev::Create(c) = e { Some(c) } else { None }).collect();
    if creates.is_empty() {
        base["kind"] = json!("nocreate");
        return vec![base];
    }
    let mut out = Vec::new();
    for (i, c) in creates.iter().enumerate() {
        let creator = c.creator.clone().unwrap_or_else(|| c.user.clone());
        let tr: Vec<&decode::Trade> = evs.iter().filter_map(|e| if let Ev::Trade(t) = e { Some(t) } else { None }).filter(|t| t.mint == c.mint).collect();
        let buys: Vec<&&decode::Trade> = tr.iter().filter(|t| t.buy).collect();
        let mut seen_b = std::collections::HashSet::new();
        let buyers: Vec<&str> = buys.iter().map(|t| t.user.as_str()).filter(|u| seen_b.insert(*u)).collect();
        let own: u64 = buys.iter().filter(|t| t.user == creator || t.user == c.user).map(|t| t.sol).sum();
        let mut r = base.clone();
        let o = r.as_object_mut().expect("object");
        for (k, v) in [
            ("kind", json!("create")), ("ci", json!(i)), ("mint", json!(c.mint)), ("curve", json!(c.curve)),
            ("user", json!(c.user)), ("creator", json!(creator)), ("creator_src", json!(if c.creator.is_some() { "event" } else { "user" })),
            ("name", json!(c.name)), ("symbol", json!(c.symbol)), ("ts", json!(c.ts)), ("vtok", json!(c.vtok)), ("vsol", json!(c.vsol)),
            ("rtok", json!(c.rtok)), ("supply", json!(c.supply)), ("tprog", json!(c.tprog)), ("mayhem", json!(c.mayhem)),
            ("cashback", json!(c.cashback)), ("quote", json!(c.quote)), ("vquote", json!(c.vquote)), ("cfee_bps", json!(c.cfee_bps)),
            ("tx_buys", json!(buys.len())), ("tx_sells", json!(tr.len() - buys.len())), ("tx_buy_sol", json!(buys.iter().map(|t| t.sol).sum::<u64>())),
            ("tx_buyers", json!(buyers)), ("own_buy_sol", json!(own)), ("fee_bps", json!(tr.first().and_then(|t| t.fee_bps))),
            ("tr_cfee_bps", json!(tr.first().and_then(|t| t.cfee_bps))), ("after_vsol", json!(tr.last().map(|t| t.vsol))),
            ("after_vtok", json!(tr.last().map(|t| t.vtok))),
        ] {
            o.insert(k.into(), v);
        }
        out.push(r);
    }
    out
}

fn save_ck(path: &str, st: &Value) -> Result<(), String> {
    let tmp = format!("{path}.tmp");
    fs::write(&tmp, st.to_string()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn run(start: &str, end: &str, out: &str) -> Result<(), String> {
    let s: i64 = start.parse().map_err(|_| "bad start")?;
    let e: i64 = end.parse().map_err(|_| "bad end")?;
    let stem = std::path::Path::new(out).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut rpc = Rpc::new(&format!("launches_{stem}"))?;
    let ck = format!("{out}.ckpt");
    let mut st: Value = fs::read_to_string(&ck).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({
        "start": s, "end": e, "token": null, "bytes": 0, "pages": 0, "listed": 0, "create": 0, "failed": 0, "nocreate": 0,
        "dup": 0, "breaks": 0, "last": [0, 0], "cpi": 0, "log": 0, "done": false}));
    if st["done"] == true {
        println!("done already: {st}");
        return Ok(());
    }
    if st["start"] != s || st["end"] != e {
        return Err("checkpoint window differs from arguments".into());
    }
    let mut f = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(out).map_err(|e| e.to_string())?;
    let bytes = st["bytes"].as_u64().unwrap_or(0);
    f.set_len(bytes).map_err(|e| e.to_string())?;
    let mut seen: HashSet<String> = HashSet::new();
    if bytes > 0 {
        f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        for l in BufReader::new(&f).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&l) {
                seen.insert(v["sig"].as_str().unwrap_or("").to_string());
            }
        }
    }
    f.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let inc = |st: &mut Value, k: &str, n: u64| st[k] = json!(st[k].as_u64().unwrap_or(0) + n);
    loop {
        let mut opts = json!({"transactionDetails": "full", "limit": 100, "encoding": "jsonParsed",
            "maxSupportedTransactionVersion": 1, "sortOrder": "asc", "filters": {"blockTime": {"gte": s, "lt": e}}});
        if let Some(t) = st["token"].as_str() {
            opts["paginationToken"] = json!(t);
        }
        let mut res = match rpc.call("getTransactionsForAddress", json!([MINT_AUTH, opts]), 10) {
            Ok(r) => r,
            Err(err) => {
                save_ck(&ck, &st)?;
                rpc.save();
                return Err(err);
            }
        };
        let data = res["data"].take();
        let data = data.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let mut buf = String::new();
        let mut last = (st["last"][0].as_u64().unwrap_or(0), st["last"][1].as_u64().unwrap_or(0));
        for tx in data {
            inc(&mut st, "listed", 1);
            let sig = tx["transaction"]["signatures"][0].as_str().unwrap_or("").to_string();
            if !seen.insert(sig) {
                inc(&mut st, "dup", 1);
                continue;
            }
            let key = (tx["slot"].as_u64().unwrap_or(0), tx["transactionIndex"].as_u64().unwrap_or(0));
            if key < last {
                inc(&mut st, "breaks", 1);
            }
            last = key;
            for r in rows(tx) {
                let k = r["kind"].as_str().unwrap_or("").to_string();
                inc(&mut st, &k, 1);
                if let Some(src) = r["src"].as_str().filter(|_| k == "create") {
                    inc(&mut st, src, 1);
                }
                buf.push_str(&r.to_string());
                buf.push('\n');
            }
        }
        st["last"] = json!([last.0, last.1]);
        f.write_all(buf.as_bytes()).map_err(|e| e.to_string())?;
        f.flush().map_err(|e| e.to_string())?;
        st["bytes"] = json!(f.stream_position().map_err(|e| e.to_string())?);
        st["token"] = res["paginationToken"].clone();
        inc(&mut st, "pages", 1);
        let finished = data.is_empty() || !st["token"].is_string();
        if finished {
            st["done"] = json!(true);
        }
        save_ck(&ck, &st)?;
        if st["pages"].as_u64().unwrap_or(0).is_multiple_of(25) || finished {
            rpc.save();
            println!("pages={} listed={} create={} failed={} credits_total={} retries={} last_bt={}",
                st["pages"], st["listed"], st["create"], st["failed"], rpc.used(), rpc.retries,
                data.last().map(|t| t["blockTime"].clone()).unwrap_or(Value::Null));
        }
        if finished {
            break;
        }
    }
    rpc.save();
    println!("DONE {st}");
    Ok(())
}

/// Gates on a launches file: G1 order, G3 counts, G5 duplicates, G9 time, roles, units.
pub fn check(path: &str) -> Result<(), String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut src: BTreeMap<String, u64> = BTreeMap::new();
    let (mut dup, mut breaks, mut no_bt, mut no_slot, mut bad_json) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut user_ne, mut mayhem, mut nonsol, mut creator_from_user, mut tok2022) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut seen = HashSet::new();
    let mut creators: HashMap<String, u64> = HashMap::new();
    let mut users = HashSet::new();
    let mut last = (0u64, 0u64);
    let (mut bt_min, mut bt_max) = (i64::MAX, i64::MIN);
    let mut per_hour: BTreeMap<i64, u64> = BTreeMap::new();
    for l in BufReader::new(f).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&l) else { bad_json += 1; continue };
        let k = v["kind"].as_str().unwrap_or("?").to_string();
        *kinds.entry(k.clone()).or_default() += 1;
        let id = format!("{}:{}", v["sig"].as_str().unwrap_or(""), v["ci"]);
        if !seen.insert(id) {
            dup += 1;
        }
        let key = (v["slot"].as_u64().unwrap_or(0), v["txi"].as_u64().unwrap_or(0));
        if key < last {
            breaks += 1;
        }
        last = key;
        match v["bt"].as_i64() {
            Some(b) => {
                bt_min = bt_min.min(b);
                bt_max = bt_max.max(b);
                if k == "create" {
                    *per_hour.entry(b / 3600).or_default() += 1;
                }
            }
            None => no_bt += 1,
        }
        if v["slot"].as_u64().is_none() {
            no_slot += 1;
        }
        if k != "create" {
            continue;
        }
        *src.entry(v["src"].as_str().unwrap_or("?").into()).or_default() += 1;
        let (c, u) = (v["creator"].as_str().unwrap_or(""), v["user"].as_str().unwrap_or(""));
        *creators.entry(c.into()).or_default() += 1;
        users.insert(u.to_string());
        user_ne += (c != u) as u64;
        creator_from_user += (v["creator_src"] == "user") as u64;
        mayhem += (v["mayhem"] == true) as u64;
        let q = v["quote"].as_str().unwrap_or(decode::SYSTEM);
        nonsol += (q != decode::SYSTEM && q != "So11111111111111111111111111111111111111112") as u64;
        tok2022 += v["tprog"].as_str().unwrap_or("").starts_with("TokenzQd") as u64;
    }
    let ph: Vec<u64> = per_hour.values().copied().collect();
    let once = creators.values().filter(|&&n| n == 1).count();
    let multi_launches: u64 = creators.values().filter(|&&n| n >= 2).sum();
    let out = json!({"file": path, "kinds": kinds, "src": src, "bad_json": bad_json, "G5_dup": dup, "G1_breaks": breaks,
        "G9_missing_bt": no_bt, "missing_slot": no_slot, "bt_range": [bt_min, bt_max], "creates_per_hour": ph,
        "creators": creators.len(), "creators_once": once, "launches_by_multi_creators": multi_launches,
        "max_launches_one_creator": creators.values().max(), "users": users.len(), "user_ne_creator": user_ne,
        "creator_from_user_field": creator_from_user, "mayhem": mayhem, "non_sol_quote": nonsol, "token2022": tok2022});
    println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
    Ok(())
}
