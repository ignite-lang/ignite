//! story:rustgen-generated-name-collisions: no Rust name the generator
//! invents (a local starting with `__`, the `Endpoint` variable of a
//! listener, a record's `default_<field>`, the program crate's name) can
//! collide with a name the Ignite program declares. Each case is its own
//! program, built with cargo; where a collision would compile and bind the
//! generator's value instead of the program's, the program's tests run and
//! assert the program's value.

use crate::common;
use crate::golden::{
    adversary_package_named, adversary_sources, adversary_substrate, adversary_tests_pass,
    cargo_build,
};

/// User names that start with `__`, spelled like the generator's own
/// locals and items: the subject `__m` of a match arm that is not a Rust
/// pattern, the payload `__x`, the fn value parameter `__a0`, the index and
/// argument temporaries `__pN` of a mutating call on an indexed receiver,
/// the arguments `__spawn_N` of `spawn`, the receiver and argument
/// temporaries `__ignite_r` and `__ignite_aN` of a call with named
/// arguments, the operands `__ignite_left` and `__ignite_right` of an
/// `assert`, the test fns `__ignite_test_N` and a fn `__ignite_x`. Every
/// one of them is the program's value in the asserts: a generator local
/// that captured it would fail them, a generator item it duplicated would
/// not build.
const UNDERSCORES: &str = "package examples.unders\n\
\n\
import std.net.{Pipe, PipeStream}\n\
import std.io.IoError\n\
\n\
type IoResult<T> = Result<T, IoError>\n\
\n\
enum Shape { Named(Text, Int), Plain }\n\
\n\
fn tag(__m: Text, s: Text) -> Text = match s {\n\
  \"a\" => __m\n\
  _ => \"none\"\n\
}\n\
\n\
fn payload(__x: Int, s: Shape) -> Int = match s {\n\
  Named(\"a\", n) => n * 10 + __x\n\
  _ => 0\n\
}\n\
\n\
fn __a0(n: Int) -> Int = n + 1\n\
\n\
fn apply(f: fn(Int) -> Int, n: Int) -> Int = f(n)\n\
\n\
fn pushed(__p0: Int, __p1: Int, __p2: Int, __p3: Int, __p4: Int, __p5: Int) -> List<List<Int>> {\n\
  var lists = [[0], [0]]\n\
  lists[1].push(__p0 + __p1 * 10 + __p2 * 100 + __p3 * 1000 + __p4 * 10000 + __p5 * 100000)\n\
  lists\n\
}\n\
\n\
fn sub3(a: Int, b: Int, c: Int) -> Int = a * 100 + b * 10 + c\n\
\n\
fn __ignite_x() -> Int = 9\n\
\n\
fn __ignite_test_0() -> Int = 4\n\
\n\
test fn emit(a: Int, b: Int, out: PipeStream) -> IoResult<Unit> {\n\
  out.write_all(\"{a},{b}\".to_bytes())?\n\
  _ = out.close()\n\
  Ok(())\n\
}\n\
\n\
test fn read_all(end: PipeStream) -> IoResult<Text> {\n\
  var got = Bytes.empty()\n\
  loop {\n\
    match end.read(64)? {\n\
      Eof => return Ok(got.to_text())\n\
      Data(bytes) => got.append(bytes)\n\
    }\n\
  }\n\
}\n\
\n\
test \"a match subject, a payload and a fn value named like the generator's locals\" {\n\
  assert tag(\"mine\", \"a\") == \"mine\"\n\
  assert payload(3, Shape.Named(\"a\", 5)) == 53\n\
  assert apply(__a0, 1) == 2\n\
  assert __ignite_x() + __ignite_test_0() == 13\n\
}\n\
\n\
test \"temporaries of a mutating call named like the program's names\" {\n\
  assert pushed(2, 3, 4, 5, 6, 7) == [[0], [0, 765432]]\n\
}\n\
\n\
test \"temporaries of named arguments named like the program's names\" {\n\
  let __ignite_r = 5\n\
  let __ignite_a0 = 7\n\
  assert 1.sub3(c = 3, b = __ignite_r) == 153\n\
  assert sub3(c = 1, b = 2, a = __ignite_a0) == 721\n\
}\n\
\n\
test \"the operands of an assert named like the program's names\" {\n\
  let __ignite_left = 3\n\
  let __ignite_right = 4\n\
  assert 4 == __ignite_left + 1\n\
  assert __ignite_right == __ignite_left + 1\n\
}\n\
\n\
test \"the arguments of spawn named like the program's names\" {\n\
  let (writer, reader) = Pipe.pair()\n\
  reader.set_idle_timeout(5000)?\n\
  let __spawn_0 = 2\n\
  spawn emit(1, __spawn_0, writer)\n\
  assert read_all(reader)? == \"1,2\"\n\
}\n";

