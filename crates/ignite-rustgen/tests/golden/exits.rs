#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::ignite_std::net::Endpoint;

pub struct Exiting {}

impl Exiting {
    pub fn connect(mut stream: ::ignite_io::TcpStream) -> Result<(), ::ignite_io::IoError> {
        let input = (stream.read(16))?;
        match input.clone() {
            ::ignite_io::Input::Eof => ::ignite_io::exit((5) as i32),
            ::ignite_io::Input::Data(_) => ::ignite_io::exit((6) as i32),
        }
    }
}

pub fn stop(code: &i64) -> Result<(), ::ignite_io::IoError> {
    let mut code = code.clone();
    ::ignite_io::log(&(String::from("stopping")));
    ::ignite_io::exit((code.clone()) as i32)
}

pub fn stop_tried() -> i64 {
    ::ignite_io::exit((7) as i32)
}

#[derive(Clone)]
pub struct Exiter {
    pub target: Endpoint,
    pub executable: String,
}

#[allow(non_snake_case)]
pub fn Exiter__main(self_: &Exiter) -> Result<(), ::ignite_io::IoError> {
    let stream = match ::ignite_io::TcpStream::connect(self_.target.address.clone(), self_.target.port.clone()) {
        Ok(stream) => stream,
        Err(error) => return Err(error.clone().with_exit_code((3) as i32)),
    };
    match Exiting::connect(stream.clone()) {
        Ok(_) => Ok(()),
        Err(error) => Err(error.clone().with_exit_code((4) as i32)),
    }
}
