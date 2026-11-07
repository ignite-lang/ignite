#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::ignite_std::net::Endpoint;

#[derive(Clone)]
pub struct Banner {
    pub text: String,
    pub width: i64,
}

impl Banner {
    #[allow(non_snake_case)]
    pub fn default_width() -> i64 {
        80i64
    }
}

#[derive(Clone)]
pub struct Greeter {
    pub listener: ::ignite_io::TcpListener,
    pub seen: ::ignite_io::Store<String>,
    pub banner: Banner,
    pub idle: i64,
    pub executable: String,
}

impl Greeter {
    #[allow(non_snake_case)]
    pub fn default_idle() -> i64 {
        30000000000i64
    }
    #[allow(non_snake_case)]
    pub fn default_executable() -> String {
        String::from("greeter")
    }
}

#[allow(non_snake_case)]
pub fn Greeter__main(self_: Greeter) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let stream = (self_.listener.accept())?;
    let id = self_.seen.next_id();
    self_.seen.put(id.clone(), self_.banner.text.clone());
    (stream.write_all((format!("{}{}", self_.banner.text, format!(" {} {}\n", { let __ignite_v = &self_.banner.width; let mut __ignite_text = String::new(); (&mut __ignite_text).push_str(&::ignite_io::quantity_debug(::ignite_io::Quantity::ByteSize, *__ignite_v)); __ignite_text }, { let __ignite_v = &self_.idle; let mut __ignite_text = String::new(); (&mut __ignite_text).push_str(&::ignite_io::quantity_debug(::ignite_io::Quantity::Duration, *__ignite_v)); __ignite_text }))).into_bytes()))?;
    match (stream.read(4096))? {
        ::ignite_io::Input::Eof => Ok(()),
        ::ignite_io::Input::Data(bytes) => if String::from_utf8_lossy(&bytes).into_owned().trim().to_string() == String::from("fail") {
            Err(::ignite_io::IoError::new(String::from("asked to fail")).with_exit_code((3) as i32))
        } else {
            Ok(())
        },
    }
}

#[derive(Clone)]
pub struct Pinger {
    pub target: Endpoint,
    pub executable: String,
}

impl Pinger {
    #[allow(non_snake_case)]
    pub fn default_executable() -> String {
        String::from("ping")
    }
}

#[allow(non_snake_case)]
pub fn Pinger__main(self_: &Pinger) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let stream = (::ignite_io::TcpStream::connect(self_.target.address.clone(), self_.target.port.clone()))?;
    match (stream.read(4096))? {
        ::ignite_io::Input::Eof => Err(::ignite_io::IoError::new(String::from("no banner"))),
        ::ignite_io::Input::Data(bytes) => {
            (stream.write_all(String::from("hello\n").into_bytes()))?;
            Ok(())
        },
    }
}
