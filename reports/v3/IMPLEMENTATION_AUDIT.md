# V3 Stage 0 — Implementation audit

Ticket: **T0** | owner: COORDINATOR (audit, no code change) | brief: V3 section 13 Stage 0 | date: 2026-11-20
Goal: map the existing repo, data and external sources to V3; list reusable code, exact gaps and every file to touch.
Pass when: every number below is reproduced by `reports/v3/audit_numbers.sh` (or a command quoted inline), and the
CRITIC scores > 9.0 with 0 open CRITICAL/HIGH.

Reproduce all numbers: `cd /root/sol && bash reports/v3/audit_numbers.sh > /tmp/a.txt && diff /tmp/a.txt reports/v3/audit_numbers.txt`
(read-only, 0 provider credits; labels `N1..N8` below refer to sections of that output).

Round 2 (after CRITIC `reports/v3/critic/T0_r1.md`, 3.0 FAIL): findings F1–F16 addressed below; new numbers are in
`audit_numbers.txt` section N9. The only code change in Stage 0 is the behaviour-neutral clippy fix of section 1
(needed for the D6 gate). No config, data or test was changed. Files written: `reports/CLAUDE-CODE-DEXSCREENER-FAMILY-V3.md`
(the brief, verbatim), `reports/v3/audit_numbers.sh`, `reports/v3/audit_numbers.txt`, this file,
`data/v3/madeonsol/*` and `data/v3/dexscreener/api_reference.html` (raw source docs/probes), `.madeonsol.env`
(key, gitignored via `*.env`, never printed). `data/` was extracted from the repo archive `data_gz/` (N2).

---

## 1. Environment and git state

| Item | Value | Source |
|---|---|---|
| VPS | vast.ai instance 61662586, 128 vCPU, 251 GB RAM, **16 GB disk** (2.1 GB used after extract), RTX 5060 Ti 16 GB (idle) | `nproc; free -g; df -h /; nvidia-smi` |
| Project path | `/root/sol` (fresh `git clone` of horiii726-debug/jaringan-dev-solana) | — |
| HEAD | `d5f0374`, 603 tracked files, 3 untracked paths (`data/`, `reports/v3/`, the brief) | N1 |
| Toolchain | rustc 1.100.0 (2026-11-09), cargo + clippy via rustup (minimal profile) | `rustc --version` |
| `cargo build --release` | OK | `/tmp/build.log` |
| `cargo test --release` | **26 passed, 0 failed** | `cargo test --release 2>&1 \| grep "test result"` |
| `cargo clippy --release --all-targets -- -D warnings` at HEAD | FAIL, 10 errors in `family.rs:356`, `entry1s.rs:55`, `deadcls.rs:82,172`, `gradml.rs:73,115`, `ml3.rs:18,19,98,204` | `cargo clippy ... 2>&1 \| grep -E "^error" -A3` |
| same, after the T0 fix (working tree) | **exit 0**; `cargo test --release` still **26 passed** | `cargo clippy --release --all-targets -- -D warnings; echo $?` |
| CatBoost binary | not installed on this VPS | `which catboost` (empty) |
| Live collector `sol-kumpul` | **does not exist on this VPS** (new instance) — nothing to restart | `ps aux` |

Clippy fix (uncommitted, `git diff --stat`: 5 files, 9+/8−), each a semantics-preserving rewrite:
`sort_by(|a,b| b.0.cmp(&a.0))` → `sort_by_key(|s| Reverse(s.0))` (both stable, same order; family.rs, deadcls.rs);
`.filter(p).last()` → `.rfind(p)` (entry1s.rs, deadcls.rs); `split('\t').last()` → `.next_back()` (gradml.rs,
ml3.rs); `x < A || x >= B` → `!(A..B).contains(&x)` (gradml.rs, ml3.rs); one blank `//!` doc line (ml3.rs). Why the
lints appear now is not established (the 18 Nov modules were committed after the clippy-clean STEP_LOG entry);
no claim is made about it. `modul_18nov/*.rs` were byte-identical copies of the pre-fix `src/` files.

## 2. Binding documents

