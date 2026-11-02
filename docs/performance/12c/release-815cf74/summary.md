# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
check | baseline | 0.059635 | 0.063794 | 1.0647 | 15/0
plan | baseline | 0.060856 | 0.063668 | 1.0589 | 15/0
warm-fmt | baseline | 0.492714 | 0.339505 | 0.5218 | 5/0
warm-clippy | baseline | 3.035275 | 3.379741 | 1.1135 | 5/0
warm-workspace | baseline | 13.192067 | 14.333174 | 1.0208 | 4/1
warm-inline | baseline | 0.851252 | 0.786390 | 0.8611 | 5/0
gate-warm | baseline | 17.369147 | 18.574086 | 1.0194 | 4/1
no-run-warm | baseline | 0.032869 | 0.025859 | 0.7867 | 5/0
no-run-touch | baseline | 2.130703 | 2.288476 | 1.2814 | 5/0
file-ignite_test | baseline | 4.971730 | 2.675017 | 0.5008 | 4/1
file-rustgen | baseline | 1.797297 | 1.866691 | 0.9804 | 4/1
check | comparison | 0.085638 | 0.063794 | 0.7429 | 15/0
plan | comparison | 0.087069 | 0.063668 | 0.7338 | 15/0
warm-fmt | comparison | 0.577434 | 0.339505 | 0.5835 | 5/0
warm-clippy | comparison | 3.556482 | 3.292444 | 1.0113 | 4/1
warm-workspace | comparison | 14.710636 | 14.333174 | 0.9722 | 2/3
warm-inline | comparison | 1.617779 | 0.770018 | 0.4898 | 4/1
gate-warm | comparison | 21.251361 | 19.037451 | 0.8327 | 4/1
no-run-warm | comparison | 0.025922 | 0.028854 | 1.1131 | 3/2
no-run-touch | comparison | 2.481940 | 2.175721 | 0.8766 | 3/2
file-ignite_test | comparison | 2.104844 | 2.527863 | 1.2010 | 4/1
file-rustgen | comparison | 3.530782 | 1.799684 | 0.5010 | 3/2

Accepted: False
Missing evidence: cold-clippy, cold-fmt, cold-inline, cold-workspace, gate-cold, todod-llvm-dev, todod-llvm-release, todod-rust-dev, todod-rust-release
Over budget: no-run-touch, warm-clippy
