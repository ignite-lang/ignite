#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use components::*;
use ignite_std::net::Endpoint;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let seen: ignite_io::Store<String> = ignite_io::Store::new();
        let port = ignite_io::args::port()?;
        let ingress = ignite_io::TcpListener::bind(String::from("127.0.0.1"), port.unwrap_or(2500))?;
        ignite_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        let banner = Banner { text: String::from("hello"), width: Banner::default_width() };
        let server = Greeter { listener: ingress.clone(), seen: seen.clone(), banner: banner.clone(), executable: String::from("greeterd"), idle: Greeter::default_idle() };
        Greeter__main(server)
    }))
}
