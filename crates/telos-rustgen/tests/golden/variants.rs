#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use telos_std::modes::{level, pick};
use telos_std::apply::describe;

pub fn program_level(n: &i64) -> i64 {
    let mut n = n.clone();
    level(&telos_std::modes::Mode::Loud) + level(&telos_std::modes::Mode::Quiet(n.clone()))
}

pub fn quietness(loud: &bool) -> i64 {
    let mut loud = loud.clone();
    match pick(&loud) {
        telos_std::modes::Mode::Loud => 0,
        telos_std::modes::Mode::Quiet(n) => n,
    }
}

pub fn both(loud: &bool) -> String {
    let mut loud = loud.clone();
    describe(&loud)
}
