#!/usr/bin/env bash
cd /root/sol
pkill -f "bash ./reput_loop.sh"
sleep 1
nohup ./reput_loop.sh > /dev/null 2>&1 < /dev/null &