#[test]
fn user_names_starting_with_two_underscores_keep_their_values() {
    adversary_tests_pass(
        "names-underscores",
        UNDERSCORES,
        &common::repository_std(),
        5,
    );
}

/// A listener `x` named as an `Endpoint` target binds `x_endpoint` in the
/// generated `main`; a listener resource named `x_endpoint` that the same
/// component binds must stay a `TcpListener`.
#[test]
fn a_resource_named_like_another_resources_endpoint_builds() {
    let source = "package examples.endpoints\n\
\n\
import std.net.Endpoint\n\
\n\
type Both {\n\
  target: Endpoint\n\
  listener: TcpListener\n\
  executable: Text\n\
}\n\
\n\
fn Both.main(self: Both) -> Result<Unit, IoError> {\n\
  log(\"{self.target.port} {self.listener.local_port()}\")\n\
  Ok(Unit)\n\
}\n\
\n\
system Endpoints {\n\
  resource x: TcpListener { address = \"127.0.0.1\", port = 7003 }\n\
  resource x_endpoint: TcpListener { address = \"127.0.0.1\", port = 0 }\n\
  component both: Both { target = x, listener = x_endpoint, executable = \"both\" }\n\
}\n";
    let dir = adversary_sources("names-endpoint-src", &[("endpoints.ignite", source)]);
    let (program, std) = common::lower_typed(
        &dir.join("endpoints.ignite"),
        &common::golden_std(),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "names-endpoint");
}

/// A record with a defaulted field `f` and an associated fn
/// `default_f`: the default and the fn keep their own values.
#[test]
fn an_associated_fn_named_like_a_fields_default_keeps_its_value() {
    let source = "package examples.defaults\n\
\n\
type R { f: Int = 1, g: Int }\n\
\n\
fn R.default_f() -> Int = 2\n\
\n\
test \"a default beside an associated fn default_f\" {\n\
  assert R { g = 0 }.f == 1\n\
  assert R.default_f() == 2\n\
}\n";
    adversary_tests_pass("names-default", source, &common::golden_std(), 1);
}

/// story:rustgen-assoc-symbol-collisions: a free fn `A__b` beside the
/// associated fn `A.b`. Both were the Rust fn `A__b`, so the crate did not
/// build; each keeps its own value.
#[test]
fn a_free_fn_named_like_an_associated_fn_keeps_its_value() {
    let source = "package examples.assocfree\n\
\n\
type A { x: Int }\n\
\n\
fn A__b() -> Int = 1\n\
\n\
fn A.b() -> Int = 2\n\
\n\
test \"a free fn A__b beside fn A.b\" {\n\
  assert A__b() == 1\n\
  assert A.b() == 2\n\
}\n";
    adversary_tests_pass("names-assoc-free", source, &common::golden_std(), 1);
}

/// `fn A_._b` beside `fn A.__b`: both were the Rust fn `A___b`.
#[test]
fn associated_fns_whose_underscores_meet_keep_their_values() {
    let source = "package examples.assocunder\n\
\n\
type A_ { x: Int }\n\
\n\
type A { x: Int }\n\
\n\
fn A_._b() -> Int = 1\n\
\n\
fn A.__b() -> Int = 2\n\
\n\
test \"fn A_._b beside fn A.__b\" {\n\
  assert A_._b() == 1\n\
  assert A.__b() == 2\n\
}\n";
    adversary_tests_pass("names-assoc-under", source, &common::golden_std(), 1);
}

/// `fn A__b.c` beside `fn A.b__c`: both were the Rust fn `A__b__c`.
#[test]
fn associated_fns_whose_separators_meet_keep_their_values() {
    let source = "package examples.assocsep\n\
\n\
type A__b { x: Int }\n\
\n\
type A { x: Int }\n\
\n\
fn A__b.c() -> Int = 1\n\
\n\
fn A.b__c() -> Int = 2\n\
\n\
test \"fn A__b.c beside fn A.b__c\" {\n\
  assert A__b.c() == 1\n\
  assert A.b__c() == 2\n\
}\n";
    adversary_tests_pass("names-assoc-sep", source, &common::golden_std(), 1);
}

/// A package whose crate name would be an extern crate every generated
/// crate names: `std`, `core`, `ignite_io`, `ignite_std`.
fn package_builds(package: &str) {
    let file = adversary_package_named(package, "other");
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &adversary_substrate());
    cargo_build(&program, &std, &format!("names-package-{package}"));
}

#[test]
fn a_package_named_std_builds() {
    package_builds("std");
}

#[test]
fn a_package_named_core_builds() {
    package_builds("core");
}

#[test]
fn a_package_named_ignite_io_builds() {
    package_builds("ignite_io");
}

#[test]
fn a_package_named_ignite_std_builds() {
    package_builds("ignite_std");
}
