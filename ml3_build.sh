#!/bin/bash
cd /root/jaringan
./target/release/jaringan ml3 build data/ml3 1 > data/ml3/build.log 2>&1
echo ML3_BUILD_DONE >> data/ml3/build.log