| Document named by V3 | Status |
|---|---|
| `CLAUDE-CODE-NETWORK-RULES.md`, `CLAUDE-CODE-NETWORK-AGENTS.md`, `CLAUDE-CODE-NETWORK-BRIEF.md` | present |
| `checkpoints/STATE.md`, `STEP_LOG.md`, `config/budget.toml` | present (STATE.md at HEAD: 38 lines, `git show HEAD:checkpoints/STATE.md \| wc -l`, dated 18 Nov, several stacked blocks instead of the section-8 template; T0 prepended a 10-line V3 block → 48 lines, N9 `state_md_lines`) |
| `CLAUDE.md`, `STATUS-SOLANA.md` | **missing in repo** |
| `reports/review_jaringan-dev-solana_2026-11-18.md`, `reports/FIX-BRIEF-jaringan-dev-solana.md` | **missing in repo** (V3 1.1 numbers come from it; STEP_LOG 18 Nov holds the honest-result lines) |
| V2 brief (`CLAUDE_CODE_DEXSCREENER_FAMILY_RESEARCH_V2_RING1.md`) | **missing** — V3 section 12 says "keep V2 sections 5, 7, 9 as written"; without V2 those are only known through V3's summary |
| `.claude/agents/critic.md` | **not created** — writing it (and the CRITIC section in AGENTS.md) was blocked by the local permission classifier on 2026-11-20; needs the owner's explicit permission. Reviews of T0 use the verbatim 3.6 text as the reviewer prompt. |

Conflict: `CLAUDE-CODE-NETWORK-AGENTS.md` says "No third agent"; V3 0/3.1 says the owner approved CRITIC → V3 wins
(more specific, owner-dated), recorded here; AGENTS.md edit pending permission.

## 3. Data inventory (N2, N3)

- Archive `data_gz/data.tgz.p_aa..p_at`: 20/20 MD5 OK; 1,906 entries = 1,851 files + 55 dirs; 1,355,146,592 bytes
  uncompressed (N2, N9). `data/` holds 1,859 files = those 1,851 + 8 files written by this audit under `data/v3/`.
- `dataset/` (in git, 207M by `du -sh dataset`): `launches/` 10 daily gz files 6–15 Nov (7,500 … 48,204 rows/day;
  6 Nov partial), `coins/`, `ml/`, `reputasi/`, `teams/`, `trades/` (curve + PumpSwap trades, gz), `wallets/`.
- `data/launches_2026-11-16/17/18.jsonl`: 46,755 / 51,174 / 3,179 rows (18 Nov partial).
- Pump.fun graduations = unpromoted on-chain source, three files (all columns `bt, mint, pool, quote, sig, slot`;
  N9 `src` lines; rows = distinct mints in each):

  | file | rows | wSOL-quote mints |
  |---|---|---|
  | `data/mig_oct.jsonl` | 38,996 | 35,363 |
  | `data/raw/migrations.jsonl` | 9,019 | 7,603 |
  | `data/mig_new.jsonl` | 1,783 | 1,545 |
  | **union** | **49,255** | **44,057** (the "44,057 graduated" of STATE.md; `roots.sh:9` uses this wSOL filter) |

- DexScreener snapshot `data/roots/ds.jsonl`: 5,992 rows, **5,984 distinct mints (8 duplicate rows)**; columns
  `dex, liq, mcap, mint, pair_created, sym, vol24` only; dexId pumpswap 5,945 / pumpfun 39 / meteora 8.
  Coverage: 5,969 of its mints are in the 49,255 union (5,247 in the wSOL union) → the snapshot covers ~12% of
  graduated mints; the rest were not found on DexScreener or not fetched (not distinguishable from stored data).
  Defects vs V3 4.1: one pair per mint (`max_by(.liquidity.usd)`, `roots.sh:21`), no raw JSON, **`mcap` =
  `.marketCap // .fdv` (`roots.sh:21`) → FDV silently substituted when marketCap is missing (T14)**, snapshot time
  not stored per row, universe seeded only from Pump.fun graduations.
- Current-snapshot counts (`mcap` as stored, may be FDV): ≥ $50k 240, ≥ $100k 152, ≥ $1M 66 (N9). These are
  **current** values, not peaks; V3 5 forbids ATH from current snapshots.
