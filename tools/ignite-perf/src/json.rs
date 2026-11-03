//! JSON values with the text format of Python's `json` module.
//!
//! The evidence under `docs/performance/` was written by `json.dumps`: `", "` and `": "`
//! separators (`","` with an indent), ASCII-only strings, Python float `repr`, and sorted keys
//! where the harness asked for them. Writing the same bytes keeps old and new records
//! comparable line by line. Objects keep insertion order so unsorted output (the progress
//! lines, the readiness report) matches too; equality follows Python's `dict` and number rules.

use std::fmt::Write as _;

#[derive(Clone, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

static NULL: Json = Json::Null;

/// An object from `(key, value)` pairs in the given order.
pub fn obj<K: Into<String>>(entries: Vec<(K, Json)>) -> Json {
    let mut value = Json::Obj(Vec::new());
    for (key, item) in entries {
        value.set(key, item);
    }
    value
}

impl Json {
    /// `json.loads`: also accepts `NaN`, `Infinity` and `-Infinity`; duplicate keys keep the
    /// first position and the last value.
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        parser.space();
        let value = parser.value()?;
        parser.space();
        if parser.at != parser.bytes.len() {
            return Err(format!("Extra data at byte {}", parser.at));
        }
        Ok(value)
    }

    /// `json.dumps(value, sort_keys=sort_keys)`.
    pub fn dumps(&self, sort_keys: bool) -> String {
        let mut out = String::new();
        self.write(&mut out, sort_keys, None, 0);
        out
    }

    /// `json.dumps(value, indent=indent, sort_keys=sort_keys)`.
    pub fn dumps_indent(&self, sort_keys: bool, indent: usize) -> String {
        let mut out = String::new();
        self.write(&mut out, sort_keys, Some(indent), 0);
        out
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value at `key`, or `null` when this is not an object or lacks the key.
    pub fn field(&self, key: &str) -> &Json {
        self.get(key).unwrap_or(&NULL)
    }

    /// The element at `index`, or `null`.
    pub fn item(&self, index: usize) -> &Json {
        match self {
            Json::Arr(items) => items.get(index).unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    /// Python `d[key] = value`: replaces in place or appends. No effect on a non-object.
    pub fn set<K: Into<String>>(&mut self, key: K, value: Json) {
        if let Json::Obj(entries) = self {
            let key = key.into();
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key, value)),
            }
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Int(n) => Some(*n as f64),
            Json::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> &[Json] {
        match self {
            Json::Arr(items) => items,
            _ => &[],
        }
    }

    pub fn as_obj(&self) -> &[(String, Json)] {
        match self {
            Json::Obj(entries) => entries,
            _ => &[],
        }
    }

    pub fn is_obj(&self) -> bool {
        matches!(self, Json::Obj(_))
    }

    /// Python truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool(b) => *b,
            Json::Int(n) => *n != 0,
            Json::Float(f) => *f != 0.0,
            Json::Str(s) => !s.is_empty(),
            Json::Arr(items) => !items.is_empty(),
            Json::Obj(entries) => !entries.is_empty(),
        }
    }

    fn write(&self, out: &mut String, sort_keys: bool, indent: Option<usize>, depth: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(n) => {
                let _ = write!(out, "{n}");
            }
            Json::Float(f) => out.push_str(&py_float(*f)),
            Json::Str(s) => write_str(out, s),
            Json::Arr(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (n, item) in items.iter().enumerate() {
                    separator(out, n, indent, depth + 1);
                    item.write(out, sort_keys, indent, depth + 1);
                }
                close(out, indent, depth);
                out.push(']');
            }
            Json::Obj(entries) => {
                if entries.is_empty() {
                    out.push_str("{}");
                    return;
                }
                let mut ordered: Vec<&(String, Json)> = entries.iter().collect();
                if sort_keys {
                    ordered.sort_by(|a, b| a.0.cmp(&b.0));
                }
                out.push('{');
                for (n, (key, item)) in ordered.into_iter().enumerate() {
                    separator(out, n, indent, depth + 1);
                    write_str(out, key);
                    out.push_str(": ");
                    item.write(out, sort_keys, indent, depth + 1);
                }
                close(out, indent, depth);
                out.push('}');
            }
        }
    }
}

fn separator(out: &mut String, n: usize, indent: Option<usize>, depth: usize) {
    match indent {
        None => {
            if n > 0 {
                out.push_str(", ");
            }
        }
        Some(width) => {
            if n > 0 {
                out.push(',');
            }
            out.push('\n');
            out.extend(std::iter::repeat_n(' ', width * depth));
        }
    }
}

