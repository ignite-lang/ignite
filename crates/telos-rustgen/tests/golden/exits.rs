#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]

pub struct Exiting;

impl Exiting {
    pub fn connect(mut stream: telos_io::TcpStream) -> Result<(), telos_io::IoError> {
        let input = (stream.read(16))?;
        match input.clone() {
            telos_io::Input::Eof => telos_io::exit((5) as i32),
            telos_io::Input::Data(_) => telos_io::exit((6) as i32),
        }
    }
}

pub fn stop(code: &i64) -> Result<(), telos_io::IoError> {
    let mut code = code.clone();
    telos_io::log(&(String::from("stopping")));
    telos_io::exit((code.clone()) as i32)
}

pub fn stop_tried() -> i64 {
    telos_io::exit((7) as i32)
}
