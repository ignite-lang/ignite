#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]
use services::*;
use ignite_std::net::Endpoint;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let front = ignite_io::TcpListener::bind(String::from("127.0.0.1"), port.unwrap_or(7000))?;
        ignite_io::log(&format!("listening on 127.0.0.1:{}", front.local_port()));
        let greeter = Greeter { listener: front.clone(), text: String::from("hello\r\n"), executable: String::from("greeter") };
        Greeter__main(&greeter)
    }))
}
