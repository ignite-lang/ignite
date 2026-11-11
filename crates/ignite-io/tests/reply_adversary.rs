//! Adversary pass 2 on `story:http-reply-cost-back`, fix rounds 2 to 5:
//! the counting allocator's `realloc`, the byte search that reads eight
//! bytes at a time, the hook that gives each worker its alternate signal
//! stack, and the overflow handler on a start with one fault signal
//! ignored. A case that needs the census or a fresh scheduler alone runs
//! in a child process: this test executable again, with that case only.

use std::alloc::{GlobalAlloc, Layout};
use std::collections::HashSet;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use ignite_io::census::{Counting, heap_bytes};
use ignite_io::text::{each_piece, find, index_of};

const CHILD: &str = "IGNITE_REPLY_ADVERSARY_CHILD";
const LIMIT: Duration = Duration::from_secs(20);

fn in_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// Run `name` of this executable alone, through `sh -c` with `prefix`
/// before the `exec`; its output and whether it had to be killed.
fn child(name: &str, prefix: &str, env: &[(&str, &str)]) -> (Output, bool) {
    let mut command = Command::new("sh");
    command
        .args(["-c", &format!("{prefix}exec \"$0\" \"$@\"")])
        .arg(std::env::current_exe().unwrap())
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env_remove("IGNITE_IO")
        .env_remove("IGNITE_WORKERS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    let start = Instant::now();
    let mut hung = false;
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > LIMIT {
            hung = true;
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    (child.wait_with_output().unwrap(), hung)
}

fn passed(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "child: {:?}\nstdout:\n{stdout}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The offset-by-offset search the builtins replace.
fn naive(haystack: &[u8], needle: &[u8], start: i64) -> Option<i64> {
    let mut at = start.max(0) as usize;
    while at + needle.len() <= haystack.len() {
        if &haystack[at..at + needle.len()] == needle {
            return Some(at as i64);
        }
        at += 1;
    }
    None
}

/// The pieces between separators, by the naive search.
fn naive_pieces(text: &[u8], separator: &[u8]) -> Vec<(usize, usize)> {
    if separator.is_empty() {
        return vec![(0, text.len())];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while let Some(at) = naive(text, separator, start as i64) {
        let at = at as usize;
        out.push((start, at));
        start = at + separator.len();
    }
    out.push((start, text.len()));
    out
}

/// `find`, `index_of` and `each_piece` against the naive search, with
/// seeds and alphabets the unit's tests do not use: two letters (a needle
/// whose first byte repeats), the bytes around 0x00, 0x7f, 0x80 and 0xff,
/// and every byte; needles up to 12 bytes, random or cut from the
/// haystack, and every start from -3 to three past the end.
#[test]
fn find_index_of_and_pieces_agree_with_the_naive_search_on_other_seeds() {
    let alphabets: [&[u8]; 3] = [b"ab", &[0x00, 0x01, 0x7f, 0x80, 0x81, 0xfe, 0xff], &[]];
    let mut checked = 0u64;
    for seed in [1u64, 2, 3, 5, 0xdead_beef, 0x1234_5678_9abc] {
        let mut state = seed;
        let mut next = |count: usize| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as usize % count.max(1)
        };
        for round in 0..3_000 {
            let alphabet = alphabets[round % 3];
            let byte = |next: &mut dyn FnMut(usize) -> usize| {
                if alphabet.is_empty() {
                    next(256) as u8
                } else {
                    alphabet[next(alphabet.len())]
                }
            };
            let haystack: Vec<u8> = (0..next(100)).map(|_| byte(&mut next)).collect();
            let needle: Vec<u8> = if next(2) == 0 || haystack.is_empty() {
                (0..next(13)).map(|_| byte(&mut next)).collect()
            } else {
                let from = next(haystack.len());
                let to = from + next((haystack.len() - from).min(12) + 1);
                haystack[from..to].to_vec()
            };
            assert_eq!(
                find(&haystack, &needle).map(|at| at as i64),
                naive(&haystack, &needle, 0),
                "find({haystack:?}, {needle:?})"
            );
            for start in -3..=haystack.len() as i64 + 3 {
                let expected = if start > haystack.len() as i64 {
                    None
                } else {
                    naive(&haystack, &needle, start)
                };
                assert_eq!(
                    index_of(&haystack, &needle, start),
                    expected,
                    "index_of({haystack:?}, {needle:?}, {start})"
                );
                checked += 1;
            }
            let mut pieces = Vec::new();
            each_piece(&haystack, &needle, |start, end| pieces.push((start, end)));
            assert_eq!(
                pieces,
                naive_pieces(&haystack, &needle),
                "each_piece({haystack:?}, {needle:?})"
            );
        }
    }
    for length in 0..=40 {
        for at in 0..=length {
            for needle in [&b"aaab"[..], b"ab", b"aab"] {
                let mut haystack = vec![b'a'; length];
                if at + needle.len() <= length {
                    haystack[at + needle.len() - 1] = b'b';
                }
                assert_eq!(
                    find(&haystack, needle).map(|found| found as i64),
                    naive(&haystack, needle, 0),
                    "{haystack:?} {needle:?}"
                );
            }
        }
    }
    assert!(checked > 1_000_000, "{checked}");
}

/// The heap bytes through `Counting::realloc` on every path: the same
/// size, a failed reallocation (null, the old block kept), an alignment
/// above the C allocator's (std moves it with an allocation and a free of
/// its own), and eight threads growing and shrinking at once. Back to the
/// start after each.
#[test]
fn a_reallocation_keeps_the_heap_bytes_exact_on_every_path() {
    let (output, hung) = child("reply_adversary::child_reallocations", "", &[]);
    assert!(!hung);
    passed(&output);
}

#[test]
fn child_reallocations() {
    if !in_child() {
        return;
    }
    let start = heap_bytes();
    let layout = Layout::from_size_align(64, 8).unwrap();
    // SAFETY: every layout is valid and non-zero; every pointer passed is
    // the live allocation of the layout passed with it; every new size is
    // non-zero and, rounded up to its alignment, at most `isize::MAX`.
    unsafe {
        let block = Counting.alloc(layout);
        assert!(!block.is_null());
        block.write_bytes(5, 64);
        let same = Counting.realloc(block, layout, 64);
        assert!(!same.is_null());
        assert_eq!(heap_bytes() - start, 64, "a reallocation to the same size");
        let huge = isize::MAX as usize - 63;
        let failed = Counting.realloc(same, layout, huge);
        assert!(failed.is_null(), "a reallocation of {huge} bytes succeeded");
        assert_eq!(heap_bytes() - start, 64, "after a failed reallocation");
        assert!((0..64).all(|at| *same.add(at) == 5), "the old block kept");
        Counting.dealloc(same, layout);
        assert_eq!(heap_bytes() - start, 0, "after the free");

        let aligned = Layout::from_size_align(100, 4096).unwrap();
        let block = Counting.alloc(aligned);
        assert!(!block.is_null());
        block.write_bytes(9, 100);
        let grown = Counting.realloc(block, aligned, 9000);
        assert!(!grown.is_null());
        assert_eq!(grown as usize % 4096, 0, "alignment kept");
        assert_eq!(heap_bytes() - start, 9000, "an over-aligned growth");
        assert!((0..100).all(|at| *grown.add(at) == 9));
        let grown_layout = Layout::from_size_align(9000, 4096).unwrap();
        let shrunk = Counting.realloc(grown, grown_layout, 10);
        assert!(!shrunk.is_null());
        assert_eq!(heap_bytes() - start, 10, "an over-aligned shrink");
        Counting.dealloc(shrunk, Layout::from_size_align(10, 4096).unwrap());
    }
    assert_eq!(heap_bytes() - start, 0, "after the over-aligned block");
    std::thread::scope(|scope| {
        for thread in 0..8usize {
            scope.spawn(move || {
                for round in 0..5_000usize {
                    let small = Layout::from_size_align(16 + thread, 8).unwrap();
                    let big = 1000 + round % 7 * 4096;
                    // SAFETY: as above.
                    unsafe {
                        let block = Counting.alloc(small);
                        assert!(!block.is_null());
                        let grown = Counting.realloc(block, small, big);
                        assert!(!grown.is_null());
                        let big_layout = Layout::from_size_align(big, 8).unwrap();
                        let shrunk = Counting.realloc(grown, big_layout, 8);
                        assert!(!shrunk.is_null());
                        Counting.dealloc(shrunk, Layout::from_size_align(8, 8).unwrap());
                    }
                }
            });
        }
    });
    assert_eq!(heap_bytes() - start, 0, "after eight threads");
}

/// The threads [`given`] ran on: every worker thread that called the hook.
static GIVEN: Mutex<Vec<ThreadId>> = Mutex::new(Vec::new());

fn given() {
    GIVEN.lock().unwrap().push(std::thread::current().id());
}

/// The hook of `signal::give_workers_an_alternate_stack` runs on every
/// worker as it starts, not only the first: with four workers allowed and
/// tasks that hold their worker, as many threads called it as workers
/// started, and every thread a task ran on is one of them.
#[test]
fn every_worker_calls_the_alternate_stack_hook() {
    let (output, hung) = child(
        "reply_adversary::child_every_worker_calls_the_hook",
        "",
        &[("IGNITE_WORKERS", "4")],
    );
    assert!(!hung);
    passed(&output);
}

#[test]
fn child_every_worker_calls_the_hook() {
    if !in_child() {
        return;
    }
    ignite_io::signal::give_workers_an_alternate_stack(given);
    let ran: &'static Mutex<HashSet<ThreadId>> = Box::leak(Box::new(Mutex::new(HashSet::new())));
    for _ in 0..16 {
        ignite_io::spawn(move || {
            ran.lock().unwrap().insert(std::thread::current().id());
            std::thread::sleep(Duration::from_millis(100));
        });
    }
    ignite_io::wait_for_tasks();
    let workers = ignite_io::scheduler_counts().workers_started;
    let calls = GIVEN.lock().unwrap().clone();
    let given: HashSet<ThreadId> = calls.iter().copied().collect();
    assert_eq!(calls.len(), given.len(), "a thread called the hook twice");
    let ran = ran.lock().unwrap().clone();
    println!(
        "workers {workers}, hook threads {}, task threads {}",
        given.len(),
        ran.len()
    );
    assert!(workers >= 2, "only {workers} worker started");
    assert_eq!(given.len() as u64, workers, "threads that called the hook");
    assert!(
        ran.is_subset(&given),
        "a task ran on a thread without the hook"
    );
}

/// A task's overflow with four workers on a start with only `SIGBUS`
/// ignored: std still installs its `SIGSEGV` handler and gives every
/// thread an alternate stack, so the overflow is reported and aborts.
#[test]
fn started_with_only_sigbus_ignored_an_overflow_is_reported() {
    let (output, hung) = child(
        "scheduler::child_overflow",
        "trap '' BUS; ",
        &[("IGNITE_SCHEDULER_CHILD", "1"), ("IGNITE_WORKERS", "4")],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!hung, "not ended within {LIMIT:?}\n{stderr}");
    assert!(stderr.contains("has overflowed its stack"), "{stderr}");
    assert_eq!(
        output.status.signal(),
        Some(6),
        "{:?}\n{stderr}",
        output.status
    );
}

/// A task's overflow with four workers on a start with only `SIGSEGV`
/// ignored: std installs its `SIGBUS` handler and gives every thread an
/// alternate stack. The process ends, within the wait, by the fault's
/// signal with no report or by the report and the abort.
#[test]
fn started_with_only_sigsegv_ignored_an_overflow_ends_the_process() {
    let (output, hung) = child(
        "scheduler::child_overflow",
        "trap '' SEGV; ",
        &[("IGNITE_SCHEDULER_CHILD", "1"), ("IGNITE_WORKERS", "4")],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!(
        "only SIGSEGV ignored: {:?}, report {}",
        output.status,
        stderr.contains("has overflowed its stack")
    );
    assert!(!hung, "not ended within {LIMIT:?}\n{stderr}");
    let reported = stderr.contains("has overflowed its stack");
    assert!(
        matches!(
            (output.status.signal(), reported),
            (Some(11 | 7), false) | (Some(6), true)
        ),
        "{:?}, reported {reported}\n{stderr}",
        output.status
    );
}
