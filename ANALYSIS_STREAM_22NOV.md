# Stream analysis — 22 Nov 2026 (gRPC coins, laptop-side, VPS offline)

Data: `data/shred/creates.jsonl` (1,008 creates, 656 unique coins, 21 Nov 06:16-~07:00 WIB) and
`data/shred/rise/rise.jsonl` (94 coins with a measured price). Dev-buy data: `data/shred/devbuy.csv`
(first 20 txs of each coin via Helius). Sample is small (~1 hour) — treat as direction, not rules.

## 1. Buying every new coin ~0.4 s after the create does not pay
| Reached at least | Coins |
|---|---|
| +10% | 61 / 94 (65%) |
| +20% | 53 (56%) |
| +50% | 33 (35%) |
| +100% | 22 (23%) |
| +400% | 6 (6%) |
| never +10% | 33 (35%) |
| last price below entry | 65 (69%) |
| rug (>= 90% off the peak) | 28 (30%) |

Buy all, sell at a target, else hold (3% round-trip cost, assuming a perfect fill at the target):
+20% -> -3.8% per trade (55/94 win); +50% -> -6.8% (42/94); +100% -> -4.9% (36/94). Peaks come 8-33 s after entry.

## 2. Current state (DexScreener pump.fun curve pairs, 22 Nov)
94 coins: 20 at/above their peak (mostly tiny $3-5k mcap, no volume), 25 at 50-99%, 21 at 10-49%, 28 dead (<10%).
Only 1 coin above $10k (s5 $44.7k), 0 above $50k, 0 graduated.

## 3. What differs at launch
| Group | n | dev SOL at create (median) | dev >= 1 SOL |
|---|---|---|---|
| rose >= 100% | 22 | 0.11 | 5% |
| rose 10-100% | 39 | 0.21 | 5% |
| quiet (< 10%) | 33 | 0.07 | 15% |
| rug | 28 | 0.12 | 4% |
| held (>= entry) | 29 | 0.11 | 7% |

By dev buy: 0-0.3 SOL (76 coins) random (28% >=2x, 34% quiet, 34% rug); 0.3-1 SOL (10) 50% held, none 2x;
1-3 SOL (7) mostly quiet (57%). Buyers in the first 3 slots: median 1 wallet in every group.
**Dev buy size and first-block buyers do not separate rises from rugs.** (Dev SOL = payer balance change incl.
~0.02 SOL fees/rent.)

Creator history inside the stream session: new creators 16% >=2x / 46% quiet / 23% rug; serial creators 34% >=2x /
18% quiet / 39% rug; both ~70% end below entry -> serial creators pump and dump.

## 4. Why the watchlist produced 0 hits
140 of 155 watchlist entities launched only one coin (one-time wallets). Watching creator wallets is useless;
the signal must come from parent (funding) wallets and repeat families, and shredrx cannot yet read SOL transfers
in the new (0x81) transaction format.

## 5. Next candidates
Pattern of the first 5-30 s (unique buyers, dev selling), creator funding source, same-day history of the creator,
metadata socials; larger sample (hundreds of coins); parent-wallet funding detection.
