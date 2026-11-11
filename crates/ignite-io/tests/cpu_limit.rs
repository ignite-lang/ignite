//! The worker limit's CPU count (story `binary-size-back`): the CPUs of
//! the thread's affinity, at most the CPU quota of its cgroup (v2
//! `cpu.max` of the group and every ancestor, v1 `cpu.cfs_quota_us` over
//! `cpu.cfs_period_us`), rounded up, at least 1. Each case reads a fixture
//! tree under `tests/fixtures/cgroup/` as `cpu_limit` reads `/`.

use std::path::{Path, PathBuf};

use ignite_io::cpu_limit;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cgroup")
        .join(name)
}

#[test]
fn a_v2_quota_of_the_group_bounds_the_cpus_rounded_up() {
    assert_eq!(cpu_limit(&fixture("v2-quota")), 2);
}

#[test]
fn a_v2_quota_of_an_ancestor_bounds_the_cpus() {
    assert_eq!(cpu_limit(&fixture("v2-parent")), 3);
}

#[test]
fn without_a_quota_the_affinity_counts() {
    assert_eq!(cpu_limit(&fixture("v2-unlimited")), 6);
    assert_eq!(cpu_limit(&fixture("v1-unlimited")), 4);
    assert_eq!(cpu_limit(&fixture("none")), 2);
}

#[test]
fn a_container_reads_the_quota_at_its_mount_and_gets_at_least_one() {
    assert_eq!(cpu_limit(&fixture("v2-container")), 1);
}

#[test]
fn an_affinity_below_the_quota_counts() {
    assert_eq!(cpu_limit(&fixture("v2-affinity")), 2);
}

#[test]
fn a_v1_quota_bounds_the_cpus() {
    assert_eq!(cpu_limit(&fixture("v1-quota")), 2);
}

/// On this host the count is the standard library's, which reads the same
/// affinity and quota with its own code.
#[test]
fn this_process_counts_as_the_standard_library_does() {
    let std = std::thread::available_parallelism().map_or(1, usize::from);
    assert_eq!(cpu_limit(Path::new("/")), std);
}
