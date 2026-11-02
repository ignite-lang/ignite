#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]
use exits::*;
use ignite_std::net::Endpoint;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let back_endpoint = Endpoint { address: String::from("127.0.0.1"), port: port.unwrap_or(7002) };
        let exiter = Exiter { target: back_endpoint.clone(), executable: String::from("exiter") };
        Exiter__main(&exiter)
    }))
}
