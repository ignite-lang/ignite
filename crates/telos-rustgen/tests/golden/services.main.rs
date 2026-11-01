#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use services::*;

fn main() {
    std::process::exit(telos_io::run(|| {
        let application = String::from("hello\r\n");
        let port = telos_io::args::port()?.unwrap_or(7000);
        let front = telos_io::TcpListener::bind(String::from("127.0.0.1"), port)?;
        telos_io::log(&format!("listening on 127.0.0.1:{}", front.local_port()));
        loop {
            let stream = front.accept()?;
            let application = application.clone();
            telos_io::spawn(move || {
                if let Err(error) = Greeting::serve(stream, application) {
                    telos_io::log(&format!("session ended with error: {}", error));
                }
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), telos_io::IoError>(())
    }))
}
