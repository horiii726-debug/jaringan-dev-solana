# P0 sample and cost (2026-11-16, measured)
- Program id 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P and IDL from github.com/pump-fun/pump-public-docs (idl/pump.json). Events: CreateEvent, TradeEvent, CompleteEvent via self-CPI (100% of creates decoded from CPI, 0 from logs).
- Universe source: getTransactionsForAddress on mint authority TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM (in every create).
- 1 h (2026-11-15 12-13 UTC): 1,437 tx = 1,336 creates + 95 failed + 6 other; 160 credits; 39 s. Day 2026-11-09: 36,936 creates, ~4,000 credits.
- 53% of creates are Mayhem mode; 14% quote is not SOL (USDC or others); 99.7% Token-2022.
- Helius billing (docs 2026-11-16): gTFA full 10 credits/100 tx; getSignaturesForAddress and getTransaction 1 credit; free RPC 10 req/s. Latency 1.7-3.4 s per archival call -> parallel shards.
- Funder lookup: ~2.2 credits per wallet. Trades per coin (30 min, succeeded only, cap 500): ~25 credits average.
- Budget: config/budget.toml cap 300,000 credits.
