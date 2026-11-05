//! Random bytes (`docs/slice.md` 21.7): the C ABI over
//! `ignite_io::Entropy`, a handle of the one table in `crate::net`.

use ignite_io::{Entropy, IoError};

use crate::net::{Handle, capability, register};
use crate::{IgniteIoError, IgniteList, status};

fn entropy(handle: i64) -> Result<Entropy, IoError> {
    capability(handle, "entropy", |found| match found {
        Handle::Entropy(entropy) => Some(entropy),
        _ => None,
    })
}

/// `i64 entropy_new()`: `Entropy.new()`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_entropy_new() -> i64 {
    register(Handle::Entropy(Entropy::new()))
}

/// `i32 entropy_open(i64* out, IoError* err)`: the resource at launch.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_entropy_open(out: *mut i64, err: *mut IgniteIoError) -> i32 {
    status(
        Entropy::open().map(|e| register(Handle::Entropy(e))),
        out,
        err,
    )
}

/// `i32 entropy_random_bytes(i64 e, i64 count, Bytes* out, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_entropy_random_bytes(
    handle: i64,
    count: i64,
    out: *mut IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    let drawn = entropy(handle).and_then(|e| e.random_bytes(count));
    status(drawn.map(|data| IgniteList::from_bytes(&data)), out, err)
}