- Provider keys on this VPS (N7, names only): `.env` present with `HELIUS_API_KEY` (added 2026-11-20 after round 1);
  `.rpc.env`, `.bitquery_token` missing (Shyft/Bitquery not available); `.madeonsol.env` present (`MADEONSOL_KEY`).
- Credit ledgers `checkpoints/credits*` are from the old instance/key; not meaningful for a new key.

## 4. Module map to V3 (reuse vs gap)

| V3 section | Existing code | Reuse | Gap (what V3 needs that does not exist) |
|---|---|---|---|
| 4.1 DexScreener | shell+curl+jq: `ds_pull.sh`, `ds_par.sh`, `roots.sh`, `dexfirst.sh` (`/tokens/v1/solana/<≤30 mints>`) | endpoint choice only | Rust module `src/dexs.rs` + `config/dexscreener.toml` (endpoints and limits from docs fetched 2026-11-20: `tokens/v1` ≤ 30 addresses, `token-pairs/v1`, `pairs/v1` 300 req/min; profiles/boosts/takeovers/ads 60 req/min); raw JSON storage; all pairs; `CURVE`/`AMM` per pair from owner program; `fdv_proxy` flag; discovery-source registry; `data_coverage_and_gaps.md`. `ureq`/`serde_json` already in Cargo.toml → no new crate. |
| 4.2 Provider | `src/rpc.rs` `Rpc` (rate limit, retry/backoff, per-job credit ledger, hard cap from `config/budget.toml`, key masked) | **yes** | signature cache (none found: `grep -n cache src/rpc.rs src/fund.rs src/family.rs src/svc.rs` empty); per-run `research_runs` row; keys (section 3). |
| 4.2 cross-check source | none | — | MadeOnSol (section 6) as cross-check only. |
| 5 Tiers/outcomes | `src/honest.rs` (reachable entry: first trade with slot > create slot and bt ≥ create+latency, NOT_FILLED, CENSORED; truncation + 1-slot-shift tests) | **yes** | `config/tiers.toml`; liquidity floor at the moment (STEP_LOG 18 Nov: honest.rs "no liquidity filter" → T15); peak-mcap history source; `DEX_REACHED` vs `PERFORMANCE_SUCCESS`. |
| 6 Creator | `src/decode.rs` (Pump CreateEvent/TradeEvent from IDL `idl/pump.json`, self-CPI + logs), `src/svc.rs::create_roles` (oldest tx: creator, user, fee_payer, signers) | decoder **yes** after golden tests | golden tests from real signatures (decode tests are synthetic round-trips only); `create_roles` fills `creator = user` when the event lacks `creator`, and for non-Pump venues `creator = fee_payer` → violates V3 6 / T5 (must be separate fields, `UNKNOWN` otherwise); no `mint_authority`, `metadata_update_authority`, tx/ix index, parser version. |
| 7 Wallet classes | `src/svc.rs::classify` (KNOWN_LIST / EXCHANGE_SERVICE / BOT_HUB / OPERATOR / UNKNOWN), `data/roots/labels.tsv`, `data/p3/exchange_list.txt`, `dataset/wallets/exchange_list.txt` | logic idea only | thresholds hard-coded (≥ 1000 sigs and ≥ 40 tx/h, ≥ 30 counterparties; `config/graph.toml` `infra_min_sigs=3000`, `busy_tx_per_hour=150`) and not measured per V3 7.1; behaviour uses the **newest** 1000 signatures (not as-of → T11); labels have **2 columns, no source/date** (N4) → cannot be reused as curated labels (rule 9); missing classes PROGRAM, PDA (off-curve), TOKEN_ACCOUNT→owner, CEX_DEPOSIT, SWAP_SERVICE, MEV_TIP, SPAM_DUSTER, PLATFORM_CREATOR, UNCLASSIFIED (fail-closed); unknown currently → `OPERATOR` when not busy (fail-open). |
| 8.1 Transfers | `decode::sol_moves` (System transfer/transferWithSeed/createAccount(WithSeed), outer + inner) | **yes** as base | instruction index per move (`sol_moves` returns only (src, dst, lamports)); cross-check of each move against pre/post balances; SPL/wSOL/Token-2022 transfers with owner-at-slot; `sol_moves` itself does not check `meta.err` (callers do in `launches.rs:16`, `fund.rs:59,125`, `family.rs` tx loop) → the V3 extractor must reject failed txs itself; rent/close/fee/tip removal; dust floor measured; `edge_candidates_removed` log. |
| 8.2–8.3 Family | `src/family.rs` (up: first-funder chain to exchange/busy wallet; down: outgoing SOL ≥ 0.05 SOL since 1 Oct), `src/fund.rs::funder_in`, `src/graph.rs`, `team.rs`, `net.rs`, `crawl.rs` | **no** (rebuild) | see section 5. |
| 9 Proofs | none | — | all 7 items. |
| 11.1–11.3 Leakage | `honest.rs` truncation test; `honest::coverage` (`jaringan coverage`) reproduced the has_fund leak | **yes** | coverage-leak test per new feature (11.3); outcome-blind tracing budget; holdout ≥ 3 newest days. |
| 11.4 Statistics | `bt.rs:82` `wilson(k, n)`; shrink `a`, base rate, no-top-5, costs ×2 in `bt.rs` report | **yes** | per-day Spearman, trial count/Bonferroni label in V3 reports. |
| 11.5 PnL | `honest.rs` (entry/exit reconstructable, CENSORED) | partly | realised vs mark-to-market columns per wallet. |

