mod famview;
mod qdev;
mod reput;
mod athgt;
mod udpprobe;
mod mosq;
mod fam2;
mod coinrise;
mod unify;
mod famath;
mod legacyclean;
mod famv3;
mod devtrace;
mod dexs;
mod decode;
mod rpc;
mod fund;
mod graph;
mod trades;
mod outcomes;
mod bt;
mod g10;
mod pm;
mod features;
mod mleval;
mod team;
mod score;
mod cats;
mod profile;
mod amm;
mod gradsim;
mod crawl;
mod bq;
mod feat_all;
mod devflow;
mod shadow;
mod history;
mod launches;
mod honest;
mod fwd;
mod lat;
mod moon;
mod rep;
mod net;
mod svc;
mod family;
mod sim5;
mod entry1s;
mod famsel;
mod deadcls;
mod gradml;
mod famexport;
mod ml3;
mod tpsim;

use std::process::ExitCode;

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().collect();
    let r = match a.get(1).map(|s| s.as_str()) {
        Some("launches") if a.len() == 5 => launches::run(&a[2], &a[3], &a[4]),
        Some("check") if a.len() == 3 => launches::check(&a[2]),
        Some("fund") if a.len() == 4 => fund::run(&a[2], &a[3]),
        Some("trades") if a.len() == 4 => trades::run(&a[2], &a[3]),
        Some("outcomes") if a.len() == 5 => outcomes::run(&split(&a[3]), &split(&a[4]), &a[2]),
        Some("bt") if a.len() == 7 => bt::run(&a[3], &a[4], &split(&a[5]), &split(&a[6]), &a[2]),
        Some("g10") if a.len() == 4 => a[2].parse().map_err(|_| "bad n".to_string()).and_then(|n| g10::run(&split(&a[3]), n)),
        Some("pm") if a.len() == 6 => pm::run(&a[3], &a[4], &a[5], &a[2]),
        Some("dq") if a.len() == 4 => features::dq(&split(&a[2]), &split(&a[3])),
        Some("feat") if a.len() == 8 => features::feat(&split(&a[2]), &split(&a[3]), &split(&a[4]), &a[5], &a[6], &a[7]),
        Some("mleval") if a.len() == 6 => mleval::run(&a[2], &a[3], &a[4], &a[5]),
        Some("team") if a.len() == 8 => a[7].parse().map_err(|_| "bad tip".to_string()).and_then(|t| team::run(&split(&a[2]), &split(&a[3]), &split(&a[4]), &a[5], &a[6], t)),
        Some("activity") if a.len() == 4 => fund::activity(&a[2], &a[3]),
        Some("serve") if a.len() == 3 => team::serve(&a[2]),
        Some("score") if a.len() == 6 => score::run(&split(&a[2]), &split(&a[3]), &split(&a[4]), &a[5]),
        Some("cats") if a.len() == 5 => cats::run(&split(&a[2]), &split(&a[3]), &a[4]),
        Some("profile") if a.len() == 5 => profile::run(&a[2], &a[3], &a[4]),
        Some("amm") if a.len() == 4 => amm::run(&a[2], &a[3]),
        Some("gradsim") if a.len() == 7 => match (a[5].parse(), a[6].parse()) { (Ok(t), Ok(p)) => gradsim::run(&split(&a[2]), &split(&a[3]), &a[4], t, p), _ => Err("bad tip/price".into()) },
        Some("crawl") if a.len() == 8 => match (a[3].parse(), a[4].parse(), a[5].parse(), a[6].parse()) { (Ok(d), Ok(m), Ok(p), Ok(s)) => crawl::run(&a[2], d, m, p, s, &a[7]), _ => Err("usage: crawl roots depth max pages since out".into()) },
        Some("migrations") if a.len() == 5 => match (a[2].parse(), a[3].parse()) { (Ok(s), Ok(e)) => amm::migrations(s, e, &a[4]), _ => Err("bad window".into()) },
        Some("bq") if a.len() == 6 => a[4].parse().map_err(|_| "bad max".to_string()).and_then(|m| bq::run(&a[2], &a[3], m, &a[5])),
        Some("creators") if a.len() == 4 => bq::creators(&a[2], &a[3]),
        Some("early") if a.len() == 5 => a[3].parse().map_err(|_| "bad n".to_string()).and_then(|n| bq::early(&a[2], n, &a[4])),
        Some("feat_all") if a.len() == 6 => feat_all::run(&split(&a[2]), &split(&a[3]), &a[4], &a[5]),
        Some("devflow") if a.len() == 4 => devflow::run(&a[2], &a[3]),
        Some("shadow") if a.len() == 4 => shadow::run(&a[2], &a[3]),
        Some("history") if a.len() == 5 => a[3].parse().map_err(|_| "bad pages".to_string()).and_then(|p| history::run(&a[2], p, &a[4])),
        Some("honest") if a.len() == 8 => a[6].parse().map_err(|_| "bad latency_ms".to_string()).and_then(|l| honest::run(&split(&a[2]), &split(&a[3]), &a[4], &a[5], l, &a[7])),
        Some("coverage") if a.len() == 5 => honest::coverage(&split(&a[2]), &split(&a[3]), &a[4]),
        Some("fwd") if a.len() == 6 && a[2] == "link" => a[5].parse().map_err(|_| "bad n".to_string()).and_then(|n| fwd::link(&a[3], &a[4], "dataset/wallets/exchange_list.txt", n)),
        Some("fwd") if a.len() == 6 && a[2] == "eval" => match (a[4].parse(), a[5].parse()) { (Ok(l), Ok(w)) => fwd::eval(&a[3], l, w), _ => Err("usage: fwd eval dir latency_s window_end".into()) },
        Some("lat") if a.len() == 4 => a[2].parse().map_err(|_| "bad secs".to_string()).and_then(|s| lat::run(s, &a[3])),
        Some("moon") if a.len() == 3 => moon::run(&a[2]),
        Some("rep") if a.len() == 6 => a[4].parse().map_err(|_| "bad cutoff".to_string()).and_then(|c| rep::run(&split(&a[2]), &split(&a[3]), c, &a[5])),
        Some("net") if a.len() == 3 => net::run(&a[2]),
        Some("svc") if a.len() == 4 => svc::run(&a[2], &a[3]),
        Some("family") if a.len() == 5 => a[4].parse().map_err(|_| "bad max".to_string()).and_then(|m| family::run(&a[2], &a[3], m)),
        Some("family_report") if a.len() == 3 => family::report(&a[2]),
        Some("sim5") if a.len() == 3 => sim5::run(&a[2]),
        Some("instant") if a.len() == 2 => sim5::instant(),
        Some("entry1s") if a.len() == 5 => entry1s::run(&a[2], &a[3], &a[4]),
        Some("devsum") if a.len() == 4 => entry1s::devsum(&a[2], &a[3]),
        Some("famdead") if a.len() == 3 => deadcls::famdead(&a[2]),
        Some("famexport") if a.len() == 3 => famexport::run(&a[2]),
        Some("ml3") if a.len() == 5 && a[2] == "build" => a[4].parse().map_err(|_| "bad lat_s".to_string()).and_then(|l| ml3::build(&a[3], l)),
        Some("ml3") if (a.len() == 4 || a.len() == 5) && a[2] == "eval" => ml3::eval(&a[3], a.get(4).map(|s| s.as_str())),
        Some("gradml") if a.len() == 4 && a[2] == "build" => gradml::build(&a[3]),
        Some("gradml") if a.len() == 5 && a[2] == "eval" => gradml::eval(&a[3], &a[4]),
        Some("deadcls") if a.len() == 4 => deadcls::deadcls(&a[2], &a[3]),
        Some("famsel") if a.len() == 4 => a[3].parse().map_err(|_| "bad cut".to_string()).and_then(|c| famsel::run(&a[2], c)),
        Some("first_buyer") if a.len() == 2 => sim5::first_buyer(),
        Some("first_buyer_real") if a.len() == 2 => sim5::first_buyer_real(),
        Some("poolcreator") if a.len() == 4 => svc::pool_creators(&a[2], &a[3]),
        Some("roles") if a.len() == 4 => svc::create_roles(&a[2], &a[3]),
        Some("devprof") if a.len() == 4 => svc::dev_profile(&a[2], &a[3]),
        Some("devhist") if a.len() == 5 => a[3].parse().map_err(|_| "bad pages".to_string()).and_then(|p| svc::dev_history(&a[2], p, &a[4])),
        Some("tpsim") if a.len() == 7 => tpsim::run(&split(&a[2]), &split(&a[3]), &split(&a[4]), &a[5], &a[6]),
        Some("kelly") if a.len() == 8 => match (a[5].parse(), a[6].parse(), a[7].parse()) { (Ok(s), Ok(p), Ok(c)) => tpsim::kelly(&a[2], &a[3], &a[4], s, p, c), _ => Err("usage: kelly train test key start_usd sol_usd cap_sol".into()) },
        Some("dexs") if a.len() == 4 => a[3].parse().map_err(|_| "bad cycles".to_string()).and_then(|c| dexs::run(&a[2], c)),
        Some("devtrace") if a.len() == 5 => devtrace::run(&a[2], &a[3], a[4] == "1"),
        Some("famv3") if a.len() == 5 => famv3::run(&a[2], &a[3], a[4] == "1"),
        Some("famreport") if a.len() == 4 => famv3::report(&a[2], &a[3]),
        Some("legacyclean") if a.len() == 5 => legacyclean::run(&a[2], &a[3], &a[4]),
        Some("famath") if a.len() == 4 => famath::run(&a[2], &a[3]),
        Some("unify") if a.len() == 6 => unify::run(&a[2], &a[3], &a[4], &a[5]),
        Some("coinrise") if a.len() == 6 => coinrise::run(&a[2], &a[3], &a[4], a[5] == "1"),
        Some("watchlist") if a.len() == 5 => coinrise::watchlist_now(&a[2], &a[3], &a[4]),
        Some("fam2") if a.len() == 5 => fam2::run(&a[2], &a[3], a[4] == "1"),
        Some("mosq") if a.len() == 4 => a[3].parse().map_err(|_| "bad max".to_string()).and_then(|m| mosq::run(&a[2], m)),
        Some("qdev") if a.len() == 5 => qdev::run(&a[2], &a[3], &a[4]),
        Some("qfam") if a.len() == 6 => qdev::run_family(&a[2], &a[3], &a[4], &a[5]),
        Some("reput") if a.len() == 8 => reput::run(&a[2], &a[3], &a[4], &a[5], &a[6], &a[7]),
        Some("athgt") if a.len() == 5 => athgt::run(&a[2], &a[3], &a[4]),
        Some("udpprobe") if a.len() == 4 => udpprobe::run(a[2].parse().unwrap_or(8001), a[3].parse().unwrap_or(600)),
        Some("athdbg") if a.len() == 5 => athgt::dbg(&a[2], a[3].parse().unwrap_or(0), a[4].parse().unwrap_or(5)),
        Some("famview") if a.len() == 5 => famview::run(&a[2], &a[3], &a[4]),
        Some("graph") if a.len() == 5 => graph::run(&split(&a[3]), &split(&a[4]), &a[2]),
        _ => Err("usage: jaringan launches <start_unix> <end_unix> <out.jsonl> | check <launches.jsonl>".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}

fn split(s: &str) -> Vec<String> {
    s.split(',').filter(|x| !x.is_empty()).map(String::from).collect()
}
