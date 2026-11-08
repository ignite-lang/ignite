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
        let __res_front = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(7000))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", __res_front.local_port()));
        let __res_server = Echod { listener: __res_front.clone(), application: Handler { name: String::from("shout"), apply: pick(&Mode::Loud) }, executable: String::from("echod") };
        Echod__main(&__res_server)
    }))
}
