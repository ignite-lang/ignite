# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
check | baseline | 0.060722 | 0.066550 | 1.1035 | 15/0
plan | baseline | 0.061611 | 0.067931 | 1.1097 | 15/0
todod-rust-dev | baseline | 3.329183 | 3.690358 | 1.0714 | 3/0
todod-llvm-dev | baseline | 0.350058 | 0.284914 | 0.8139 | 3/0
todod-rust-release | baseline | 4.396556 | 4.477056 | 1.0374 | 3/0
todod-llvm-release | baseline | 0.875156 | 0.796622 | 0.9244 | 3/0
warm-fmt | baseline | 0.572945 | 0.309360 | 0.4484 | 3/0
warm-clippy | baseline | 3.670938 | 3.846683 | 1.0279 | 3/0
warm-workspace | baseline | 16.638451 | 17.617258 | 1.0899 | 2/1
warm-inline | baseline | 0.915051 | 0.991238 | 1.0308 | 3/0
gate-warm | baseline | 21.916662 | 22.299122 | 1.0348 | 2/1
no-run-warm | baseline | 0.034356 | 0.030421 | 0.8885 | 2/0
no-run-touch | baseline | 2.528898 | 2.754842 | 1.0895 | 2/0
file-ignite_test | baseline | 6.120709 | 2.294871 | 0.3777 | 2/0
file-rustgen | baseline | 2.069858 | 1.960871 | 0.9471 | 2/0
check | comparison | 0.087808 | 0.066550 | 0.7635 | 15/0
plan | comparison | 0.088732 | 0.067931 | 0.7660 | 15/0
todod-rust-dev | comparison | 3.456969 | 3.690358 | 1.0136 | 3/0
todod-llvm-dev | comparison | 0.337897 | 0.284914 | 0.8432 | 3/0
todod-rust-release | comparison | 4.538282 | 4.477056 | 0.9709 | 3/0
todod-llvm-release | comparison | 0.824183 | 0.796622 | 0.9650 | 3/0
warm-fmt | comparison | 0.618539 | 0.274005 | 0.4420 | 2/0
warm-clippy | comparison | 3.986790 | 3.494851 | 0.8895 | 2/0
warm-workspace | comparison | 18.552657 | 17.617258 | 0.9442 | 2/0
warm-inline | comparison | 2.064579 | 0.911612 | 0.4483 | 2/0
gate-warm | comparison | 25.223880 | 22.299122 | 0.8802 | 2/0
no-run-warm | comparison | 0.043605 | 0.030421 | 0.6963 | 2/0
no-run-touch | comparison | 4.045072 | 2.754842 | 0.6859 | 2/0
file-ignite_test | comparison | 2.555089 | 2.294871 | 0.8988 | 2/0
file-rustgen | comparison | 4.937320 | 1.960871 | 0.3966 | 2/0

Accepted: False
Missing evidence: cold-clippy, cold-fmt, cold-inline, cold-workspace, file-ignite_test, file-rustgen, gate-cold, gate-warm, no-run-touch, no-run-warm, warm-workspace
Over budget: check, plan
