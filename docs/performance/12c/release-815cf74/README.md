# Release evidence and remaining performance gaps

User decision on 2026-11-02: release the tested changes now, retain the evidence,
and defer further performance recovery. Slice 12c performance acceptance is **not met**.

The frozen implementation is `815cf741b359af1bb6df16fe7f48b7a5b1e304ba`. Later
release commits change documentation only. Full preparation and every candidate
warm gate passed: 660 Rust tests, five ignored, and 148 inline cases per backend.
The release readiness record verifies exact manual backoff, real restart at
104.162 ms, probe/standing recovery, stop and replay. Four terminal/todo example
builds also pass.

| Unit | 11b median s | Candidate median s | Matched ratio | Valid pairs |
|---|---:|---:|---:|---:|
| check | 0.059635 | 0.063794 | 1.0647 | 15 |
| plan | 0.060856 | 0.063668 | 1.0589 | 15 |
| warm-fmt | 0.492714 | 0.339505 | 0.5218 | 5 |
| warm-clippy | 3.035275 | 3.379741 | 1.1135 | 5 |
| warm-workspace | 13.192067 | 14.333174 | 1.0208 | 4 |
| warm-inline | 0.851252 | 0.786390 | 0.8611 | 5 |
| gate-warm | 17.369147 | 18.574086 | 1.0194 | 4 |
| no-run-warm | 0.032869 | 0.025859 | 0.7867 | 5 |
| no-run-touch | 2.130703 | 2.288476 | 1.2814 | 5 |
| file-ignite_test | 4.971730 | 2.675017 | 0.5008 | 4 |
| file-rustgen | 1.797297 | 1.866691 | 0.9804 | 4 |

Clippy (1.1135) and checker-touch no-run (1.2814) remain above the 1.10 limit.
Warm workspace median was 14.333 s against the 10.8 s target; host load rose
after a successful 6.25 preflight, so comparable low-load acceptance was not
established. Both allowed replacement passes were used; no further favourable
samples were collected. An unchanged 11b client-status case failed once with
connection reset, and the published comparison also failed once. All raw samples
and exclusions remain here; the candidate had no correctness failure.

Cold gate and final-candidate todo program timing guards were not collected before
the user chose to release. Published-comparison warm workspace evidence is insufficient.
These are explicit evidence gaps, not passes or adjustments to the baseline.
The earlier c6 program comparison is retained separately and does not substitute
for final-candidate acceptance.

Production fixes, rejected experiments, prior candidate failures, profiles and
journals are retained in the surrounding `docs/performance/12c/` directories.
