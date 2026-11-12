#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::ignite_std::text::{Handler, Mode, shout};
use ::ignite_std::protocol::line::Line;
use ::ignite_std::net::Endpoint;

pub fn quiet(line: &String) -> String {
    return line.trim().to_string();
}

pub fn pick(mode: &Mode) -> ::std::sync::Arc<dyn Fn(String) -> String + Send + Sync> {
    match mode {
        Mode::Loud => (::std::sync::Arc::new(|__a0: String| shout(&__a0)) as ::std::sync::Arc<dyn Fn(String) -> String + Send + Sync>),
        Mode::Quiet => (::std::sync::Arc::new(|__a0: String| quiet(&__a0)) as ::std::sync::Arc<dyn Fn(String) -> String + Send + Sync>),
    }
}

pub fn apply_twice(f: &(dyn Fn(String) -> String + Send + Sync), line: String) -> String {
    return f(f(line));
}

pub fn relay(stream: &::ignite_io::TcpStream, handler: &Handler) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let copy = handler.clone();
    { let __spawn_0 = stream.clone(); let __spawn_1 = copy; ::ignite_io::spawn(move || { if let Err(error) = Line::serve(__spawn_0, __spawn_1) { ::ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    { let __spawn_0 = stream.clone(); let __spawn_1 = handler.name.clone(); ::ignite_io::spawn(move || { if let Err(error) = relay_log(&__spawn_0, &__spawn_1) { ::ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    return Line::serve(stream.clone(), handler.clone());
}

pub fn relay_log(stream: &::ignite_io::TcpStream, name: &String) -> ::std::result::Result<(), ::ignite_io::IoError> {
    ::ignite_io::log(&(name.clone()));
    return Ok(());
}

#[derive(Clone)]
pub struct Echod {
    pub listener: ::ignite_io::TcpListener,
    pub application: Handler,
    pub executable: String,
}

#[allow(non_snake_case)]
pub fn Echod__main(self_: &Echod) -> ::std::result::Result<(), ::ignite_io::IoError> {
    loop {
        let stream = (self_.listener.accept())?;
        { let __spawn_0 = stream; let __spawn_1 = self_.application.clone(); ::ignite_io::spawn(move || { if let Err(error) = Line::serve(__spawn_0, __spawn_1) { ::ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    }
}

#[derive(Clone)]
pub struct Echo {
    pub target: Endpoint,
    pub executable: String,
}

#[allow(non_snake_case)]
pub fn Echo__main(self_: &Echo) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let stream = match ::ignite_io::TcpStream::connect(self_.target.address.clone(), self_.target.port) {
        Ok(stream) => stream,
        Err(error) => return Err(error.clone().with_exit_code((3) as i32)),
    };
    match Line::connect(stream.clone()) {
        Ok(_) => Ok(()),
        Err(error) => Err(error.clone().with_exit_code((4) as i32)),
    }
}
