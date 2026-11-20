# System map — from data collection to live trading

This is the map of the whole system: what each part does, where its data goes, how to run it, and what is still
missing before going live. Details: [README.md](README.md), [GRPC_SHREDSTREAM.md](GRPC_SHREDSTREAM.md),
[RINGKASAN_V3.md](RINGKASAN_V3.md), numbers in [STATUS_V3.md](STATUS_V3.md).

## 1. The big picture

```
 OFFLINE: build the watchlist (hours, repeats every 10 min)                LIVE: act on new coins (milliseconds)
 ─────────────────────────────────────────────────────────────            ───────────────────────────────────────
 DexScreener + on-chain graduations          (dexs)                        ShredStream gRPC (AllenHark)
        │ coins on DexScreener                                                    │ every new Solana tx, ~0.4 s after the block
        ▼                                                                         ▼
 creator of each coin + its full coin history (devtrace, Helius)           shredrx: decode -> Pump.fun create?
        │                                                                         │ creator / user / payer
        ▼                                                                         ▼
 funding links between wallets -> families    (fam2, famview)              in watchlist.json? (+ new wallets the
   exchanges, bots, program accounts removed                                parent just funded)
        │                                                                         │ YES
        ▼                                                                         ▼
 ATH of every graduated coin                   (athgt, PumpSwap pool)      [PAPER now]  record the hit
 rise / rug on the bonding curve               (coinrise)                  [LIVE later] decide (rules / ML) ->
        │                                                                                size (1/4 Kelly) ->
        ▼                                                                                buy via Jito bundle ->
 reputation + screen + entities                (reput)                                   hold to DEX / exit rule
        │   eliminate: rugs often, thin coins, sells in first seconds, launch bots
        ▼
 watchlist.json  ─────────────────────────────────────────────────────────►  read by shredrx every 60 s
```

## 2. Components
| Step | Program (Rust) | Input | Output | Runs |
|---|---|---|---|---|
| Coin universe | `jaringan dexs` | DexScreener API, Pump.fun migrations (Helius) | `data/v3/universe/` | every 5 min |
| Dev histories | `jaringan devtrace` (2 shards) | universe, Helius `getTransactionsForAddress` | `data/v3/dev/devs.jsonl`, `roles.jsonl` | loop |
| Families | `jaringan fam2` (3 shards) + `famview` | dev histories, Helius + Tatum/Shyft/public RPC | `data/v3/fam2/`, `data/v3/view/` | loop |
| Curve rise / rug | `jaringan coinrise` | dev coins, Helius | `data/v3/rise/` | loop |
| ATH (DEX) | `jaringan athgt` (4 shards) | graduations, PumpSwap pool trades | `data/v3/ath/ath_*.jsonl` | loop |
| Reputation + watchlist | `jaringan reput` (`reput_loop.sh`) | all of the above + `config/reput.toml` | `data/v3/reput/` (`watchlist.json`, `entitas.tsv`, archives) | every 10 min |
| Live detection | `shredrx` | AllenHark gRPC, `watchlist.json` | `data/shred/creates.jsonl`, `hits.jsonl`, `dynamic_watch.jsonl` | always on |
| Rise of live coins | `jaringan streamrise` (`rise_loop.sh`) | `creates.jsonl`, Helius | `data/shred/rise/rise.jsonl` | loop |
| Reports | `grpc_report.sh`, `upgrade_repo.sh` | the data | `data/shred/GRPC_RESULTS.md`, `STATUS_V3.md`, exports | on commit |

## 3. Start / stop (on the VPS, `/root/sol`)
```
bash start_all.sh          # devtrace, fam2, coinrise, athgt
bash restart_reput.sh      # reputation + watchlist every 10 min
bash start_shredrx.sh      # live gRPC receiver
setsid nohup bash rise_loop.sh &   # rise of live-caught coins
bash stop_helius.sh        # stop every Helius job
```
Keys (never committed): `.env` (Helius, Shyft), `.allenhark.env` (gRPC), `.tatum.env`, `.madeonsol.env`, `.coinstats.env`.

## 4. Speed budget (why it matters)
| Stage | Today | Needed for live |
|---|---|---|
| Create seen | ~0.4 s after the block (gRPC) | same |
| Decision (watchlist lookup) | in memory, < 1 ms | same |
| Buy landed | not built | next slot (~0.4 s) via Jito bundle + tip |
| Plain RPC (old way) | ~2.6 s late | not usable |

## 5. Still missing before live (in order)
1. **Complete data**: all dev histories + ATH for every creator (Helius credits / rate limit).
2. **Decode the new transaction format (0x81)** fully in `shredrx`: mint and parent -> new-wallet SOL transfers.
3. **Paper trading**: on each watchlist hit, simulate the buy at the real reachable price, hold to DEX / exit rule,
   log P/L and latency (module `shadow` exists from 18 Nov as a base).
4. **Backtest + ML** (`checkpoints/PLAN_V3_PREDIKSI.md`): past launches of watchlist entities, entry at slot+1,
   walk-forward train / validate / one-time test, simple-rule baseline; ML only if it beats the baseline.
5. **Execution module**: wallet key handling, Jito bundle sender, slippage limits, 1/4 Kelly sizing, kill switch,
   daily loss cap.
6. **Infrastructure**: a VPS near Frankfurt / Amsterdam with UDP (raw ShredStream) or a paid gRPC plan; enough
   vast.ai / VPS balance; GitHub deploy key on the VPS.
7. **Go-live gate**: live trading only on the owner's explicit order, small size first.

## 6. Known risks
- Most freshly launched coins fall (live test: ~75% end below the first reachable price) -> only watchlist entities.
- Spam bots resend the same create; shreds include failed attempts -> confirm the coin exists before acting.
- One-time creator wallets -> the parent wallet is the real signal; parents can change too.
- Thin / buy-only pools fake huge market caps -> excluded from "success".
