#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]

pub mod apply {
    use crate::modes::{level, pick};

    pub fn louder(n: &i64) -> i64 {
        let mut n = n.clone();
        ignite_io::checked::add(level(&crate::modes::Mode::Loud), level(&crate::modes::Mode::Quiet(n.clone())), "crates/ignite-rustgen/tests/golden/variants/std/apply.ignite", 5, 28)
    }

    pub fn describe(loud: &bool) -> String {
        let mut loud = loud.clone();
        match pick(&loud) {
            crate::modes::Mode::Loud => String::from("loud"),
            crate::modes::Mode::Quiet(_) => String::from("quiet"),
        }
    }
}

pub mod modes {

    #[derive(Clone)]
    pub enum Mode {
        Loud,
        Quiet(i64),
    }

    pub fn level(mode: &Mode) -> i64 {
        match mode.clone() {
            Mode::Loud => 10,
            Mode::Quiet(n) => n,
        }
    }

    pub fn pick(loud: &bool) -> Mode {
        let mut loud = loud.clone();
        if loud {
            Mode::Loud
        } else {
            Mode::Quiet(1)
        }
    }
}
