//! The part of JSON (RFC 8259) the todo service reads and writes: a strict parser and the
//! encoder of `std.json` (`quote`: `"`, `\`, the C0 controls escaped, everything else as is).

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Number,
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    /// Member `name` of an object, the first when repeated.
    pub fn get(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }
}

pub fn parse(text: &[u8]) -> Option<Value> {
    let mut parser = Parser { s: text, at: 0 };
    parser.space();
    let value = parser.value(0)?;
    parser.space();
    (parser.at == text.len()).then_some(value)
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &[u8], value: Value) -> Option<Value> {
        if self.s[self.at..].starts_with(word) {
            self.at += word.len();
            Some(value)
        } else {
            None
        }
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > 128 {
            return None;
        }
        match self.peek()? {
            b'n' => self.literal(b"null", Value::Null),
            b't' => self.literal(b"true", Value::Bool(true)),
            b'f' => self.literal(b"false", Value::Bool(false)),
            b'"' => self.string().map(Value::Str),
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.peek() == Some(b']') {
                    self.at += 1;
                    return Some(Value::Array(items));
                }
                loop {
                    self.space();
                    items.push(self.value(depth + 1)?);
                    self.space();
                    match self.peek()? {
                        b',' => self.at += 1,
                        b']' => {
                            self.at += 1;
                            return Some(Value::Array(items));
                        }
                        _ => return None,
                    }
                }
            }
            b'{' => {
                self.at += 1;
                let mut members = Vec::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Some(Value::Object(members));
                }
                loop {
                    self.space();
                    if self.peek()? != b'"' {
                        return None;
                    }
                    let key = self.string()?;
                    self.space();
                    if self.peek()? != b':' {
                        return None;
                    }
                    self.at += 1;
                    self.space();
                    members.push((key, self.value(depth + 1)?));
                    self.space();
                    match self.peek()? {
                        b',' => self.at += 1,
                        b'}' => {
                            self.at += 1;
                            return Some(Value::Object(members));
                        }
                        _ => return None,
                    }
                }
            }
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek()? {
            b'0' => self.at += 1,
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return None,
        }
        let mut integral = true;
        if self.peek() == Some(b'.') {
            integral = false;
            self.at += 1;
            let digits = self.at;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
            if self.at == digits {
                return None;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            integral = false;
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            let digits = self.at;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
            if self.at == digits {
                return None;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.at]).ok()?;
        if integral
            && text != "-0"
            && let Ok(n) = text.parse()
        {
            return Some(Value::Int(n));
        }
        Some(Value::Number)
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.s.get(self.at..self.at + 4)?;
        let text = std::str::from_utf8(digits).ok()?;
        let value = u32::from_str_radix(text, 16).ok()?;
        self.at += 4;
        Some(value)
    }

    fn string(&mut self) -> Option<String> {
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let b = self.peek()?;
            self.at += 1;
            match b {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = self.peek()?;
                    self.at += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xd800..0xdc00).contains(&code) {
                                if self.s.get(self.at..self.at + 2)? != b"\\u" {
                                    return None;
                                }
                                self.at += 2;
                                let low = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&low) {
                                    return None;
                                }
                                code = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                            }
                            let c = char::from_u32(code)?;
                            let mut buffer = [0; 4];
                            out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                        }
                        _ => return None,
                    }
                }
                0..=0x1f => return None,
                _ => out.push(b),
            }
        }
    }
}

/// `text` as a JSON string, as `std.json.quote` writes it.
pub fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