fn close(out: &mut String, indent: Option<usize>, depth: usize) {
    if let Some(width) = indent {
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', width * depth));
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// Python `repr(float)`: the shortest round-trip digits, positional when the decimal exponent
/// is in -4..16 (with `.0` when integral), otherwise `d.ddde±XX`.
pub fn py_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sci = format!("{f:e}");
    let (sign, rest) = match sci.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", sci.as_str()),
    };
    let (mantissa, exponent) = rest.split_once('e').expect("{:e} has an exponent");
    let exponent: i32 = exponent.parse().expect("{:e} exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let point = exponent + 1;
    let mut out = String::from(sign);
    if -4 < point && point <= 16 {
        if point <= 0 {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-point) as usize));
            out.push_str(&digits);
        } else if point as usize >= digits.len() {
            out.push_str(&digits);
            out.extend(std::iter::repeat_n('0', point as usize - digits.len()));
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let e = point - 1;
        let _ = write!(out, "e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    out
}

impl PartialEq for Json {
    fn eq(&self, other: &Json) -> bool {
        match (self, other) {
            (Json::Null, Json::Null) => true,
            (Json::Bool(a), Json::Bool(b)) => a == b,
            (Json::Str(a), Json::Str(b)) => a == b,
            (Json::Arr(a), Json::Arr(b)) => a == b,
            (Json::Obj(a), Json::Obj(b)) => {
                a.len() == b.len() && a.iter().all(|(k, v)| other.get(k) == Some(v))
            }
            (Json::Int(a), Json::Int(b)) => a == b,
            (a, b) => match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            },
        }
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Json {
        Json::Str(s.into())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Json {
        Json::Str(s)
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Json {
        Json::Bool(b)
    }
}

impl From<i64> for Json {
    fn from(n: i64) -> Json {
        Json::Int(n)
    }
}

impl From<usize> for Json {
    fn from(n: usize) -> Json {
        Json::Int(n as i64)
    }
}

impl From<f64> for Json {
    fn from(f: f64) -> Json {
        Json::Float(f)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(items: Vec<T>) -> Json {
        Json::Arr(items.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(value: Option<T>) -> Json {
        value.map_or(Json::Null, Into::into)
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn error<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.at))
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            self.error("Expecting value")
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'N') => self.literal("NaN", Json::Float(f64::NAN)),
            Some(b'I') => self.literal("Infinity", Json::Float(f64::INFINITY)),
            Some(b'-') if self.bytes[self.at..].starts_with(b"-Infinity") => {
                self.literal("-Infinity", Json::Float(f64::NEG_INFINITY))
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.error("Expecting value"),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.at += 1;
        let mut value = Json::Obj(Vec::new());
        self.space();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(value);
        }
        loop {
            self.space();
            if self.bytes.get(self.at) != Some(&b'"') {
                return self.error("Expecting property name enclosed in double quotes");
            }
            let key = self.string()?;
            self.space();
            if self.bytes.get(self.at) != Some(&b':') {
                return self.error("Expecting ':' delimiter");
            }
            self.at += 1;
            self.space();
            let item = self.value()?;
            value.set(key, item);
            self.space();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(value);
                }
                _ => return self.error("Expecting ',' delimiter"),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.space();
            items.push(self.value()?);
            self.space();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return self.error("Expecting ',' delimiter"),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok());
        match digits {
            Some(n) => {
                self.at += 4;
                Ok(n)
            }
            None => self.error("Invalid \\uXXXX escape"),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while !matches!(self.bytes.get(self.at), None | Some(b'"' | b'\\')) {
                if self.bytes[self.at] < 0x20 {
                    return self.error("Invalid control character");
                }
                self.at += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.at])
                    .map_err(|_| format!("Invalid UTF-8 at byte {start}"))?,
            );
            match self.bytes.get(self.at) {
                None => return self.error("Unterminated string"),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                _ => {}
            }
            self.at += 1;
            let escape = self.bytes.get(self.at).copied();
            self.at += 1;
            match escape {
                Some(b'"') => out.push('"'),
                Some(b'\\') => out.push('\\'),
                Some(b'/') => out.push('/'),
                Some(b'b') => out.push('\u{8}'),
                Some(b'f') => out.push('\u{c}'),
                Some(b'n') => out.push('\n'),
                Some(b'r') => out.push('\r'),
                Some(b't') => out.push('\t'),
                Some(b'u') => {
                    let mut code = self.hex4()?;
                    if (0xd800..0xdc00).contains(&code) && self.bytes[self.at..].starts_with(b"\\u")
                    {
                        let back = self.at;
                        self.at += 2;
                        let low = self.hex4()?;
                        if (0xdc00..0xe000).contains(&low) {
                            code = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                        } else {
                            self.at = back;
                        }
                    }
                    out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                }
                _ => return self.error("Invalid \\escape"),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        let mut float = false;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        while let Some(b) = self.bytes.get(self.at) {
            match b {
                b'0'..=b'9' => {}
                b'.' | b'e' | b'E' | b'+' => float = true,
                b'-' => float = true,
                _ => break,
            }
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).expect("ASCII number");
        if !float && let Ok(n) = text.parse::<i64>() {
            return Ok(Json::Int(n));
        }
        match text.parse::<f64>() {
            Ok(f) => Ok(Json::Float(f)),
            Err(_) => {
                self.at = start;
                self.error("Invalid number")
            }
        }
    }
}
