#!/usr/bin/env python3
"""18 Nov: koin DexScreener yang naik ratusan/ribuan % -> dev -> keluarga atau individu.
Hanya membaca data yang sudah terkumpul (0 kredit Helius).
Kenaikan dari candle jam PumpSwap (GeckoTerminal, harga dalam SOL):
  dari_lulus = ATH / open candle pertama ; dari_luncur = ATH / harga awal Pump.fun (2.8e-8 SOL).
Keluarga dev (aturan ketat pemilik): funder/penerima bukan exchange/bot/wallet ramai, dan transfer >= 0.05 SOL.
  K1 funder dev yang sama juga mendanai dev koin DexScreener lain
  K2 dev menerima SOL dari dev DexScreener lain / funder dev lain sebelum launch
  K3 dev mengirim untung ke dev DexScreener lain / funder dev lain
"""
import json, glob, os, sys
from collections import defaultdict

R = "/root/jaringan/data/roots"
LAUNCH_PX = 2.8e-8
GRAD_PX = 4.1e-7


def jl(p):
    out = []
    if not os.path.exists(p):
        return out
    for line in open(p):
        line = line.strip()
        if line:
            try:
                out.append(json.loads(line))
            except Exception:
                pass
    return out


ds = {d["mint"]: d for d in jl(f"{R}/ds.jsonl")}
creator = {}
for d in jl(f"{R}/pool_creators.jsonl"):
    creator[d["mint"]] = d["coin_creator"]
for d in jl(f"{R}/alive_roles.jsonl") + jl(f"{R}/roles.jsonl") + jl(f"{R}/roles2.jsonl"):
    if d.get("creator"):
        creator[d["mint"]] = d["creator"]

labels = {}
for line in open(f"{R}/labels.tsv"):
    p = line.rstrip("\n").split("\t")
    if len(p) >= 2:
        labels[p[0]] = p[1]
svc_ns = {d["w"]: d.get("n_sigs", 0) for d in jl(f"{R}/svc.jsonl")}


def bad(w):
    l = labels.get(w, "")
    if l in ("EXCHANGE_SERVICE", "BOT_HUB", "KNOWN_LIST"):
        return l
    if l == "OPERATOR" and svc_ns.get(w, 0) >= 1000:
        return "OPERATOR_ramai"
    if w.startswith("1111111111"):
        return "SYSTEM"
    return ""


# funder dev (transfer langsung)
funder = {}
for p in ("ds_fund_l1.jsonl", "alive_fund.jsonl", "a_fund_l1.jsonl"):
    for d in jl(f"{R}/{p}"):
        if d.get("funder") and d.get("kind") == "transfer" and d["w"] not in funder:
            funder[d["w"]] = (d["funder"], (d.get("lam") or 0) / 1e9)
funder2 = {}
for d in jl(f"{R}/a_fund_l2.jsonl"):
    if d.get("funder") and d.get("kind") == "transfer":
        funder2[d["w"]] = d["funder"]

ds_devs = defaultdict(list)  # dev -> mints DexScreener
for m in ds:
    if m in creator:
        ds_devs[creator[m]].append(m)
funded_devs = defaultdict(set)  # funder -> devs DexScreener
for dv in ds_devs:
    if dv in funder:
        funded_devs[funder[dv][0]].add(dv)

prof = {d["w"]: d for d in jl(f"{R}/alive_devprof.jsonl")}

