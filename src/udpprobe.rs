//! UDP probe (21 Nov, ShredStream trial): bind 0.0.0.0:<port>, count packets / bytes / source addresses for <secs>,
//! print a summary line every 10 s and the first bytes of the first packet. Answers "do raw shreds reach this box?".
//! Usage: jaringan udpprobe <port> <secs>

use serde_json::json;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

pub fn run(port: u16, secs: u64) -> Result<(), String> {
    let sock = UdpSocket::bind(("0.0.0.0", port)).map_err(|e| format!("bind {port}: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_secs(1))).map_err(|e| e.to_string())?;
    let (start, mut last) = (Instant::now(), Instant::now());
    let (mut n, mut bytes, mut first): (u64, u64, Option<String>) = (0, 0, None);
    let mut src: HashMap<String, u64> = HashMap::new();
    let mut buf = [0u8; 2048];
    println!("{}", json!({"listening": format!("0.0.0.0:{port}"), "secs": secs}));
    while start.elapsed() < Duration::from_secs(secs) {
        if let Ok((k, from)) = sock.recv_from(&mut buf) {
            n += 1;
            bytes += k as u64;
            *src.entry(from.ip().to_string()).or_insert(0) += 1;
            if first.is_none() {
                first = Some(buf[..k.min(32)].iter().map(|b| format!("{b:02x}")).collect());
            }
        }
        if last.elapsed() >= Duration::from_secs(10) {
            last = Instant::now();
            println!("{}", json!({"t_s": start.elapsed().as_secs(), "packets": n, "bytes": bytes, "sources": src, "first_bytes": first}));
        }
    }
    println!("{}", json!({"done": true, "packets": n, "bytes": bytes, "sources": src, "first_bytes": first}));
    Ok(())
}
