#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]

pub mod net {

    #[derive(Clone)]
    pub struct Endpoint {
        pub address: String,
        pub port: i64,
    }
}

pub mod protocol {

    pub mod line {
        use crate::text::Handler;

        pub struct Line;

        impl Line {
            pub fn serve(mut stream: ignite_io::TcpStream, mut handler: Handler) -> Result<(), ignite_io::IoError> {
                let apply = handler.apply.clone();
                loop {
                    let input = (stream.read(1024))?;
                    match input.clone() {
                        ignite_io::Input::Eof => {
                            return Ok(());
                        },
                        ignite_io::Input::Data(bytes) => {
                            let reply = apply(String::from_utf8_lossy(&bytes).into_owned());
                            (stream.write_all(reply.clone().into_bytes()))?
                        },
                    };
                }
            }

            pub fn connect(mut stream: ignite_io::TcpStream) -> Result<(), ignite_io::IoError> {
                { let __spawn_0 = stream.clone(); ignite_io::spawn(move || { if let Err(error) = pump(&__spawn_0) { ignite_io::log(&format!("task ended with error: {}", error)); } }); };
                loop {
                    let input = (stream.read(1024))?;
                    match input.clone() {
                        ignite_io::Input::Eof => {
                            return Ok(());
                        },
                        ignite_io::Input::Data(bytes) => {
                            (ignite_io::Stdout::write_all(bytes.clone()))?
                        },
                    };
                }
            }
        }

        pub fn pump(stream: &ignite_io::TcpStream) -> Result<(), ignite_io::IoError> {
            loop {
                let input = (ignite_io::Stdin::read(1024))?;
                match input.clone() {
                    ignite_io::Input::Eof => {
                        (stream.close())?;
                        return Ok(());
                    },
                    ignite_io::Input::Data(bytes) => {
                        (stream.write_all(bytes.clone()))?
                    },
                };
            }
        }
    }
}

pub mod text {

    #[derive(Clone)]
    pub struct Handler {
        pub name: String,
        pub apply: std::sync::Arc<dyn Fn(String) -> String + Send + Sync>,
    }

    #[derive(Clone)]
    pub enum Mode {
        Loud,
        Quiet,
    }

    #[allow(non_snake_case)]
    pub fn SUFFIX() -> String {
        String::from("!")
    }

    pub fn shout(line: &String) -> String {
        return format!("{}{}", line, SUFFIX());
    }
}
