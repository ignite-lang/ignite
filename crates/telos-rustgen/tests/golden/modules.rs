#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use telos_std::text::{Handler, Mode, shout};
use telos_std::protocol::line::Line;

pub fn quiet(line: &String) -> String {
    return line.trim().to_string();
}

pub fn pick(mode: &Mode) -> fn(String) -> String {
    match mode.clone() {
        Mode::Loud => ((|__a0: String| shout(&__a0)) as fn(String) -> String),
        Mode::Quiet => ((|__a0: String| quiet(&__a0)) as fn(String) -> String),
    }
}

pub fn apply_twice(f: &fn(String) -> String, line: &String) -> String {
    let mut f = f.clone();
    return f(f(line.clone()));
}

pub fn relay(stream: &telos_io::TcpStream, handler: &Handler) -> Result<(), telos_io::IoError> {
    let copy = handler.clone();
    { let __spawn_0 = stream.clone(); let __spawn_1 = copy.clone(); telos_io::spawn(move || { if let Err(error) = Line::serve(__spawn_0, __spawn_1) { telos_io::log(&format!("task ended with error: {}", error)); } }); };
    { let __spawn_0 = stream.clone(); let __spawn_1 = handler.name.clone(); telos_io::spawn(move || { if let Err(error) = relay_log(&__spawn_0, &__spawn_1) { telos_io::log(&format!("task ended with error: {}", error)); } }); };
    return Line::serve(stream.clone(), handler.clone());
}

pub fn relay_log(stream: &telos_io::TcpStream, name: &String) -> Result<(), telos_io::IoError> {
    telos_io::log(&(name.clone()));
    return Ok(());
}
