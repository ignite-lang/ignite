#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::ignite_std::net::Endpoint;

pub struct Greeting {}

impl Greeting {
    pub fn serve(mut stream: ::ignite_io::TcpStream, mut text: String) -> ::std::result::Result<(), ::ignite_io::IoError> {
        (stream.write_all(text.clone().into_bytes()))?;
        (stream.close())?;
        return Ok(());
    }

    pub fn connect(mut stream: ::ignite_io::TcpStream) -> ::std::result::Result<(), ::ignite_io::IoError> {
        loop {
            let input = (stream.read(1024))?;
            match input.clone() {
                ::ignite_io::Input::Eof => {
                    return Ok(());
                },
                ::ignite_io::Input::Data(bytes) => {
                    ::ignite_io::log(&((bytes.len() as i64).to_string()))
                },
            };
        }
    }
}

#[derive(Clone)]
pub struct Greeter {
    pub listener: ::ignite_io::TcpListener,
    pub text: String,
    pub executable: String,
}

#[allow(non_snake_case)]
pub fn Greeter__main(self_: &Greeter) -> ::std::result::Result<(), ::ignite_io::IoError> {
    loop {
        let stream = (self_.listener.accept())?;
        { let __spawn_0 = stream; let __spawn_1 = self_.text.clone(); ::ignite_io::spawn(move || { if let Err(error) = Greeting::serve(__spawn_0, __spawn_1) { ::ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    }
}

#[derive(Clone)]
pub struct Reader {
    pub target: Endpoint,
    pub executable: String,
}

#[allow(non_snake_case)]
pub fn Reader__main(self_: &Reader) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let stream = match ::ignite_io::TcpStream::connect(self_.target.address.clone(), self_.target.port) {
        Ok(stream) => stream,
        Err(error) => return Err(error.clone().with_exit_code((3) as i32)),
    };
    match Greeting::connect(stream.clone()) {
        Ok(_) => Ok(()),
        Err(error) => Err(error.clone().with_exit_code((4) as i32)),
    }
}
