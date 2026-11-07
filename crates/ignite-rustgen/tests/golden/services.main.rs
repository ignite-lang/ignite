#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::services::*;
use ::ignite_std::net::Endpoint;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __res_front = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(7000))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", __res_front.local_port()));
        let __res_greeter = Greeter { listener: __res_front.clone(), text: String::from("hello\r\n"), executable: String::from("greeter") };
        Greeter__main(&__res_greeter)
    }))
}
