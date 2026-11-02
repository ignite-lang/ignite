# Checkpoint d8a9cb7: remaining budgets

This immutable candidate fixed syntax-fixture publication. Three matched warm
pairs and fifteen check/plan pairs completed without candidate failures. The
source manifests, executing harness, raw rows, logs and restart journals are
retained here. `summary.md` reports the actual matched ratios. This checkpoint
does not satisfy performance acceptance.

Generator tests recovered matched 11b time (0.9753 ratio); file-level CLI tests
fell to 2.093 s. Warm workspace tests took 11.072 s, exceeding the absolute
10.8 s goal despite their passing 1.0805 ratio. Formatting, clippy, inline tests,
the aggregate warm gate and checker-touch no-run builds still exceeded their
ratio limits. Plan missed narrowly at 1.101956. Cold and program guards were
not collected for this superseded candidate; they remain required for closure.

The next changes address measured formatting scheduling, JSON-writing work and
duplicate empty Cargo harnesses. Their diagnostic results cannot substitute for
a new complete frozen comparison. All previous samples remain in this record.
