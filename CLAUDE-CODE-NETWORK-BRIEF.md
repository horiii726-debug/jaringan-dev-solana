# NETWORK BRIEF: Solana dev-network detection, instant-entry backtest, self-learning model

Full specification. Rules of conduct are in `CLAUDE-CODE-NETWORK-RULES.md` and win over this file on safety.
**Read only the section you need for the current step.** Evidence tags: **[V]** measured by a command on real data ·
**[I]** inferred · **[U]** unknown, must be measured before use.

Contents: 1 Objective · 2 Definitions · 3 What the data already shows · 4 Data defects that must not recur ·
5 Architecture and speed · 6 Data quality gates · 7 Wallet network graph · 8 Network score · 9 Backtest ·
10 Machine learning · 11 Loss post-mortem · 12 Leak, overfit and bug checklist · 13 Phases · 14 Budget ·
15 Files · 16 Unknowns

---

## 1. Objective

Owner's hypothesis: coins that rise are launched by networks (syndicates) that rotate creator wallets. So the thing
to track is the network, not one wallet. When a coin is created, look up the creator's network, and buy at once if
the network is good.

Four claims must be proven on real data, in this order. If one fails, stop that branch and report.

| # | Claim | Proven when |
|---|---|---|
| H1 | Creator wallets can be linked into networks by on-chain evidence | a measured share of single-launch creators shares a non-exchange funder with another creator |
| H2 | A network's past coins predict its next coin | out-of-time, per day: coins of "good" networks beat the base rate, from the buyer's entry price |
| H3 | The edge survives a realistic entry delay and all costs | backtest at the measured delay, costs doubled, still not losing on most days |
| H4 | The decision can be made in time | measured event-to-decision latency fits inside the delay used in H3 |

"No edge" is a valid result. Never present research as ready for live trading.

## 2. Definitions

| Term | Meaning |
|---|---|
| Creator | the `creator` field of the create event/instruction, decoded against the current IDL |
| Create signer (`user`) | the wallet in the create event's `user` field. Not always the creator |
| Fee payer | pays the transaction fee. Never assumed to be the dev |
| Funder | the wallet that sent the first observed incoming SOL to a wallet |
| Network | a set of wallets joined by at least two independent evidence types (section 7), with a confidence |
| Infrastructure wallet | exchange hot wallet, bridge, launchpad/platform account, router, program-owned account. Never a network member and never an edge |
| Decision time `t_d` | the moment our collector receives the create event |
| Entry delay `d` | time from the create slot to our fill. Measured, never assumed zero |
| As-of | a value computed only from data with slot ≤ the create slot of the coin being scored |
| Moonshot | defined once in `config/labels.toml` before outcomes are computed: a multiple of **our entry price** within a horizon. Never a multiple of the creation price |

## 3. What the data already shows (repo data, 2–3 Nov 2026, measured 15 Nov 2026)

Tool and saved output: `reports/dev_history_20261115/` (`dev.rs`, std-only Rust; input = the `create` rows of
`hist/`, `hist2/`, `hist_dex/` on stdin, argument =
`reports/moonshot_reconstruction/20261109_local_f34775c/od2/token_outcomes.csv`; `CLEAN=1` keeps only DBC coins with
complete history). Re-run before quoting.

- 4,697 launches from 2,613 creators. 2,188 creators launched once; 19 launched more than 20 times. [V]
- Pump.fun: 2,065 launches, 1,090 creators, 879 with one launch (81%). Yet 1,186 launches (57%) come from the 211
  creators with two or more. [V]
- Pump.fun: `user` differs from `creator` in 390 of 2,065 creates. Counted by `user` there are 861 wallets. [V]
- Outcome "2x within 2 h, from the price at age 120 s", clean labels only: 68 of 1,685 (4.0%). [V]
- Creators with an earlier launch whose outcome had closed and was a 2x: 5 labeled cases in total. Not enough to
  say anything about H2. [V]
