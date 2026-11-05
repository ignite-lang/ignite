#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

pub type IoResult<T> = Result<T, ignite_io::IoError>;

pub struct Relay;

impl Relay {
    pub fn serve<S: ignite_io::ByteStream + Clone + Send + Sync + 'static>(mut stream: S, mut limit: i64) -> IoResult<()> {
        (<S as ignite_io::ByteStream>::set_idle_timeout(&stream, limit.clone()))?;
        loop {
            match (<S as ignite_io::ByteStream>::read(&stream, 4096))? {
                ignite_io::Input::Eof => return <S as ignite_io::ByteStream>::close(&stream),
                ignite_io::Input::Data(bytes) => {
                    (<S as ignite_io::ByteStream>::write_all(&stream, bytes.clone()))?
                },
            };
        }
    }
}

pub fn identity<T: Clone + Send + Sync + 'static>(value: T) -> T {
    value
}

pub fn relay<S: ignite_io::ByteStream + Clone + Send + Sync + 'static>(stream: &S) -> IoResult<()> {
    match (<S as ignite_io::ByteStream>::read(&stream, 16))? {
        ignite_io::Input::Eof => {
        },
        ignite_io::Input::Data(bytes) => {
            (<S as ignite_io::ByteStream>::write_all(&stream, identity(bytes.clone())))?
        },
    };
    let copy = stream.clone();
    (<S as ignite_io::ByteStream>::write_all(&copy, String::from("x").into_bytes()))?;
    <S as ignite_io::ByteStream>::close(&stream)
}

pub fn once<S: ignite_io::ByteStream + Clone + Send + Sync + 'static>(stream: &S) -> IoResult<()> {
    <S as ignite_io::ByteStream>::write_all(&stream, String::from("y").into_bytes())
}

pub fn forward<T: ignite_io::ByteStream + Clone + Send + Sync + 'static>(inner: &T) -> IoResult<()> {
    (once::<T>(inner))?;
    { let __spawn_0 = inner.clone(); ignite_io::spawn(move || { if let Err(error) = relay::<T>(&__spawn_0) { ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    relay::<T>(inner)
}

pub fn over_tcp(stream: &ignite_io::TcpStream) -> IoResult<()> {
    (stream.set_idle_timeout(100))?;
    (relay::<ignite_io::TcpStream>(stream))?;
    (forward::<ignite_io::TcpStream>(stream))?;
    { let __spawn_0 = stream.clone(); ignite_io::spawn(move || { if let Err(error) = relay::<ignite_io::TcpStream>(&__spawn_0) { ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    Relay::serve::<ignite_io::TcpStream>(stream.clone(), 5000)
}

pub fn over_pipe(end: &ignite_io::PipeStream) -> IoResult<i64> {
    (end.set_idle_timeout(100))?;
    (end.write_all(String::from("z").into_bytes()))?;
    (relay::<ignite_io::PipeStream>(end))?;
    (once::<ignite_io::PipeStream>(end))?;
    (forward::<ignite_io::PipeStream>(end))?;
    { let __spawn_0 = end.clone(); let __spawn_1 = 5000; ignite_io::spawn(move || { if let Err(error) = Relay::serve::<ignite_io::PipeStream>(__spawn_0, __spawn_1) { ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    Ok(end.received_batches())
}

#[derive(Clone)]
pub struct Server {
    pub listener: ignite_io::TcpListener,
    pub limit: i64,
    pub executable: String,
}

#[allow(non_snake_case)]
pub fn Server__main(self_: &Server) -> IoResult<()> {
    loop {
        let stream = (self_.listener.accept())?;
        { let __spawn_0 = stream.clone(); let __spawn_1 = self_.limit.clone(); ignite_io::spawn(move || { if let Err(error) = Relay::serve::<ignite_io::TcpStream>(__spawn_0, __spawn_1) { ignite_io::log(&format!("task ended with error: {}", error)); } }); };
    }
}
