#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use services::*;

fn main() {
    std::process::exit(telos_io::run(|| {
        let port = telos_io::args::port()?.unwrap_or(7001);
        let stream = telos_io::TcpStream::connect(String::from("0.0.0.0"), port).map_err(|error| error.with_exit_code(3))?;
        Greeting::connect(stream).map_err(|error| error.with_exit_code(4))
    }))
}
