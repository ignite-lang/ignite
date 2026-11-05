# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
todod-rust-dev | baseline | 3.915892 | 4.323999 | 1.1415 | 4/0
todod-llvm-dev | baseline | 0.383357 | 0.476221 | 1.2422 | 3/1
todod-rust-release | baseline | 5.144812 | 7.096324 | 1.4238 | 3/1
todod-llvm-release | baseline | 1.367612 | 0.998626 | 1.0504 | 4/0
check | baseline | 0.150353 | 0.176673 | 1.0889 | 15/0
plan | baseline | 0.143713 | 0.153323 | 1.0632 | 15/0
cold-clippy | baseline | 10.147638 | 24.374409 | 2.4020 | 1/0
cold-workspace | baseline | 1031.206862 | 763.852012 | 0.7407 | 1/0
cold-inline | baseline | 28.012004 | 19.173200 | 0.6845 | 1/0

Accepted: False
Missing evidence: cold-fmt, file-ignite_test, file-rustgen, gate-cold, gate-warm, no-run-touch, no-run-warm, warm-clippy, warm-fmt, warm-inline, warm-workspace
Over budget: cold-clippy, todod-llvm-dev, todod-rust-dev, todod-rust-release
