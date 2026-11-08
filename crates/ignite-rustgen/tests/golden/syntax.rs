#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

pub type IoResult<T> = ::std::result::Result<T, ::ignite_io::IoError>;

#[derive(Clone)]
pub struct Reply {
    pub lines: Vec<String>,
    pub action: Action,
    pub code: i64,
}

impl Reply {
    #[allow(non_snake_case)]
    pub fn default_action() -> Action {
        Action::Continue
    }
    #[allow(non_snake_case)]
    pub fn default_code() -> i64 {
        ::ignite_io::checked::add(BASE(), 1, "crates/ignite-rustgen/tests/golden/syntax.ignite", 12, 15)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Continue,
    Close,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Light {
    Off,
    On,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Power {
    Off,
    Full,
}

#[derive(Clone)]
pub struct Handler {
    pub name: String,
    pub respond: ::std::sync::Arc<dyn Fn(String) -> String + Send + Sync>,
}

#[allow(non_snake_case)]
pub fn BASE() -> i64 {
    41
}

#[allow(non_snake_case)]
pub fn QUIT() -> String {
    String::from("quit")
}

#[allow(non_snake_case)]
pub fn SPACE() -> u8 {
    32
}

#[allow(non_snake_case)]
pub fn lowest() -> i64 {
    3
}

pub fn shout(t: &String) -> String {
    format!("{}!", t)
}

pub fn twice(x: &i64) -> i64 {
    let mut x = x.clone();
    ::ignite_io::checked::mul(x, 2, "crates/ignite-rustgen/tests/golden/syntax.ignite", 30, 27)
}

pub fn lower_case(n: &i64) -> String {
    let mut n = n.clone();
    match n.clone() {
        __m if (*(&__m) == lowest()) => String::from("lowest"),
        other => format!("other {}", other),
    }
}

pub fn swap(p: &(i64, String)) -> (String, i64) {
    let (n, t) = p.clone();
    (t.clone(), n.clone())
}

pub fn describe(n: &i64) -> String {
    let mut n = n.clone();
    match n.clone() {
        0 | 1 => String::from("small"),
        __m if (*(&__m) == BASE()) => String::from("base"),
        _ => String::from("other"),
    }
}

pub fn classify(words: &Vec<String>) -> String {
    match words {
        __m if (__m.len() == 0) => String::from("none"),
        __m if (__m.len() == 1) => { let w = ((&__m[0])).clone(); format!("one {}", w) },
        __m if (__m.len() == 2 && { let __p2 = &__m[0]; (*__p2 == QUIT()) }) => String::from("quit with argument"),
        __m if (__m.len() == 2) => { let a = ((&__m[0])).clone(); let b = ((&__m[1])).clone(); format!("two {}{}", a, b) },
        _ => String::from("many"),
    }
}

pub fn nested(v: &::std::option::Option<(bool, Vec<i64>)>) -> String {
    match v {
        __m if (match __m { Some(__p6) => { let (__p7, __p8) = __p6; (*__p7 == true) && (__p8.len() == 2) }, _ => false }) => { let x = ((&(&(match __m { Some(__x) => __x, _ => unreachable!() }).1)[0])).clone(); let y = ((&(&(match __m { Some(__x) => __x, _ => unreachable!() }).1)[1])).clone(); format!("pair {} {}", x, y) },
        Some((false, _)) | None => String::from("off or none"),
        Some((true, _)) => String::from("other"),
        #[allow(unreachable_patterns)]
        _ => ::ignite_io::fail("match", "crates/ignite-rustgen/tests/golden/syntax.ignite", 63, 3, "no arm matched"),
    }
}

pub fn vowel(w: &String) -> bool {
    match w {
        __m if ((*__m == "a") || (*__m == "e") || (*__m == "i") || (*__m == "o") || (*__m == "u")) => true,
        _ => false,
    }
}

pub fn first_positive(values: &Vec<i64>) -> i64 {
    for v in (values.clone()) {
        if v > 0 {
            return v.clone();
        };
    }
    0
}

pub fn sign(n: &i64) -> String {
    let mut n = n.clone();
    let s = match n.clone() {
        0 => return String::from("zero"),
        _ => if n > 0 {
            String::from("+")
        } else {
            String::from("-")
        },
    };
    s
}

pub fn count_spaces(bytes: &Vec<u8>) -> i64 {
    let mut n = 0;
    for c in (bytes.clone()) {
        match c.clone() {
            __m if (*(&__m) == SPACE()) => {
                n = ::ignite_io::checked::add(n, 1, "crates/ignite-rustgen/tests/golden/syntax.ignite", 99, 13);
            },
            _ => {
            },
        };
    }
    n
}

pub fn show(text: &String) -> IoResult<()> {
    let mut bytes = text.clone().into_bytes();
    bytes.push(10);
    ::ignite_io::Stdout::write_all(bytes.clone())
}

pub struct Syntax {}

impl Syntax {
    pub fn connect(mut stream: ::ignite_io::TcpStream) -> ::std::result::Result<(), ::ignite_io::IoError> {
        let (word, number) = swap(&(7, String::from("seven")));
        (show(&(format!("{} {}", word, number))))?;
        (show(&(format!("{} {} {}", describe(&0), describe(&41), describe(&9)))))?;
        (show(&classify(&vec![])))?;
        (show(&classify(&vec![String::from("x")])))?;
        (show(&classify(&vec![String::from("quit"), String::from("now")])))?;
        (show(&classify(&vec![String::from("a"), String::from("b")])))?;
        (show(&classify(&vec![String::from("a"), String::from("b"), String::from("c")])))?;
        (show(&nested(&Some((true, vec![1, 2])))))?;
        (show(&nested(&Some((false, vec![1])))))?;
        (show(&nested(&None)))?;
        (show(&nested(&Some((true, vec![3])))))?;
        let e = vowel({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("e")); &*TEXT });
        let x = vowel({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("x")); &*TEXT });
        (show(&(format!("{} {}", e, x))))?;
        (show(&first_positive(&vec![::ignite_io::checked::neg(1, "crates/ignite-rustgen/tests/golden/syntax.ignite", 130, 26), 0, 5, 6]).to_string()))?;
        (show(&(format!("{}{}{}", sign(&0), sign(&3), sign(&(::ignite_io::checked::neg(3, "crates/ignite-rustgen/tests/golden/syntax.ignite", 131, 35)))))))?;
        (show(&count_spaces(&String::from("a b c ").into_bytes()).to_string()))?;
        let n = 21;
        (show(&twice(&n).to_string()))?;
        (show(&shout({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("hey")); &*TEXT })))?;
        let h = Handler { name: String::from("h"), respond: (::std::sync::Arc::new(|__a0: String| shout(&__a0)) as ::std::sync::Arc<dyn Fn(String) -> String + Send + Sync>) };
        (show(&(h.respond)(h.name)))?;
        let r = Reply { lines: vec![String::from("l")], action: Reply::default_action(), code: Reply::default_code() };
        (show(&(format!("code {}", r.code))))?;
        match &r.action {
            Action::Continue => (show({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("continue")); &*TEXT }))?,
            Action::Close => (show({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("close")); &*TEXT }))?,
        };
        let flag = true;
        let space = SPACE();
        (show(&(format!("{} {} {}", flag, space, String::from("t")))))?;
        let mut pair = (0, String::from(""));
        let mut i = 0;
        while i < 3000 {
            let (count, text) = pair.clone();
            pair = (::ignite_io::checked::add(count, 1, "crates/ignite-rustgen/tests/golden/syntax.ignite", 151, 15), format!("{}x", text));
            i = ::ignite_io::checked::add(i, 1, "crates/ignite-rustgen/tests/golden/syntax.ignite", 152, 11);
        }
        let (count, text) = pair.clone();
        let size = (text.len() as i64);
        (show(&(format!("{} {}", count, size))))?;
        let spare = (1, String::from("a"));
        let (mut p, mut q) = spare.clone();
        p = ::ignite_io::checked::add(p, 1, "crates/ignite-rustgen/tests/golden/syntax.ignite", 159, 9);
        q.push_str("b");
        (show(&(format!("{}{}", p, q))))?;
        let _ = twice(&1);
        let _ = twice(&2);
        let light: Light = Light::Off;
        match &light {
            Light::Off => (show({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("light off")); &*TEXT }))?,
            Light::On => (show({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("light on")); &*TEXT }))?,
        };
        (show(&(format!("{} {}", lower_case(&3), lower_case(&4)))))?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Runner {
    pub address: String,
    pub port: i64,
}

#[allow(non_snake_case)]
pub fn Runner__main(self_: &Runner) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let stream = match ::ignite_io::TcpStream::connect(self_.address.clone(), self_.port.clone()) {
        Ok(stream) => stream,
        Err(error) => return Err(error.clone().with_exit_code((3) as i32)),
    };
    match Syntax::connect(stream.clone()) {
        Ok(_) => Ok(()),
        Err(error) => Err(error.clone().with_exit_code((4) as i32)),
    }
}
