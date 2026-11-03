//! The hand-written XML reader of `ignite rfc import` (`docs/slice.md` 19.8): the subset
//! of XML an RFCXML v3 file uses, read into a tree.
//!
//! It reads elements, attributes in single or double quotes and character data; it skips
//! the XML declaration, processing instructions, comments, CDATA sections and a DOCTYPE
//! without an internal subset; it decodes the five predefined entities and numeric
//! character references. Any other entity, a DOCTYPE with an internal subset, a mismatched
//! or unclosed element and a file over [`LIMIT`] bytes are errors. There is no validation
//! and no namespace processing: prefixed names are kept as written.

/// The largest file the reader accepts: 16 MiB.
pub const LIMIT: usize = 16 << 20;

/// An element: its name as written, its attributes in document order, its children, and
/// the byte offset of its `<` in the file.
#[derive(Debug)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub offset: usize,
}

#[derive(Debug)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    /// The value of the attribute `name`, the first one if it is written twice.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The child elements, in document order.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    /// The first child element named `name`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|element| element.name == name)
    }
}

/// A reader error: the byte offset it was found at and its message.
#[derive(Debug)]
pub struct Error {
    pub offset: usize,
    pub message: String,
}

fn error<T>(offset: usize, message: impl Into<String>) -> Result<T, Error> {
    Err(Error {
        offset,
        message: message.into(),
    })
}

/// 1-based line and column of a byte offset of `text`; the column counts characters.
pub fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |at| at + 1);
    (line, before[start..].chars().count() + 1)
}

/// Reads `bytes` into the root element.
pub fn parse(bytes: &[u8]) -> Result<Element, Error> {
    if bytes.len() > LIMIT {
        return error(0, "the file is larger than 16 MiB");
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(utf8) => return error(utf8.valid_up_to(), "the file is not UTF-8"),
    };
    Reader { text, at: 0 }.document()
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}

impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn starts(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn skip_space(&mut self) {
        let trimmed = self.rest().trim_start_matches(is_space);
        self.at = self.text.len() - trimmed.len();
    }

    /// Skips past the next `end`, which must follow.
    fn skip_past(&mut self, end: &str, what: &str) -> Result<(), Error> {
        let start = self.at;
        match self.rest().find(end) {
            Some(found) => {
                self.at += found + end.len();
                Ok(())
            }
            None => error(start, format!("{what} is not closed")),
        }
    }

    fn document(mut self) -> Result<Element, Error> {
        let mut stack: Vec<Element> = Vec::new();
        let mut root: Option<Element> = None;
        while self.at < self.text.len() {
            let start = self.at;
            if self.starts("<?") {
                self.skip_past("?>", "a processing instruction")?;
            } else if self.starts("<!--") {
                self.skip_past("-->", "a comment")?;
            } else if self.starts("<![CDATA[") {
                self.skip_past("]]>", "a CDATA section")?;
            } else if self.starts("<!DOCTYPE") {
                self.doctype()?;
            } else if self.starts("</") {
                self.at += 2;
                let name = self.name()?;
                self.skip_space();
                if !self.starts(">") {
                    return error(self.at, format!("expected `>` to close `</{name}`"));
                }
                self.at += 1;
                let Some(open) = stack.pop() else {
                    return error(start, format!("`</{name}>` closes no open element"));
                };
                if open.name != name {
                    return error(
                        open.offset,
                        format!("element `{}` is not closed before `</{name}>`", open.name),
                    );
                }
                self.attach(open, &mut stack, &mut root)?;
            } else if self.starts("<") {
                let (element, empty) = self.start_tag()?;
                if empty {
                    self.attach(element, &mut stack, &mut root)?;
                } else {
                    stack.push(element);
                }
            } else {
                let end = self
                    .rest()
                    .find('<')
                    .map_or(self.text.len(), |n| self.at + n);
                let data = decode(self.text, self.at, end)?;
                self.at = end;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Text(data)),
                    None if data.chars().all(is_space) => {}
                    None => return error(start, "character data outside the root element"),
                }
            }
        }
        if let Some(open) = stack.pop() {
            return error(
                open.offset,
                format!("element `{}` is not closed", open.name),
            );
        }
        match root {
            Some(root) => Ok(root),
            None => error(self.text.len(), "the file has no root element"),
        }
    }

    fn attach(
        &self,
        element: Element,
        stack: &mut [Element],
        root: &mut Option<Element>,
    ) -> Result<(), Error> {
        match stack.last_mut() {
            Some(parent) => parent.children.push(Node::Element(element)),
            None if root.is_none() => *root = Some(element),
            None => return error(element.offset, "a second root element"),
        }
        Ok(())
    }

    /// `<!DOCTYPE name SYSTEM "…">`; an internal subset `[…]` is an error.
    fn doctype(&mut self) -> Result<(), Error> {
        let start = self.at;
        self.at += "<!DOCTYPE".len();
        while let Some(c) = self.peek() {
            match c {
                '>' => {
                    self.at += 1;
                    return Ok(());
                }
                '[' => {
                    return error(
                        self.at,
                        "a DOCTYPE with an internal subset is not supported",
                    );
                }
                '"' | '\'' => {
                    self.at += 1;
                    self.skip_past(&c.to_string(), "a quoted DOCTYPE literal")?;
                }
                _ => self.at += c.len_utf8(),
            }
        }
        error(start, "the DOCTYPE is not closed")
    }

    fn name(&mut self) -> Result<String, Error> {
        let rest = self.rest();
        let len = rest
            .find(|c: char| is_space(c) || matches!(c, '/' | '>' | '=' | '<'))
            .unwrap_or(rest.len());
        if len == 0 {
            return error(self.at, "expected a name");
        }
        let name = rest[..len].to_string();
        self.at += len;
        Ok(name)
    }

    /// A start tag and whether it is empty (`/>`).
    fn start_tag(&mut self) -> Result<(Element, bool), Error> {
        let offset = self.at;
        self.at += 1;
        let name = self.name()?;
        let mut attributes = Vec::new();
        loop {
            self.skip_space();
            if self.starts("/>") {
                self.at += 2;
                return Ok((element(name, attributes, offset), true));
            }
            if self.starts(">") {
                self.at += 1;
                return Ok((element(name, attributes, offset), false));
            }
            if self.at >= self.text.len() {
                return error(offset, format!("the start tag of `{name}` is not closed"));
            }
            let key = self.name()?;
            self.skip_space();
            if !self.starts("=") {
                return error(self.at, format!("expected `=` after the attribute `{key}`"));
            }
            self.at += 1;
            self.skip_space();
            let quote = match self.peek() {
                Some(quote @ ('"' | '\'')) => quote,
                _ => return error(self.at, format!("expected a quoted value of `{key}`")),
            };
            self.at += 1;
            let Some(len) = self.rest().find(quote) else {
                return error(self.at - 1, format!("the value of `{key}` is not closed"));
            };
            let end = self.at + len;
            if let Some(lt) = self.text[self.at..end].find('<') {
                return error(self.at + lt, format!("`<` in the value of `{key}`"));
            }
            let value = decode(self.text, self.at, end)?;
            self.at = end + 1;
            attributes.push((key, value));
        }
    }
}

fn element(name: String, attributes: Vec<(String, String)>, offset: usize) -> Element {
    Element {
        name,
        attributes,
        children: Vec::new(),
        offset,
    }
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// The text of `text[start..end]` with its entity and character references decoded.
fn decode(text: &str, start: usize, end: usize) -> Result<String, Error> {
    let raw = &text[start..end];
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let offset = end - rest.len() + amp;
        let after = &rest[amp + 1..];
        let Some(semi) = after.find(';') else {
            return error(offset, "an entity reference without `;`");
        };
        let name = &after[..semi];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => match name.strip_prefix('#') {
                Some(number) => {
                    let code = match number.strip_prefix('x') {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => number.parse::<u32>().ok(),
                    };
                    match code.and_then(char::from_u32) {
                        Some(c) => Some(c),
                        None => {
                            return error(
                                offset,
                                format!("`&{name};` is not a valid character reference"),
                            );
                        }
                    }
                }
                None => None,
            },
        };
        match decoded {
            Some(c) => out.push(c),
            None => return error(offset, format!("unknown entity `&{name};`")),
        }
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
