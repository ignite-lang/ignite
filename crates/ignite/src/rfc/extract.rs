//! Extraction of requirements from an RFCXML tree (`docs/slice.md` 19.8, "Extraction"):
//! text blocks, their rendered text, sentences, BCP 14 marks, levels and parties.

use std::collections::{BTreeMap, BTreeSet};

use super::xml::{Element, Node};

/// One requirement: a sentence with at least one `<bcp14>` mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    /// The `pn` of the innermost `<section>`, without `section-`.
    pub section: String,
    /// 1-based within `section`, in document order, over the whole file.
    pub ordinal: usize,
    /// The variant of `std.rfc.Level`.
    pub level: &'static str,
    /// The variant of `std.rfc.Party`.
    pub party: &'static str,
    pub text: String,
}

impl Requirement {
    /// `R` + the section with `.` as `_` and letters upper-cased + `_` + the ordinal.
    pub fn id(&self) -> String {
        format!(
            "R{}_{}",
            self.section.replace('.', "_").to_uppercase(),
            self.ordinal
        )
    }
}

/// A `<bcp14>` mark whose content is no BCP 14 keyword: the mark's element offset and
/// its content.
#[derive(Debug)]
pub struct UnknownKeyword {
    pub offset: usize,
    pub keyword: String,
}

/// Every requirement of the document under `root`, in document order.
pub fn requirements(root: &Element) -> Result<Vec<Requirement>, UnknownKeyword> {
    let mut anchors = BTreeSet::new();
    reference_anchors(root, &mut anchors);
    let mut walk = Walk {
        anchors,
        counts: BTreeMap::new(),
        found: Vec::new(),
    };
    walk.children(root, None)?;
    Ok(walk.found)
}

/// The rendered text of an element's content, whitespace collapsed and trimmed.
pub fn plain_text(element: &Element) -> String {
    let mut text = Text::default();
    let anchors = BTreeSet::new();
    let mut marks = Vec::new();
    // An element outside the text blocks holds no marks that count.
    let _ = render_children(element, &anchors, &mut text, &mut marks);
    text.text
}

fn reference_anchors(element: &Element, anchors: &mut BTreeSet<String>) {
    if element.name == "reference"
        && let Some(anchor) = element.attribute("anchor")
    {
        anchors.insert(anchor.to_string());
    }
    for child in element.elements() {
        reference_anchors(child, anchors);
    }
}

struct Walk {
    anchors: BTreeSet<String>,
    counts: BTreeMap<String, usize>,
    found: Vec<Requirement>,
}

impl Walk {
    fn children(&mut self, element: &Element, section: Option<&str>) -> Result<(), UnknownKeyword> {
        for child in element.elements() {
            self.element(child, section)?;
        }
        Ok(())
    }

    fn element(&mut self, element: &Element, section: Option<&str>) -> Result<(), UnknownKeyword> {
        match element.name.as_str() {
            "front" | "back" => Ok(()),
            "section" => {
                let inner = element
                    .attribute("pn")
                    .and_then(|pn| pn.strip_prefix("section-"));
                self.children(element, inner)
            }
            _ if is_block(element) => match section {
                Some(section) => self.block(element, section),
                None => Ok(()),
            },
            _ => self.children(element, section),
        }
    }

    fn block(&mut self, element: &Element, section: &str) -> Result<(), UnknownKeyword> {
        let mut text = Text::default();
        let mut marks = Vec::new();
        render_children(element, &self.anchors, &mut text, &mut marks)?;
        for (start, end) in sentences(&text.text) {
            let mut inside = marks.iter().filter(|mark| (start..end).contains(&mark.at));
            let Some(first) = inside.next() else {
                continue;
            };
            let sentence = &text.text[start..end];
            let ordinal = self.counts.entry(section.to_string()).or_default();
            *ordinal += 1;
            self.found.push(Requirement {
                section: section.to_string(),
                ordinal: *ordinal,
                level: first.level,
                party: party(&sentence[..first.at - start]),
                text: sentence.to_string(),
            });
        }
        Ok(())
    }
}

/// A `<t>`, or an `<li>`, `<dd>`, `<td>` or `<th>` with no `<t>` child.
fn is_block(element: &Element) -> bool {
    match element.name.as_str() {
        "t" => true,
        "li" | "dd" | "td" | "th" => element.child("t").is_none(),
        _ => false,
    }
}

/// Rendered text with runs of whitespace as one space, trimmed: a space is written only
/// before the next non-space character.
#[derive(Default)]
struct Text {
    text: String,
    space: bool,
}

impl Text {
    fn push(&mut self, s: &str) {
        for c in s.chars() {
            if c.is_whitespace() {
                self.space = true;
            } else {
                if self.space && !self.text.is_empty() {
                    self.text.push(' ');
                }
                self.space = false;
                self.text.push(c);
            }
        }
    }

    /// The offset the next non-space character will have.
    fn next_offset(&self) -> usize {
        self.text.len() + usize::from(self.space && !self.text.is_empty())
    }
}

/// A `<bcp14>` mark: the offset of its keyword in the block's text, and its level.
struct Mark {
    at: usize,
    level: &'static str,
}

