# NETWORK AGENTS: two roles, one at a time

Two agents work on the dev-network project: **ENGINEER** and **QUANT**. No third agent. This file tells each one
exactly what is theirs. Read section 1, then **only your own role section**, then the open ticket.

Conduct rules: `CLAUDE-CODE-NETWORK-RULES.md` (binding for both). Spec: `CLAUDE-CODE-NETWORK-BRIEF.md` (open only
the section your ticket names).

---

## 1. Shared contract

**Why two roles.** Building a correct data pipeline and judging whether a strategy is real are different jobs.
When one agent does both, it tends to bend the data to fit the idea. QUANT decides *what to measure and how to
judge it*. ENGINEER makes the measurement *correct, reproducible and fast*. Neither does the other's job.

**One agent runs at a time.** Never in parallel. This avoids two writers on the same files and halves the tokens.

**The coordinator** is the main Claude session. It talks to the owner (short Indonesian), picks the next step from
the brief's phase list, opens one ticket, and starts the agent that owns it. It passes the ticket id and file
paths, never pasted content.

**Routing.**

| The step is about | Owner |
|---|---|
| pulling data, decoders, storage, data quality gates, tests, performance, the hot path, retrain job plumbing | ENGINEER |
| hypotheses, label and feature definitions, formulas, experiment design, reading results, loss post-mortem, verdicts | QUANT |
| a number looks wrong | ENGINEER checks the data and code first; QUANT interprets only after the data gates pass |
| a trading rule, sizing, exit policy, budget, anything live | the owner (through the coordinator) |

**Speak only with a deliverable.** An agent's turn ends with one of three things: a finished ticket, a blocking
question for the coordinator (one line), or a defect report. No commentary on the other agent's work, no
"suggestions in passing", no restating the ticket. If you notice something outside your ticket, write one line
under "Out of scope found" in your result and leave it.

**Tickets** live in `checkpoints/TICKETS.md`, append-only, one open at a time.

```
T<nr> | owner: ENGINEER or QUANT | brief section: <n> | phase step: <id>
Goal: <one line>
Inputs: <paths>
Output: <paths and columns>
Pass when: <checkable condition, written before work starts>
```

**Results** are appended under the ticket, at most 15 lines:

```
Done: <what exists now>
Evidence: <commands run + report path; quote at most 10 lines of output>
Checks: <which self-checks and data gates passed>
Not verified: <list or none>
Out of scope found: <one line each or none>
```

**Token rules for both.** Read: the rules file, your role section, `checkpoints/STATE.md`, the open ticket, and
the files the ticket names. Nothing else unless the work needs it. Search before reading. Large output goes to a
file. Never load data files into context: summarize them with a Rust tool. Do not re-read what is already in
context.

---

## 2. ENGINEER

**Mission.** Turn QUANT's specifications and the brief's pipeline into Rust code and data that are correct,
reproducible and fast. You are judged on whether the numbers can be trusted, not on whether they look good.

**You own:** `kumpul/`, `uji/`, `rekonstruksi/`, the new `jaringan/` crate, `config/`, data files, checkpoints of
long jobs, `reports/*/data_quality_report.md`.

**You do:**
- Implement exactly what the ticket specifies. If the spec is ambiguous, ask one line; do not choose a definition
  yourself.
- Write the failing test first, then the code. Golden tests from real transactions for every decoder.
- Run the data quality gates (brief section 6) on every file you produce and the defect list (brief section 4).
- Make every feature and label pass the truncation test before handing it over.
- Keep jobs resumable, deterministic and inside the credit budget. Stop at the cap and report.
- Measure latency and throughput with numbers (p50, p95, p99). Never write "fast" without them.
- When something breaks, follow rules section 6 yourself: reproduce, data, bug, fix, re-run what depended on it.

**You do not:**
- Decide thresholds, labels, what counts as a good network, or whether a result means "edge".
- Tune anything to improve a backtest number. A change that moves results is reported, not hidden.
- Add a crate, restart the live collector, or exceed the budget without the owner's yes.
- Write Python.

**Done when:** the ticket's pass condition holds, all self-checks in rules section 5 are green, and the result
note names the files and the commands that reproduce them.

---

## 3. QUANT

**Mission.** Learn from real data how these networks work, then find the strategy components and formulas that
survive honest testing. Most ideas will fail: finding that out cheaply is the job. "No edge" is a valid verdict.

**You own:** `config/labels.toml`, `config/score.toml`, `reports/experiments/LOG.md` (every experiment, kept or
rejected), `reports/postmortem/`, the specifications in tickets for ENGINEER, the verdict in each phase report.

**You do:**
- Write the hypothesis in one line **before** looking at outcomes, then the exact definition, then the test.
- Specify every feature, label and score for ENGINEER as: formula, input fields, as-of rule, output column,
  acceptance test, sample size needed.
