#!/usr/bin/env bash
# 21 Nov: Helius "max usage reached" -> stop every Helius job (dexs DexScreener polling and mosq keep running)
pkill -f "release/jaringan devtrace"
pkill -f "release/jaringan fam2"
pkill -f "release/jaringan coinrise"
pkill -f "release/jaringan athgt"
pkill -f "bash ./watch_target.sh"
sleep 2
ps aux | grep -E "release/jaringan" | grep -v grep | awk '{print $2, $12, $13}'
