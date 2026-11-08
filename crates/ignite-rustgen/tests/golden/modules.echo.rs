#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::modules::*;
#[global_allocator]
static __IGNITE_ALLOCATOR: ::ignite_io::census::Counting = ::ignite_io::census::Counting;
use ::ignite_std::text::{Handler, Mode, shout};
use ::ignite_std::protocol::line::Line;
use ::ignite_std::net::Endpoint;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __endpoint_front = Endpoint { address: String::from("127.0.0.1"), port: __port.unwrap_or(7000) };
        let __res_client = Echo { target: __endpoint_front.clone(), executable: String::from("echo") };
        Echo__main(&__res_client)
    }))
}
