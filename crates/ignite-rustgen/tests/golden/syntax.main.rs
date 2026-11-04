#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use syntax::*;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let syntax = Runner { address: String::from("127.0.0.1"), port: 1 };
        Runner__main(&syntax)
    }))
}
