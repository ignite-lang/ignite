//! Cryptographic pins for admitted local inputs and retained artifacts.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

pub fn bytes(input: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(input))
}

pub fn file(path: &Path) -> io::Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
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
}
