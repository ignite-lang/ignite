//! Adversarial cases for `ignite_io::Entropy` (`docs/slice.md` 21.7, row
//! 20): what the unit tests in `src/entropy.rs` do not say.

use std::collections::HashSet;

use ignite_io::Entropy;

fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// 21.7: every count in `0..=65536` gives exactly that many bytes, and a
/// full draw is not a constant.
#[test]
fn every_bound_gives_its_count() {
    let entropy = Entropy::open().unwrap();
    for count in [0, 1, 2, 4095, 4096, 4097, 65535, 65536] {
        let drawn = entropy.random_bytes(count).unwrap();
        assert_eq!(drawn.len() as i64, count);
    }
    let full = entropy.random_bytes(65536).unwrap();
    assert!(full.iter().any(|b| *b != full[0]));
    for count in [65537, 1 << 32, (1 << 32) + 5, -65536, -(1 << 32)] {
        assert_eq!(
            entropy.random_bytes(count).map_err(|e| e.message),
            Err("random_bytes: count must be in 0..=65536".to_string())
        );
    }
}

/// 21.3: several tasks may draw from one `Entropy` at once; every draw has
/// its count and no two draws of 16 bytes are equal.
#[test]
fn concurrent_draws_are_whole_and_distinct() {
    let entropy = Entropy::open().unwrap();
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let entropy = entropy.clone();
            std::thread::spawn(move || {
                (0..200)
                    .map(|_| entropy.random_bytes(16).unwrap())
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut seen = HashSet::new();
    for t in threads {
        for drawn in t.join().unwrap() {
            assert_eq!(drawn.len(), 16);
            assert!(seen.insert(drawn), "a repeated draw");
        }
    }
    assert_eq!(seen.len(), 16 * 200);
}

/// Row 14: a child started after the source is open does not hold it.
#[test]
fn a_child_does_not_inherit_the_source() {
    let entropy = Entropy::open().unwrap();
    entropy.random_bytes(1).unwrap();
    let listed = std::process::Command::new("/bin/ls")
        .arg("-l")
        .arg("/proc/self/fd")
        .output()
        .unwrap();
    assert!(listed.status.success());
    let listed = String::from_utf8_lossy(&listed.stdout);
    assert!(!listed.contains("/dev/urandom"), "{listed}");
}

/// 21.7: an `Entropy` that is dropped releases its descriptor.
#[test]
fn dropped_sources_are_closed() {
    Entropy::new().random_bytes(1).unwrap();
    let before = open_descriptors();
    for _ in 0..2_000 {
        let entropy = Entropy::new();
        entropy.random_bytes(8).unwrap();
    }
    let after = open_descriptors();
    assert!(
        after < before + 64,
        "{before} descriptors before, {after} after"
    );
}
