# jaringan-dev-solana — V3 (Pump.fun creators & dev families from DexScreener coins)

Latest numbers (refreshed on every commit): **[STATUS_V3.md](STATUS_V3.md)**. Fix details: [RINGKASAN_V3.md](RINGKASAN_V3.md).
Live coin detection (ShredStream gRPC): [GRPC_SHREDSTREAM.md](GRPC_SHREDSTREAM.md) and [data/shred/GRPC_RESULTS.md](data/shred/GRPC_RESULTS.md).
ML + execution plan: [checkpoints/PLAN_V3_PREDIKSI.md](checkpoints/PLAN_V3_PREDIKSI.md). Resume point: [checkpoints/STATE.md](checkpoints/STATE.md).

All pre-V3 result data (before 20 Nov: not cleaned of exchanges/bots, ATH from the old formula) was removed or
rebuilt from the current V3 data. Data lives in `data/v3/`.

## Goal
1. Collect the creators (devs) of Pump.fun coins that are on DexScreener and trace every coin they ever launched.
2. Merge wallets of the same operator into one family (funding parent -> dev wallets). Exchanges, bots and program
   accounts are dropped.
3. Score reputation: successful coins (ATH >= $50k, not a thin / buy-only pool), rugs, thin coins, how fast the dev sells.
4. Watchlist = families / creators that pass -> watch DEV + PARENT (INDUK) + INTERMEDIARY (PERANTARA) wallets ->
   when they create a new coin, buy as early as possible (same / next block) and hold until the DEX.
   Paper first; live only on the owner's explicit order.

## Findings
- **78% of creators launch only one coin** (one-time wallets). The next coin comes from a new wallet funded by the
  parent, so the PARENT must be watched, not just the creator wallet.
- **The old PumpSwap ATH formula was 26% too low**: pools now carry virtual SOL reserves + token supply in their events.
- **14% of graduations are paired with USDC / another token**, not SOL -> previously read as fake multi-million ATHs
  (SPONGEBOB "$9.5M").
- **Fake giant ATHs**: thin pools (USEF "$97M" from 58 trades) and buy-only pools (EXEN 4,078 buys vs 42 sells).
- **Some official graduations land in an empty pool** (~0.15 SOL, price ~0) -> those coins failed; verified with
  execution prices, not a misread.
- **Exchange / bot cleanup**: every linking wallet is classified; exchanges, super-busy bot wallets and program
  accounts never link a family. Devs that are launch bots are eliminated (`DEV_BOT`).
- **ShredStream gRPC**: Pump.fun creates seen ~0.15-0.9 s after the block (plain RPC: ~2.6 s). Much of the stream
  uses the new Solana transaction format (first byte 0x81). Many creators are spam bots (same name repeatedly).
- **The new Helius key is limited to ~15 calls/s**; above that it answers HTTP 429.
- The vast.ai server **cannot receive UDP** -> raw (UDP) ShredStream does not work there; gRPC does.

## Repository layout
| Path | Content |
|---|---|
| `src/` | Rust program `jaringan` (subcommands). V3 uses: `dexs`, `devtrace`, `fam2`, `famview`, `coinrise`, `athgt`, `reput`, `streamrise`, `udpprobe`, `athdbg` |
| `shredrx/` | ShredStream gRPC receiver (detects Pump.fun creates, matches the watchlist) |
| `config/` | rules & thresholds (`reput.toml` = creator screen, `devtrace.toml`, `budget.toml` = credit cap) |
| `data/v3/reput/` | `watchlist.json/tsv`, `watchlist_koin.tsv` (project archive), `entitas.tsv` (families + independents), `dev_reputation.tsv` & `creator_coins.tsv` (per creator) |
| `data/v3/export/family_anggota.tsv` | every family member: root, wallet, role (DEV/INDUK/PERANTARA/PENERIMA) |
| `family_export/` | family summary, family coins, family wallets (rebuilt from cleaned V3 data) |
| `data/v3/view/` | families active <= 30 days, independent devs, one-time wallets |
| `data/v3/dev/` | coin history per dev (`devs.jsonl`) + creator per coin (`roles.jsonl`) |
| `data/v3/fam2/` | funding links between wallets + wallet classes (exchange / bot / ordinary) |
| `data/v3/ath/` | ATH per coin (formula 3) |
| `data/v3/universe/` | DexScreener coins + on-chain Pump.fun graduations |
| `data/shred/` | live coins from the gRPC stream, rises, results report |
| `data_gz/` | compressed backup of the current `data/v3` |
| `*.sh` | operations: `start_all.sh`, `stop_helius.sh`, `start_shredrx.sh`, `reput_loop.sh`, `grpc_report.sh`, `restart_*.sh` |

## Running (on the VPS)
```
bash start_all.sh        # devtrace, fam2, coinrise, athgt (rate sized to the Helius key)
bash restart_reput.sh    # reputation + watchlist every 10 minutes
bash start_shredrx.sh    # ShredStream gRPC receiver (needs .allenhark.env)
bash grpc_report.sh      # rewrite data/shred/GRPC_RESULTS.md
bash stop_helius.sh      # stop every Helius job
```
API keys only live in `.env*` files on the server (never committed).
