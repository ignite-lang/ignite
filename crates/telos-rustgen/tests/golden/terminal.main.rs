#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use terminal::*;
use telos_std::console::{Console, Reply, first_word, rest_after_word};
use telos_std::protocol::telnet::Telnet;

fn main() {
    std::process::exit(telos_io::run(|| {
        let application = Console { prompt: PROMPT(), respond: ((|__a0: String| execute(&__a0)) as fn(String) -> Reply), maximum_line: MAXIMUM_LINE() };
        let port = telos_io::args::port()?.unwrap_or(2323);
        let ingress = telos_io::TcpListener::bind(String::from("127.0.0.1"), port)?;
        telos_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        loop {
            let stream = ingress.accept()?;
            let application = application.clone();
            telos_io::spawn(move || {
                if let Err(error) = Telnet::serve(stream, application) {
                    telos_io::log(&format!("session ended with error: {}", error));
                }
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), telos_io::IoError>(())
    }))
}
