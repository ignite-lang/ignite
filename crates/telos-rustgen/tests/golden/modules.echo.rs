#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use modules::*;
use telos_std::text::{Handler, Mode, shout};
use telos_std::protocol::line::Line;

fn main() {
    std::process::exit(telos_io::run(|| {
        let port = telos_io::args::port()?.unwrap_or(7000);
        let stream = telos_io::TcpStream::connect(String::from("127.0.0.1"), port).map_err(|error| error.with_exit_code(3))?;
        Line::connect(stream).map_err(|error| error.with_exit_code(4))
    }))
}