Section 12 logical datasets (V3 12 + additions):

| Dataset | Existing | Gap |
|---|---|---|
| `dex_tokens` | `data/roots/ds.jsonl` (reduced, 1 pair) | rebuild in Stage 1 with raw JSON, `fdv` separate |
| `dex_pairs` | none | Stage 1, all pairs, `CURVE`/`AMM` |
| `token_outcomes` | `dataset/coins/outcomes_j1.csv.gz` (legacy, invalid per 18 Nov review), `honest.rs` output | tiers + liquidity floor, reachable entry |
| `token_creators` | `data/roots/alive_roles.jsonl` (via `svc::create_roles`, roles merged per section 4 row 6) | Stage 2 separate roles |
| `wallets` | `data/family_a/wallets.jsonl` (legacy family members) | Stage 3 |
| `wallet_transfers` | none valid (`family_a/edges.jsonl` = balance-delta legacy) | Stage 3 extractor |
| `token_trades` | `dataset/trades/*.gz`, `data/launches_*.jsonl` | reuse after gates G1–G12 |
| `family_edges` | legacy `family_a/edges.jsonl` | Stage 4 with E-id, grade, as-of |
| `wallet_token_behavior`, `family_token_behavior` | none | Stage 5 |
| `research_runs` | `checkpoints/credits/*.txt` (credits per job only) | one row per run: command, version, credits, hashes |
| `wallet_classes`, `edge_candidates_removed` | `data/roots/labels.tsv` (2 columns, no source) | Stage 3 |

Other: `devtrace.py` (Python) is in the repo root — not used, must stay unused (rule 3). Many analyses were awk/shell;
V3 analyses will be Rust.

## 5. Old family data measured against V3 (owner asked 2026-11-20 to clean exchange-linked data)

From N5 (`data/family_a`, `family_export/`):
- `family_a/edges.jsonl`: 61,915 edges (54,683 distinct signatures), 89 roots; top 3 roots hold 8,661 / 5,146 /
  4,355 edges (giant-component candidates, T9). 10,283 edges < 0.1 SOL, 42,410 < 1 SOL; materiality relative to the
  receiver's spend is not checked (V3 E1).
- Edges with an endpoint in the 91-address infra union (labels EXCHANGE_SERVICE/BOT_HUB/KNOWN_LIST + both exchange
  lists): **0** — listed exchanges were stopped at. But the stop list itself has no source/date.
- Busy wallets kept as members: 803 of 2,714 `family_a` wallets have `busy=true`; in `family_export` 703 rows are
  role `RAMAI` (305–400 tx in the sample) inside **55 of 97 families** → under V3 these are unclassified-or-infra
  nodes inside families (cap 6.0 condition).