- Creators with an earlier closed launch and no 2x: 0 of 132 hit 2x. Direction only: too few events. [V]
- Meteora DBC creators with 49–78 launches in a day exist. Probably platform accounts. [I]
- Earlier research (`uji/LOG-EKSPERIMEN.md`, not re-run): no tested variant made money after costs; the ranking
  model finds risers but trades exit at 0.80–0.84x on average.

Limits: under 2 days of data, Pump.fun rows pre-filtered to coins with ≥ 20 transactions in 2 minutes, no funding
data, no PumpSwap rows. These numbers are a starting point, not conclusions.

## 4. Data defects that must not recur

Each one produced a wrong conclusion before. Every new pull is checked against this list (gates in section 6).

| Defect | What happened | Rule now |
|---|---|---|
| Survivor filter | Pump.fun history kept only coins with ≥ 20 tx in 2 min (`tarik.rs`, `MIN_TX_2M`) | the launch universe has **no** activity filter; every create is a row |
| Winner-only pull | DBC rows pulled from a list of graduates: 60 of 60 "hit 2x", which made dev history look 96% right | label only coins whose history is complete to the declared horizon; report the others separately |
| Cut at graduation | 0 PumpSwap rows, so graduated coins were valued at the curve peak | follow the coin after migration or mark it censored; never fill a missing future |
| Broken price after migration | DAMM v2 price jumps above 300x from unit errors | keep raw integer amounts and decimals from the mint account; reconcile price against reserves |
| Failed transactions dropped | failure rate unknown | store failed transactions with a flag |
| Same-slot order lost | order inside a slot was file order | store slot, transaction index and instruction index |
| Two clocks mixed | live rows use receive time, history uses chain time | store both in every row; never compare one against the other |
| Drained-pool spikes | a 1,196x "moonshot" from an empty pool | a price counts only with a liquidity floor at that moment |
| Mayhem coins | reserves do not match price | flag and report separately |

## 5. Architecture and speed

### 5.1 Two paths

```
SLOW PATH (batch, minutes to hours)                 HOT PATH (per event, in memory)
backfill -> funding graph -> clusters                WebSocket create event
-> network scores (as-of) -> model training          -> decode -> wallet lookup -> score lookup
-> writes one read-only snapshot  ================>   -> model -> signal (shadow) -> journal
```

The hot path never calls the network and never recomputes history. Everything it needs is precomputed by the slow
path and swapped in as one snapshot.

### 5.2 Watching many wallets at once

- **One** subscription covers every launch: `logsSubscribe` on the Pump.fun program (the existing `sol-kumpul`
  pattern). The number of watched wallets does not change the number of connections.
- The watchlist is a hash map in memory: wallet → network id → score. A lookup costs the same for 100 wallets or
  for 1,000,000.
- **Know the wallet before it creates.** A network that rotates wallets must fund the new wallet first. Watch
  outgoing SOL transfers from network wallets and add each funded wallet to the watchlist as "candidate member"
  with the evidence. Then the create event is a pure lookup. How to subscribe to many accounts on the current
  provider plan is [U]: read the provider's current WebSocket reference first, and fall back to polling recent
  signatures of the highest-score networks only.
- A creator that is not in the map is "unknown" at `t_d`. A funder lookup by RPC at that moment is allowed only if
  the measured round trip fits the delay budget; otherwise the coin is skipped for the network strategy
  (fail closed) and the lookup still runs afterwards to grow the graph.

### 5.3 Fast decisions

- Model inference runs in-process in Rust on a fixed-size feature vector. No process spawn, no file read, no
  allocation-heavy work per event. CatBoost is trained with the C++ binary and applied by a Rust evaluator that
  must match the binary within 1e-5 on a stored check file.
