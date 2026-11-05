//! Files (`docs/slice.md` 21.4): the C ABI over `ignite_io::Directory`. A
//! directory is a handle of the one table in `crate::net`; it lives until
//! the process exits. Every operation is `ignite-io`'s; results are copied
//! into the calling task's arena.

use ignite_io::{Directory, IoError};

use crate::net::{Handle, capability, register};
use crate::{IgniteIoError, IgniteList, bytes, status};

fn directory(handle: i64) -> Result<Directory, IoError> {
    capability(handle, "directory", |found| match found {
        Handle::Directory(directory) => Some(directory),
        _ => None,
    })
}

fn text(value: *const IgniteList) -> String {
    String::from_utf8_lossy(bytes(value)).into_owned()
}

/// `i32 directory_launch(Text* path, i8 create, Text* flag, i64* out,
/// IoError* err)`: the resource at launch (21.4, 21.6); the value of `flag`
/// from the arguments `init` stored, when `flag` is not empty and given,
/// replaces `path`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_launch(
    path: *const IgniteList,
    create: i8,
    flag: *const IgniteList,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let flag = text(flag);
        let opened = crate::process::flag_value(&flag).and_then(|given| {
            let path = given.unwrap_or_else(|| text(path));
            Directory::open(&path, create != 0)
        });
        status(opened.map(|d| register(Handle::Directory(d))), out, err)
    })
}

/// `i32 directory_temporary(i64* out, IoError* err)`: `Directory.temporary()`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_temporary(out: *mut i64, err: *mut IgniteIoError) -> i32 {
    crate::entry(move || {
        let made = Directory::temporary().map(|d| register(Handle::Directory(d)));
        status(made, out, err)
    })
}

/// `i32 directory_read_file(i64 d, Text* path, Bytes* out, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_read_file(
    handle: i64,
    path: *const IgniteList,
    out: *mut IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let read = directory(handle).and_then(|d| d.read_file(text(path)));
        status(read.map(|data| IgniteList::from_bytes(&data)), out, err)
    })
}

/// `i32 directory_write_file(i64 d, Text* path, Bytes* data, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_write_file(
    handle: i64,
    path: *const IgniteList,
    data: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let written = directory(handle).and_then(|d| d.write_file(text(path), bytes(data)));
        status(written, std::ptr::null_mut(), err)
    })
}

/// `i32 directory_read_directory(i64 d, Text* path, List<Text>* out,
/// IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_read_directory(
    handle: i64,
    path: *const IgniteList,
    out: *mut IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let listed = directory(handle).and_then(|d| d.read_directory(text(path)));
        status(listed.map(|names| IgniteList::from_texts(&names)), out, err)
    })
}

/// `i32 directory_create_directory(i64 d, Text* path, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_create_directory(
    handle: i64,
    path: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let made = directory(handle).and_then(|d| d.create_directory(text(path)));
        status(made, std::ptr::null_mut(), err)
    })
}

/// `i32 directory_remove(i64 d, Text* path, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_remove(
    handle: i64,
    path: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let removed = directory(handle).and_then(|d| d.remove(text(path)));
        status(removed, std::ptr::null_mut(), err)
    })
}

/// `i32 directory_remove_tree(i64 d, Text* path, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_remove_tree(
    handle: i64,
    path: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let removed = directory(handle).and_then(|d| d.remove_tree(text(path)));
        status(removed, std::ptr::null_mut(), err)
    })
}

/// `i32 directory_absolute_path(i64 d, Text* out, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_directory_absolute_path(
    handle: i64,
    out: *mut IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let path = directory(handle).and_then(|d| d.absolute_path());
        status(path.map(|p| IgniteList::from_bytes(p.as_bytes())), out, err)
    })
}
