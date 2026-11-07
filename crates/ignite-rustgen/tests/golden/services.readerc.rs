#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::services::*;
use ::ignite_std::net::Endpoint;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __endpoint_back = Endpoint { address: String::from("0.0.0.0"), port: __port.unwrap_or(7001) };
        let __res_reader = Reader { target: __endpoint_back.clone(), executable: String::from("readerc") };
        Reader__main(&__res_reader)
    }))
}
