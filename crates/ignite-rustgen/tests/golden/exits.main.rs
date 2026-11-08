#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::exits::*;
#[global_allocator]
static __IGNITE_ALLOCATOR: ::ignite_io::census::Counting = ::ignite_io::census::Counting;
use ::ignite_std::net::Endpoint;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __endpoint_back = Endpoint { address: String::from("127.0.0.1"), port: __port.unwrap_or(7002) };
        let __res_exiter = Exiter { target: __endpoint_back.clone(), executable: String::from("exiter") };
        Exiter__main(&__res_exiter)
    }))
}