- Snapshot swap is atomic (build new, then replace the pointer). The hot path never waits on the slow path.
- A reconnect triggers a backfill from the last confirmed slot. Events are deduplicated by signature.
- **Measure, never claim.** Report p50, p95, p99 for: chain slot time → event received, received → decoded,
  decoded → decision. Solana slots are about 0.4 s; the entry delay used in the backtest must be at least the
  measured p95 of the whole chain plus the time to land a transaction [U].

Speed is not the edge here. Without Jito the first slots belong to snipers. The edge, if any, is knowing the
network. The backtest decides whether that is enough at our real delay.

## 6. Data quality gates (every pull, every file)

Provider: Helius RPC behind a small Rust interface (`get_address_history`, `get_transaction`,
`get_program_events`, `stream_program_events`), so another provider can be added. Method names, prices and limits
change: check the live API reference before relying on any of them.

A file is usable only when all gates pass. Failures go to `reports/<phase>/data_quality_report.md` and to
`data_quality_issues.jsonl`; nothing is skipped silently.

| Gate | Check | Pass |
|---|---|---|
| G1 Continuity | each page starts where the previous cursor ended; slots never go backwards | 0 breaks |
| G2 Errors | every HTTP error, timeout and rate limit is retried with backoff and logged; an unrecovered range is recorded as a gap | 0 silent skips |
| G3 Count | signatures listed = transactions fetched = rows decoded + rows logged as undecodable | exact |
| G4 Decode | undecodable share per program and per instruction | reported; above 0.5% stops the step |
| G5 Duplicates | by signature + instruction index | removed and counted |
| G6 Roles | creator, `user`, fee payer and signers stored as separate fields; fixture test on real transactions where they differ | 100% on fixtures |
| G7 Units | raw integer amounts kept; decimals read from the mint; no fallback constant | 0 fallbacks used |
| G8 Price | curve trades: price within 2% of the constant-product price from the reserves in the row | failures above 1% stop the step |
| G9 Time | chain time and slot in every row; receive time added for live rows | 0 missing |
| G10 Independent check | random sample of at least 200 launches re-fetched with plain `getTransaction` from a second RPC endpoint | creator and mint match ≥ 99%; every mismatch explained |
| G11 Coverage | report states the slot/date window, the gaps, the share of coins with complete history | written |
| G12 Section 4 list | each defect in section 4 checked by name | all "not present" or flagged |

Golden tests: every decoder has tests built from real transactions (signature + expected values). No decoder is
used without them.

## 7. Wallet network graph

### 7.1 Edges (each stored with signatures, amounts, slots, first and last seen, count)

| Edge | Weight class |
|---|---|
| A funds B's first SOL shortly before B creates a coin | strong |
| A and B funded by the same non-infrastructure wallet | strong |
| creator sends tokens to B, B sells | strong |
| sale proceeds of A and B go to the same wallet | strong |
| B buys in the create slot of A's coin, repeatedly across coins | medium |
| same create-signer or fee payer for different creators | medium |
| one same-slot buy, a shared name or image | weak: never enough alone |

### 7.2 Rules

- A network needs **two independent evidence types**, at least one strong. One weak signal joins nothing.
- Infrastructure wallets are detected by measured fan-out (they fund or receive from very many unrelated
  wallets); the cut-off is set from the measured distribution and stored in `config/graph.toml`. They are removed
  as nodes before clustering.
- Tracing is bounded: maximum depth and a credit cap per wallet in config. Record where and why a trace stopped.
- Every edge has an as-of slot. A network as seen at coin X's creation uses only edges with slot ≤ X's create
  slot. A wallet that joins a network later must not change earlier scores.
- Clusters are versioned. A rebuild never rewrites the raw evidence.

## 8. Network score (as-of, from the buyer's side)

For a coin created at slot `s` by a creator in network `N`:

- History of `N` = its earlier coins whose outcome window closed before `s`.
- Each past coin's outcome is the return a buyer would have had: entry at the measured delay, the frozen exit
  policy, all costs. Not the network's own profit and not the raw peak.