- Upward chains ended at an exchange/service for 58 of 97 exported families (EXCHANGE 2, EXCHANGE_SERVICE 21,
  KNOWN_LIST 35), "induk_akumulasi" heuristic 31, max_depth 8.
- Method defects (code reading): `family.rs::transfers` uses **balance deltas only** (forbidden alone by 8.1) and
  counts any account gaining ≥ 0.05 SOL — PDAs/vaults/token accounts are not excluded (T7/T8); `fund.rs::funder_in`
  takes the first transfer of the oldest tx with no dust/materiality check and falls back to the largest balance
  drop (T6); every single edge unions (no second independent evidence, 8.3/T9); no as-of slot on edges (T11);
  `family.rs` has no tests.
- Legacy graph reconciliation (N9): `family_a/wallets.jsonl` has 92 roots, `edges.jsonl` 89 roots, the export 97
  families, of which 8 are not edge roots (the export is built by `famexport` from `up.jsonl` + `wallets.jsonl` +
  `creates.jsonl`, not from edges, so sizes differ, e.g. a family with 2 exported wallets can have >100 edges);
  3,075 signatures produce > 1 edge (max 20 recipients in one tx = disperse/bot pattern, T19); 3,049 child wallets
  have > 1 parent; 9 exported wallets sit in > 1 family; export row count reconciled: 2,745 rows = role sum 2,745
  (the earlier 2,744 was a `wc -l` undercount, file has no trailing newline; script fixed). CRITIC r1 also found
  3 of 6 DEV wallets in 5 sampled families unreachable by any down edge. All of this confirms the verdict below.

Verdict for reuse: old families, `family_export/*.csv` and `data/roots/labels.tsv` are **not evidence under V3**.
Proposal (non-destructive, rule 7): leave files in place, tag them `LEGACY_PRE_V3` in STATE.md, never read them as
input to Stage 3/4; raw transactions they reference (signatures) may be re-fetched through the cache.

## 6. External sources checked 2026-11-20

**DexScreener** (`data/v3/dexscreener/api_reference.html`, fetched today): `GET /tokens/v1/{chainId}/{tokenAddresses}`
(up to 30 addresses), `/token-pairs/v1/{chainId}/{tokenAddress}`, `/latest/dex/pairs/{chainId}/{pairId}`, search —
300 req/min; token profiles / boosts / community takeovers / ads / orders — 60 req/min. No enumeration of all Solana
tokens (gap stays as V3 4.1 says).

**MadeOnSol** (owner-provided key; OpenAPI 1.20.0, 181 paths, saved `data/v3/madeonsol/openapi.json`; 3 calls used):
- Key tier **BASIC**: 200 calls/day, burst 60/min (`probe_me.json`). Free-tier live feeds are delayed 5 min.
- FREE gate: `/token/{mint}` (includes `deployer.wallet`, `market_cap`, `fdv_usd`), `/deployer-hunter/{wallet}`,
  `/deployer-hunter/{wallet}/tokens`. PRO gate: `/wallet/{address}/funding` ("shared-funder evidence"),
  `/wallet/{address}/flags`, `/wallet/batch/classify`, `/deployer-hunter/{wallet}/as-of`, `/tokens/{mint}/bundles`,
  `/tokens/{mint}/peak-history`.
- Cross-check 1/1: MadeOnSol deployer of 3MG (`CZiKhMmp…pump`) = our creator `55vi54w1…` (N6). n=1, not a rate.
- Use under V3: cross-check source only (4.2) — never creates an edge; current-state deployer stats are not as-of
  (look-ahead if used as a feature). Quota 200/day (N9) = at most 200 `/token/{mint}` cross-checks per day
  (spot-check only, not the universe).

## 7. Gaps per stage and files to touch

