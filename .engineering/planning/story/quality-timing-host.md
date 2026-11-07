---
format: aep.planning-md/3
id: story:quality-timing-host
kind: story
status: active
title: Timing metrics on a single-tenant EC2 metal host
relations:
- depends_on: story:quality-ratchet
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: docs/quality.md
- confidence: inferred
  path: quality/
- confidence: inferred
  path: tools/ignite-perf/src
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T21:02:47Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-07T21:02:47Z", actor: "human:timo", revision: 4}
---
# Story: quality-timing-host

## Outcome

The timing metrics of `docs/quality.md` are measured on a machine that runs nothing else: one command starts an EC2 `c7i.metal-24xl` spot instance (a whole host), builds and measures the base and head trees interleaved, appends the records to `quality/history.jsonl` with the host named, and terminates the instance; the release gate reads timing rows only from such runs.

## Context

- Timo, 2026-11-07: timing metrics only make sense "on a dedicated machine where we know only this particular thing is running"; option A of the assessment (EC2 bare metal spot over EC2 dedicated tenancy and Fly.io, whose `performance` machines share hosts with other customers, docs.fly.io/machines/cpu-performance).
- Account: AWS profile `babelforce-dev-admin` (account `523757638725`), region `eu-central-1`. `babelforce-dev` (DeveloperAccess) cannot call EC2. Checked 2026-11-07: `run-instances --dry-run` of `c7i.4xlarge` succeeds; standard on-demand vCPU quota 1920; `c7i.metal-24xl` 96 vCPU, 192 GiB, on-demand 4.89 USD/h, spot 0.96 to 1.49 USD/h by zone.
- Builds on `story:quality-ratchet` (`quality/history.jsonl`, `task quality`, the timing modes of `tools/ignite-perf`).
- Method stays `docs/slice.md` 15.1 / `tools/ignite-perf`: interleaved pairs of base and head on one host; on metal, SMT and turbo pinned where the instance allows it (to be verified on the host, not assumed).

## Acceptance

- `task quality-remote BASE=<rev> HEAD=<rev>` (an `ignite-perf` mode; it drives the `aws` CLI through `std::process::Command`, no new dependency) launches one spot `c7i.metal-24xl` with a tag naming the run, copies both trees, runs the timing modes, fetches the records, and terminates the instance on every exit path (success, failure, interrupt); a test of the mode's plan (the commands it would run, a `--dry-run`) runs in `task gate` without AWS.
- Every record carries `host` (instance type, instance id, CPU model, kernel, whether SMT and turbo were off); `quality/trend.md` timing columns come only from `host` runs of that type.
- The release steps in `AGENTS.md` § Releases run it and refuse the release on a gated timing row worse than the last release beyond its tolerance.
- One real run is done and committed: the timing bar of `main` at the time, with its cost (instance-hours from the run's own clock) stated in the record.
- No instance tagged by the mode is left running after the run (`aws ec2 describe-instances` by tag, checked by the mode at its end).
- `task gate` green.

## Out of Scope

- A standing machine or CI runner; Fly.io.
