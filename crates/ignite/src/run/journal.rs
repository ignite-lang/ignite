//! The journal (`docs/slice.md` 16.5): one JSON record per line in
//! `<state>/journal.jsonl`, `sync_data` after every `effect` record before
//! its dispatch (O30), a lock file `<state>/lock` that admits one run per
//! state directory (E126 when another holds it), and the records already
//! there, which a run with the same model appends to (O31). `--fresh`
//! renames the old journal to `journal.<n>.jsonl`. A final line without
//! `\n`, from a crash while writing, is dropped on open, the file is cut
//! back to the complete lines, and the drop is reported once by the caller;
//! the header and model checks and E127 are the engine's, which owns the
//! encoding.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use nix::fcntl::{Flock, FlockArg};

/// The journal file's name in the state directory.
pub const FILE: &str = "journal.jsonl";
/// The lock file's name in the state directory.
pub const LOCK: &str = "lock";

/// Why a journal could not be opened.
#[derive(Debug)]
pub enum JournalError {
    /// Another `ignite run` holds the lock of this state directory (E126).
    Locked(PathBuf),
    Io(io::Error),
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournalError::Locked(path) => {
                write!(f, "another `ignite run` holds {}", path.display())
            }
            JournalError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for JournalError {}

impl From<io::Error> for JournalError {
    fn from(error: io::Error) -> JournalError {
        JournalError::Io(error)
    }
}

/// An open journal: its file, the records it held when opened (the header
/// first), and the `seq` of the next record.
#[derive(Debug)]
pub struct Journal {
    pub path: PathBuf,
    /// The `seq` the next appended record carries: the number of records.
    pub next_seq: u64,
    /// The complete lines present at open, without their `\n`.
    pub records: Vec<String>,
    /// A final line without `\n` was dropped at open.
    pub dropped_torn_line: bool,
    file: File,
    _lock: Flock<File>,
}

impl Journal {
    /// Open or create the journal of `state`, taking its lock; `fresh`
    /// renames an existing journal first.
    pub fn open(state: &Path, fresh: bool) -> Result<Journal, JournalError> {
        std::fs::create_dir_all(state)?;
        let lock_path = state.join(LOCK);
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)?;
        let lock = match Flock::lock(lock_file, FlockArg::LockExclusiveNonblock) {
            Ok(lock) => lock,
            Err((_, nix::errno::Errno::EWOULDBLOCK)) => {
                return Err(JournalError::Locked(lock_path));
            }
            Err((_, errno)) => return Err(JournalError::Io(errno.into())),
        };
        let path = state.join(FILE);
        if fresh && path.exists() {
            std::fs::rename(&path, archive_name(state))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        let kept = content
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |at| at + 1);
        let dropped_torn_line = kept < content.len();
        if dropped_torn_line {
            file.set_len(kept as u64)?;
        }
        content.truncate(kept);
        let records: Vec<String> = String::from_utf8_lossy(&content)
            .lines()
            .map(str::to_string)
            .collect();
        let file = OpenOptions::new().append(true).open(&path)?;
        Ok(Journal {
            path,
            next_seq: records.len() as u64,
            records,
            dropped_torn_line,
            file,
            _lock: lock,
        })
    }

    /// Append `record`, a complete JSON object without its `\n` that
    /// carries `"seq": self.next_seq`, and return that `seq`. With
    /// `durable` the data is synced before this returns (an `effect`
    /// record, O30).
    pub fn append(&mut self, record: &str, durable: bool) -> io::Result<u64> {
        let mut line = Vec::with_capacity(record.len() + 1);
        line.extend_from_slice(record.as_bytes());
        line.push(b'\n');
        self.file.write_all(&line)?;
        if durable {
            self.file.sync_data()?;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        Ok(seq)
    }
}

/// `<state>/journal.<n>.jsonl` for the smallest `n` from 1 not taken.
fn archive_name(state: &Path) -> PathBuf {
    (1..)
        .map(|n| state.join(format!("journal.{n}.jsonl")))
        .find(|candidate| !candidate.exists())
        .expect("an unused archive name")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh state directory under the temporary directory (`TMPDIR`),
    /// unique per test and process.
    fn state(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("ignite-journal-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn records_get_consecutive_seqs_and_persist() {
        let state = state("seqs");
        {
            let mut journal = Journal::open(&state, false).unwrap();
            assert_eq!(journal.next_seq, 0);
            assert!(journal.records.is_empty());
            assert!(!journal.dropped_torn_line);
            assert_eq!(
                journal
                    .append(r#"{"kind":"header","seq":0}"#, false)
                    .unwrap(),
                0
            );
            assert_eq!(
                journal
                    .append(r#"{"kind":"effect","seq":1}"#, true)
                    .unwrap(),
                1
            );
            assert_eq!(journal.next_seq, 2);
        }
        let journal = Journal::open(&state, false).unwrap();
        assert_eq!(journal.next_seq, 2);
        assert_eq!(
            journal.records,
            vec![
                r#"{"kind":"header","seq":0}"#.to_string(),
                r#"{"kind":"effect","seq":1}"#.to_string()
            ]
        );
        assert_eq!(
            std::fs::read_to_string(&journal.path).unwrap(),
            "{\"kind\":\"header\",\"seq\":0}\n{\"kind\":\"effect\",\"seq\":1}\n"
        );
        drop(journal);
        let _ = std::fs::remove_dir_all(&state);
    }

    #[test]
    fn a_torn_final_line_is_dropped_and_cut() {
        let state = state("torn");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(
            state.join(FILE),
            "{\"seq\":0}\n{\"seq\":1}\n{\"seq\":2,\"kind\":\"eff",
        )
        .unwrap();
        let mut journal = Journal::open(&state, false).unwrap();
        assert!(journal.dropped_torn_line);
        assert_eq!(journal.records.len(), 2);
        assert_eq!(journal.next_seq, 2);
        assert_eq!(journal.append(r#"{"seq":2}"#, true).unwrap(), 2);
        assert_eq!(
            std::fs::read_to_string(&journal.path).unwrap(),
            "{\"seq\":0}\n{\"seq\":1}\n{\"seq\":2}\n"
        );
        drop(journal);
        let _ = std::fs::remove_dir_all(&state);
    }

    #[test]
    fn the_lock_admits_one_run() {
        let state = state("lock");
        let first = Journal::open(&state, false).unwrap();
        match Journal::open(&state, false) {
            Err(JournalError::Locked(path)) => assert_eq!(path, state.join(LOCK)),
            other => panic!("{other:?}"),
        }
        drop(first);
        assert!(Journal::open(&state, false).is_ok());
        let _ = std::fs::remove_dir_all(&state);
    }

    #[test]
    fn fresh_renames_the_old_journal() {
        let state = state("fresh");
        {
            let mut journal = Journal::open(&state, false).unwrap();
            journal.append(r#"{"seq":0}"#, false).unwrap();
        }
        {
            let mut journal = Journal::open(&state, true).unwrap();
            assert!(journal.records.is_empty());
            assert_eq!(journal.next_seq, 0);
            journal.append(r#"{"seq":0,"fresh":1}"#, false).unwrap();
        }
        let journal = Journal::open(&state, true).unwrap();
        assert!(journal.records.is_empty());
        assert_eq!(
            std::fs::read_to_string(state.join("journal.1.jsonl")).unwrap(),
            "{\"seq\":0}\n"
        );
        assert_eq!(
            std::fs::read_to_string(state.join("journal.2.jsonl")).unwrap(),
            "{\"seq\":0,\"fresh\":1}\n"
        );
        drop(journal);
        let _ = std::fs::remove_dir_all(&state);
    }
}
