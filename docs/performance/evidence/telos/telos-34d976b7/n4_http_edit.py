#!/usr/bin/env python3
import sys

path = sys.argv[1]
src = open(path, encoding="utf-8").read()


def sub(old, new, count=1):
    global src
    n = src.count(old)
    if n != count:
        raise SystemExit(f"expected {count} of {old!r}, found {n}")
    src = src.replace(old, new)


sub(
    "import std.text.{bytes_find_from, split, split_once, contains, to_lower, is_digit}\n",
    "import std.text.*\nimport std.option.unwrap_or\n",
)
sub(
    """// A JSON response: `json(201, value)`.
fn json(status: Int, value: Json) -> Response =""",
    """// A JSON response: `Response.json(201, value)`.
fn Response.json(status: Int, value: Json) -> Response =""",
)
sub(
    """// A plain-text response.
fn plain(status: Int, body: Text) -> Response =""",
    """// A plain-text response.
fn Response.plain(status: Int, body: Text) -> Response =""",
)
sub(
    "if status == 204 { Response { status } } else { plain(status, reason(status) + \"\\n\") }",
    "if status == 204 { Response { status } } else { Response.plain(status, reason(status) + \"\\n\") }",
)
sub(
    """  for pair in response.headers {
    let (name, value) = pair
    if has_line_break(name) || has_line_break(value) {""",
    """  for header in response.headers {
    if has_line_break(header.0) || has_line_break(header.1) {""",
)
sub(
    """  for header in response.headers {
    let (name, value) = header
    head = head + "{name}: {value}\\r\\n"
  }""",
    """  for header in response.headers {
    head = head + "{header.0}: {header.1}\\r\\n"
  }""",
)
sub("  assert json(201, Null) ==", "  assert Response.json(201, Null) ==")
sub("  assert plain(200, \"hi\") ==", "  assert Response.plain(200, \"hi\") ==")
sub("  let fine = plain(200,", "  let fine = Response.plain(200,")
sub(
    """  let created = render(json(201, Object([])), true).to_text()
  assert created == "HTTP/1.1 201 Created\\r\\nContent-Type: application/json\\r\\nContent-Length: 2\\r\\nConnection: keep-alive\\r\\n\\r\\n{{}}\"""",
    """  let created = render(Response.json(201, Object([])), true).to_text()
  assert created == "HTTP/1.1 201 Created\\r\\nContent-Type: application/json\\r\\nContent-Length: 2\\r\\nConnection: keep-alive\\r\\n\\r\\n" + r"{}\"""",
)
sub(
    """  for pair in request.headers {
    let (key, value) = pair
    if key == name {
      return Some(value)
    }
  }""",
    """  for header in request.headers {
    if header.0 == name {
      return Some(header.1)
    }
  }""",
)
sub("  var out: List<Text> = List.empty()\n", "  var out = []\n")
sub(
    """  for pair in request.headers {
    let (name, value) = pair
    if name == "connection" {
      for item in value.split(",") {""",
    """  for header in request.headers {
    if header.0 == "connection" {
      for item in header.1.split(",") {""",
)
sub(
    """  b'|' | b'~' => true
  _ => is_digit(b) || b >= b'a' && b <= b'z' || b >= b'A' && b <= b'Z'
}""",
    """  b'|' | b'~' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => true
  _ => false
}""",
)
sub(
    """  let (path, query) = match parts.at(1).split_once("?") {
    Some(pair) => pair
    None => (parts.at(1), "")
  }""",
    """  let (path, query) = parts.at(1).split_once("?").unwrap_or((parts.at(1), ""))""",
)
sub("  var headers: List<(Text, Text)> = List.empty()\n", "  var headers = []\n")
sub(
    """  match header(request, "transfer-encoding") {
    Some(_) => return Ok(Reject(501))
    None => {}
  }
  var lengths: List<Text> = List.empty()
  for pair in request.headers {
    let (name, value) = pair
    if name == "content-length" {
      lengths.push(value)
    }
  }""",
    """  if header(request, "transfer-encoding") != None {
    return Ok(Reject(501))
  }
  var lengths = []
  for header in request.headers {
    if header.0 == "content-length" {
      lengths.push(header.1)
    }
  }""",
)
sub(
    """  plain(200, "{request.method} {request.path}?{request.query} {request.body}")""",
    """  Response.plain(200, "{request.method} {request.path}?{request.query} {request.body}")""",
)
open(path, "w", encoding="utf-8").write(src)
