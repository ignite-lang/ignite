# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
cold-fmt | baseline | 0.785668 | 0.540179 | 0.6875 | 1/0
cold-clippy | baseline | 7.737836 | 5.548350 | 0.7170 | 1/0
cold-inline | baseline | 12.028888 | 11.159120 | 0.9277 | 1/0

Accepted: False
Missing evidence: check, cold-workspace, file-ignite_test, gate-cold, gate-warm, no-run-touch, no-run-warm, plan, todod-llvm-dev, todod-llvm-release, todod-rust-dev, todod-rust-release, warm-clippy, warm-fmt, warm-inline, warm-workspace
Over budget: 
