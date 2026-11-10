#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

pub mod apply {
    use crate::modes::pick;

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
        match mode {
            Mode::Loud => 10,
            Mode::Quiet(n) => { let n = (n).clone(); n },
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
