T0 | owner: COORDINATOR | brief section: V3 13 Stage 0 | phase step: V3-S0
Goal: implementation audit of repo, data and sources against V3
Inputs: repo HEAD d5f0374, data_gz/, dataset/, src/, DexScreener docs, MadeOnSol OpenAPI
Output: reports/v3/IMPLEMENTATION_AUDIT.md, reports/v3/audit_numbers.sh/.txt
Pass when: all numbers reproduced by audit_numbers.sh or inline command; CRITIC > 9.0, 0 open CRITICAL/HIGH
Result r1: CRITIC 3.0 FAIL (reports/v3/critic/T0_r1.md); r2 fixes: clippy green (5 files, behaviour-neutral), sources/union/coverage/legacy reconciliation added (N9)
