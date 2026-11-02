# Rejected private scope-map experiment

The checker-only prototype replaced eight private local-scope BTreeMap
constructors and their field type with standard RandomState HashMaps. Scope
entries were never enumerated; public ordered symbol/export maps were unchanged.
Before and after CLIs were freshly built and retained outside the repository.

Three interleaved check/plan pairs and all raw rows are retained here. Check's
paired median gain was effectively zero, while host load around 50 made the
plan timings noisy. Canonical plan outputs matched. The experiment failed the
requirement to improve both CLI paths and was rejected without favourable
repeats. The original checker source and freshly restored CLI matched their
before hashes exactly. No production scope-map change remains.
