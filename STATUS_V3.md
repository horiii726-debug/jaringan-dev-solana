# V3 status — 2026-11-20 23:51 UTC

Generated automatically from the data at this commit (`jaringan reput`, `jaringan famview`).
Data collection is STILL RUNNING; numbers grow with later commits.

## Creators (only creators of coins seen on DexScreener)
- Creators: **4384** (dropped, no DexScreener coin: 5004)
- History not traced yet: 3002
- Coins recorded: 27594; coins with a measured ATH: 630; ATH >= $50k (real): 212; >= $100k: 129; >= $1M: 29
- Empty pools (graduated, price ~0): 691; doubtful ATH data: 0
- Creator status: BELUM_CUKUP_DATA 2898, ELIMINASI 754, LOLOS_BELUM_TERBUKTI 583, LOLOS_DATA_KURANG 149
- Elimination reasons: DEV_BOT 13, JUAL_DETIK_AWAL 174, KOIN_SEPI 560, SERING_RUG 77
  (SERING_RUG = rugs often, KOIN_SEPI = thin coins, JUAL_DETIK_AWAL = sells in the first seconds, DEV_BOT = launch bot)

## Families (one operator = one entity)
- Families active <= 30 days: **536**, wallets in them: 4803
- Wallet roles: DEV 719, INDUK 348, PENERIMA 3167, PERANTARA 569 (INDUK = funding parent, PERANTARA = intermediary, PENERIMA = receives profits)
- Independent active devs: 816; one-time dev wallets: 216
- Wallet classes checked: INFRA_BUSY 282, INFRA_LIST 24, NON_WALLET 1264, ORDINARY 4315, (INFRA_LIST = exchange, INFRA_BUSY = bot / super-busy wallet, NON_WALLET = program account; none of them links a family)

## Watchlist
- Entities: **146** (families 26, independents 120)
- Wallets watched (DEV + INDUK + PERANTARA): **219**
- Coins in the watchlist archive: 454, successful (ATH >= $50k): 154
- Entity status: FAMILY:BELUM_CUKUP_DATA 48, FAMILY:ELIMINASI 190, FAMILY:LOLOS_BELUM_TERBUKTI 179, FAMILY:LOLOS_BERKUALITAS 2, FAMILY:LOLOS_DATA_KURANG 24, INDEPENDEN:BELUM_CUKUP_DATA 2835, INDEPENDEN:ELIMINASI 514, INDEPENDEN:LOLOS_BELUM_TERBUKTI 343, INDEPENDEN:LOLOS_DATA_KURANG 120

## Done
- Creators & families from DexScreener coins, screen (rugs / thin coins / early sells / launch bots), watchlist + project archive.
- ATH formula 3 (validated +-5% vs GeckoTerminal); non-SOL pools, thin pools and buy-only pools excluded.
- ShredStream gRPC receiver: Pump.fun creates seen ~0.15-0.9 s after the block, matched to the watchlist (see GRPC_SHREDSTREAM.md).
- Rows broken by the Helius quota cleaned; jobs stop on quota; request rate sized to the key.

## Not done yet
- Data incomplete (ATH and dev histories still running; see "history not traced yet").
- Paper mode (simulated buy when a watchlist entity creates a coin, hold to the DEX, P/L and latency).
- Historical backtest + ML (will a watchlist entity's next coin rise again), walk-forward, simple-rule baseline.
- Mint of new-format (0x81) stream transactions and parent -> new-wallet transfers in that format.

## Problems
- Helius key limited to ~15 calls/s (slow); remaining credits must be checked on the Helius dashboard.
- Low vast.ai balance: the server stops when it runs out.
- ShredStream trials end 21 Nov; longer tests need a paid plan. The vast server cannot receive UDP.
- The VPS has no GitHub login (pushes go through the laptop).
- No watchlist entity has created a coin during the stream tests yet (matching not proven on a real case).
