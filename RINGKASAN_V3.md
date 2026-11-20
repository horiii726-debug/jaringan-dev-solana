# V3 summary — creator reputation, watchlist, ShredStream (21 Nov 2026)

Old result data (dataset/, data_baru_18nov/, old reports/, old scripts) was removed or rebuilt: much of it was ruled
invalid in the 18 Nov review, it was not cleaned of exchanges/bots, and its ATH used a wrong formula. Git history
still has it. Current data lives in `data/v3/`; numbers in [STATUS_V3.md](STATUS_V3.md).

## Goal
Collect creators (devs) and their families from coins on DexScreener -> score reputation (coins reaching >= $50k,
few rugs, no selling in the first seconds, coins not thin) -> watchlist -> watch DEV + PARENT + INTERMEDIARY
wallets -> when a watchlist entity creates a new coin, buy as early as possible (same / next slot) and hold to the
DEX. ML & execution plan: `checkpoints/PLAN_V3_PREDIKSI.md`.

## Key fixes
1. PumpSwap ATH formula was 26% too low: Buy/Sell events now carry `virtual_quote_reserves` and `base_supply`.
   Price = (quote + virtual quote) / base x supply. Validated +-5% against GeckoTerminal.
2. 14% of graduations pair with USDC / another token, not SOL -> previously read as fake multi-million ATHs
   (e.g. SPONGEBOB "$9.5M"). Only SOL-quoted pools are used now.
3. Confirmed ATH: the price must hold for 3 consecutive trades; events with implausible fields are dropped;
   computed price cross-checked against small-trade execution prices (DATA_RAGU when they often disagree).
4. Fake "successes" dropped: thin pools (< 200 trades / < 50 traders) and buy-only pools (sells < 5%).
5. 7.8k creators had never been traced (orphans in roles.jsonl) -> devtrace v6/v7 queues them.
6. When the Helius quota ran out, devtrace/coinrise wrote empty rows that counted as done -> they now stop; the
   broken rows were removed.
7. Parallel writers append one line per write (no interleaved lines between processes).
8. Request rate sized to the Helius key (~15 calls/s); Shyft + Tatum + public RPC serve light calls.
9. Launch-bot devs (wallets classed as exchange / super-busy) are eliminated as `DEV_BOT`.
10. ShredStream: spam bots resend the same create; failed attempts appear in shreds before the real coin
    (40-80 s earlier) -> the rise tool matches each stream create to the on-chain create closest in time.

## Screening rules (config/reput.toml)
- Only creators whose coins appear on DexScreener.
- ELIMINATED: rugs > 40% of measured coins | >= 50% thin coins | first sell <= 10 s on >= 50% of coins | launch bot.
- Success = ATH >= $50k (not thin / buy-only). A family is one entity (all its DEV wallets judged together).
- Watchlist = passed entities with >= 1 successful coin, active <= 30 days. Watched wallets: DEV + PARENT +
  INTERMEDIARY (78% of creators use one-time wallets; the next coin comes from a new wallet funded by the parent).

## Data files (data/v3/)
- `reput/watchlist.json|tsv` — watchlist + watched wallets; `reput/watchlist_koin.tsv` — project archive per entity.
- `reput/entitas.tsv` — every entity + status/reason; `reput/dev_reputation.tsv`, `creator_coins.tsv` — per creator.
- `ath/ath_*.jsonl` — ATH per coin (formula 3); `dev/` — dev histories; `fam2/`, `view/`, `export/` — families.
- `universe/` — DexScreener coins + on-chain graduations.

## Programs (Rust)
- `src/reput.rs` (reputation + screen + watchlist), `src/athgt.rs` (ATH from pool trades), `src/devtrace.rs` (dev
  histories), `src/streamrise.rs` (rise of live-caught coins), `shredrx/` (ShredStream gRPC receiver).
- Run everything: `bash start_all.sh` + `bash start_shredrx.sh`; stop: `bash stop_helius.sh`.