fn render_children(
    element: &Element,
    anchors: &BTreeSet<String>,
    out: &mut Text,
    marks: &mut Vec<Mark>,
) -> Result<(), UnknownKeyword> {
    for node in &element.children {
        match node {
            Node::Text(data) => out.push(data),
            Node::Element(child) => render(child, anchors, out, marks)?,
        }
    }
    Ok(())
}

fn render(
    element: &Element,
    anchors: &BTreeSet<String>,
    out: &mut Text,
    marks: &mut Vec<Mark>,
) -> Result<(), UnknownKeyword> {
    let content = |marks: &mut Vec<Mark>| -> Result<String, UnknownKeyword> {
        let mut inner = Text::default();
        render_children(element, anchors, &mut inner, marks)?;
        Ok(inner.text)
    };
    match element.name.as_str() {
        "iref" => Ok(()),
        "bcp14" => {
            let keyword = content(&mut Vec::new())?;
            let Some(level) = level(&keyword) else {
                return Err(UnknownKeyword {
                    offset: element.offset,
                    keyword,
                });
            };
            marks.push(Mark {
                at: out.next_offset(),
                level,
            });
            out.push(&keyword);
            Ok(())
        }
        "xref" | "eref" => {
            let mut inner_marks = Vec::new();
            let inner = content(&mut inner_marks)?;
            if !inner.is_empty() {
                let base = out.next_offset();
                marks.extend(inner_marks.into_iter().map(|mark| Mark {
                    at: base + mark.at,
                    level: mark.level,
                }));
                out.push(&inner);
            } else if element.name == "eref" {
                out.push(element.attribute("target").unwrap_or(""));
            } else {
                out.push(&empty_xref(element, anchors));
            }
            Ok(())
        }
        _ => render_children(element, anchors, out, marks),
    }
}

/// The rendering of an empty `<xref>`.
fn empty_xref(element: &Element, anchors: &BTreeSet<String>) -> String {
    let target = element.attribute("target").unwrap_or("");
    let derived = element.attribute("derivedContent").unwrap_or(target);
    match element.attribute("section") {
        Some(section) if element.attribute("sectionFormat") == Some("comma") => {
            format!("[{derived}], Section {section}")
        }
        Some(section) => format!("Section {section} of [{derived}]"),
        None if anchors.contains(target) => format!("[{derived}]"),
        None => derived.to_string(),
    }
}

/// The level of a BCP 14 keyword, whitespace already collapsed.
fn level(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "MUST" => "Must",
        "MUST NOT" => "MustNot",
        "REQUIRED" => "Required",
        "SHALL" => "Shall",
        "SHALL NOT" => "ShallNot",
        "SHOULD" => "Should",
        "SHOULD NOT" => "ShouldNot",
        "RECOMMENDED" => "Recommended",
        "NOT RECOMMENDED" => "NotRecommended",
        "MAY" => "May",
        "OPTIONAL" => "Optional",
        _ => return None,
    })
}

/// The byte ranges of the sentences of `text`. A sentence ends at `.`, `?` or `!`,
/// optionally followed by `)` or `"`, then a space and an upper-case ASCII letter, `"`
/// or `(`.
fn sentences(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        if matches!(bytes[at], b'.' | b'?' | b'!') {
            let mut end = at + 1;
            if matches!(bytes.get(end), Some(b')' | b'"')) {
                end += 1;
            }
            if bytes.get(end) == Some(&b' ')
                && bytes
                    .get(end + 1)
                    .is_some_and(|&next| next.is_ascii_uppercase() || matches!(next, b'"' | b'('))
            {
                ranges.push((start, end));
                start = end + 1;
                at = start;
                continue;
            }
        }
        at += 1;
    }
    if start < bytes.len() {
        ranges.push((start, bytes.len()));
    }
    ranges
}

/// Party words, matched case-insensitively as whole words.
const PARTIES: &[(&str, &str)] = &[
    ("server", "Server"),
    ("servers", "Server"),
    ("origin server", "Server"),
    ("client", "Client"),
    ("clients", "Client"),
    ("user agent", "Client"),
    ("user agents", "Client"),
    ("sender", "Sender"),
    ("senders", "Sender"),
    ("recipient", "Recipient"),
    ("recipients", "Recipient"),
    ("receiver", "Recipient"),
    ("proxy", "Proxy"),
    ("proxies", "Proxy"),
    ("intermediary", "Proxy"),
    ("intermediaries", "Proxy"),
    ("gateway", "Proxy"),
    ("cache", "Proxy"),
    ("caches", "Proxy"),
];

/// The party of the last party word in `before` (the sentence up to its first mark), or
/// `Any`.
fn party(before: &str) -> &'static str {
    let lower = before.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let word_char = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_alphanumeric);
    let mut last: Option<(usize, &'static str)> = None;
    for &(word, party) in PARTIES {
        for (start, _) in lower.match_indices(word) {
            let end = start + word.len();
            let whole = (start == 0 || !word_char(start - 1)) && !word_char(end);
            if whole && last.is_none_or(|(best, _)| end > best) {
                last = Some((end, party));
            }
        }
    }
    last.map_or("Any", |(_, party)| party)
}
