#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

pub mod net {
}

pub mod protocol {

    pub mod line {
    }
}

pub mod text {

    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Mode {
        Loud,
        Quiet,
    }

    #[allow(non_snake_case)]
    pub fn SUFFIX() -> String {
        String::from("!")
    }

    pub fn shout(line: &String) -> String {
        return format!("{}{}", line, SUFFIX());
    }
}