| Stage | New / changed files | Blocked on |
|---|---|---|
| 1.0 | done in T0 (section 1); commit together with Stage 1 work (no push) | — |
| 1 | `src/dexs.rs` (new), `src/main.rs` (subcommand), `config/dexscreener.toml`, `config/tiers.toml`, `data/v3/universe/` (registry + raw JSON), `reports/v3/data_coverage_and_gaps.md` | — (DexScreener is free; no key) |
| 2 | `src/svc.rs::create_roles` → new `src/creators.rs` (separate roles, UNKNOWN), golden tests in `tests/golden/` from real signatures | Helius key (.env) |
| 3 | `src/classes.rs`, `src/xfer.rs` (new), `config/graph.toml` (v2, measured), `config/labels_infra.tsv` (with source/date), `reports/v3/wallet_cleaning_report.md` | Helius key; off-curve check: a new crate (owner yes), an in-crate ed25519 point-decompression routine (no crate, but must be golden-tested against known PDAs), or a `getAccountInfo` owner check (credits) |
| 4 | `src/famv3.rs` (new; `family.rs` kept as legacy), `reports/v3/false_link_review.md` | Stage 3 |
| 5–6 | `src/timeline.rs`, reuse `honest.rs`; CatBoost GPU optional | CatBoost binary install (dependency → owner yes) |
| 7 | shadow only, reuse `shadow.rs` | Stage 6 result |

Credits: Stage 1 = 0 provider credits. DexScreener load for the 49,255-mint union: ⌈49,255/30⌉ = 1,642
`tokens/v1` calls → ≥ 6 min at 300/min (N9 `ds_calls_for_union_all`; lower bound, retries extra). Stage 2+
estimate: from a measured 50-coin sample once the Helius key is in place (key added to `.env` on 2026-11-20 after
this audit's round 1; budget cap for it still to be set by the owner).

## 8. Owner requests 2026-11-20 mapped

- **Exchange-linked old data**: section 5 — old families not reused; rebuild under V3 7–8 (T1 rule: a hot-wallet
  withdrawal links nobody).
- **Universe from DexScreener**: Stage 1 (all venues, all pairs), plus the unpromoted migration feed (T16).
- **"Successful coin → wallets linked to its dev = one family"**: V3 8.3 rule (STRONG path + 2nd independent
  evidence). Losers and random controls must be traced with the same depth (T17), otherwise any pattern is
  survivorship.
- **Owner 2026-11-20: trace only devs of coins that reached a tier (≥ $50k, ≥ $100k, …; higher is better), not
  every coin** (saves time and credits). Plan for Stage 2: tracing set = coins whose **peak** (V3 5, liquidity
  floor, not the current snapshot) reached ≥ $50k, ordered by tier, plus a **random control sample** of
  below-tier coins of fixed size chosen before outcomes are looked at, traced with the same depth and budget
  (T17, V3 11.3). Without controls a family pattern cannot be told apart from what every dev does. Control size
  is a QUANT decision in the Stage 2 plan.
- **Use the GPU**: graph/transfer work is CPU-bound (128 cores); the GPU is useful for CatBoost GPU in Stage 6.
- **"Zero error, zero bug"**: enforced as: clippy + tests green, golden tests from real transactions, truncation and
  coverage-leak tests, CRITIC gate > 9.0. This lowers bug risk; it cannot prove zero bugs.

## 9. Open items needing the owner

1. Permission to write `.claude/agents/critic.md` and the CRITIC section in AGENTS.md (blocked locally).
2. Helius key: present in `/root/sol/.env` since 2026-11-20 (`getSlot` OK); credit cap for it not yet set.
3. Missing docs (CLAUDE.md, STATUS-SOLANA.md, review 18 Nov, FIX-BRIEF, V2) — copy to the VPS if they exist.
4. Disk is 16 GB: raw DexScreener + transaction caches must stay compressed; may need a bigger disk later.
5. MadeOnSol BASIC (200/day) = spot-check only; PRO would add funding/flags/bundles (paid → owner decision).
6. `audit_numbers.sh` counts with shell/jq/awk (NETWORK-RULES 2.3: shell only to launch programs). Accepted for a
   read-only Stage 0 inventory; from Stage 1 all counts come from Rust subcommands. Owner may overrule.

T0 is logged in `checkpoints/TICKETS.md` and `STEP_LOG.md`; `STATE.md` gets a V3 block on top (rules section 4).
