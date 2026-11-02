//! Cryptographic pins for admitted local inputs and retained artifacts.
//!
//! Every digest is SHA-256 over `domain || 0x00 || content` and printed as
//! `sha256:<hex>`, so a pin of one kind can never be replayed as another:
//! file contents use [`FILE`], plan ids, identities and revisions their own
//! domain (`docs/slice.md` 17.3). [`bytes`] is the undomained primitive for
//! callers that prefix their own format tag (the host journal's hash chain)
//! or hash random state bytes.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

/// Domain of a file's content: sources, configuration, compiler, toolchain,
/// runtime and artifact bytes.
pub const FILE: &str = "ignite-file/0.1";

/// SHA-256 of `input` without a domain; the caller supplies its own tag.
pub fn bytes(input: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(input))
}

/// SHA-256 of `input` under `domain`.
pub fn tagged(domain: &str, input: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash.update(input);
    format!("sha256:{:x}", hash.finalize())
}

/// The [`FILE`] digest of content already in memory; equal to [`file`] of a
/// file holding these bytes.
pub fn content(input: &[u8]) -> String {
    tagged(FILE, input)
}

/// The [`FILE`] digest of the file at `path`, streamed.
pub fn file(path: &Path) -> io::Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    hash.update(FILE.as_bytes());
    hash.update([0]);
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Ok(format!("sha256:{:x}", hash.finalize()));
        }
        hash.update(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn published_sha256_vectors() {
        assert_eq!(
            super::bytes(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            super::bytes(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn file_digests_are_domain_separated_and_stream_like_content() {
        assert_eq!(
            super::content(b"abc"),
            super::bytes(b"ignite-file/0.1\0abc")
        );
        assert_ne!(super::content(b"abc"), super::bytes(b"abc"));
        assert_ne!(super::content(b"abc"), super::tagged("other/0.1", b"abc"));
        let executable = std::env::current_exe().unwrap();
        assert_eq!(
            super::file(&executable).unwrap(),
            super::content(&std::fs::read(&executable).unwrap())
        );
    }
}
