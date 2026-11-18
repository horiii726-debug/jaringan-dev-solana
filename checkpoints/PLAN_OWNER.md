# Owner phase plan (16 Nov 2026) - binding order: one phase at a time, test, report, checkpoint, then next.
P0 SYSTEM AUDIT - codebase, Helius RPC/WS, storage, VPS, credit use, collector status. Do not modify working systems without reason.
P1 COLLECT DEV WALLETS - all creates; creator/dev wallets; tx history, create time, signers, fee payers, funding.
P2 RECONSTRUCT COIN HISTORY - every coin per dev: ATH, rise from OUR entry price, moonshots, rugs, buy/sell activity, returns after costs.
P3 BUILD FUNDING TEAMS - funder = TEAM n with its dev wallets (verified on-chain funding). Exclude exchanges/infrastructure.
P4 TEAM REPUTATION - GOOD / BAD / TOO_FEW / UNKNOWN. Never GOOD from one moonshot.
P5 BACKTEST - do good teams consistently give profitable coins at our real entry? fees, slippage, rug risk, entry delay.
P6 ML - time-based validation, no future leakage.
P7 SHADOW MONITORING - new coin by good-team dev -> identify team, reputation, market/risk check, signal fast; record accuracy + latency before any live.