- Score = shrunk estimate: `(k + a·p0) / (n + a)` for hit rate and the same idea for mean log return, where `p0` is
  the base rate measured on training days only and `a` is set on training days only. One lucky coin must not make
  a network "good".
- States: `UNKNOWN` (no closed history), `TOO_FEW`, `GOOD`, `BAD`. Cut-offs come from training days and live in
  `config/score.toml`. `UNKNOWN` is never treated as bad or as good.
- Also store per network: launches per day, share of coins where the creator sold in the first minute, share of
  coins with create-slot bundles, median time to the first creator sell.

## 9. Backtest: buy at creation, gated by the network

1. Universe: every create in the window. No activity filter.
2. Decision at `t_d` using only: the create transaction itself, the as-of network snapshot, as-of creator history,
   market state before `t_d`.
3. Entry price: the price our order would get at `t_d + d`, for `d` in a pre-registered set (measured p50, p95,
   and 1 s, 2 s, 5 s). Computed from the curve reserves after all trades up to that moment, plus our own price
   impact. **Never the creation price.**
4. If the coin has no observable state at `t_d + d`, the trade is recorded as "not filled", not as a win or loss.
5. Exits: the owner's current rule (no take-profit, no trailing: exit on creator/network selling, a large sell,
   buy pressure falling) is the baseline. Research variants are reported next to it, each as its own experiment:
   exit when network wallets sell, partial sale on a sharp rise, time stop. The owner chooses; nothing is switched.
6. Costs in every run: protocol and creator fee read from the decoded rows (`fee_bps`, `cfee_bps`), base fee,
   measured priority fee, curve slippage at our size, measured failed-transaction rate, a sandwich haircut. Then
   the same run with costs doubled.
7. Sizing: quarter Kelly + compounding, with a cap as a share of curve liquidity. If Kelly is ≤ 0 the result is
   "do not trade".
8. Report per day and in total: trades, not filled, win %, average win, average loss, risk-reward, breakeven
   win %, profit factor, PnL, PnL without the top 5 trades, max drawdown. Compare against two baselines: buy every
   coin, and buy coins of `UNKNOWN` creators.
9. Pass: better than both baselines on most days, realized win % above breakeven, not losing with doubled costs.

## 10. Machine learning

Start from the existing code in `uji/src/ml.rs` (CatBoost through the C++ binary, MLP in Rust, walk-forward).
Read it before adding anything. No new model family until this one is shown to be the limit.

### 10.1 Targets (defined in `config/labels.toml` before any outcome is computed)

| Target | Type | Loss | Why |
|---|---|---|---|
| `net_win`: PnL after costs > 0 under the frozen entry and exit | binary | Logloss | the thing we are paid for |
| `rug_early`: creator or network dumps within the first minutes | binary | Logloss, PR-AUC reported | the main way to lose |
| `log_ret`: clipped log return after costs | regression | RMSE on the clipped value | magnitude, for sizing |

The label depends on the exit policy. When the exit policy changes, labels are rebuilt and models retrained.
Class weights and time-decay weights (newer days weigh more) are set on training days only.
A model is judged by trading PnL under section 9, not by AUC alone. Probabilities are calibrated on a
time-ordered validation fold before any absolute threshold or sizing uses them.

### 10.2 Features (all as-of `t_d`)

Network score and state, network size and age, creator's own closed history, funding fingerprint (time from
funding to create, amount, funder class), create-transaction facts (creator's initial buy inside the create
transaction, `user` ≠ `creator`, token program, quote), market heat before `t_d` (creates and trades per minute).
Each feature: one-line hypothesis written first, truncation test, single-feature AUC, leave-one-out test, kept or
rejected in the log.

### 10.3 Training protocol

Walk-forward by day. Purge at least the label horizon between train and test. Newest days held out and opened
once. Cluster-aware: a network never sits in both train and test when the cold-start model is evaluated.
Attacks on every candidate: label shuffle (AUC about 0.5), drop the top 5 trades, change the seed, double costs.

