#!/usr/bin/env bash
# pause athgt shards (validation of the price formula, 21 Nov)
pkill -f "release/jaringan athgt"
sleep 1
pgrep -fc "release/jaringan athgt" || true
