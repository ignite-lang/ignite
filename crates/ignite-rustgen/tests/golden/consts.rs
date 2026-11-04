#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]
use ignite_std::text::{Mode, SUFFIX, shout};

#[allow(non_snake_case)]
pub fn LIMIT() -> i64 {
    4096
}

#[allow(non_snake_case)]
pub fn DOUBLE_LIMIT() -> i64 {
    ignite_io::checked::mul(LIMIT(), 2, "crates/ignite-rustgen/tests/golden/consts.ignite", 7, 27)
}

#[allow(non_snake_case)]
pub fn GREETING() -> String {
    format!("{}{}", String::from("hello"), SUFFIX())
}

#[allow(non_snake_case)]
pub fn LOUD() -> Mode {
    Mode::Loud
}

#[derive(Clone)]
pub enum Verdict {
    Accept(i64),
    Reject(String),
}

pub fn judge(size: &i64) -> Verdict {
    let mut size = size.clone();
    if size > LIMIT() {
        return Verdict::Reject(format!("{}{}", String::from("too long: "), size.clone().to_string()));
    };
    return Verdict::Accept(ignite_io::checked::sub(DOUBLE_LIMIT(), size, "crates/ignite-rustgen/tests/golden/consts.ignite", 20, 25));
}

pub fn describe(verdict: &Verdict, mode: &Mode) -> String {
    match verdict {
        Verdict::Accept(room) => { let room = (room).clone(); {
            match mode {
                Mode::Loud => shout(&GREETING()),
                Mode::Quiet => room.clone().to_string(),
            }
        } },
        Verdict::Reject(reason) => reason.clone(),
    }
}

pub fn admit(size: &i64) -> Result<i64, ignite_io::IoError> {
    let mut size = size.clone();
    match judge(&size) {
        Verdict::Accept(room) => Ok(room.clone()),
        Verdict::Reject(reason) => Err(ignite_io::IoError::new(reason.clone())),
    }
}

pub fn or_limit(value: &Option<i64>) -> i64 {
    match value.clone() {
        Some(v) => v,
        None => LIMIT(),
    }
}

pub fn default_mode() -> Mode {
    return LOUD();
}
