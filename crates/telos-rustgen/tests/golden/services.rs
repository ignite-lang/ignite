#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]

pub struct Greeting;

impl Greeting {
    pub fn serve(mut stream: telos_io::TcpStream, mut text: String) -> Result<(), telos_io::IoError> {
        (stream.write_all(text.clone().into_bytes()))?;
        (stream.close())?;
        return Ok(());
    }

    pub fn connect(mut stream: telos_io::TcpStream) -> Result<(), telos_io::IoError> {
        loop {
            let input = (stream.read(1024))?;
            match input.clone() {
                telos_io::Input::Eof => {
                    return Ok(());
                },
                telos_io::Input::Data(bytes) => {
                    telos_io::log(&((bytes.len() as i64).to_string()))
                },
            };
        }
    }
}
