#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use counters::*;

fn main() {
    std::process::exit(telos_io::run(|| {
        let counts: telos_io::Store<i64> = telos_io::Store::new();
        let application = App { state: counts.clone(), handle: ((|__a0: telos_io::Store<i64>, __a1: String| respond(&__a0, &__a1)) as fn(telos_io::Store<i64>, String) -> String) };
        let port = telos_io::args::port()?.unwrap_or(7400);
        let ingress = telos_io::TcpListener::bind(String::from("127.0.0.1"), port)?;
        telos_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        loop {
            let stream = ingress.accept()?;
            let application = application.clone();
            telos_io::spawn(move || {
                if let Err(error) = Lines::serve(stream, application) {
                    telos_io::log(&format!("session ended with error: {}", error));
                }
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), telos_io::IoError>(())
    }))
}
