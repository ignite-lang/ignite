# Checkpoint 0a42338: initialization timeout

This frozen candidate contains the validated spawn-completion scheduling
correction and selective owned type-query snapshots. Smoke checks passed.
Preparation stopped after a different failure in
`drain::rollout_drains_the_old_process_and_starts_the_new_rust`.

The helper waited 15 seconds for Started and saw zero records. Stderr showed
Cargo waiting on package-cache and shared build-directory locks. The run was
still building its generated Rust project and had not emitted its first
stdout segment record. The on-disk header can be written before building and
does not mark initialization complete. The case failed before controller lifecycle timing began;
the unchanged real restart case passed in this run. Host load was about 26
at the workspace step, which took 62.315 seconds.

The complete failed preparation, source manifests and commands are retained.
No warm/cold/CLI acceptance samples were collected around the failed candidate.
The next check distinguishes bounded initial build/setup from the existing
15-second live protocol waits, while leaving every build/wait inside the
measured workspace gate and retaining the 100–500 ms restart assertion.