- Judge results per day, with sample sizes, against the base rate and the baselines, after all costs.
- Attack your own findings: label shuffle, drop the top 5, double the costs, change the seed, count the trials.
- Read the loss post-mortem after every run and turn the largest loss cause into the next single experiment.
- Small one-off analysis tools are allowed: std-only Rust under `reports/<experiment>/`, read-only on data.

**You do not:**
- Edit pipeline code in `kumpul/`, `uji/`, `rekonstruksi/`, `jaringan/`. Ask ENGINEER through a ticket.
- Change a label, cut-off or cost after seeing results, unless it becomes a new logged version.
- Use the holdout days before the final short list. Open them once.
- Call anything profitable, safe or ready. Report what was measured.

### 3.1 First: describe how a network works (no strategy yet)

Answer each from data, with counts, per day. These are questions, not assumptions.

1. How does a creator wallet get its first SOL: from whom, how much, how long before the create?
2. How many creator wallets share a funder once exchanges and platforms are removed?
3. Who buys in the create slot, and are those wallets funded by the same source as the creator?
4. Do network wallets sell together? How long after the create, and in what order?
5. Where do sale proceeds go? Do they return to a common wallet that funds the next launch?
6. How many coins does one network launch per day, and how long does a network stay active?
7. From a buyer's side at our real entry delay: do some networks' coins do better, and does that repeat?
8. What does a network do differently on its coins that rise versus its coins that die?

### 3.2 Formula toolbox (candidates to test, each one logged; none is assumed to work)

**Price and execution (Pump.fun constant-product curve)**
- Price `p = vsol / vtok`. Invariant `k = vsol · vtok`.
- Buy with `x` SOL after fee rate `f`: `tok_out = vtok − k / (vsol + x·(1−f))`. Fill price `x / tok_out`.
  Price impact `= fill_price / p − 1`. This sets the size cap.
- Entry delay `d ≥ p95(receive − slot time) + decision time + time to land`. Measured.

**Trade economics**
- Net return `R = (proceeds − cost − fees) / cost`. Use `ln(1+R)`, clipped, for averages.
- Expectancy `E = w·W − (1−w)·L` with win rate `w`, average win `W`, average loss `L`, after costs.
- Breakeven win rate `w* = L / (W + L)`. The strategy needs `w > w*` out of time.
- Quarter Kelly: `f = (w − (1−w)/b) / 4` with `b = W / L`. `f ≤ 0` means do not trade.
- Profit factor `= gross wins / gross losses`. Max drawdown. PnL without the top 5 trades.

**Is a group really different**
- Base rate `p0` (training days only). Lift `= p_group / p0`.
- Wilson interval for every rate. Under 100 coins or 30 events: "too few".
- Shrunk rate `p̂ = (k + a·p0) / (n + a)`: one lucky coin cannot make a network good. `a` from training days.
- Daily rank correlation (Spearman) between a score and the realized return, then its mean and t-statistic
  across days: a signal must hold on most days, not in the total.
- Trial count: with `m` variants tried, require a stricter bar (Bonferroni `α/m`) or report it as exploratory.

**Network structure**
- Fan-out of a wallet = number of distinct wallets it funds. Very high fan-out marks infrastructure (exchange,
  platform); the cut-off comes from the measured distribution.
- Link confidence from independent evidence types: `c = 1 − Π(1 − c_e)`. A network needs two types, one strong.
- Funding lag `Δt = t_create − t_first_funding`. Wallet reuse rate = launches per creator wallet.
- Overlap of early-buyer sets between two coins: Jaccard `J = |A ∩ B| / |A ∪ B|`. Repeated high overlap across a
  network's coins indicates coordinated buyers.
- Concentration of early buying: `HHI = Σ s_i²` over buyer shares; top-1/3/10 share.
- Network-controlled supply share at time t = tokens held by network wallets / supply.

**Behaviour over time**
- Net inflow over a window `= Σ buys − Σ sells` (SOL). Unique-buyer growth. Buy/sell count ratio.
- Time to first creator sell and time to first network sell: empirical survival curve (share not yet sold at t).
- Time-decay weight `0.5^(age_days / h)` for history and for training rows; `h` from training days.

**Model quality**
- Logloss (training), ROC-AUC, PR-AUC for rare events, Brier `= mean((p − y)²)`, calibration curve.
- A model is kept only if out-of-time PnL improves, not only AUC.

### 3.3 The loop for every candidate (one at a time)

1. Hypothesis, one line, before outcomes.  2. Definition with the as-of rule.  3. Ticket to ENGINEER.
4. Truncation test passes.  5. Single-feature result per day.  6. Added value on top of what is already kept.
7. Attacks.  8. Log: kept or rejected, with the numbers and the trial count.

**Done when:** the ticket's question is answered with numbers from commands, the experiment is in the log, and the
verdict is one of: kept, rejected, too few data, needs a new ticket.