rows = []
for f in glob.glob(f"{R}/gt/*.hour.json"):
    mint = os.path.basename(f).split(".")[0]
    try:
        js = json.load(open(f))
        c = js["data"]["attributes"]["ohlcv_list"]
    except Exception:
        continue
    if not c or mint not in ds:
        continue
    c.sort(key=lambda x: x[0])
    # ATH dari harga CLOSE (wick satu trade diabaikan); harga lulus Pump.fun ~ 85 SOL / 206.9M token
    closes = [x[4] for x in c if x[4] and x[4] > 0]
    if len(closes) < 2:
        continue
    first = GRAD_PX
    ath = max(closes)
    now = closes[-1]
    sol_usd = (ds[mint].get("mcap") or 0) / (now * 1e9) if now else 0
    if not first or first <= 0:
        continue
    dv = creator.get(mint, "?")
    pr = prof.get(dv, {})
    fu, fsol = funder.get(dv, (None, 0))
    sig = []
    why_unknown = ""
    if fu:
        b = bad(fu)
        if b:
            why_unknown = f"dana dari {b}"
        else:
            others = funded_devs.get(fu, set()) - {dv}
            if others:
                sig.append(f"K1 funder {fu[:8]} juga danai {len(others)} dev lain")
            if fu in ds_devs:
                sig.append(f"K1 funder {fu[:8]} sendiri dev koin DS")
    else:
        why_unknown = "funder tak ketemu (wallet lama/ramai)"
    for x in pr.get("inflow_before", []):
        w = x.get("from")
        if x.get("sol", 0) >= 0.05 and w and not bad(w) and w != dv and (w in ds_devs or (funded_devs.get(w, set()) - {dv})):
            sig.append(f"K2 terima {x['sol']:.2f} SOL dari {w[:8]} (dev/funder lain)")
    for x in pr.get("outflow_after", []):
        w = x.get("to")
        if x.get("sol", 0) >= 0.05 and w and not bad(w) and w != dv and (w in ds_devs or (funded_devs.get(w, set()) - {dv})):
            sig.append(f"K3 kirim {x['sol']:.2f} SOL ke {w[:8]} (dev/funder lain)")
    if sig:
        cls = "KELUARGA"
    elif why_unknown:
        cls = "TAK_TERLACAK"
    else:
        cls = "INDIVIDU"
    out_sum = sum(x.get("sol", 0) for x in pr.get("outflow_after", []))
    rows.append(dict(
        mint=mint, sym=ds[mint].get("sym"), mcap=ds[mint].get("mcap") or 0,
        up_grad=100 * (ath / first - 1), up_launch=100 * (ath / LAUNCH_PX - 1), now_vs_grad=100 * (now / first - 1),
        dev=dv, n_ds_coins=len(ds_devs.get(dv, [])), funder=fu, fsol=fsol, f2=funder2.get(fu) if fu else None,
        one_time=pr.get("one_time_wallet"), age=pr.get("age_days_at_launch"), out_sol=out_sum,
        cls=cls, sig=sig, unk=why_unknown, sol_usd=sol_usd))

import statistics
med = statistics.median([r["sol_usd"] for r in rows if r["sol_usd"] > 0])
print(f"cek harga: SOL implisit median ${med:.0f}; koin harga tak cocok mcap (dibuang): "
      f"{sum(1 for r in rows if not (0.5*med < r['sol_usd'] < 2*med))}")
rows = [r for r in rows if 0.5 * med < r["sol_usd"] < 2 * med]
rows.sort(key=lambda r: -r["up_grad"])
json.dump(rows, open(f"{R}/devtrace.json", "w"))
thr = float(sys.argv[1]) if len(sys.argv) > 1 else 100
big = [r for r in rows if r["up_grad"] >= thr]
print(f"koin dgn candle={len(rows)}  naik>={thr:.0f}% sejak lulus={len(big)}")
for k in ("KELUARGA", "INDIVIDU", "TAK_TERLACAK"):
    print(f"  {k}: {sum(1 for r in big if r['cls']==k)}")
print("\nsym | naik sejak lulus | dari luncur | sekarang | mcap | dev | koin DS dev | funder | kelas | bukti")
for r in big[:60]:
    print(f"{r['sym']} | +{r['up_grad']:.0f}% | +{r['up_launch']:.0f}% | {r['now_vs_grad']:+.0f}% | ${r['mcap']/1e3:.0f}k | "
          f"{r['dev'][:8]} | {r['n_ds_coins']} | {(r['funder'] or '-')[:8]} {r['fsol']:.2f}SOL | {r['cls']} | "
          f"{'; '.join(r['sig']) or r['unk']}")
