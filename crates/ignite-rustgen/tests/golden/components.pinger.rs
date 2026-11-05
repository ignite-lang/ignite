#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use components::*;
use ignite_std::net::Endpoint;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let ingress_endpoint = Endpoint { address: String::from("127.0.0.1"), port: port.unwrap_or(2500) };
        let client = Pinger { target: ingress_endpoint.clone(), executable: String::from("pinger") };
        Pinger__main(&client)
    }))
}
