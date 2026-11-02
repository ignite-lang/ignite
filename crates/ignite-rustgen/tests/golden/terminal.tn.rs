#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]
use terminal::*;
use ignite_std::console::CommandConsole;
use ignite_std::protocol::telnet::{TelnetServer, TelnetClient};

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let ingress_endpoint = ignite_std::net::Endpoint { address: String::from("127.0.0.1"), port: port.unwrap_or(2323) };
        let client = TelnetClient { target: ingress_endpoint.clone(), executable: String::from("tn") };
        ignite_std::protocol::telnet::TelnetClient__main(&client)
    }))
}
