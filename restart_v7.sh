#!/usr/bin/env bash
# 21 Nov: DexScreener-creators-only scope (devtrace v7, athgt scope, reput only_dexscreener) -> restart the three jobs
cd /root/sol
grep -q "jaringan reput data/v3/universe " reput_loop.sh || sed -i "s|jaringan reput data/v3/dev |jaringan reput data/v3/universe data/v3/dev |" reput_loop.sh
bash restart_dev4.sh
bash start_ath.sh