### 10.4 The model learns by itself (no owner prompt needed)

A scheduled Rust job runs after each completed data day and after every `N` new closed outcomes (`N` in config):

1. Check the new data against section 6. Bad data is quarantined and **not** trained on.
2. Rebuild labels and as-of features for the new rows.
3. MLP: online update on the new outcomes plus replay from recent ones.
4. CatBoost: train a challenger on all data up to yesterday.
5. Score champion and challenger on the newest out-of-time fold: AUC, PR-AUC, Brier, and backtest PnL.
6. Promote the challenger only if it is not worse on AUC **and** on PnL. A challenger trained on shuffled labels
   must be rejected by this same code (unit test).
7. Drift guard: if the champion's out-of-time AUC falls below the configured floor, the model output is switched
   off (rules only) until a challenger passes.
8. Write one line per run to `reports/ml/retrain_log.md`: data range, rows, seed, metrics of both, decision.

Automatic promotion applies to research and shadow only. Anything that would affect real money is the owner's
decision.

## 11. Loss post-mortem: learn from every loss at once

Every closed trade (backtest and shadow) gets one primary reason code, computed by rule from the trade's own data:

| Code | Rule (data after entry) |
|---|---|
| `CREATOR_DUMP` | creator sold before our exit and price fell |
| `NETWORK_DUMP` | network wallets sold before our exit |
| `BUNDLE_DUMP` | create-slot buyers sold as a group |
| `NO_FOLLOW` | too few new buyers after entry |
| `LATE_ENTRY` | entry price already far above the price at the create slot |
| `EXIT_LATE` | peak was above entry, exit below it |
| `COSTS` | gross result positive, net negative |
| `NOT_FILLED` / `FAILED_TX` | never entered, or the transaction would have failed |
| `OTHER` | none of the above: must be inspected by hand in a sample |

After every backtest run and every shadow day, the job writes `reports/postmortem/<date>.md`: count and SOL lost
per code, per day, and the top networks by loss. Then, without being asked:

- the largest loss code becomes the next experiment (one hypothesis, one change, logged);
- wallets and networks behind `CREATOR_DUMP`, `NETWORK_DUMP`, `BUNDLE_DUMP` have their scores updated from that
  slot onward (as-of, never backwards);
- if a code can be predicted at `t_d`, it becomes a candidate feature and goes through 10.2;
- if `EXIT_LATE` or `COSTS` dominate, the problem is the exit or the size, not the selection: say so plainly.

## 12. Leak, overfit and bug checklist (run at the end of every phase)

1. Truncation test on every feature and label. Future-shift test must fail.
2. No value derived from the whole dataset enters a feature (base rates, cut-offs, scalers: training days only).
3. Network membership and scores are as-of. Re-scoring an old coin with today's graph is a leak.
4. Labels defined and frozen before outcomes are looked at. Any change gets a new version id.
5. Per-day results, not only totals. Minimum sample rule. Every trial counted and reported.
6. Too-good rule (rules file, section 5).
7. Costs on, costs doubled, top 5 removed, seed changed, labels shuffled.
8. `cargo test --release`, `cargo clippy -- -D warnings`, golden decode tests, determinism hash, restart test.
9. No `unwrap()`/`expect()` on the hot path, no silenced errors, no `unsafe`.
10. Data gates of section 6 on every input file of the phase.

## 13. Phases (one step at a time; each step has a pass condition)

