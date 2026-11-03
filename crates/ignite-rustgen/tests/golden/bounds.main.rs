#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]
use bounds::*;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let ingress = ignite_io::TcpListener::bind(String::from("127.0.0.1"), port.unwrap_or(7600))?;
        ignite_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        let server = Server { listener: ingress.clone(), limit: 5000, executable: String::from("relayd") };
        Server__main(&server)
    }))
}
