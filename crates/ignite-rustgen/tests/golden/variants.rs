#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ignite_std::modes::{level, pick};
use ignite_std::apply::describe;

pub fn program_level(n: &i64) -> i64 {
    let mut n = n.clone();
    ignite_io::checked::add(level(&ignite_std::modes::Mode::Loud), level(&ignite_std::modes::Mode::Quiet(n.clone())), "crates/ignite-rustgen/tests/golden/variants.ignite", 6, 35)
}

pub fn quietness(loud: &bool) -> i64 {
    let mut loud = loud.clone();
    match pick(&loud) {
        ignite_std::modes::Mode::Loud => 0,
        ignite_std::modes::Mode::Quiet(n) => n,
    }
}

pub fn both(loud: &bool) -> String {
    let mut loud = loud.clone();
    describe(&loud)
}
