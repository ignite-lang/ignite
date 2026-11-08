#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::terminal::*;
#[global_allocator]
static __IGNITE_ALLOCATOR: ::ignite_io::census::Counting = ::ignite_io::census::Counting;
use ::ignite_std::console::CommandConsole;
use ::ignite_std::protocol::telnet::{TelnetServer, TelnetClient};

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __endpoint_ingress = ::ignite_std::net::Endpoint { address: String::from("127.0.0.1"), port: __port.unwrap_or(2323) };
        let __res_client = TelnetClient { target: __endpoint_ingress.clone(), executable: String::from("tn") };
        ::ignite_std::protocol::telnet::TelnetClient__main(&__res_client)
    }))
}
