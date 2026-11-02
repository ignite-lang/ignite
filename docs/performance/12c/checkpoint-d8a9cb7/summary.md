# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
warm-fmt | baseline | 0.441692 | 0.524395 | 1.1798 | 3/0
warm-clippy | baseline | 2.413575 | 2.824463 | 1.1778 | 3/0
warm-workspace | baseline | 10.246994 | 11.071572 | 1.0805 | 3/0
warm-inline | baseline | 0.701539 | 0.784041 | 1.1202 | 3/0
gate-warm | baseline | 13.745651 | 15.087185 | 1.1101 | 3/0
no-run-warm | baseline | 0.024995 | 0.026024 | 1.0599 | 3/0
no-run-touch | baseline | 1.798223 | 2.179506 | 1.2531 | 3/0
file-ignite_test | baseline | 4.431325 | 2.092535 | 0.4693 | 3/0
file-rustgen | baseline | 1.718599 | 1.673384 | 0.9753 | 3/0
check | baseline | 0.060287 | 0.066618 | 1.0938 | 15/0
plan | baseline | 0.060288 | 0.067028 | 1.1020 | 15/0
warm-fmt | comparison | 0.522010 | 0.524395 | 1.0075 | 3/0
warm-clippy | comparison | 2.777405 | 2.824463 | 1.0115 | 3/0
warm-workspace | comparison | 12.736488 | 11.071572 | 0.8768 | 3/0
warm-inline | comparison | 1.191110 | 0.784041 | 0.6617 | 3/0
gate-warm | comparison | 17.186378 | 15.087185 | 0.8821 | 3/0
no-run-warm | comparison | 0.025075 | 0.026024 | 1.0010 | 3/0
no-run-touch | comparison | 2.247513 | 2.179506 | 0.9559 | 3/0
file-ignite_test | comparison | 2.188549 | 2.092535 | 0.9561 | 3/0
file-rustgen | comparison | 3.365316 | 1.673384 | 0.4945 | 3/0
check | comparison | 0.086901 | 0.066618 | 0.7652 | 15/0
plan | comparison | 0.086770 | 0.067028 | 0.7684 | 15/0

Accepted: False
Missing evidence: cold-clippy, cold-fmt, cold-inline, cold-workspace, gate-cold, todod-llvm-dev, todod-llvm-release, todod-rust-dev, todod-rust-release
Over budget: gate-warm, no-run-touch, plan, warm-clippy, warm-fmt, warm-inline
