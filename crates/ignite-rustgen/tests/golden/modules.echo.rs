#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use modules::*;
use ignite_std::text::{Handler, Mode, shout};
use ignite_std::protocol::line::Line;
use ignite_std::net::Endpoint;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let front_endpoint = Endpoint { address: String::from("127.0.0.1"), port: port.unwrap_or(7000) };
        let client = Echo { target: front_endpoint.clone(), executable: String::from("echo") };
        Echo__main(&client)
    }))
}
