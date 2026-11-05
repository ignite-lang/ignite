# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
apply-cycle-terminal-rust | baseline | 30.870069 | 28.496502 | 0.9099 | 5/0
apply-cycle-terminal-llvm | baseline | 28.268573 | 24.667427 | 0.9665 | 5/0

Accepted: False
Missing evidence: check, cold-clippy, cold-fmt, cold-inline, cold-workspace, file-ignite_test, file-rustgen, gate-cold, gate-warm, no-run-touch, no-run-warm, plan, todod-llvm-dev, todod-llvm-release, todod-rust-dev, todod-rust-release, warm-clippy, warm-fmt, warm-inline, warm-workspace
Over budget: 
