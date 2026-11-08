#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::bounds::*;
#[global_allocator]
static __IGNITE_ALLOCATOR: ::ignite_io::census::Counting = ::ignite_io::census::Counting;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __res_ingress = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(7600))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", __res_ingress.local_port()));
        let __res_server = Server { listener: __res_ingress.clone(), limit: 5000, executable: String::from("relayd") };
        Server__main(&__res_server)
    }))
}
