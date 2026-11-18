#!/bin/bash
cd /root/jaringan
./target/release/jaringan devhist data/family_a/devs.txt 10 data/family_a/devhist.jsonl
echo DEVHIST_DONE
