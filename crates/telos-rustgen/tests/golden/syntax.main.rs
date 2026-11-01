#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use syntax::*;

fn main() {
    std::process::exit(telos_io::run(|| {
        let port = telos_io::args::port()?.unwrap_or(1);
        let stream = telos_io::TcpStream::connect(String::from("127.0.0.1"), port).map_err(|error| error.with_exit_code(3))?;
        Syntax::connect(stream).map_err(|error| error.with_exit_code(4))
    }))
}
