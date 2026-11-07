#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use components::*;
use ::ignite_std::net::Endpoint;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __endpoint_ingress = Endpoint { address: String::from("127.0.0.1"), port: __port.unwrap_or(2500) };
        let client = Pinger { target: __endpoint_ingress.clone(), executable: String::from("pinger") };
        Pinger__main(&client)
    }))
}
