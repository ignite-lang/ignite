#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::components::*;
#[global_allocator]
static __IGNITE_ALLOCATOR: ::ignite_io::census::Counting = ::ignite_io::census::Counting;
use ::ignite_std::net::Endpoint;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __res_seen: ::ignite_io::Store<String> = ::ignite_io::Store::new();
        let __port = ::ignite_io::args::port()?;
        let __res_ingress = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(2500))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", __res_ingress.local_port()));
        let __res_banner = Banner { text: String::from("hello"), width: Banner::default_width() };
        let __res_server = Greeter { listener: __res_ingress.clone(), seen: __res_seen.clone(), banner: __res_banner.clone(), executable: String::from("greeterd"), idle: Greeter::default_idle() };
        Greeter__main(__res_server)
    }))
}
