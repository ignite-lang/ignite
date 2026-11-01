#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]

pub type IoResult<T> = Result<T, telos_io::IoError>;

#[derive(Clone, Debug)]
pub struct Pair<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static> {
    pub first: A,
    pub second: B,
}

#[derive(Clone, Debug)]
pub struct Labelled<T: Clone + Send + Sync + 'static> {
    pub value: T,
    pub label: String,
}

impl<T: Clone + Send + Sync + 'static> Labelled<T> {
    #[allow(non_snake_case)]
    pub fn default_label() -> String {
        String::from("none")
    }
}

#[derive(Clone, Debug)]
pub enum Choice<T: Clone + Send + Sync + 'static> {
    Nothing,
    One(T),
    Two(T, T),
}

#[derive(Clone, Debug)]
pub struct App<S: Clone + Send + Sync + 'static> {
    pub state: S,
    pub handle: fn(S, String) -> String,
}

pub fn swap<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>(p: &Pair<A, B>) -> Pair<B, A> {
    Pair { first: p.second.clone(), second: p.first.clone() }
}

pub fn or<T: Clone + Send + Sync + 'static>(value: &Option<T>, fallback: &T) -> T {
    match value.clone() {
        Some(present) => present,
        None => fallback.clone(),
    }
}

pub fn count<T: Clone + Send + Sync + 'static>(choice: &Choice<T>) -> i64 {
    match choice.clone() {
        Choice::Nothing => 0,
        Choice::One(_) => 1,
        Choice::Two(_, _) => 2,
    }
}

pub fn twice<T: Clone + Send + Sync + 'static>(f: &fn(T) -> T, value: &T) -> T {
    let mut f = f.clone();
    f(f(value.clone()))
}

pub fn identity<T: Clone + Send + Sync + 'static>(value: &T) -> T {
    value.clone()
}

pub fn bump(n: &i64) -> i64 {
    let mut n = n.clone();
    n + 1
}

pub fn show(n: &i64) -> String {
    let mut n = n.clone();
    n.to_string()
}

pub fn generic_line() -> String {
    let pair = swap(&(Pair { first: 1, second: String::from("one") }));
    let labelled = Labelled { value: pair.second.clone(), label: Labelled::<i64>::default_label() };
    let choice: Choice<String> = Choice::Two(String::from("a"), String::from("b"));
    let id: fn(i64) -> i64 = ((|__a0: i64| identity::<i64>(&__a0)) as fn(i64) -> i64);
    format!("{}{}{}{}{}{}{}{}{}", pair.first.clone(), String::from(" "), labelled.value.to_string(), String::from(" "), labelled.label.clone(), String::from(" "), count(&choice).to_string(), String::from(" "), twice(&((|__a0: i64| bump(&__a0)) as fn(i64) -> i64), &id(5)).to_string())
}

pub fn number(line: &String, command: &String) -> Option<i64> {
    match line.strip_prefix(&command.clone()).map(|s| s.to_string()) {
        Some(rest) => Some(rest.clone()).filter(|t: &String| !t.starts_with('+')).and_then(|t| t.parse::<i64>().ok()),
        None => None,
    }
}

pub fn respond(counts: &telos_io::Store<i64>, line: &String) -> String {
    if (*line) == String::from("new") {
        let id = counts.next_id();
        counts.put(id.clone(), 0);
        return id.to_string();
    };
    if (*line) == String::from("all") {
        return counts.values().iter().cloned().map(((|__a0: i64| show(&__a0)) as fn(i64) -> String)).collect::<Vec<_>>().join(&String::from(","));
    };
    if (*line) == String::from("generic") {
        return generic_line();
    };
    match number(line, &String::from("inc ")) {
        Some(id) => return show(&or(&counts.update(id.clone(), ((|__a0: i64| bump(&__a0)) as fn(i64) -> i64)), &(-1))),
        None => {
        },
    };
    match number(line, &String::from("get ")) {
        Some(id) => return match counts.get(id.clone()) {
            Some(value) => show(&value),
            None => String::from("missing"),
        },
        None => {
        },
    };
    match number(line, &String::from("del ")) {
        Some(id) => return match counts.remove(id.clone()) {
            Some(value) => format!("{}{}", String::from("removed "), value.to_string()),
            None => String::from("missing"),
        },
        None => {
        },
    };
    match number(line, &String::from("byte ")) {
        Some(n) => match u8::try_from(n).ok() {
            Some(b) => format!("{}{}", String::from("byte "), (b as i64).to_string()),
            None => String::from("not a byte"),
        },
        None => String::from("unknown"),
    }
}

pub struct Lines;

impl Lines {
    pub fn serve<S: Clone + Send + Sync + 'static>(mut stream: telos_io::TcpStream, mut app: App<S>) -> IoResult<()> {
        (stream.set_idle_timeout(5000))?;
        loop {
            match (stream.read(4096))? {
                telos_io::Input::Eof => return Ok(()),
                telos_io::Input::Data(bytes) => {
                    let reply = (app.handle)(app.state.clone(), String::from_utf8_lossy(&bytes).into_owned().trim().to_string());
                    (stream.write_all((format!("{}{}", reply, String::from("\n"))).clone().into_bytes()))?
                },
            };
        }
    }
}
