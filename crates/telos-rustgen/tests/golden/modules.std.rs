#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]

pub mod protocol {

    pub mod line {
        use crate::text::Handler;

        pub struct Line;

        impl Line {
            pub fn serve(mut stream: telos_io::TcpStream, mut handler: Handler) -> Result<(), telos_io::IoError> {
                let apply = handler.apply.clone();
                loop {
                    let input = (stream.read(1024))?;
                    match input.clone() {
                        telos_io::Input::Eof => {
                            return Ok(());
                        },
                        telos_io::Input::Data(bytes) => {
                            let reply = apply(String::from_utf8_lossy(&bytes).into_owned());
                            (stream.write_all(reply.clone().into_bytes()))?
                        },
                    };
                }
            }

            pub fn connect(mut stream: telos_io::TcpStream) -> Result<(), telos_io::IoError> {
                { let __spawn_0 = stream.clone(); telos_io::spawn(move || { if let Err(error) = pump(&__spawn_0) { telos_io::log(&format!("task ended with error: {}", error)); } }); };
                loop {
                    let input = (stream.read(1024))?;
                    match input.clone() {
                        telos_io::Input::Eof => {
                            return Ok(());
                        },
                        telos_io::Input::Data(bytes) => {
                            (telos_io::Stdout::write_all(bytes.clone()))?
                        },
                    };
                }
            }
        }

        pub fn pump(stream: &telos_io::TcpStream) -> Result<(), telos_io::IoError> {
            loop {
                let input = (telos_io::Stdin::read(1024))?;
                match input.clone() {
                    telos_io::Input::Eof => {
                        (stream.close())?;
                        return Ok(());
                    },
                    telos_io::Input::Data(bytes) => {
                        (stream.write_all(bytes.clone()))?
                    },
                };
            }
        }
    }
}

pub mod text {

    #[derive(Clone, Debug)]
    pub struct Handler {
        pub name: String,
        pub apply: fn(String) -> String,
    }

    #[derive(Clone, Debug)]
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