| Phase | Steps | Deliverables | Gate to pass |
|---|---|---|---|
| **P0 Audit and sample** | P0.1 read the repo and the VPS state · P0.2 verify program ids and IDL against current official docs · P0.3 one-hour sample: creates per hour, credits per call type, receive-vs-chain delay from live data · P0.4 extrapolate the cost of the window, set `config/budget.toml` | `reports/p0/sample_cost.md`, `config/*.toml`, `.env.example` (names only) | cost estimate written; owner approves the budget |
| **P1 Launch universe** | P1.1 puller for all creates in the window, resumable · P1.2 decode with roles separated · P1.3 gates G1–G12 · P1.4 creator table and platform-account detection | `data/launches_<day>.jsonl.gz`, `reports/p1/discovery_summary.md`, `reports/p1/data_quality_report.md` | G10 ≥ 99%, no silent gaps, budget kept |
| **P2 Outcomes** | P2.1 trades per coin on the curve and after migration · P2.2 measured entry delay set · P2.3 buyer-side outcomes at each delay · P2.4 failed-transaction rate | `data/outcomes_v1.csv`, coverage report | price gate G8, censoring explicit |
| **P3 Funding graph** | P3.1 first funder for a random sample of 200 single-launch creators (tests H1 cheaply) · P3.2 decide from the measured rate whether to continue · P3.3 bounded tracing for all creators and create-slot buyers · P3.4 infrastructure removal and clustering | `data/edges_v1.jsonl.gz`, `data/networks_v1.csv`, `reports/p3/graph_report.md` | H1 reported with counts; false-link sample reviewed |
| **P4 Network score** | as-of scores, shrinkage, states | `data/network_scores_v1.csv` | truncation test passes |
| **P5 Backtest** | section 9 in full, post-mortem of section 11 | `reports/p5/backtest.md` | H2 and H3 answered per day |
| **P6 Model** | section 10, auto-retrain job, champion/challenger | models, `reports/ml/` | beats rules-only out of time, or reported as no gain |
| **P7 Shadow** | hot path of section 5, latency report, signals journal, daily post-mortem | `reports/p7/latency.md`, shadow journal | H4 answered; at least 50 shadow trades, win ≥ 35%, equity rising on 2 consecutive days |
| **Live** | out of scope | none | owner's explicit word only |

P3.1 runs early on purpose: if almost no single-launch creator shares a funder, H1 fails cheaply and the plan
changes before the expensive tracing.

## 14. Budget

- `config/budget.toml`: window start and end, `max_provider_credits`, per-wallet trace cap, sample sizes.
- Known so far: `getTransactionsForAddress` cost 10 credits per 100 full transactions and 10 credits flat for
  signatures; the free plan had 1,000,000 credits per month (from `STATUS-SOLANA.md`; re-check).
- One live hour on 6 Nov 2026 had 1,546 new coins. A 30-day window is then about 1.1 million creates [I]:
  creates alone fit the free plan, trades and funding traces for every coin do not. So: measure in P0, then pick
  the window (for example 7 days) or a random sample of coins. A sample must be random, never chosen by outcome.
- Signature-only and count modes first. Full payloads only for rows that need decoding.
- When the cap would be exceeded: stop cleanly at a checkpoint and report. Never continue on hope.

## 15. Files and layout

Keep the existing structure (`kumpul/`, `uji/`, `rekonstruksi/`). New code goes into the crate it belongs to, or
one new crate `jaringan/` for graph and scores. Data stays JSONL.gz and CSV (existing convention); Parquet needs a
new dependency, so it needs the owner's yes. Large data and secrets are never committed. Reports go under
`reports/<phase>/`, state under `checkpoints/`, configuration under `config/`.

The Windows PC cannot build `kumpul` (no gcc/dlltool); `uji` and `rekonstruksi` build there. Collection and
provider calls run on the VPS (`/root/sol`).

## 16. Unknowns to measure first

- How many creator wallets share a funder (H1). [U]
- Whether any network repeats success from the buyer's side (H2). [U]
- Our real delay from create slot to received event, and to a landed transaction. [U]
- Current Pump.fun fees and whether `fee_bps`/`cfee_bps` in old rows still hold. [U]
- Which WebSocket subscriptions the current provider plan allows for many accounts. [U]
- Failed-transaction and sandwich rates at our size. [U]
- Whether the 49–78 launches-per-day DBC creators are platform accounts. [U]
