# Ignite slices 1 to 16: language subset, standard library, substrate and lowering

Status: implementation contract for the compiler slice, 2026-11-01. It narrows
`design.md` to what `examples/terminal/terminal.ignite` and `std/` need. Where the two
differ, this file wins for the slice and the design keeps the long-term intent.

Direction (Timo, 2026-11-01): `crates/` holds a compiler written in Rust (lexer, parser, AST,
checker, Rust code generator) and the IO substrate. The first target emits Rust; LLVM is a
later target. Programs are linked against a standard library written in Ignite (`std/`).
`protocol` is a primitive of the language; protocols (Telnet now, HTTP/1 and HTTP/2 later) are
declared in Ignite in the standard library. The only hand-written Rust a program runs is the
substrate `crates/ignite-io`: TCP streams, stdin/stdout byte streams, the runtime and process
arguments. Nothing named telnet lives under `crates/`.

Slice 0 (server only, Telnet in the example, hand-written reference client and server) is
superseded.

## 1 Pipeline

```
std/**/*.ignite  --parse, check, type--> modules --rustgen--> ignite-std crate --+
terminal.ignite  --parse, check, type (with std exports)--> --rustgen--> program crate --cargo build--> terminald, tn
                                                                  ignite-io (substrate) --+
```

| stage | crate | output |
|---|---|---|
| lex, parse | `crates/ignite-syntax` | `ast::Program`, diagnostics with spans |
| check | `crates/ignite-check` | name resolution against imported modules, effect rules, protocol roles |
| type | `crates/ignite-types` | a type for every expression (`docs/llvm.md` section 2); E040 to E048 fail `check` and both build targets |
| generate | `crates/ignite-rustgen` | the `ignite-std` crate and the program crate |
| substrate | `crates/ignite-io` | `TcpListener`, `TcpStream`, `Stdin`, `Stdout`, `Input`, `IoError`, `run`, `spawn`, `log`, `args` |
| driver | `crates/ignite` | `ignite check`, `ignite emit`, `ignite build` |

The frontend checks a program completely before either target generates code. `ignite-check`
resolves names and enforces the effect rules (section 5, including the `requires` sets,
E016), protocol rules, record literals (E047) and type expressions (E048). `ignite-types`
then gives every expression a type, checks every `match` for exhaustiveness (E046) and
resolves every call (`docs/llvm.md` section 2). A program that passes both is valid for the
Rust and the LLVM target. Diagnostics from rustc on generated code mean a generator bug; they
are shown as-is, prefixed with a note that they refer to generated code.

### 1.1 `ignite check --json` and `--stdin-for`

```text
ignite check [--std DIR] [--json] FILE
ignite check [--std DIR] [--json] --stdin-for FILE
```

- `--json` prints one JSON document on stdout, compact, followed by a newline, and nothing on
  stderr for diagnostics:
  `{"diagnostics":[{"code","end":{"column","line","offset"},"message","path","start":{...}}]}`.
  `path` is the file the diagnostic is in (the program, a sibling module or a standard-library
  file, as the text form names it); `code` and `message` are the text form's; `line` and
  `column` are 1-based, `column` in characters, as in section 8; `offset` is in bytes from the
  file's start, and `start` to `end` slices the offending text. The order is the text form's.
  A clean program prints `{"diagnostics":[]}`.
- `--stdin-for FILE` checks the text on stdin as the program at `FILE`: diagnostics name
  `FILE`, its sibling modules (section 14.4) and the standard library are read from disk, and
  `FILE` is neither read nor written. It takes the place of the positional `FILE`.
- Exit statuses are those of the text form: 0 clean, 1 diagnostics, 3 a file (or stdin) could
  not be read; a read error is reported on stderr as text also under `--json`.
- Diagnostics stop at the first failing stage and module, as in the text form; the text form
  is unchanged.
- Library API (add-only): `ignite::check_text(file, text, std)`,
  `ignite::diagnostics_json(source, diagnostics)`. Tests: `check_json::` in
  `crates/ignite/tests/check_json.rs`.

## 2 Lexical structure

- Source is UTF-8. Whitespace is insignificant except that a newline ends a statement or a
  field. The lexer emits one `Newline` token for any run of line breaks.
- Comments: `//` to end of line. No block comments.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Type, enum, record, protocol and variant names start
  with an uppercase letter by convention; the parser does not enforce case.
- Integer literals: decimal with optional `_` separators, or `0x` hexadecimal. No suffixes. A
  literal above `9223372036854775807` is E001, except `9223372036854775808` directly after a
  prefix `-`: `-9223372036854775808` is the smallest `Int`.
- String literals: `"..."` with escapes `\n \r \t \v \f \0 \\ \" \xNN` (`\v` is vertical tab
  `0B`, `\f` form feed `0C`). No multi-line strings.
  String escape `\xNN` covers `00` to `7F` only; strings are UTF-8 text, not bytes.
  An unknown escape is E001; the message shows it with one backslash. A `\` at the end of a
  line leaves the string unterminated (E001).
- Interpolation: inside a string literal, `{e}` is an expression. The text up to the matching
  `}` is lexed as tokens and parsed as one expression; it may not contain a string literal or
  a line break, and `{}` is empty (E001 for each). `{{` and `}}` are literal braces; a lone
  `}` is E001. The desugaring is in section 10.1; the parts may have type `Text`, `Int`,
  `Byte`, `Bool` or `Bytes` (each has `to_text`). A byte literal `b'c'` may appear inside `{ }`,
  braces and quotes in it included. A `{` whose `}` does not follow on the same line is E001
  "unclosed `{` in interpolation; write `{{` for a literal brace".
- Byte literals: `b'c'` is an integer literal whose value is the byte of one ASCII character
  or escape (`\n \r \t \v \f \0 \\ \' \" \xNN`, `NN` up to `FF`). `b"..."` is a byte string of ASCII
  characters and the same escapes, meaning `Bytes.of([..])`. Anything else is E001. `\'`
  is an escape in byte literals and byte strings, not in text strings.
- A lexer error ends the token stream at its position. The parser still runs on the tokens
  before it, and its diagnostics there are reported with the lexer's, in source order.
- Keywords: `package import type enum fn const system resource component protocol let var
  return match if else while loop for in break continue await spawn requires true false test
  assert`. `await` is an error (section 10.2); `test` and `assert` are section 12.
- Punctuation: `( ) { } [ ] < > , . : ; = => -> ? ! + - * / % == != <= >= && || |`.
- Quantity literals (`4096 bytes`, `200 ms`) and percentages are not in the slice.

## 3 Grammar

`NL` is the newline token. `sep` is `,` or `NL` in fields, variants, record literals, match
arms and properties, and `;` or `NL` between statements; it may repeat. Newlines are skipped
inside `( )` and `[ ]`, after a binary operator, after `=`, `=>`, `->`, `,`, `{`, and before
`}`. A postfix `(`, `[`, `.`, `?` must start on the same line as its operand. `else` must
follow the closing `}` of its `if` on the same line. Both hold inside `( )` and `[ ]` too,
where other newlines are skipped: `[a` NL `(b)]` is E001, not a call of `a`. So a long
`requires [a, b, c]` breaks after `[` or any `,`, with `]` on a line of its own if wanted;
`requires` and `[` share a line.

```
program    := NL* decl (NL+ decl)* NL*
decl       := package | import | record | alias | enum | fn | const | system | protocol
            | test | 'test' (fn | record | enum | const)          test-only item (section 12)
test       := 'test' STRING block                                 a test (section 12)
package    := 'package' path
import     := 'import' path ('.' '{' ident (',' ident)* '}')?
path       := name ('.' name)*            name: an identifier or a keyword (`std.protocol.telnet`)
record     := 'type' ident tparams? '{' (field sep)* field? '}'
tparams    := '<' ident (',' ident)* '>'                                  type parameters (slice 7)
field      := ident ':' type ('=' expr)?                                   default value
alias      := 'type' ident ('<' ident (',' ident)* '>')? '=' type
enum       := 'enum' ident tparams? '{' (variant sep)* variant? '}'
variant    := ident ('(' type (',' type)* ')')?
fn         := 'fn' ident tparams? '(' (param (',' param)*)? ')' ('->' type)? requires? (block | '=' expr)
param      := ident ':' type
const      := 'const' ident ':' type '=' expr
requires   := 'requires' '[' (effect (',' effect)*)? ']'
effect     := path ('.' '*')?                                         `x.*`: every effect on `x`
type       := ident ('<' type (',' type)* '>')?
            | 'fn' '(' (fparam (',' fparam)*)? ')' ('->' type)? requires?  function type
            | '(' ')'                                                     Unit
            | '(' type ')'                                                grouping
            | '(' type ',' type (',' type)* ')'                           tuple
fparam     := (ident ':')? type                     all parameters named or none (E001)
protocol   := 'protocol' ident '{' NL* (fn NL*)* '}'
block      := '{' (stmt sep)* stmt? '}'
stmt       := ('let' | 'var') ident (':' type)? '=' expr
            | ('let' | 'var') pattern '=' expr          a binding, `_` or a tuple of those
            | '_' '=' expr
            | place '=' expr
            | 'return' expr?
            | 'while' expr block
            | 'loop' block
            | 'for' ident 'in' expr block
            | 'break' | 'continue'
            | 'assert' expr (',' expr)?                  condition, message (section 12)
            | expr
place      := ident ('.' ident)*
expr       := or
or         := and ('||' and)*
and        := equality ('&&' equality)*
equality   := compare (('==' | '!=') compare)?
compare    := bitor (('<' | '>' | '<=' | '>=') bitor)?
bitor      := bitxor ('|' bitxor)*                  section 29
bitxor     := bitand ('^' bitand)*
bitand     := shift ('&' shift)*
shift      := additive (('<<' | '>>') additive)*    `<<`, `>>`: two touching `<` or `>`
additive   := multiplicative (('+' | '-') multiplicative)*
multiplicative := unary (('*' | '/' | '%') unary)*
unary      := ('!' | '-' | '~') unary | 'await' unary | 'spawn' unary | postfix
postfix    := primary ( '(' args ')' | '.' ident ('(' args ')')? | '[' expr ']' | '?' )*
args       := (expr (',' expr)*)?
primary    := INT | STRING | BYTE | BYTES | 'true' | 'false'
            | ident
            | ident '{' (init sep)* init? '}'                            record literal
            | '[' (expr (',' expr)*)? ']'                                list literal
            | 'match' expr '{' (arm sep)* arm? '}'
            | 'if' expr block ('else' ('if' expr block ('else' block)? | block))?
            | 'return' expr?                      no value when NL ; , } ) ] follows
            | '(' ')'                                                    the Unit value
            | '(' expr ')'
            | '(' expr ',' expr (',' expr)* ')'                          tuple
init       := ident ('=' expr)?                                         `a` means `a = a`
arm        := pattern '=>' (block | expr)
pattern    := alt ('|' alt)*                                            or-pattern
alt        := '_' | INT | BYTE | STRING | 'true' | 'false'
            | '(' ')' | '(' pattern ')' | '(' pattern ',' pattern (',' pattern)* ')'
            | '[' (pattern (',' pattern)*)? ']'                          list of that length
            | path ('(' pattern (',' pattern)* ')')?
system     := 'system' ident '{' (resource | component)* '}'
resource   := 'resource' ident ':' ident ('<' type (',' type)* '>')?
              ('{' (ident '=' expr sep)* (ident '=' expr)? '}')?
component  := 'component' ident ':' ident '{' (ident '=' expr sep)* (ident '=' expr)? '}'
```

Since slice 14 the type parameters of a fn, an associated fn and a role take a bound
(`<S: ByteStream>`); `fn_type_params` is in section 18.3.

`type N = ...` and `type N<P> = ...` are aliases; `type N { ... }` and `type N<P> { ... }`
are records. A `requires` after a fn type's return type belongs to the innermost fn type
(section 11.7). A one-element tuple, `(a,)` as a value, type or pattern, is E001. In a type, a
`>` that closes type arguments may touch `=`: `Option<Int>= None` and `type Box<T>= T` read
`>=` as `>` then `=`. A declaration keyword (`fn`, `type`, `enum`, `const`, ..) inside a
block is one E001 (``fn` declarations are only allowed at the top level; move it out of the
block``); the parser skips the declaration and goes on. The pattern of `let` and `var` must
be irrefutable: a binding, `_` or a tuple of those (E001 otherwise; use `match`).

Brackets `( ) [ ] { }`, type arguments, prefix operators and `return` with a value nest at
most 256 levels deep; the first level over the limit is E054 "nesting deeper than 256; split
the expression". Chains of binary operators and interpolations are not nesting and have no
such limit; the `ignite` CLI compiles on a thread with a 256 MiB stack, which holds chains of
10,000 terms.

AST: a function type `fn(A, B) -> R` is `Type { name: "fn", args: [A, B, R] }` (return type
last; `Unit` when `-> R` is omitted); `Type::is_fn`, `fn_params`, `fn_ret` read it. A tuple
type `(A, B)` is `Type { name: "Tuple", args: [A, B] }` (`Type::is_tuple`); `()` as a type is
`Unit`. An alias is `Decl::Alias(AliasDecl { name, params, ty, span })`; a field default is
`Field.default`. A protocol is `Decl::Protocol(ProtocolDecl { name, fns, span })`. `spawn e`
is `ExprKind::Spawn(e)`. A constant is `Decl::Const(ConstDecl { name, ty, value, span })`.
`return` in expression position is `ExprKind::Return`; at the start of a statement it is
`StmtKind::Return`. `(a, b)` is `ExprKind::Tuple`; `()` is `ExprKind::Tuple(vec![])`. A
pattern `Enum.Variant(p)` is `PatternKind::Variant { path: [Enum, Variant], args: [p] }`; a
single lowercase name without arguments is a `Binding` (in a `match` arm it is a constant
pattern when it names a constant in scope, section 4.3); `true`/`false`, `p | q`, `(p, q)`
and `[p, q]` are `Bool`, `Or`, `Tuple` and `List`. `let (a, b) = e`, `let _ = e` and `_ = e`
are `StmtKind::LetPattern`. Expression bodies, field punning, interpolation, byte literals
and `await` are desugared by the parser (section 10.1). A test is `Decl::Test(TestDecl)`, a
test-only item carries `test_only: true`, and `assert` is `StmtKind::Assert` (section 12.1).

A record literal `Ident { ... }` is only parsed when `Ident` starts with an uppercase letter,
so `if x { ... }` is not misread. As in Rust, a record literal is not parsed directly in the
condition of `if` and `while`, the scrutinee of `match` or the iterable of `for`; write it in
parentheses there. So `while i < LIMIT { ... }` reads `LIMIT` as a name and the block as the
loop body. A `match` arm or `if` with a block whose last statement is
an expression has that expression as its value. `match` and `if` are expressions.

Path expressions `Action.Continue` and `Command.Echo(text)` parse as field access and method
call on the identifier `Action`; the checker turns an uppercase-first identifier followed by
`.Variant` into a variant reference. `TcpListener.bind(...)` is a substrate static call;
`Telnet.serve(...)` is a protocol role call. Any other `x.f(..)` is a method call (section
4.8).

## 4 Declarations, modules and names

- `package` names the module. For a program, the last segment is the Cargo package name.
  For a standard-library file, `package` must equal its module path (`std/protocol/telnet.ignite`
  declares `package std.protocol.telnet`); a mismatch is E031.
- Record (`type`), alias, `enum`, `fn`, `const`, `protocol` and `system` names share one
  module-level namespace.
  Redefinition is E004; so is a declaration named like a builtin type or `Tuple`. Imported
  names share it too: defining a name that is also imported is E004.
- Tests take no part in the namespace; test-only items do, but only tests and test-only items
  of their module may use them, and they are not exported (E050, E051, section 12.2).
- Variants are referenced bare (`Help`) or qualified (`Command.Help`). A bare variant
  resolves against the expected type: the type its context requires (an annotation, a
  parameter, a return type, a record field, an element, the scrutinee of a pattern). The
  expected type's enum decides it whether or not that enum is imported: `action = Close` in a
  `Reply` literal needs no `import std.console.Action`. When the expected type does not decide
  it, only enums in scope take part: the one that declares the name owns it; when several do
  (own, imported or builtin), it is E005. A bare name that only an enum out of scope declares
  is then E003 (unknown name, or unknown variant in a pattern; also E003 for such a variant
  with a payload used without arguments). The type checker reports these E005 and E003;
  `ignite-check` passes a bare name that is a variant of an enum in any loaded module.
  A variant with a payload used bare is E003 (``variant `X` takes 1 payload value; call it``)
  however it resolves: by the expected type, by the one enum in scope that declares it, or
  when two enums in scope declare it and the expected type picks one.
- `Some`, `None`, `Ok`, `Err`, `Unit`, `Data`, `Eof` are builtin variants (section 7).
- Local scopes: a block introduces a scope; `let`, `var`, `for` and pattern bindings shadow.
  Assignment to a `let` binding is E006. Use of an unbound name is E003.

### 4.1 Imports and the standard library

- `import a.b.{X, Y}` imports `X` and `Y` from module `a.b`. `import a.b.X` imports `X` from
  module `a.b` (the last segment is the name). Importing a whole module is not in the slice.
- `std.net` and `std.io` are the substrate: their names are the builtins of section 7 and an
  import of them only checks that the name is a builtin (E031 otherwise). Builtins are usable
  without an import.
- Every other `std.*` module is a file under the standard-library root: module `std.a.b` is
  `<root>/a/b.ignite`. The root is `std/` in this repository (`ignite --std PATH` overrides it;
  the driver's default is `CARGO_MANIFEST_DIR/../../std`, canonicalized).
- An import of a module that does not exist, or of a name the module does not define, is
  E031. Imports among standard-library modules that form a cycle are E032.
- A module exports every record (with its field defaults), alias, enum (with its variants),
  fn, const and protocol it defines. Imports are not re-exported.
- An imported enum brings its variants into scope for bare use, like an own enum.
- Imports of non-`std` modules are E031 in this slice (a program is one file plus the
  standard library).

### 4.2 Protocols

`protocol Name { fn ... }` declares the roles of a protocol over one TCP stream:

| role | signature | used by |
|---|---|---|
| `serve` | `fn serve(stream: TcpStream, application: A) -> Result<Unit, IoError> requires [...]` | the accept loop of a server kind's `main` (`TelnetServer`, `HttpServer<S>`, section 14.5) |
| `connect` | `fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [...]` | a client kind's `main` (`TelnetClient`) |

- A protocol has at least one role. Each role declares `requires` and has the parameter count
  above; any other fn name inside a protocol, a role without `requires` or a wrong parameter
  count is E030. Helper functions are ordinary module-level fns next to the protocol.
- The application type `A` is chosen by the protocol (`Console` for Telnet); the slice does
  not check it.
- A role is called as `Name.serve(stream, app)` / `Name.connect(stream)`; both calls are
  effectful.
- Message grammars, negotiation state and roles beyond these two are a later slice.
- Since slice 14 a role's stream may be a `ByteStream`-bounded type parameter and `serve` may
  take a limits record as a third parameter (section 18.6).

### 4.3 Constants

- `const NAME: Type = expr` defines a module-level constant. The value is pure: an effectful
  call or `spawn` in it is E010. It may use other constants and pure fns. Constants are
  exported and imported like fns. Names are UPPER_SNAKE by convention.
- A constant is a value, not a place: it cannot be assigned (E006).
- A payload-less name in a `match` arm pattern that names a constant in scope is a constant
  pattern, whatever its case (`LIMIT` and `limit` alike): it matches a value equal to the
  constant and binds nothing, so it does not cover the scrutinee for E046. In a `let` pattern
  a name always binds. The constant's type must be the scrutinee's (E040) and one
  with `==`: `Int`, `Byte`, `Bool` or `Text` (E045). A constant with payload patterns,
  `LIMIT(x)`, is E003.
- Constants whose values refer to each other in a cycle are E015.
- A constant may not share its name with a variant in scope (E004). Calling a constant,
  `N(x)`, is E003.

### 4.4 Errors

- `IoError.new(message: Text) -> IoError` is a pure builtin static that makes an error value,
  so Ignite code can fail with `return Err(IoError.new("..."))`. `error.message` is its text.

### 4.5 Function values

- A fn type `fn(A, B) -> R` is the type of a pure function. A pure fn's name in value
  position is a function value. An effectful function, declared or inferred (section 5), is
  a value only where an effectful fn type `fn(n: A) -> R requires [..]` that covers its
  effects is expected; elsewhere E017, reported by the type checker (section 11.7).
- A local, parameter or pattern binding can be called like a function: `respond(line)`. Such
  a call is pure. `ignite-check` does not check its arity; the type checker (`docs/llvm.md`
  section 2) reports E043.
- A field of fn type is called directly: `console.respond(line)` (section 4.8). Binding it
  first, `let respond = console.respond`, also works.

### 4.6 Type aliases

- `type Name = T` and `type Name<P, Q> = T` name another type. Inside `T`, `P` and `Q` are the
  parameters; `Name<A, B>` is `T` with `A` and `B` substituted. An alias is the type it names:
  no value, pattern or diagnostic distinguishes them.
- Aliases are exported and imported like records. A wrong number of type arguments is E048;
  so is an alias that refers to itself, directly or through other aliases.

### 4.7 Field defaults

- `field: T = e` gives a record field a default. `e` is typed against `T` in the module that
  declares the record, like a constant value: no locals, pure (E010), may use the module's
  constants and pure fns.
- A record literal may omit a field with a default; it gets the default's value. Omitting a
  field without one is E047 (``missing field `y` in `Point` literal; it has no default``).
- `R { a, b = e }` is `R { a = a, b = e }` (field punning).

### 4.8 Method calls

`x.f(args)`, where `x` is not the head of a path, is resolved by the receiver's type, in
this order:

1. a builtin method of the receiver type (sections 6 and 7, and `to_text` on `Int`, `Byte`,
   `Bool`, `Text`, `Bytes`);
2. a field `f` of fn type of the receiver's record: its value is called with `args`;
3. a module-level fn `f` in scope (own or imported, not a local) whose first parameter
   accepts the receiver: it is called with `(x, args..)` (uniform function call syntax).

A builtin method name (`len`, `to_text`, `set`, ...) on a receiver type that has that builtin
is always resolved by 1; on a receiver type without it, it is resolved by 3 when a module-level
fn of that name is in scope (section 13.1 row 5) and is E041 otherwise. A user fn may share the
name of a builtin method of its first parameter's type: no declaration is refused for it,
method syntax on that type calls the builtin (`pairs.set(0, p)` and `l[i] = v` on a `List`),
and a plain call `set(pairs, k, v)` calls the user fn. A field of fn type and a fn with the same name both
matching is E041 (ambiguous). A fn whose first parameter does not accept the receiver, or no
candidate at all, is E041. `ignite-check` checks the arity of a call by 3 (E003, receiver
counted) and its effects; the type checker decides between 2 and 3.

## 5 Effects

- A function with a `requires` clause **declares** its effects. A function without one has
  them **inferred**: it is effectful when its body performs an effectful operation, and its
  effect set is the union of what those operations need (section 5.1). Inference is a fixed
  point over the module's functions, so mutual recursion is covered. It is computed per
  strongly connected component of the call graph, callees first, each component repeated
  until its sets stop growing; so the cost does not depend on declaration order. A function
  that performs no effectful operation is **pure**.
- Effectful operations: an effectful substrate method or static (section 5.1 table), a call
  to an effectful function (also by UFCS), a protocol role call, and `spawn`.
- An effectful call needs no marker. `await e` is E001 (section 10.2).
- A pure context, a constant's value or a field default, may not perform an effectful
  operation: E010, ``effectful call `fetch` outside an effectful function: the value of
  constant `FETCHED` is pure`` (for a default: ``the default of field `Config.port` is
  pure``); `spawn` there is E010 too.
- `spawn e`: the operand must be a call to an effectful user function (also by UFCS) or a
  protocol role whose result is `Result<Unit, IoError>` (E013 when the operand is not such a
  call). `spawn` returns `Unit` at once; the task runs concurrently and ends with the process.
- An effectful function is a value only where an effectful fn type covering its effects is
  expected (E017, sections 4.5 and 11.7). Calling a value of effectful fn type, a
  `Store` method and `set_idle_timeout` are effectful operations (section 11.7).
- A test is an effect root: its body may perform any effectful operation,
  `spawn` included, has no `requires` and gets no E016 (section 12.3). A test-only fn follows
  the rules of every fn.
- Lowering (slice 4): every `fn` → `fn`. The substrate blocks and every task is an OS thread
  (section 7), so an effectful call needs no Rust counterpart.

### 5.1 Effect sets

An effect is written `<capability>.<operation>`. The capability is a parameter of the
function (`stream.read`) or an ambient one (`stdin.read`). Every operation an effectful
function performs needs its effect. A function with `requires` must list it; a missing one is
E016, reported at the call, e.g.
``missing effect `stream.write` for `write_text`; add it to the `requires` of `session` ``.
A function without `requires` gets it added to its inferred set.

| operation | effect it needs |
|---|---|
| `x.read(n)` | `r.read` for each root `r` of `x` |
| `x.write_all(b)` | `r.write` for each root `r` of `x` |
| `x.close()` | `r.write` for each root `r` of `x` (closing shuts down the write half) |
| `x.accept()` | `r.accept` for each root `r` of `x` |
| `Stdin.read(n)` | `stdin.read` |
| `Stdin.is_terminal()`, `Stdin.set_raw(on)` | `stdin.mode` |
| `Stdout.write_all(b)` | `stdout.write` |
| `Process.exit(code)` | `process.exit` |
| `TcpListener.bind(a, p)`, `TcpListener.bind_kept(a, p)` | `net.listen` |
| `TcpStream.connect(a, p)` | `net.connect` |
| `f(a, ..)`, `a.f(..)`, `P.role(a, ..)`, called or spawned | each entry `q.op` of the callee's declared or inferred set whose `q` is a parameter of the callee becomes `r.op` for each root `r` of the argument passed for `q` (the receiver of `a.f(..)` is the first argument); every other entry (`stdin.read`) is needed unchanged |
| `TcpStream.connect_within(a, p, t)` (26.3) | `net.connect` |
| `x.timed_out()` (26.2) | `r.read` for each root `r` of `x` |

`x.local_port()` is pure and needs nothing. `x.set_idle_timeout(ms)` needs `r.write`; the
`Store` methods, calls of values of effectful fn type and `x.*` are in section 11.7.

**Roots.** The roots of an expression are the parameters of the enclosing function its value
may come from:

- a parameter `p` has the root `p`;
- a local (`let`, `var`, `for`, pattern binding) has the roots of the expression it was bound
  from; an assignment `x = e` (or `x.f = e`) adds the roots of `e` to those of `x`, whatever
  the control flow, so a `var` assigned from two parameters needs effects on both; `l.push(e)`,
  `l.set(i, e)`, `l.append(e)` and `m.put(k, e)` add the roots of their arguments to those of
  the local `l` or `m` (the rows of `ignite_check::IN_PLACE` that store an argument,
  `ignite_check::stores_argument`);
- `e.f`, `e[i]`, `e.copy()`, `e.at(i)`, `e.slice(a, b)` and `e?` have the roots of `e`; so
  do the container reads that return an element on a receiver `e` of declared type `Map`:
  `e.get(k)`, `e.remove(k)` and `e.keys()` (since slice 17: a `requires []` fn could otherwise
  use a `Setting` or `Launcher` held in a `Map`);
  `a.concat(b)` has those of `a` and `b`;
- a tuple, list literal or record literal has the roots of its elements or field values;
- `if`, `match` and a block have the roots of every value they may produce: the last
  expression of each branch, arm or block, with that branch's locals in scope;
- the result of any call has the roots of each argument whose type can hold a substrate
  handle (`TcpStream`, `TcpListener`, `Store`, `Stdin`, `Stdout`, and since slice 17
  `Directory`, `Launcher`, `ChildProcess`, `Entropy`, `Setting` (21.3); directly or in a field,
  payload, element or type argument; a type parameter or an unknown type may). This covers a
  user fn called by name or by UFCS (the receiver is an argument), pure or effectful,
  `requires []` included; a fn value or fn-typed field; a variant constructor (`Some(s)`); a
  protocol role; and every `Store` method, `get`, `remove`, `values`, `update` and `upsert`
  included (receiver and arguments). A callee cannot launder a parameter by returning it;
- every other expression has no roots: literals, operators, the other builtin methods and the
  results of substrate operations. Such a value is owned by the function: a stream returned by
  `accept` or `TcpStream.connect` needs no effect for its own operations or for the calls it
  is passed to. Obtaining it needed `r.accept` or `net.connect`.

Further rules:

- Declaring an effect that is not needed is allowed.
- System mains are generated and not checked. They and the component kinds' `K.main` fns are
  the roots of a program's effect graph; `K.main` has no `requires` (its set is inferred), a
  protocol role declares the full set its body and its callees need, and the generated main
  grants everything.
- A callee's requirement is resolved by its parameter names, so the caller's own names do not
  matter: `write_text(out, ..)` in a function with parameter `out` needs `out.write`.
- E016 is checked only in a function with `requires`. A function with an inferred set never
  reports E016; its callers with `requires` do. Constants and field defaults are pure and
  never E016.
- A requirement with no `.` (`net`) is an ambient effect name like any other; no operation
  needs it.

## 6 Rust lowering

Every generated file starts with
`#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]`.

| Ignite | Rust |
|---|---|
| `Text` | `String` |
| `Bytes` | `Vec<u8>` |
| `Byte` | `u8` |
| `Int` | `i64` |
| `Bool` | `bool` |
| `Unit` (type) | `()` |
| `List<T>` | `Vec<T>`; an enum payload or record field: `::ignite_io::List<T>` (shared lists, below) |
| `Option<T>`, `Result<T, E>` | `::std::option::Option<T>`, `::std::result::Result<T, E>`; the variants `Some`, `None`, `Ok`, `Err` bare |
| `fn(A, B) -> R` | `::std::sync::Arc<dyn Fn(A, B) -> R + Send + Sync>` (13.3) |
| `TcpListener`, `TcpStream`, `Stdin`, `Stdout`, `Input`, `IoError` | `::ignite_io::` same name |
| record `type R { a: T }` | `#[derive(Clone, Debug)] pub struct R { pub a: T }` |
| `enum E { A, B(T) }` | `#[derive(Clone, Debug)] pub enum E { A, B(T) }` |
| `fn f(p: T) -> U` | `pub fn f(p: &T) -> U`, or `pub fn f(p: T) -> U` when `f` consumes `p` (parameters, below) |
| `fn f(...) requires [...]` | `pub fn f(...)` |
| `type R<A> {..}`, `enum E<A> {..}`, `fn f<A>(..)`, a role `fn serve<A>(..)` | `pub struct R<A: Clone + Send + Sync + 'static>` and so on: every type parameter carries these bounds (11.1) |
| `Store<T>` | `::ignite_io::Store<T>` |
| `fn(p: A) -> R requires [p.*]` | `::std::sync::Arc<dyn Fn(A) -> R + Send + Sync>` |
| `protocol P { fn serve ... fn connect ... }` | `pub struct P {}` (braced, so it names no value a binding could match) and `impl P { pub fn serve ... pub fn connect ... }` |
| `P.serve(a, b)` | `P::serve(a, b)` |
| `import std.a.b.{X, Y}` in a program | `use ::ignite_std::a::b::{X, Y};` |
| `import std.a.b.{X, Y}` in a std module | `use crate::a::b::{X, Y};` |
| `import std.net...`, `import std.io...` | nothing |
| `let x = e` / `var x = e` | `let x = e;` / `let mut x = e;` |
| `"text"` | `String::from("text")` |
| `[a, b]` | `vec![a, b]` |
| `R { a = e }` | `R { a: e }` |
| `Unit` (value) | `()` |
| `E.V`, `V`, `E.V(x)` | `E::V`, `E::V(x)` |
| `Some(x)`, `None`, `Ok(x)`, `Err(e)` | same |
| `Data(b)`, `Eof` | `::ignite_io::Input::Data(b)`, `::ignite_io::Input::Eof` |
| `x.f` | `x.f` |
| `e?` | `(e)?` |
| `await e` | `(e)` |
| `spawn f(a, b)` | `{ let __spawn_0 = a; let __spawn_1 = b; ::ignite_io::spawn(move || { if let Err(error) = f(__spawn_0, __spawn_1) { ::ignite_io::log(&format!("task ended with error: {}", error)); } }); }` |
| `match e { p => v }` | `match e { p => v, }` |
| `if c { a } else { b }` | same |
| `while`, `loop`, `for x in e`, `break`, `continue`, `return` | same |
| `a + b` etc. | same on numbers |
| `a + b + c` on `Text` | `format!("{}{}{}", a, b, c)` (one call per left-folded chain); `format!` borrows each part, a literal part is inlined into the template with its braces doubled, and a part `x.to_text()` of Text, Int, Byte or Bool (each `{x}` of an interpolation) passes `x` |
| `a == b` | same |
| `(a, b)`, type `(A, B)` | `(a, b)`, `(A, B)` |
| `()` | `()` |
| `let (a, b) = e` / `var (a, b) = e`, `let _ = e` | `let (a, b) = e;` / `let (mut a, mut b) = e;`, `let _ = e;` |
| `return e` as an expression | `return e` |
| `type A<T> = B` | `pub type A<T> = B;` |
| `type R { f: T = e }` | the struct, and `impl R { #[allow(non_snake_case)] pub fn default_f() -> T { e } }` |
| `R { a = x }` omitting defaulted `f` | `R { a: x, f: R::default_f() }` |
| `x.f(a)`, UFCS (`Callee::Fn`) | `f(x, a)` (the receiver is a value argument) |
| `x.respond(a)`, fn field (`Callee::FieldFn`) | `(x.respond)(a)` |
| `V` resolved by expected type | `E::V` from the callee the typer records |
| patterns `true`, `p \| q`, `(p, q)` | same |

Every generated path into the standard library, the substrate and the `ignite-std` crate
starts at the extern crate (`::std::`, `::ignite_io::`, `::ignite_std::`), and the builtin enums
are written `::std::option::Option` and `::std::result::Result`, so no program record, type
parameter or sibling module (`std`, `ignite_io`, `ignite_std`, `Option`, `Result`) can shadow them.
A program name that is a Rust primitive type (`i64`, `bool`, `u8`, `str`, …) gets a trailing `_`,
as a prelude name does.

The `spawn` arguments are evaluated (and cloned, ownership rule) before the task starts, so
the spawning function keeps its own copies; an argument the moves analysis marks (dead after the
`spawn`, 20.3.1) is moved into the task instead.

**Ownership rule.** Rust moves what Ignite copies. A *place* (an identifier, or a place
followed by `.field`) used in a value position is emitted with `.clone()`, except where
`TypedProgram::moves` marks it (20.3): that place is dead after its move position, and it is
emitted bare, so Rust moves it. Value positions:
call and method arguments, `let`/`var` initializers, assignment right-hand sides, `match`
scrutinees, `return` operands, list elements, record field values, `for` iterables, and
operands of `await`, `?`, `!` and `-`. Non-value positions: the receiver of a method call or
field access, the left-hand side of an assignment, and operands of binary arithmetic,
comparison and logic operators. Every substrate handle implements `Clone` so this rule has
no exceptions.

**Parameters.** A parameter of a fn or associated fn that its fn consumes
(`TypedProgram::consumed`, 20.3.3: changed in place on some path or moved on every path) is
declared by value, `p: T`, or `mut p: T` when the body assigns or changes it in place, and
has no entry copy. A protocol role's parameters are `mut name: T` by value, consumed or not
(a generated `main` calls a role with owned values). Every other parameter is a reference
`p: &T`, so a call copies nothing: an argument that is a place is passed as `&place`, a
parameter that is itself a reference as it is, any other argument as `&(value)`; `l.at(i)`
on a place `l` passes the element borrowed from the list, `::ignite_io::checked::at(&l, i,
<site>)`, unless another argument of the call changes `l`. Such a parameter of type `Int`,
`Byte`, `Bool`, `Unit` or a fn type (an `Arc`, copied by a reference count) is copied once on entry: `let mut p = p.clone();`. An argument to a
consumed parameter or to a role parameter is a value: a place that moves is emitted bare,
any other place with `.clone()`, so `s = step(s, i)` passes `s` without a copy and `let t =
step(s, 1); t.count + s.count` passes `s.clone()`. In the body a reference parameter is
dereferenced as an operand of an operator, a condition or an index (`(*p)`), and copied
where it is the result of a block, arm or branch. A fn of the module or an imported one used
as a value is a closure in an `Arc` cast to the fn value type (13.3); it takes its arguments
by value and passes each as its parameter is declared:
`(::std::sync::Arc::new(|__a0: i64| f(&__a0)) as ::std::sync::Arc<dyn Fn(i64) -> i64 + Send +
Sync>)`, and `step(__a0, &__a1)` for a `step` that consumes its first parameter. The calls
the generator writes itself follow the same rule: `spawn f(a)` calls `f(&__spawn_0)`, or
`f(__spawn_0)` when `f` consumes the parameter; a generated `main` calls `K.main(&value)`,
or `K.main(value)` when `K.main` consumes `self`; a `to_text` fn that shows a value in an
`assert` or `?` report gets `&value`, or `value.clone()` when it consumes it.

**Block values.** A block in *value position* emits its last statement, when it is an
expression, without a trailing `;` so it becomes the block's value. Value positions: a
`match` arm body, both branches of an `if` that is itself in value position, and the body of
a function whose declared return type is present and not `Unit`. Every other block (`while`,
`loop`, `for` bodies, an `if` without `else`, an `if` in statement position, a function body
with no return type or `Unit`) ends every statement with `;`. An `if` is in value position
when it is a `let`/`var` initializer, a `return` operand, an argument, an arm body, or the
last statement of a value-position block.

**`for` over a list.** An exception to the ownership rule, when the types are known: a `for`
whose iterable is a place of type `List<T>`, `T` being `Text`, a list, map, `Option`,
`Result`, tuple, record or enum, and whose body neither assigns nor changes in place the
element or the variable the place is rooted at, iterates `(place).iter()` and copies no list.
In the body the element is a reference, read like a reference parameter. Every other `for`
copies its iterable.

**`match` on a place.** The same exception for a scrutinee, when the types are known: a
`match` whose scrutinee is a place of one of those types, not `Copy` in Rust (an `Option`,
`Result` or tuple of `Int`, `Byte`, `Bool` or `Unit` is), and none of whose arms assigns or
changes in place the variable the place is rooted at, matches `&place` (a reference parameter
as it is) and copies no scrutinee. A binding of one of those types that is not `Copy` is a
reference in its arm, read like a reference parameter; any other binding is copied at the
start of its arm (`{ let n = (n).clone(); .. }`). An arm whose pattern becomes a guard binds
`__m` to the reference. Every other `match` copies its scrutinee. Since
`story:moves-for-pattern-bindings` a binding of a type parameter is a reference too (`&T`, read
as a generic fn's reference parameter is), so it is copied once, where a value is taken, and
not at the start of its arm and again there (20.3.1).

**Shared lists in payloads and fields.** Unit `rust-shared-payload-lists` (design D of
`docs/value-copies.md`). An enum payload or a record field declared `List<T>`, `T` not `Byte`
(so `Bytes` stays `Vec<u8>`), is `::ignite_io::List<T>` on the Rust target: `None` for the
empty list, else an `Arc<Vec<T>>`, with an atomic count (values cross `spawn` and channels), no
`DerefMut` and no `unsafe`. The declared type decides, before type arguments are substituted,
so every instance of a generic declaration has one layout. Construction wraps the `Vec`
(`Val::Rec(::ignite_io::List::from_vec(fields))`, also in record updates, defaults and
assignments to the field); a read derefs (`(*r.items)`; a payload binding is `let l = &**l;`
when the `match` lends its scrutinee, and `let l = &*l;`, read like a reference parameter,
when the scrutinee is a value no local holds and the arm neither changes nor moves `l`; in an
arm whose pattern becomes a guard, such as `Some(Tag("k", l))`, the same `l` is the `&Vec`
read out of `__m` instead of a copy of it); a change goes through `make_mut` (`(*r.items.make_mut()).push(x)`, also under indexes), except
`clear`, which is `::ignite_io::List::clear`: a shared list takes the empty storage without a
copy, an unshared one drops its elements and keeps its buffer; a field or payload that moves
into a `Vec` position, a payload binding of a value no local holds that its arm changes or
moves, and a field of a value no local holds (`f().items`), is
`::ignite_io::List::into_vec(..)`, which takes the `Vec` when nothing else shares it and
copies it otherwise. Substrate records with list fields (`Invocation`, `TlsOptions`) convert
at the call with `.into()`. The LLVM target and the evaluator are unchanged.
`CODEGEN_REVISION` is `ignite-codegen/0.1.0-53` since this unit.

What a copy costs now: copying a value that holds such a list is one increment per list,
whatever the list holds; the first change through one holder after a share copies the
elements once (each element that is itself a shared list by an increment); a change of a
list nothing else shares copies nothing; reading a payload of a value no local holds
(`match lookup(v, i) { Some(Seq(l)) => l.len() }`) copies nothing; clearing a shared list
copies nothing. On the workload of `docs/value-copies.md`, one request at s1 goes from 607,784
allocations and 250,639,812 instructions to 51,172 and 17,498,837 (release). Still copied
(allocations per operation at M = 10 / 200 elements):

- the elements of a shared payload or field taken out into a list of its own
  (`match v { Words(l) => l }` copies the texts once);
- a payload binding of a value no local holds that its arm changes, moves or stores
  (`match lookup(v, i) { Some(Seq(l)) => Shelf { id = 1, words = l } }`) copies the elements
  when the value still shares them;
- a field declared `Option<List<T>>` or a tuple holding a list: not shared, copied in full on
  every copy of the value (11.0 / 201.0, as before the unit);
- the inner lists of a `List<List<T>>` field: the outer list is shared, its elements are
  plain `Vec`s, so the first change copies every inner list in full (43 / 803; before the
  unit 42 / 802);
- lists in locals, parameters and results, `Text`, `Bytes`, `Map`, and every value on the
  LLVM target.

What dropping a deep value costs: the Rust target drops a record that holds a list of itself
recursively, and the shared list's `Arc` adds frames to each level. On the 8 MiB main stack a
chain of such records runs at 35,000 levels and overflows at 40,000 (before the unit it ran to
between 70,000 and 80,000); a chain of enum values each holding a list of the next runs at
35,000 and overflows at 50,000 (before the unit it overflowed at 20,000). A `Drop for List`
that drops the last holder's `Vec` in its own frame lowered both (records and enum chains
overflow at 35,000) and was withdrawn.

What a path that never shares pays: each payload or field list built is a second block,
one `malloc` and one `free` more than a `Vec`. `terminald`, whose `echo` reply builds one
`Reply.lines` per round, goes from 3,480 to 3,602 instructions per round (+3.5 %, callgrind,
200 rounds after 50); the workload's table build at s1 from 6,414,688 to 7,443,117
instructions (+16.0 %) and from 10,016 to 11,551 allocations. `todod`'s `GET /todos/1`, which
copies a header list, goes from 21,464 and 21,875 (two runs) to 19,219 instructions per
round.

**Builtin methods** are lowered by name; the receiver type is not known, so each name has one
meaning. The generator emits exactly the Rust on the right.

| Ignite | Rust | note |
|---|---|---|
| `t.len()` | `(t.len() as i64)` | Text, Bytes, List |
| `t.is_empty()` | `t.is_empty()` | |
| `t.trim()` | `t.trim().to_string()` | |
| `t.starts_with(p)` | `t.starts_with(&p)`; `t.starts_with("..")` for a literal (20.3.7) | |
| `t.strip_prefix(p)` | `t.strip_prefix(&p).map(\|s\| s.to_string())`; a literal as a `&str` (20.3.7) | |
| `t.concat(u)` | `format!("{}{}", t, u)` | |
| `t.to_bytes()` | `t.clone().into_bytes()`; `t.into_bytes()` when `t` is a fresh text nothing else owns (a literal, a `+` chain or interpolation, `to_text` of Int, Byte or Bool, `concat`, `trim`) | `t.as_bytes()` to a lent bytes parameter and in the other borrowed forms of 20.3.7 |
| `b.to_text()` | `String::from_utf8_lossy(&b).into_owned()` | |
| `b.slice(i, j).to_text()` | `String::from_utf8_lossy(::ignite_io::checked::slice_ref(&b, i, j, <site>)).into_owned()`: the slice is borrowed, not copied, with the failure of `checked::slice` | Bytes |
| `b.index_of(n, i) -> Option<Int>` | `::ignite_io::text::index_of(&b, &n, i)`: the offset of the first `n` at or after `i` (a negative `i` reads as 0; `None` when `i` is past the end, also for an empty `n`), by a scan for the first byte of `n` eight bytes at a time (a word XOR eight copies of the byte, then the zero-byte test) and a compare of the rest (story `unsafe-sites-back`; before, `memchr` or `memmem`: the word scan costs no more instructions per request of `todod`); `t.to_bytes()` and a checked `slice` are borrowed, not copied. The LLVM runtime's `bytes_index_of` calls the same function (-1 for `None`). Story `text-fast-search` | Bytes |
| `t.pieces(s) -> List<Text>` | `::ignite_io::text::pieces(&t, &s)`: the texts between the separators, empty pieces kept, `[t]` for an empty `s`; no piece is checked for UTF-8 again, since valid text cut at a valid separator stays valid. `std.text.split` is this builtin. The LLVM runtime's `text_pieces` uses the same search. Story `text-fast-search` | Text |
| `b.at(i)` | `b[(i) as usize]`, then `.clone()` when the element is not `Int`, `Byte` or `Bool` (also `l[i]`) | Bytes, List |
| `l.map(f)` | `l.iter().cloned().map(f).collect::<Vec<_>>()` | List; `l.iter().map(..)` without the copy in the cases of 20.3.7 |
| `b.to_int()` | `(b as i64)` | Byte |
| `i.to_byte()` | `u8::try_from(i).ok()` | Int |
| `Int.parse(t)` | `::ignite_io::parse_int(&t)`: decimal digits with an optional leading `-`, `None` for a leading `+` and for anything `str::parse::<i64>` rejects; the LLVM runtime's `int_parse` calls the same function | static |
| `Int.parse(b.to_text())`, `Int.parse(b.slice(i, j).to_text())` | `::ignite_io::parse_int(&b)`, `::ignite_io::parse_int(::ignite_io::checked::slice_ref(&b, i, j, <site>))`: no `Text` is built; bytes that are not UTF-8 give `None`, as their lossy text does. The LLVM target passes `b` (or the slice) to `int_parse` without `text_from_bytes_lossy` | static |
| `s.next_id()`, `s.get(id)`, `s.put(id, v)`, `s.remove(id)`, `s.values()`, `s.update(id, f)`, `s.upsert(id, f)` | the same `::ignite_io::Store` method | Store |
| `s.set_idle_timeout(ms)` | `s.set_idle_timeout(ms)` | TcpStream |
| `b.slice(i, j)` | `b[(i) as usize..(j) as usize].to_vec()` | |
| `b.push(x)` | `b.push(x)` | Bytes, List |
| `b.append(c)` | `b.extend_from_slice(&c)`, `c` borrowed; `&c.clone()` when `c` is rooted at the variable of `b` | Bytes, List |
| `b.clear()` | `b.clear()` | |
| `l.join(sep)` | `l.join(&sep)`; `l.join("..")` for a literal | List<Text> |
| `x.copy()` | `x.clone()` | |
| `Bytes.empty()` | `Vec::<u8>::new()` | static |
| `Bytes.of(list)` | `(list).iter().map(\|x\| *x as u8).collect::<Vec<u8>>()` | static |
| `Bytes.of([])`, `b""` | `Vec::<u8>::new()` | static; an empty list has no element type to infer |
| `List.empty()` | `Vec::new()` | static |
| `List.with_capacity(n)`, `Bytes.with_capacity(n)` | `Vec::with_capacity((n).max(0) as usize)`, `Vec::<u8>::with_capacity(..)` | static; section 31 |
| `Int.to_text(i)` | `i.to_string()` | static |
| `log(t)` | `::ignite_io::log(&t)` | free function, writes a line to stderr |

`rest_after_word` and `first_word` are not builtins: `std.console` defines both in Ignite, with
the same whitespace rule (ASCII space, tab, CR, LF, FF, VT).

| Ignite | Rust |
|---|---|
| `const N: T = e`, `T` `Copy` in Rust | `#[allow(non_snake_case)] pub fn N() -> T { e }` |
| `const N: T = e`, any other `T` (built once, "Constants" below) | `#[allow(non_snake_case)] pub fn N() -> &'static T { static VALUE: ::std::sync::OnceLock<T> = ::std::sync::OnceLock::new(); if let Some(value) = VALUE.get() { return value; } let value = { e }; VALUE.get_or_init(\|\| value) }` |
| `N` (a constant, own or imported) in any expression | `N()`; a value position of a constant built once `N().clone()` |
| `IoError.new(m)` | `::ignite_io::IoError::new(m)` |
| `Enum.Variant(p)` pattern | `Enum::Variant(p)` |
| `x.to_text()` on `Int`, `Byte`, `Bool` | `x.to_string()` |
| `t.to_text()` on `Text` | `t.clone()` |

**Guarded arms.** Rust cannot match a `String` against a text pattern or a `Vec` against a
list pattern, and a constant is not a Rust pattern. An arm whose pattern contains a text,
list or constant pattern anywhere becomes `__m if <test> => { let a = (<access>).clone(); ...
body }`: `<test>` evaluates the whole pattern against `&__m` (`*s == N()` for a constant,
`*s == (*N())` for one built once,
`*s == "x"` for text, `s.len() == 2 && ...` for a list, a `match` with `_ => false` for a
variant payload), and each binding is read out of `&__m` by its path (`&s.0`, `&s[1]`,
`match s { E::V(_, __x) => __x, _ => unreachable!() }`) and cloned. Every other arm is a
Rust pattern as written.

**Constants.** Story `rust-target-constants-built-once`. A constant whose type is `Copy` in
Rust (`Int`, `Byte`, `U32`, `Bool`, `Unit`, a quantity, an `Option`, `Result` or tuple of
these) stays a fn that returns its value. Every other constant (`Text`, `Bytes`, a list, a
map, a record, an enum, a fn value) is built once per process: `N()` returns a `&'static T`
into a `OnceLock` (the row above). The type is the one the type checker gives the constant
(`TypedProgram::const_type`, aliases resolved), so the declaring module and every importer,
`std` or a sibling, agree on the form; without types every constant stays a fn.

- A use reads it as a reference parameter is read (Parameters, above): passed as it is to a
  reference parameter (`&**F()` for a fn constant lent to a parameter that only calls it, the
  `dyn Fn` behind the `Arc`), `(*N())` as an operand, the receiver of a builtin (`N().len()`,
  `N().as_bytes()` for a `to_bytes` the 20.3.7 forms lend), the argument of a builtin that
  only reads it, the constant or a field of it (`m.get(KEY())`, `contains_key`, `remove`,
  `starts_with`, `strip_prefix`, `join`, `append`, `pieces`, `Bytes.index_of`; a `std` fn
  such as `ends_with`, `split` or `find` takes it as a reference parameter), an element or
  field read in place (`checked::at(N(), i, ..)`, `N().f`), `for x in (N()).iter()`, a `for
  (k, v)` over a `Map` constant (`(M()).iter()`, the entries lent as a list's elements are),
  a `match` on `N()` with its bindings lent, and `format!` arguments. Such a read allocates
  nothing. Two reads copy what they read, as the same read of a parameter or a local list
  does (story `rust-target-read-only-bindings-borrow`): a field of an element as a receiver
  (`N.items[0].name.len()`, `checked::at(..).name.clone().len()`) and an element
  interpolated (`"{N.tags[i]}!"`, `checked::at(..).clone()`), one allocation per read each.
- A value position (a by-value argument, a `let` or `var` initialiser, a `return` operand
  or result, a list element, a record field, a `send`, a `spawn` argument) writes
  `N().clone()`: one copy, which no other use reads, so a later change of it is seen by no
  other use. A closure captures nothing for it: `N()` names a `'static`.
- A change in place of a constant (`C.push(x)`, `C[i] = x`, `C.f.clear()`, `C.put(k, v)`:
  the checker accepts these and refuses only an assignment to a field, E006) is written on
  the copy `N().clone()` and is lost, as before, as on the LLVM target (`docs/llvm.md`
  section 2) and in the evaluator.
- The generated code has no `unsafe`. Inside the `ignite-std` crate and the program crate
  each constant has one cell, so it is built once per process, where the LLVM target builds
  an aggregate constant once per process and IR module.

The order of effects. A constant's value is pure (E010, 4.3) but may call pure fns, and a
pure fn may `log` (not an effect) or fail (an overflow, an index out of range, no arm). Such an
initialiser exists. The first use that finds the cell empty evaluates the value outside the
cell, then publishes it; every later use reads the published value. So a `log` in it prints
once per process, at the first use, where it printed at every use before the story (the LLVM
target printed it once already); two tasks at the first use at once both evaluate it and both
log, and the one that publishes second drops its value and reads the first one's. A failure
publishes nothing: each later use evaluates it again and fails as every use did before. An
initialiser that reaches its own constant through a fn body (E015 follows only direct
references) recurses until the stack overflows, as before, since it is evaluated outside the
cell and no use waits for it (a `LazyLock`, or `OnceLock::get_or_init` around the
evaluation, would deadlock or panic there instead).

What it costs, counted by `constants::` (memcheck's `total heap usage`, 1,001 rounds less 1,
per round): reading a 16-byte `Text`, a 12-byte `Bytes`, a list of nine pairs of texts and a
record with a text and a list of two texts went from 3, 3, 38 and 16 allocations per round
(48, 36, 976 and 289 bytes) to 0 on the Rust target. A round that gives constants and a
field of one to `Map.contains`, `Map.get`, `starts_with`, `ends_with`, `contains` and
`Bytes.index_of` makes 0, and one over a `Map` constant 0 (`constants_adversary::` counts a
`for` over a map of three entries, `m.get(KEY)` and `t.starts_with(KEY)` at 0 where the
first form of this story made 4.001, 1.001 and 1.006: a copy of the map, of the key). The
two reads above that copy make 1 (`PAIRS[i].1.len()`) and 2 (`"{BOUNDS.tags[i]}!"`, the
copy and the text built). The LLVM target, unchanged, makes 0
`malloc` calls per round for all of them, in the same count (under 0.002 per round and 0.2
bytes; its first mention copies the constant to the heap once, `docs/llvm.md` section 2; what
a task's arena takes is not a `malloc` and is not in this count), so no story follows there
from this count. The form costs 6 lines of generated Rust per constant built once; the release
`todod` stripped grew from 878,064 to 878,752 bytes and `terminald` stayed at 600,176 (row 3.9's
method: `ignite build --target rust --release`, then `strip`); one `GET /todos/1` of `todod`,
`valgrind --tool=callgrind` over 200 replies after 50, went from 125,363 to 125,367
instructions (dev) and from 19,757 to 19,760 (release), its allocations stay 36 (`reply_cost::`'s
count): that path reads no constant built once. A form that is cheaper still, a `&'static
str` or `&'static [u8]` Rust builds at compile time, was not taken: every reference parameter
is a `&String` or `&Vec<u8>`, so such a constant would need a second calling convention or a
copy at each use that passes it on.

The generator reads types (`generate_typed`, `generate_std_typed`) for UFCS, fn-field calls,
`to_text`, `+` on `Text`, constant patterns and variants resolved by expected type; without
types it falls back to the name-based rules above.

Any other method name is passed through unchanged (`x.m(a)` → `x.m(a)`), so substrate
methods need no table entry. Substrate methods are written in Ignite with the same names as in
Rust: `bind`, `accept`, `connect`, `read`, `write_all`, `close`, `local_port`. Substrate
statics: `TcpListener.bind`, `TcpStream.connect`, `Stdin.read`, `Stdout.write_all` lower to
`::ignite_io::<Owner>::<name>(args)`.

`Process.exit(c)` lowers to `::ignite_io::exit((c) as i32)`, which returns `!`, so it has type
Never in Rust as in Ignite and needs no `?`. `e?` where `e` has type Never lowers to `e`
alone (without types: where `e` is a `Process.exit` call).

A bare variant whose enum the module neither declares nor imports (resolved by expected type,
section 4) lowers to the enum's path in `ignite-std`: `::ignite_std::console::Action::Close` in a
program, `crate::console::Action::Close` inside `ignite-std`, in expressions and patterns.

### 6.1 System lowering

A `system` produces one executable per executable component (section 14.5): a component
whose kind `K` is a record type with `fn K.main(self: K) -> Result<Unit, IoError>`. The
compiler-known kinds `Service` and `Client` of slices 1 to 9 are retired in slice 10: their
accept loop, connect and exit codes are Ignite in `std.protocol.telnet.TelnetServer`,
`std.protocol.telnet.TelnetClient` and `std.protocol.http.HttpServer` (section 14.6), and
the backends know no component kind by name.

Slice 17 adds the resource kinds `Directory`, `Launcher`, `Entropy` and `Setting`, created by
the generated `main` after the stores and before the listeners: section 21 (21.3, 21.8)
supersedes this section where they differ.

```ignite
system Terminal {
  resource ingress: TcpListener { address = "127.0.0.1", port = 2323 }
  component console: CommandConsole { prompt = "terminal> ", commands = module("examples.terminal.commands") }
  component server: TelnetServer { listener = ingress, application = console, executable = "terminald" }
  component client: TelnetClient { target = ingress, executable = "tn" }
}
```

The generated `main` of an executable (the Rust shown is the Rust target's; the LLVM
target's `main_<executable>.ll` does the same through `ignite_rt_*`, `docs/llvm.md`
section 6):

```rust
// src/bin/terminald.rs
use ::terminal::*;
use ::ignite_std::protocol::telnet::TelnetServer;      // the program's imports are repeated here
fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __res_ingress = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(2323))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", __res_ingress.local_port()));
        let __res_console = CommandConsole { prompt: String::from("terminal> "), commands: ConsoleCommands { respond: ... }, .. };
        let __res_server = TelnetServer { listener: __res_ingress, application: __res_console.clone(), executable: String::from("terminald"), .. };
        TelnetServer__main(&__res_server)
    }))
}

// src/bin/tn.rs
fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let __endpoint_ingress = Endpoint { address: String::from("127.0.0.1"), port: __port.unwrap_or(2323) };
        let __res_client = TelnetClient { target: __endpoint_ingress.clone(), executable: String::from("tn") };
        TelnetClient__main(&__res_client)
    }))
}
```

In order: `::ignite_io::run`; every `Store` resource the component (or an embedded component)
uses, created first (`let s: ::ignite_io::Store<T> = ::ignite_io::Store::new();`); every
`TcpListener` resource it uses as a listener, bound and logged as `listening on
<address>:<port>` (the first one takes the `--port` override; `--port 0` binds a free port
and the line reports it); the `Endpoint` value of every `TcpListener` resource it names
where an `Endpoint` is expected (section 14.5; `--port` applies to it too); the embedded
components' values in dependency order; the component's own value with the kind's defaults
for omitted fields, `executable` defaulting to the component name and a `Signal` output
set to its reference text; then `K.main(value)`. Every local `main` binds is `__port`,
`__res_<name>` (a resource or component) or `__endpoint_<name>` (an `Endpoint` value), with
every `_` of the name written `_0`: each starts with `__` and never ends in `_`. The Rust name of
a program item that contains `__` ends in `_`, and an associated fn symbol never starts with
`__`, so no local of `main` is spelled like a fn a property calls. The program crate is named
from the extern root (`use ::<crate>::*;`, `::<crate>::<module>::…`), so a program item named
like the crate does not shadow it.

Exit statuses (design section 11.7):

| status | source |
|---|---|
| 0 | `K.main` returned `Ok` |
| 1 | bind failed, or `K.main` returned an `IoError` without an exit code of its own |
| 2 | `--port` malformed or out of range |
| 3 | `TelnetClient.main`: connecting to the target failed (`IoError.with_exit_code(3)`) |
| 4 | `TelnetClient.main`: the `connect` role returned `Err` (`with_exit_code(4)`) |

A `K.main` chooses its exit codes with `IoError.with_exit_code` (section 7); the generated
`main` only maps `Err(e)` to `e.exit_code` after printing `error: <message>` to stderr. A
resource of an unknown kind or a `system` in a std module is E020 from the type checker;
the component rules are E020 and E104 to E109 (section 14.11). A program with no `system`
produces a library crate with no executable.
### 6.2 Generated projects

For `ignite build terminal.ignite --target rust --out DIR` (the default target until
2026-11-05; since `story:llvm-default-target` a build without `--target` is the LLVM target of
`docs/llvm.md`):

```
DIR/Cargo.toml          [package] name = <package last segment>, edition 2024, [workspace] empty,
                        [profile.release] overflow-checks = true, codegen-units = 1, lto = true, [profile.release.package.nix] and [profile.release.package.corosensei] opt-level = "s" (a shorter start, story `binary-size-back`)
                        [profile.dev.package."*"] debug = false, [profile.dev.package.ignite-io], [profile.dev.package."<package>"] and, with std, [profile.dev.package."ignite-std"] debug = "line-tables-only", split-debuginfo = "unpacked" (the crates from outside the project without debug information, `ignite-io`, the program and the standard library with line tables for backtraces, in `.dwo` files beside the objects; build time and dev size, rows 6.2 and 3.2)
                        [profile.dev.build-override] debug-assertions = false, overflow-checks = false (build scripts and their dependencies; build time, row 6.2)
                        [dependencies] ignite-io = { path = <substrate> }, ignite-std = { path = "std" } when the program imports a std module
                        one [[bin]] per component: name = executable, path = src/bin/<executable>.rs
DIR/src/lib.rs          the program's imports as `use`, every type, enum, protocol and fn
DIR/src/bin/<exe>.rs    `use <package>::*;`, the program's imports as `use`, plus the main above
DIR/std/Cargo.toml      [package] name = "ignite-std", version "0.0.0-h<content hash>" so programs sharing a target dir do not share a stale artifact, edition 2024, [dependencies] ignite-io
DIR/std/src/lib.rs      one nested `pub mod` per std module (`pub mod protocol { pub mod telnet { ... } }`)
```

Every `.ignite` file under the standard-library root is part of `ignite-std`, checked in
import order. When `DIR/Cargo.lock` does not exist, the build copies the lock of the
workspace the substrate lives in (`<substrate>/../../Cargo.lock`, when there is one) to it,
so cargo resolves from its versions and the local cache instead of asking the registry
(row 6.2). `ignite build` then runs `cargo build --manifest-path DIR/Cargo.toml
--target-dir TARGET` with `TARGET` from `--target-dir` (default `DIR/target`) and prints one
line per built executable: `built TARGET/debug/<executable>`. Exit status: 0 built, 1
diagnostics (in any file, rendered with that file's path), 2 cargo failed (its output passed
through), 3 a file could not be read or written.

A release build on Linux (`--release`) of a program with an executable writes two more files
and links the executables with the first (`story:rust-peak-memory-at-one-connection`, row 3.8
of `docs/quality.md`); a program without an executable gets none of the three additions:

```
DIR/layout.ld           a linker script added to the linker's own layout (`INSERT`): in `.text`, the functions of the
                        standard library's backtrace symbolizer (`std::backtrace_rs`, `gimli`, `addr2line`,
                        `rustc_demangle`, `miniz_oxide`; about 170 kB that run only to print a backtrace) in one run
                        before every other function, `.init` after `.text`
DIR/layout.rs           a build script that gives `-T DIR/layout.ld` as link arguments of the executables
DIR/Cargo.toml          [package] also has links = "ignite-layout" and build = "layout.rs"
```

Fat LTO leaves the symbolizer between functions a server runs at its start, and the kernel
maps the pages of a file around each page a process touches (64 KiB by default), so without
the script the symbolizer is resident memory of every process. `ignite build` gives cargo the
same link arguments itself (`--config target.<triple>.ignite-layout.rustc-link-arg-bins=[...]`),
which overrides the build script: cargo neither compiles nor runs it, and the arguments are
no `rustflags`, so they reach the executables' links only and replace nothing the user
configured. The build script is for a `cargo build --release` by hand in `DIR`. The script
changes no instruction; the size of an executable changes by the padding between its
functions (tens of bytes, either way). The LLVM linker (rustc's on x86-64) and GNU ld read
it. A linker that does not (gold, which has no `INSERT`) fails the link. When a release build
with the script fails, the driver asks the linker: in `TARGET/ignite-layout-probe` it builds a
program of one empty `main` with the script and, if that fails, without it, by the same cargo
in the same environment and for the same target, showing no output of the two. Only when the
first fails and the second builds does the driver print a note, leave
`TARGET/ignite-no-release-layout` (later release builds into `TARGET` do not try the script),
write the manifest without the two keys and build again. A build that failed for any other
reason fails once, with cargo's output as it is, and leaves the layout on. A manifest without
the two keys (after that fallback, and a dev build's) names neither file: `layout.ld` and
`layout.rs` of an earlier release build stay in `DIR` unused, and `cargo build` of it compiles
nothing more.

## 7 Substrate `ignite-io`

The only hand-written Rust a generated program runs, and the one implementation of IO for
every target: the Rust target's generated code calls it directly, the LLVM target through the
C ABI adapter `ignite-rt` (`docs/llvm.md` section 5).

Slice 14 adds `Pipe`, `PipeStream`, `ByteStream` and the `ignite_rt_pipe_*` entries: 18.8 and
18.9 supersede this section where they differ.

Slice 17 adds files (`Directory`), child processes (`Launcher`, `ChildProcess`), random bytes
(`Entropy`), launch arguments (`Setting`, `args::flag_from_args`) and the exit hooks: section 21
supersedes this section where they differ.

Slice 4: blocking. Sockets are `std::net`, standard input is one unbuffered `read(0)` per
call, every task is an OS thread (`spawn`), and there is no event loop and no tokio. An
operation that may wait is a plain `fn` with the name and signature its async version will
have minus `async`, so slice 5 (tokio as the event loop inside `ignite-io`) changes this crate,
the generator's lowering of `await`, `spawn` and `run`, and `ignite-rt`, not the operations'
callers.

```rust
pub struct IoError { .. }                       // Display, Debug, Clone, std::error::Error
pub enum Input { Data(Vec<u8>), Eof }          // Clone, Debug
#[derive(Clone)] pub struct TcpListener { .. }
#[derive(Clone)] pub struct TcpStream { .. }    // read half and write half, one lock each
impl TcpListener {
    pub fn bind(address: String, port: i64) -> Result<TcpListener, IoError>;
    pub fn accept(&self) -> Result<TcpStream, IoError>;             // retries transient failures (below)
    pub fn local_port(&self) -> i64;
}
impl TcpStream {
    pub fn connect(address: String, port: i64) -> Result<TcpStream, IoError>;
    pub fn read(&self, maximum: i64) -> Result<Input, IoError>;   // one read, up to maximum bytes
    pub fn read_into(&self, buffer: &mut [u8]) -> Result<usize, IoError>; // read into the caller's buffer; 0 is end of input
    pub fn write_all(&self, data: Vec<u8>) -> Result<(), IoError>; // appends to the send buffer
    pub fn write_from(&self, data: &[u8]) -> Result<(), IoError>; // write_all of a borrowed slice
    pub fn close(&self) -> Result<(), IoError>;                   // send the buffer, shutdown write half
}
pub const SEND_BUFFER: usize = 65536;
pub const ACCEPT_BACKOFF_FIRST: Duration;                               // 10 ms: first pause of accept after a transient failure
pub const ACCEPT_BACKOFF_LAST: Duration;                                // 1 s: longest pause
pub fn flush();                                                  // send what the current task wrote
pub struct Stdin;
impl Stdin { pub fn read(maximum: i64) -> Result<Input, IoError>; }        // one read of standard input
pub struct Stdout;
impl Stdout { pub fn write_all(data: Vec<u8>) -> Result<(), IoError>; }    // write and flush
```

A `read` waiting on a `TcpStream` holds only the read lock, so another task can write to the
same stream at the same time.

**Write coalescing.** Sockets have `TCP_NODELAY` set, and the substrate batches writes
itself. `write_all` appends to the stream's send buffer and records the stream with the
writing thread; it sends at once only when the buffer reaches `SEND_BUFFER` bytes. The
buffer is sent:
- before every operation that may wait: a read of any stream or of standard input,
  `accept` and `connect`;
- by `close`, at the end of each task, when `run`'s `main` ends, by `exit`, and when the
  last handle of the stream is dropped;
- when a thread ends, or by `flush()`.

So a task sends everything it wrote before it waits for anything, and a server that
answers a batch of pipelined requests sends the answers in one system call. A write is
not on the wire until its writer waits, ends or flushes. Code that waits outside the
substrate, such as a test thread blocked on a channel, calls `flush()` first. A failed
send is kept with its stream and reported by that stream's next `read`, `write_all` or
`close`.

Substrate operations take owned `String` and `Vec<u8>` arguments because generated code
passes every Ignite value by value (a literal is `String::from(..)`, a place is cloned).

```rust
pub fn run<F>(main: F) -> i32 where F: FnOnce() -> Result<(), IoError>;  // main on the calling thread; Err → stderr, error.exit_code(); does not wait for tasks or blocked stdin reads; restores the terminal mode
pub fn spawn<F>(task: F) where F: FnOnce() + Send + 'static;             // an idle thread or a new one; returns at once; never panics
pub fn try_spawn<F>(task: F) -> Result<(), IoError> where F: FnOnce() + Send + 'static; // spawn, Err when no thread can start
pub const LINGER: Duration;                                              // 1 s: how long a thread without a task waits for one
pub fn released_memory();                                                // mark freed heap memory for the next trim (Tasks, below)
pub fn log(line: &str);                                                  // the line and a newline to stderr in one write
pub fn exit(code: i32) -> !;                                             // restores the terminal mode, flushes stdout, exits the process
pub mod args {
    pub fn port_override() -> Option<i64>;                               // parses --port N from std::env::args
    pub fn port() -> Result<Option<i64>, IoError>;                       // same; malformed or out of range is Err with exit code 2
}
impl IoError {
    pub fn new(message: impl Into<String>) -> IoError;                  // exit code 1
    pub fn with_exit_code(self, code: i32) -> IoError;
    pub fn exit_code(&self) -> i32;
}
pub struct Process;
impl Process {
    pub fn exit(code: i64) -> Result<(), IoError>;            // restores the terminal mode, then exits the process
}
impl Stdin {
    pub fn is_terminal() -> Result<bool, IoError>;
    pub fn set_raw(on: bool) -> Result<(), IoError>;
}
```

**IO modes** (`docs/readiness-io.md`, phase R1). The substrate runs spawned tasks in one of
two modes, chosen once per process by the environment variable `IGNITE_IO`: `readiness`
(the default; also any value other than `threads`) or `threads`. Both modes run the same
operations with the same results; only cost differs. `main` runs on the process's main
thread in both.

```rust
pub enum IoMode { Threads, Readiness }
pub fn io_mode() -> IoMode;                                  // IGNITE_IO, read once
pub fn worker_limit() -> usize;                              // IGNITE_WORKERS, else available_parallelism
pub const TASK_STACK: usize;                                 // 2 MiB: the stack of a task in `readiness`
pub trait TaskLocals { fn swap(&mut self); }                 // a runtime's per-task thread-locals
pub fn set_task_locals(make: fn() -> Box<dyn TaskLocals>);   // each new task gets make()
```

- `readiness`: a task is a stackful coroutine with its own `TASK_STACK` stack (`mmap`, one
  guard page below it) on a **worker**, an OS thread named `worker` that runs its tasks until
  each waits and then waits in `epoll_wait` for a descriptor, a timer or a wake. A worker
  starts when a task needs one, up to `worker_limit()`. `spawn` from a task places the new
  task on the caller's worker while that worker has at most one task more than the least
  loaded worker (a worker not yet started counts as one with none). Otherwise, and from a
  thread that is no worker, it places the task on an idle worker, else on a new worker, else
  on the one with the fewest tasks. A worker whose only task is blocked in a read (below)
  counts as busy: placement passes it, and only when every worker is busy and none can start
  does a task go to it, to run when the read returns. A task stays on its worker (no
  stealing, no migration). A worker without a task waits up to
  `LINGER` from the end of its last task for one, then ends, so an idle server runs no worker. A wait of a task (a read, a
  write that finds the socket buffer full, `accept`, a channel, a pipe, a timer, a child,
  standard input) parks the task and lets its worker run another. Sockets block, as in
  `threads` mode (an accepted one from a single `accept4` with `SOCK_CLOEXEC`); a task's
  `recv` and `send` pass `MSG_DONTWAIT`. A listener has `TCP_NODELAY` and `SO_OOBINLINE`,
  which the sockets it accepts inherit. A socket is registered edge-triggered with the
  worker's epoll by its first wait there that parks its task, for input, urgent data and
  hang-up, and for room to write only from the first wait for room on; the drop of its last
  handle forgets the registration, and the close removes it from epoll. A read that left the
  socket empty (it read less than it asked for, or nothing) makes the next read of the stream
  wait for readiness before its first `recv`; so does the first read of a stream that sent
  first. A read that filled its buffer lets the next one try at once. A worker creates
  its epoll and eventfd when a task first waits for a descriptor through them, so tasks that
  wait only for each other, or only alone on a worker they may block, cost no descriptor; a
  worker that cannot create them (the process is out of
  descriptors) looks at the descriptor every 10 ms instead, until it can. A worker runs at
  most 128 tasks in a row before it polls for descriptors without waiting. The only task of a
  worker with nothing else to run waits for its descriptor with one `poll` on the worker's
  thread, as a blocked thread would, registered or not, and a hand-over to the worker ends
  that wait and parks the task. A read after a read that emptied the socket, without a
  deadline of its own, by such a task, on a worker with no hand-over pending, is one
  blocking `recv` instead, while fewer tasks live than `worker_limit()` (never with one
  worker), so a spawn meanwhile finds an idle worker or starts one; the worker counts as busy
  for that read. On a worker without epoll, the `poll` of its only task blocks the worker
  in the same way and under the same bounds, until as many tasks live as `worker_limit()`:
  from then on every such `poll` ends and waits through the worker's epoll. The first read of
  a stream never blocks its worker in `recv`, so connections opened before their first
  request spread over the workers. `accept` off a worker (the main
  thread's accept loop) waits for a connection before its `accept4`. Each wait reads the
  clock once.
  `connect`, name resolution, `Directory`, `Launcher` and stdout run inline on the worker. The
  idle timeout (26.2) is `SO_RCVTIMEO` for a blocking `recv`, set before the first such
  `recv`, and the deadline of the read's wait otherwise. A panic in a task ends the task, not
  its worker, as `task ended with error: <report>`; a stack overflow writes `thread 'worker'
  has overflowed its stack` and aborts the process, as Rust does on a thread. Thread-locals
  are the worker's: a runtime that keeps per-task state in thread-locals registers
  `set_task_locals`, whose `swap` exchanges them whenever a different task resumes, and
  borrows none across a wait.
- `threads`: every task is an OS thread, as below. Sockets block: a read or send waits in the
  kernel and the idle timeout is `SO_RCVTIMEO`.

**Tasks.** In `threads` mode a task runs on one OS thread for as long as it runs. A thread
that finishes a task waits up to `LINGER` for the next one; `spawn` hands a task to the
thread that went idle most recently and starts a new thread only when none is idle. A task
that panics ends its thread. Thread-local state survives from one task to the next on the
same thread. In both modes the substrate's pending writes are always empty between tasks,
because each task ends with a flush and flushes before it waits. When the operating system
cannot start a thread (or map a task's stack), the task is dropped without running (a task
that owns a stream closes it), the first such failure after a successful start is logged as
`cannot start a task: <reason>`, and the process continues.

**Freed memory.** A `Store` that replaces or removes a value, and an LLVM task whose arenas
held more than 1 MiB at its end, call `released_memory`. On Linux with glibc the substrate
then calls `malloc_trim(0)` after a task ends, at most once a second and on one thread at a
time, and when a thread that lingered without a task ends; on other targets it does
nothing. The trim runs after the task's flush and after the task stops counting as live,
so neither its writes nor the drain (16.13) wait for it.

**Accept.** `accept` retries a failure that passes with time: out of descriptors
(`EMFILE`, `ENFILE`), buffers (`ENOBUFS`) or memory, or a connection aborted before it
was accepted. It pauses 10 ms, doubling up to 1 s, between attempts and logs the first
failure of a run as `accept: <reason>; retrying`. A service therefore outlives a burst of
connections beyond its descriptor limit and resumes when descriptors free up.

**Terminal mode.** `Stdin.set_raw(true)` saves the current terminal attributes (once) and
switches standard input to non-canonical mode without echo, signals (`ISIG`), flow control
(`IXON`) or CR translation (`ICRNL`), one byte per read; output processing (`OPOST`) stays on.
`Stdin.set_raw(false)` restores the saved attributes. `run` restores them when `main` ends,
with `Ok` or `Err`, and when it panics. A process killed by a signal cannot restore them.
On a non-terminal standard input both calls are no-ops returning `Ok`.

Builtin names visible to Ignite programs: types `Text Bytes Byte Int Bool Unit List Option
Result TcpListener TcpStream Stdin Stdout Input IoError`; variants `Some None Ok Err Data Eof`;
value `Unit`; statics `Bytes.empty Bytes.of List.empty Int.to_text TcpListener.bind
TcpStream.connect Stdin.read Stdin.is_terminal Stdin.set_raw Stdout.write_all Process.exit IoError.new
Store.new` (tests only, section 12.3); free function `log`. Effectful substrate
operations: `bind`, `accept`, `connect`, `read`, `write_all`, `close`, and the statics
`TcpListener.bind`, `TcpStream.connect`, `Stdin.read`, `Stdin.is_terminal`, `Stdin.set_raw`,
`Stdout.write_all`, `Process.exit`. `IoError.new` is pure. Type name `Process` is a builtin
static owner only.

## 8 Diagnostics

Format: `<file>:<line>:<col>: error[E<nnn>]: <message>`, one per line, sorted by position,
line and column 1-based. The parser recovers at the next top-level keyword after a syntax
error so one run reports several errors; a declaration inside a block gives one error and is
skipped. Rendering indexes the line starts of a file once, so its cost is linear in the
number of diagnostics (20,000 in well under a second).

Codes of the syntax and name stages (`ignite-syntax`, `ignite-check`):

| code | meaning |
|---|---|
| E001 | syntax error; also a malformed interpolation or byte literal, a refutable `let` pattern, a one-element tuple, and `await` (section 10.2) |
| E002 | unexpected end of file |
| E003 | unknown name; also wrong arity of a call (a UFCS call counts the receiver), `log` used as a value, a constant with payload patterns, an associated fn whose owner is no record or enum of the module (section 13.8) |
| E004 | duplicate definition, including a declaration named like a builtin or `Tuple`, a role or record field declared twice, an associated fn declared twice or named like a variant of its owner, and a closure parameter repeated |
| E005 | ambiguous bare variant (section 4); reported by the type checker since slice 6 |
| E006 | assignment to an immutable binding or a constant; since slice 9 also `push`, `append` or `clear` (since 25.4 also `set` and `l[i] = v`) through a `let`, a `for` element or a pattern binding, on `x.to_bytes()`, or on a place rooted at a binding a closure captured (section 13.11; since story:shared-ownership-analysis also `put` and `remove` on a `Map` and a field, index or tuple field of a capture, 20.3.3) |
| E010 | effectful call or `spawn` in a pure context: a constant's value or a field default (section 5) |
| E011 | retired in slice 6 (was: effectful call not awaited) |
| E012 | retired in slice 6 (was: `await` on a pure call) |
| E013 | `spawn` of something that is not a call of an effectful user function or role, including a substrate operation |
| E014 | retired in slice 6 (was: constant used as a pattern; constant patterns are allowed, section 4.3) |
| E015 | constant cycle |
| E016 | effect missing from `requires` (section 5.1); a closure's effects not on its own parameters are charged where it is created (section 13.3) |
| E017 | effectful function used as a value where no effectful fn type covering its effects is expected (section 4.5); reported by the type checker since slice 7; also a fn passed to `List.map` with an effect on its parameter or one the caller's `requires` does not cover (20.4.3) |
| E020 | unsupported or malformed resource, component, contract, policy or assembly (also: a kind that is no record type, two components with one executable name, a `system` in a std module); reported by the type checker since slice 10. The Rust backend keeps E020 for a malformed resource and, when it runs without the typed program, for every component (a component is executable only through the type checker) |
| E021 | reserved (unused since slice 1) |
| E030 | protocol role missing or malformed, a role without `requires`, a call to a role the protocol lacks (the component rules of slices 1 to 9, `Client` without `connect` and `Service` without `serve`, retired with those kinds in slice 10: a kind's `main` calls the role, and a missing role is this E030 at the call); since slice 14 a `serve` role takes 2 parameters `(stream, application)` or 3 `(stream, limits, application)` (section 18.6) |
| E031 | unknown module or imported name (including a name imported from `std.net` or `std.io` that is neither a builtin nor declared in that module's file), or an import from a module that is neither `std.*` nor a sibling of the program's package (section 14.4) |
| E032 | import cycle |
| E049 | a binding inside an or-pattern |
| E050 | two tests of one module with the same name (section 12.2) |
| E051 | a test-only item used outside tests and test-only items, or imported (section 12.2), or `Store.new()` there (section 12.3), or `Pipe.pair()` there (section 18.4), or since slice 17 `Directory.temporary()`, `Launcher.new()`, `Entropy.new()` or `Setting.of(..)` there (section 21.8), or `CommandLine.of(..)` there (section 21.19); a UFCS use is reported by the type checker |
| E053 | a test name that is empty or interpolated; reported by the parser (section 12.1) |
| E041 | (slice 9) a name two glob imports provide, used without an explicit import (section 13.1 row 6) |
| E054 | nesting deeper than 256 levels of brackets, type arguments, prefix operators or `return` (section 3); string literals nested deeper than 32 interpolations (section 13.1 row 1) |
| E055 | a record that contains itself through fields and tuples alone, so a value would be infinite; recursion through an enum payload, `Option`, `Result` or `List` is allowed and boxed (section 13.4) |
| E056 | a parameter of a fn, role, fn type or closure named like an ambient capability: `stdin`, `stdout`, `process`, `net`, since slice 17 also `clock` (section 13.11, 21.1 probe p1) |
| E057 | a type parameter of a generic record, enum or alias that no field, payload or aliased type uses (section 13.11) |
| E099 | (slice 14) a bound naming an interface other than `ByteStream` or more than one interface, or `ByteStream` in a type position (section 18.7) |
| E091 | (slice 15) a catalogue constant, of type `std.rfc.Requirement` or `std.rfc.Spec`, whose value is not literal data (section 19.4); the consistency trigger is the evaluator's (below) |
| E076 | (slice 16) a second `..` in a list pattern, or a `..name` that is not the last element (section 20.5); reported by the parser |

Keywords are accepted as names in package and import paths, property names and
record-literal field names.

Codes of the type stage (`docs/llvm.md` section 7), reported by `ignite check` and both build
targets: E003 bare variant of an enum out of scope that the expected type does not decide,
or a payload variant used bare (section 4), E005 ambiguous variant, E040 type mismatch,
E041 unknown method or field for the receiver type (also an ambiguous or inapplicable method call, section 4.8; a tuple index past the tuple), E042 type cannot
be inferred (also a type parameter neither the expected type nor later uses in the body decide, a closure parameter without type, and an effectful operation
whose effects `ignite-check` could not check, section 11.7), E043 wrong number of arguments (also a range pattern whose start is above its end), E044 integer literal out of range for `Byte` or `U32` (32.2),
E045 operator not defined for the operand type (also `==` on a type that is not comparable, section 13.5, a constant pattern of a type without
`==`, and a range bound that is no literal or constant with a literal value), E046 `match` not exhaustive (ranges split `Int` and `Byte` into intervals, section 13.7), E047 record literal with a missing (and not defaulted),
unknown or duplicate field, E048 unknown type name, wrong number of type arguments or an
alias that refers to itself, E052 `assert` condition not `Bool` (section 12.3). Since slice 14 also E096: a use instantiates a `ByteStream`-bounded type
parameter with a type that does not implement `ByteStream` (section 18.7).
`ignite-check` reports E047 and E048 (an unknown record in a record literal is E048, not
E003); `ignite-types` reports them too for a module it is given unchecked, and alone reports
alias cycles.

Codes of slice 10 (section 14.11), all from the type checker: E100 brace literal without an
expected type, E101 quantity operands that do not fit an operator, E102 `module("p")` not
found or a field it cannot fill, E103 named argument misuse, E104 declaration property
(three texts, section 14.11), E105 component cycle, E106 `Map` key type, E107 quantity
literal out of range, E108 assembly binding key, E109 `K.main` signature (since slice 12
also the `K.machine` and `K.validate` hook signatures, fixture `e109_hook_signature` under
`crates/ignite-types/tests/fixtures`). Inside a `system`
or `assembly` the type checker also reports E003 for an unknown name, E041 for `System.x`
when `S` has no member `x`, E047 for a repeated map key or a `TcpListener` property it does
not know, and E045 for `%` on a quantity.

Codes of slice 11 (section 15.10): E116 effectful property expression, from the type
checker; from `ignite-eval`, through `ignite check`, `ignite plan` and `ignite explain`:
E110 member neither bound nor included by the assembly, E111 member bound twice, E112
binding not admitted (the reason `admits` returns), E113 `includes` differs from the
embedded components, E114 no assembly or system selectable, E115 evaluation limit (fuel or
value size), E117 failure during evaluation (14.8 report text), E118 value the model cannot
encode, E119 `ignite explain` path that names nothing. Fixtures: E116 under
`crates/ignite-types/tests/fixtures`, E110 to E115 and E117 to E119 under
`crates/ignite/tests/fixtures`, named as in section 15.10.

Codes of slice 12 (section 16.17): from `ignite-eval`, through `ignite check`, `ignite plan`
and `ignite run`: E120 declaration its kind's `validate` refuses (the reason `validate`
returns), E121 contract signal without a producer the assembly runs, E122 run-time `Ref`
that resolves to no running member or endpoint, E123 probe timing (`timeout >= interval`
or not positive), E124 contract window shorter than its signal's interval, E129 hosted
member whose kind lacks the entry its host needs; from `ignite run` and `ignite replay`:
E125 nothing to run, E126 journal of another model or format, or a held lock, E127 journal
that does not replay (first diverging `seq`), E128 reducer failure at run time (14.8 report
text). Fixtures: all under `crates/ignite/tests/fixtures`, named as in section 16.17.

Codes of slice 15 (section 19.12). Clauses also give E001 (malformed clause), E003 (unknown
spec or requirement), E004 (a spec named twice in `conforms`) and E040 (a clause argument of
the wrong type), section 19.6.

| code | meaning |
|---|---|
| E090 | a MUST-class requirement in scope of a conforming protocol with no status, or a requirement with two statuses (two messages); from `ignite-eval`, through `ignite check` and `ignite conformance` (section 19.7) |
| E091 | a catalogue constant that is not literal data, from `ignite-check`; an inconsistent catalogue in the checked file (an ordinal under 1, a name that is not the id, a requirement not listed once in the `Spec`, a foreign `rfc`, two `Spec` constants), from `ignite-eval` (section 19.4) |
| E092 | a `satisfies`, `verifies` or status naming a requirement of a spec that its protocol, or no protocol of its module, conforms to; from `ignite-eval` (section 19.7) |

Fixtures: E091 (literal data) under `crates/ignite-check/tests/fixtures`; E090, E091
(consistency) and E092 under `crates/ignite/tests/fixtures`, named as in section 19.12.

Codes of slice 16 (section 20.7): E076 above. Changed meanings: E003 is no longer reported for a
callee that is an expression (`not callable`); E040 also for a callee whose type is no fn type;
E042 also for a call of an effectful fn-typed expression that is no name and that `ignite-check`
gives no type (a call's result), and, since story `effectful-map-escape`, for a local of
effectful fn type without a type `ignite-check` can tell passed to `List.map` where the caller has
no `requires` (20.4.3); E046 splits a list
column by length and names the missing lengths as list patterns. Fixtures: `e076_rest` and
`e001_rest_outside_list` under `crates/ignite-syntax/tests/fixtures`; `e040_not_callable`,
`e043_expression_call_arity`, `e042_effectful_expression_callee`, `e040_rest_binding` and
`e046_rest` under `crates/ignite-types/tests/fixtures`.

Codes of slice 17 (section 21.9): from the type checker, E140 a `Text` literal path argument
of a `Directory` method that is absolute, has a `..` component or a NUL, and E141 a flag of a
`Setting` or `Directory` resource that another resource of the system also uses, or that is
`--port` or `--ref`; from `ignite run` and `ignite plan --for-apply`, E142 a `Setting` without
`default` in a member the run or the plan would start. Slice 17 also extends E104 (malformed
`Directory`, `Launcher` and `Setting` properties), E047 (unknown properties of those kinds and
`Entropy`), E020 (the resource kinds the text lists), E051 (the test-only constructors), E016
and E042 (the capability methods, section 21.3) and E056 (`clock`, section 21.1). Fixtures:
E140 and E141 under `crates/ignite-types/tests/fixtures`, E142 under
`crates/ignite/tests/fixtures`, named as in section 21.9.

Codes of slice 18 (section 22.10): from the check of the system model (`ignite-eval`), E150 a
member whose machine subscribes to a signal and whose kind declares no `classify(self, sample)`
hook over the signal's element type, and E151 a signal the assembly runs and a machine
subscribes to whose element type is not an outcome type (no fn, handle, reference or signal,
section 22.3). Slice 18 also drops the "unknown `good` outcome" text of E120 (an unknown
predicate is E003) and extends E109 to the `classify` hook. Fixtures: `e150_classify_missing`
and `e151_undecodable_element` under `crates/ignite/tests/fixtures`, `e109_classify_signature`
under `crates/ignite-types/tests/fixtures`.

Fixtures: E001 (including a keyword as a variable, fn and type name, beside
`valid_keyword_fields`), E002, E053 and E054 under `crates/ignite-syntax/tests/fixtures`;
E003, E004, E006, E010, E013, E015, E016, E030, E031, E041 (glob conflict), E047, E048, E049,
E050, E051, E055, E056, E057 and E099 under `crates/ignite-check/tests/fixtures`; E003 (variant out
of scope, payload variant used bare), E005, E017, E040 to E046, E048 (alias cycle), E051
(UFCS), E052, E096 and E100 to E109 (E104 three fixtures: `e104_required_field`,
`e104_output_field`, `e104_loopback`; `valid_system` mirrors the design file with kinds
declared locally, because the fixture standard library has no `std/protocol` and the
fixture must type without it; the real kinds are checked by `cli::check_design_file_is_silent`)
under `crates/ignite-types/tests/fixtures`.

## 9 Acceptance for the slice

1. `ignite check examples/terminal/terminal.ignite` exits 0 with no output.
2. `ignite build examples/terminal/terminal.ignite --out <dir>` prints `built .../terminald` and
   `built .../tn`.
3. `examples/` holds `.ignite` files only.
4. `terminald --port 0` prints `listening on 127.0.0.1:<port>`; a raw TCP client sees the
   transcript of design section 11.6: prompt `terminal> `, `help`, `echo hello`, `status`,
   `quit` → `bye` and close; an unknown word answers `unknown command: <word>`; `IAC DO ECHO`
   is answered with `IAC WONT ECHO`, `IAC WILL NAWS` with `IAC DONT NAWS`; `IAC IAC` is a
   literal byte; `CR LF` and `CR NUL` both end a line; `IAC AYT` answers `[yes]`;
   subnegotiation is skipped and bounded at 1024 bytes; `IAC IP` closes; a line over 4096
   bytes answers `line too long` and closes.
5. `tn --port <port>` with standard input `help\necho hello\nstatus\nquit\n` against that
   server writes the server's text (prompts and replies, Telnet commands removed) to standard
   output and exits 0 after the server closes.
6. `tn` connected to a listener that sends `IAC DO ECHO` answers `IAC WONT ECHO`; `IAC WILL x`
   is answered `IAC DONT x`.
7. `tn` exits 0 when standard input ends and the server then closes.
8. `tn` exit statuses: 2 for `--port abc`, 3 when nothing listens on the port, 4 when the
   server sends a subnegotiation over 1024 bytes (`std.protocol.telnet` returns
   `Err(IoError.new(..))`).
9. `tn` on a terminal (tested through a pseudo-terminal): raw mode with local line editing in
   Ignite (printable bytes echoed, backspace 8/127 erases, Enter 13 sends the line as CR LF and echoes `\n`, which the terminal's output processing
   shows as CR LF);
   `Ctrl+C` (3) sends `IAC IP`; `Ctrl+D` (4) on an empty line closes the write half;
   `Ctrl+]` (29) prints `\r\ntelnet> ` and reads one local command: `quit` or `close` ends
   the session at once with `Process.exit(0)`, also when the server keeps the connection open, an empty line returns to the session, `help` lists the commands,
   anything else prints `unknown command`. After `tn` exits, the terminal is back in
   canonical mode with echo.
10. Ignite sources use `const` for their limits (no zero-argument fns standing in for
    constants) and qualified variant patterns where a bare name is ambiguous or unclear.
11. Every diagnostic code in section 8 that is not retired has a fixture under
   `crates/ignite-syntax/tests/fixtures`, `crates/ignite-check/tests/fixtures` or
   `crates/ignite-types/tests/fixtures` (E020 by `crates/ignite-types` fixtures and
   `crates/ignite-rustgen` unit tests for the resource and untyped cases, E031, E032 by
   `crates/ignite` driver tests).

## 10 Slice 6: surface syntax

Slice 6 shortens Ignite source without new runtime behaviour. Sections 2 to 5 and 8 are
rewritten to include it; this section is the contract between the frontend
(`ignite-syntax`, `ignite-check`, `ignite-types`) and the backends (`ignite-rustgen`,
`ignite-llvm`) and states, per feature, where it is handled. "Desugar" means the parser
produces existing AST forms, so backends see nothing new.

| # | feature | source | handled by |
|---|---|---|---|
| 1 | no `await` | `let input = stream.read(4096)?` | parser: `await e` is E001 with a hint (section 10.2); `ExprKind::Await` is no longer produced. Checker: E011 and E012 retired; an effectful call or `spawn` in a pure context (constant value, field default) is E010 ``effectful call `X` outside an effectful function: …`` |
| 2 | inferred `requires` | a fn without `requires` that performs effects is effectful; its set is inferred (fixed point over the module's fns, section 5). A fn with `requires` is checked against its list (E016). Protocol roles must declare `requires` (E030). `FnDecl::is_effectful` only says whether `requires` is written: `Symbols.functions[..].effectful`, `Symbols.effects` and `FnSig.effectful` carry the inferred answer. An effectful fn as a value is E017 | checker, typer |
| 3 | expression body | `fn f(x: Int) -> Int = x + 1` | desugar: a block with one `Expr` statement |
| 4 | tuples | type `(A, B)` = `Type { name: "Tuple" }`, value `ExprKind::Tuple`, pattern `PatternKind::Tuple`, `let (a, b) = e` = `StmtKind::LetPattern`; `()` is the Unit value (`ExprKind::Tuple(vec![])`, type `Unit`). `Ty::Tuple(Vec<Ty>)`, two or more elements | all layers. Rust: Rust tuples. LLVM: an anonymous struct with C layout, copied by value with element glue |
| 5 | patterns | `true`/`false` (`PatternKind::Bool`), or-patterns `a \| b` (no bindings inside an or-pattern, E049 otherwise), constant patterns (a bare name or payload-less path that resolves to a constant, whatever its case: `lowest` is a constant pattern when a constant `lowest` is in scope, a binding otherwise; backends read `TypedProgram::pattern_const` before treating a name as a binding; compared with `==`; type `Int`, `Byte`, `Bool` or `Text`, else E045; payload patterns E003), tuple patterns, list patterns `[p, q]` (exact length), `()` | parser, checker (E014 retired), typer (exhaustiveness E046 covers Bool, tuples, or-patterns at any depth; list, literal and constant patterns need a catch-all), both backends |
| 6 | default field values | `type Reply { lines: List<Text>, action: Action = Continue }`; a literal may omit a defaulted field (E047 only for fields without default). The default is typed like a constant of the record's module (pure, E010). It is lowered like a constant of the record's module named `<Record>.<field>`; an omitting literal calls it | parser, checker, typer, both backends |
| 7 | field punning | `Reply { lines }` = `Reply { lines = lines }`; also `,` as separator on one line | desugar |
| 8 | variant inference | bare `Continue`, `Interpret`, `Some`... resolve against the expected type (section 4), whether or not its enum is imported; else the one enum in scope declaring the name; else E005, now reported by the typer (the checker no longer does), or E003 when only enums out of scope declare it | checker (passes such names), typer |
| 9 | interpolation | `"too long: {n} bytes"`; parts are expressions of type Text, Int, Byte, Bool or Bytes (E041 for any other); `{{` and `}}` are literal braces | lexer + desugar to `"too long: " + n.to_text() + " bytes"`; new builtin method `to_text` on Int, Byte, Bool, Text (both backends) |
| 10 | `+` on Text | `a + b` | typer (`BinaryOp::Add` on Text is concatenation), both backends |
| 11 | byte literals | `b' '` is an `Int` literal of the byte (typed `Byte` by context, E044 range), `b'\n'`, `b'\\'`, `b'\''`, `b'\xNN'`; `b"\r\n"` is `Bytes.of([13, 10])` | lexer, desugar |
| 12 | `return` expression | `Eof => return Ok(())`; `ExprKind::Return`, type Never. `StmtKind::Return` stays for statements | parser, typer, both backends |
| 13 | discard | `_ = e` and `let _ = e` = `StmtKind::LetPattern` with `PatternKind::Wildcard` | parser |
| 14 | type alias | `type IoResult<T> = Result<T, IoError>`; `Decl::Alias`; exported and importable like records; resolved entirely by the typer (backends see the target type in `Ty`); an alias cycle is E048 | parser, checker (names, arity), typer |
| 15 | UFCS | `x.f(a)` where the receiver type has no builtin method or fn-typed field `f` and a module-level fn `f` in scope (own or imported, never a local) has a first parameter that accepts the receiver: `Callee::Fn` on the `Method` node; backends pass the receiver as first argument. A builtin method name is never UFCS; field and fn both matching is E041 (section 4.8) | checker (arity, effects), typer, both backends |
| 16 | fn-field call | `application.respond(line)` where `respond` is a record field of fn type: `Callee::FieldFn` on the `Method` node; backends call the field's value with `args` | typer, both backends |
| 17 | `for b in bytes` | `for` over `List<T>` including `Bytes`, already in the grammar; tested on both targets | tests |

Rules:
- Backends may assume the typer has accepted the program: every new node has a type, every
  `Method` node a callee, every constant pattern an entry in `pattern_const`.
- `unimplemented!("slice 6: …")` arms left by the skeleton must all be gone at the end.
- Generated Rust and IR goldens change only where a feature is used.
- Std and example sources are rewritten to use these features after the frontend and
  backends land (acceptance: `std/protocol/telnet.ignite` at most 400 lines excluding tests, no
  line over 100 characters; `examples/terminal/terminal.ignite` without `return`, `await` or
  `Action.Continue`).

### 10.1 Frontend contract for the backends

Settled by the frontend (S1); the backends rely on exactly this.

**Desugarings.** The parser produces these existing forms; nothing new reaches a backend.

| source | AST |
|---|---|
| `await e` | E001 (10.2); `ExprKind::Await` is never produced |
| `fn f(..) -> T requires [..] = e` | `body: Block { stmts: [Stmt { kind: StmtKind::Expr(e), span: e.span }], span: e.span }` |
| `R { a, b = e }` | `fields: [FieldInit { name: a, value: Expr { kind: Name(a), span: a.span } }, FieldInit { b = e }]` |
| `"x{a}y{b}"` | `((Str("x") + a.to_text()) + Str("y")) + b.to_text()`: `Binary { op: Add }` folded to the left; each `{e}` is `Method { receiver: e, name: "to_text", args: [] }` spanning `e`; empty text pieces are left out, so `"{n}"` is `n.to_text()` alone. Every `Binary` node spans the whole literal. `{{` and `}}` are `{` and `}` in any string literal |
| `b'c'` | `ExprKind::Int(byte)` in expressions, `PatternKind::Int(byte)` in patterns (escapes `\n \r \t \v \f \0 \\ \' \" \xNN`, `NN` up to `FF`) |
| `b"..."` | `Method { receiver: Name("Bytes"), name: "of", args: [List([Int(b0), Int(b1), ..])] }`, the form `Bytes.of([..])` parses to; `b""` has an empty list |
| `let _ = e`, `_ = e` | `StmtKind::LetPattern { pattern: Wildcard, value: e, mutable: false }` |
| `let (a, b) = e` / `var (a, b) = e` | `StmtKind::LetPattern { pattern: Tuple([a, b]), mutable: false / true }`; the pattern is a binding, `_` or a tuple of those |
| `()` | `ExprKind::Tuple(vec![])`, type `Unit` |
| `(a, b)` type | `Type { name: "Tuple", args: [a, b] }`; `()` as a type is `Type { name: "Unit" }` |

**Calls on `Method` nodes.** `TypedProgram::callee(method_expr)` is always set:

| callee | meaning | lowering |
|---|---|---|
| `Builtin(b)` | builtin method, as before | as before |
| `Fn(q)` | UFCS: `x.f(a, b)` calls fn `q` with arguments `(x, a, b)` | evaluate the receiver first, then `args` in order, and call `q`; the receiver is an ordinary value argument (ownership rule: clone a place) |
| `FieldFn { record, field }` | `x.respond(line)` where `respond` is a field of fn type of record `record` (`type_of(receiver)` is `Ty::Record(record)`) | evaluate the receiver, read the field, call its value with `args`; the receiver is not an argument |

`Callee::Fn` on a `Method` node only ever names a module-level fn (own or imported), never a
local. `type_of(method_expr)` is the callee's return type in all three cases.

**Field defaults.** The default is `Field.default: Some(expr)` in the `RecordDecl` of the
module that declares the record. That module's `TypedProgram` types it: `type_of(expr)` is
the field's type, and it is typed like a constant value (no locals, pure, may use the
module's constants and pure fns). `TypedProgram::field_has_default(&record, field)` answers
for records of the module and of every module it was typed against. A record literal that
omits a field omits only defaulted fields; the backend fills each from the default of the
record's module, lowered like a constant of that module named `<Record>.<field>` (how it is
named in generated code is the backend's choice, section 6).

**`to_text`.** `x.to_text()` is a `Method` with `Callee::Builtin`, chosen by receiver type:

| receiver | `Builtin` | text |
|---|---|---|
| `Int` | `IntToTextMethod` | decimal, as `Int.to_text(i)` (`IntToText` stays the static) |
| `Byte` | `ByteToText` | decimal `0` to `255` |
| `Bool` | `BoolToText` | `true` / `false` |
| `Text` | `TextToText` | the text itself (a copy) |
| `Bytes` | `BytesToText` | lossy UTF-8, unchanged |

**`Text + Text`.** `Binary { op: Add }` whose operands have type `Text`; `type_of` of the
node is `Ty::Text`. No callee is recorded. Lower as concatenation, left operand first.
`+` on any other non-numeric type is E045.

**`return` expression.** `ExprKind::Return(Some(e))` checks `e` against the enclosing fn's
return type; `Return(None)` only in a fn returning `Unit`. Its type is `Ty::Never`.

**Patterns.** `Bool(b)` on `Bool`. `Or(alternatives)`: two or more, binding nothing.
`Tuple(ps)` on `Ty::Tuple` of the same length; `Tuple([])` (`()`) on `Unit` matches every
value. `List(ps)`: a `List<T>` of exactly `ps.len()` elements, each matched in order.
Constant pattern: a `Variant { path: [NAME], args: [] }` or a `Binding(name)` for which
`TypedProgram::pattern_const(pattern)` is `Some(q)`; it matches when the value `== q`'s
value (its type is one with `==`: `Int`, `Byte`, `Bool`, `Text`). Bindings in patterns are
typed by name span as before (`binding_type`).

**Type aliases.** No `Ty` ever names an alias. `TypedProgram::resolved_type(&Type)` gives
the `Ty` of every type expression of the module (signatures, fields, payloads, constants,
`let` annotations), so a backend that lowers an AST `Type` reads it there instead of
interpreting the name. `ModuleTypes.aliases` carries exported aliases for importers;
backends do not need it.

**New `Builtin` variants:** `IntToTextMethod`, `ByteToText`, `BoolToText`, `TextToText`.
**New `ModuleTypes` fields:** `aliases`, `defaults`; construct `ModuleTypes` with
`..Default::default()` (`crates/ignite-llvm/src/layout.rs` `module_types` needs it).
**New `TypedProgram` methods:** `resolved_type(&Type)`, `field_has_default(&QualName, &str)`.

**Variants.** A bare variant resolved against the expected type gets the same
`Callee::Variant` as a qualified one; backends see no difference. Its `enum_` may name an
enum the module does not import (section 4); a backend names it by its `QualName`, not by
a name in scope (Rust: section 6).

**`Process.exit`.** Type Never on both targets: no `?` is needed after it (section 6).

**Effects.** `FnDecl::requires` may be `None` for an effectful fn (inferred); read
`FnSig.effectful` (from `TypedProgram::exports().fns` or the imported `ModuleTypes`), never
`FnDecl::is_effectful`. Backends do not read effect sets.

### 10.2 Deprecated `await`

The flip is done: `ignite_syntax::parser::AWAIT_IS_ERROR` is `true`, so `await e` is E001
``remove `await`: effectful calls need no marker``, shown by the syntax fixture
`e001_await.ignite`. The `std/` and `examples/` sources, the goldens and the tests no longer
use `await`; `grep -rln await crates std examples` lists only the lexer's keyword, the
parser's error path and that fixture. `ParseOptions { await_is_error: false }` still parses
`await e` as `e`; nothing in the repository uses it. `ExprKind::Await` stays in the public AST
and is never produced.

## 11 Slice 7: todo backend

Slice 7 adds what an HTTP backend with shared in-memory state needs, and the example
`examples/todo/todo.ignite` (executable `todod`). This section is the contract between the
frontend (T1: `ignite-syntax`, `ignite-check`, `ignite-types`), the backends and substrate
(T2: `ignite-rustgen`, `ignite-llvm`, `ignite-rt`, `ignite-io`) and the Ignite sources (T3:
`std/text.ignite`, `std/json.ignite`, `std/protocol/http.ignite`, `examples/todo/`,
`crates/ignite/tests/todo.rs`). T1 refines it as decisions settle; T2 and T3 read it.

### 11.1 Generics

- Records, enums, fns, protocol roles and aliases take type parameters:
  `type Pair<A, B> { first: A, second: B }`, `enum Tree<T> { Leaf, Node(T) }`,
  `fn swap<A, B>(p: Pair<A, B>) -> Pair<B, A> = ...`, `fn serve<S>(stream: TcpStream, app: HttpApp<S>) ...`.
- No bounds. A type parameter supports only what every type supports: binding, passing,
  returning, storing in records, lists, tuples, `Option`, `Result`, `Store`. `==`, operators,
  methods and `to_text` on a type parameter are E045/E041. Since slice 14 a fn or role type
  parameter may be bounded by `ByteStream`, which gives it the stream methods (section 18.3).
- Type arguments are inferred at calls from the arguments and the expected type; explicit
  arguments are not in the grammar (E042 when inference fails; annotate the binding).
- `Ty::Param(String)` inside generic bodies; `Ty::Record`/`Ty::Enum` gain type arguments
  (T1 decides the representation, add-only). `TypedProgram` exposes, per generic call and per
  generic type use, the concrete instantiation (`instance(&Expr) -> Option<&[Ty]>` or similar).
- Rust target: Rust generics with `Clone + Send + Sync + 'static` on every parameter.
  LLVM target: monomorphization per instance; mangled names carry the instance
  (`"<module>.<name><Int,Text>"`); copy glue per instance.

### 11.2 Effectful fn types

- A fn type may name parameters and carry effects:
  `fn(state: S, request: Request) -> Response requires [state.*]`.
- `p.*` means any effect on argument `p` (and on values reached through it). The plain
  form `fn(A) -> R` stays pure.
- A fn value is assignable to an effectful fn type if its inferred or declared effects map
  onto the type's effects through the parameter positions. E017 stays for an effectful fn
  used where a pure fn type is expected.
- Calling an effectful fn value (local, parameter, `Callee::FieldFn`) requires the caller to
  hold the mapped effects; a role that calls `app.handle(app.state, request)` declares
  `requires [app.*]`. `x.*` in a `requires` list covers every `x.op`.

### 11.3 `Store<T>`

- Builtin generic substrate type, `T` any type. Effects: `s.read`, `s.write`.

| method | type | effect |
|---|---|---|
| `s.next_id()` | `Int` (1, 2, 3, … per store, atomic) | `s.write` |
| `s.get(id)` | `Option<T>` | `s.read` |
| `s.put(id, value)` | `Unit` (insert or replace) | `s.write` |
| `s.remove(id)` | `Option<T>` (the removed value) | `s.write` |
| `s.values()` | `List<T>` in ascending id order | `s.read` |
| `s.update(id, f: fn(T) -> T)` | `Option<T>` (the new value), atomic, `f` pure | `s.write` |
| `s.upsert(id, f: fn(Option<T>) -> T)` | `T` (the stored value): `f` of the current value, `None` when `id` holds nothing, atomic, `f` pure | `s.write` |

- Value semantics: `get`/`values` return copies; `put` stores a copy.
- One implementation in `ignite-io`: `Store<T: Clone + Send + Sync>` over an `Arc` of 16 shards
  (`Sync` because a value an `update` or `upsert` has taken stays shared with the readers of
  `len` and `values` while the taker copies it),
  each a `Mutex<BTreeMap<i64, _>>` with a `Condvar`, chosen by a Fibonacci hash of the id,
  plus an `AtomicI64` counter (story `store-lock-contention`). Every method is atomic for the
  ids it touches; `len` and `values` lock every shard in shard order, and `values` merges the
  shards into ascending id order. The Rust target uses it directly: its `update` and `upsert`
  hold the shard's lock only to take the value out of its slot (marked taken, the value shared
  with the slot) and to put the new value back, and copy the old value, run `f` and drop the old
  value with no lock held; a task that reaches a taken id waits until it is put back, and a
  panic in `f` puts the old value back. `len` and `values` wait for no taken slot: they read a
  taken slot's value from before the update (an absent id an `upsert` took is absent), so a
  task waits for another's `f` only on the same id (story `store-adversary-findings`). `ignite-rt` exposes `ignite_rt_store_*` over a `Store<HeapValue>`, where
  `HeapValue` is a pointer to a malloc'd deep copy made and freed by generator glue
  (`copy_to_heap.<T>`, `copy_from_heap.<T>`, `free_heap.<T>`), passed to `store_new`.
  `get`, `values` and `update`'s result use the store's borrowing reads (`read`,
  `read_values`, `replace_with`): `copy_from_heap` copies the stored value straight into the
  calling task's arena under the shard's lock, so a read makes one copy and frees nothing. `update`'s
  thunk `store_update.<T>` runs under the shard's lock behind a callback guard: a failure inside `f`
  (14.8) leaves the old value, the lock is released, then the task ends with the failure's
  report, as on the Rust target, where the failure unwinds through `ignite_io::Store`.
- `upsert` is the atomic insert-or-update: two tasks that upsert one absent id lose neither
  write, where `update` (which answers `None` for it) followed by `put` can. `ignite_io`'s
  `upsert` takes the value out as `update` does; `ignite_rt_store_upsert` calls
  the thunk `store_upsert.<T>(f, old)` under the lock with the stored heap value, or null for an
  absent id, which the thunk passes to `f` as `None`. The thunk has `update`'s callback guard.
- Created as a system resource (11.4). `Store.new()` creates one in tests and test-only items
  only (section 12.3).

### 11.4 System

- `resource todos: Store<Todo>` (no properties) declares a store. Both mains create it
  before the accept loop.
- Resource names are values inside component property expressions:
  `application = HttpApp { state = todos, handle }`.
- A protocol may declare only `serve`; a kind's `main` calling `connect` on it is E030 at the
  call (the `Client` component rule of slices 7 to 9 went with that kind, section 14.5).

### 11.5 New builtins

- `List.map(f: fn(T) -> U) -> List<U>`, `f` pure.
- `Int.parse(text: Text) -> Option<Int>` (decimal, optional leading `-`).
- `Byte.to_int()`, `Int.to_byte()` (`Option<Byte>`, `None` outside 0..255).
- `TcpStream.set_idle_timeout(ms: Int)`; effect `stream.write`. A read that waits longer
  ends with `Eof`.
- Text work that can be Ignite (split, find, to_lower, starts/ends_with, trim) is Ignite in
  `std/text.ignite`, used by UFCS. T3 lists any primitive it cannot write in Ignite.

### 11.6 Std and example (T3)

- `std/text.ignite`, `std/json.ignite` (`enum Json`, `encode`, `parse -> Result<Json,
  JsonError>`, accessors), `std/protocol/http.ignite` (HTTP/1.1 server role, keep-alive,
  `Content-Length`, limits header 8 KiB → 431 and body 1 MiB → 413, errors 400/404/405,
  idle timeout 30 s, `Response.json`).
- `examples/todo/todo.ignite` ≤ 100 lines: `GET /todos`, `POST /todos`, `GET /todos/{id}`,
  `PATCH /todos/{id}`, `DELETE /todos/{id}`; JSON bodies `{"id":1,"title":"milk","done":false}`.
- `crates/ignite/tests/todo.rs`: both targets, real socket; CRUD flow, 400/404/405/413,
  keep-alive (3 requests on one connection), 16 connections × 100 POSTs → 1600 distinct ids.
- Since slice 8 the behaviour of `std.text`, `std.json`, `std.console`, `std.protocol.telnet`,
  `std.protocol.http` and the examples is tested inline (`test` blocks, section 12); `task
  test-ignite` runs them on both targets.

### 11.7 Frontend contract (T1)

Settled by T1; T2 builds against it, T3 writes Ignite against it.

**Surface syntax.**

| source | meaning |
|---|---|
| `type Pair<A, B> { first: A, second: B }` | generic record; `type N<P> = T` stays an alias |
| `enum Tree<T> { Leaf, Node(T, List<Tree<T>>) }` | generic enum; it holds itself only through `List` (E055) |
| `fn swap<A, B>(p: Pair<A, B>) -> Pair<B, A>`, roles too | generic fn or role |
| `fn(state: S, request: Request) -> Response requires [state.*]` | effectful fn type: parameters named (all or none, E001 for a mix), `requires` after the return type |
| `requires [app.*]` | `*` as the last segment of an effect: every effect on `app` |
| `resource todos: Store<Todo>` | resource with type arguments; the `{ .. }` is optional |
| `items.map(f)`, `Int.parse(text)`, `b.to_int()`, `i.to_byte()`, `stream.set_idle_timeout(ms)` | new builtins: methods, except the static `Int.parse` |

`requires` after a fn type's return type belongs to that fn type (the innermost one). A fn
declaration whose return type is a fn type puts it in parentheses before its own `requires`:
`fn make() -> (fn(Int) -> Int) requires [..]`. Type arguments are never written at a use
(`swap(p)`, not `swap<Int, Text>(p)`).

**AST (add-only fields).** `RecordDecl.type_params`, `EnumDecl.type_params`,
`FnDecl.type_params`: `Vec<Ident>` (roles are `FnDecl`). `Type.fn_names: Vec<Ident>`: empty,
or one name per parameter of a fn type. `Type.requires: Option<Vec<Path>>`: `Some` makes a fn
type effectful (`requires []` included). `ResourceDecl.args: Vec<Type>`: the kind's type
arguments (`Store<Todo>` is `kind: Store, args: [Todo]`). `p.*` is a `Path` whose last
segment is the identifier `*`; `Path::dotted` gives `p.*`.

**`Ty` (new variants).**

| variant | meaning | backends |
|---|---|---|
| `Param(String)` | a type parameter, inside a generic declaration only | Rust: the parameter; LLVM: substitute the instance |
| `GenericRecord(QualName, Vec<Ty>)` | a record that declares type parameters, with its arguments (`Param`s inside its own declaration) | Rust: `R<A, B>`; LLVM: one layout per instance |
| `GenericEnum(QualName, Vec<Ty>)` | the same for an enum | same |
| `Store(Box<Ty>)` | `Store<T>` | Rust: `ignite_io::Store<T>`; LLVM: `ignite_rt_store_*` |
| `EffectFn(Vec<Ty>, Box<Ty>, FnEffects)` | an effectful fn type; `FnEffects { names, requires }` | lower exactly like `Fn(params, ret)`; ignore the effects |

A record or enum without type parameters stays `Ty::Record` / `Ty::Enum`, so a program that
uses no generics produces no new `Ty`. A generic one is always `GenericRecord` /
`GenericEnum`, never `Record` / `Enum`. `FnEffects` compares by parameter position, so
`fn(a: Int) requires [a.*]` equals `fn(b: Int) requires [b.*]`.
`Ty::substitute(&self, params, args)` replaces `Param(params[i])` by `args[i]`.

**`Builtin` (new variants).** `ListMap` (`l.map(f)`, `f: fn(T) -> U` pure, result
`List<U>`), `IntParse` (`Int.parse(t) -> Option<Int>`: decimal, optional leading `-`, no
spaces, `None` on overflow), `ByteToInt` (`b.to_int() -> Int`), `IntToByte`
(`i.to_byte() -> Option<Byte>`), `TcpStreamSetIdleTimeout`
(`s.set_idle_timeout(ms) -> Result<Unit, IoError>`), `StoreNextId`, `StoreGet`, `StorePut`,
`StoreRemove`, `StoreValues`, `StoreUpdate` (signatures in 11.3; `update`'s `f` is pure).
`Callee` has no new variant: a call of an effectful fn value is `Callee::Local` or
`Callee::FieldFn` as before.

**Instances.** `TypedProgram::instance(&Expr) -> Option<&[Ty]>` gives the type arguments, in
the order of the declaration's type parameters, of:

- a `Call` or `Method` whose callee is a generic fn (also by UFCS) or a generic role;
- a `Name` that is a generic fn used as a value;
- a variant of a generic enum, as a value or a constructor call (also a pattern's enum, by
  the scrutinee's `GenericEnum` type);
- a record literal of a generic record;
- the value expression of a component's `protocol` property: the instance of the role the
  component runs (`serve` for a `Service`, `connect` for a `Client`; since slice 10 the role
  call is in the kind's `main` and is an ordinary call).

Inside a generic declaration the arguments may contain `Param`; the LLVM target substitutes
the enclosing instance. `TypedProgram::type_params(&QualName) -> Option<&[String]>` gives the
type parameters of a generic record, enum or fn of the module or a module it was typed
against; `role_type_params(&QualName, role)` those of a role. `ModuleTypes.generics` carries
them (key: the name, or `Protocol.role`). `record_fields`, `enum_variants` and `FnSig` hold
the declared types with `Param`; instantiate with `Ty::substitute`. `type_of` is always the
instantiated type, `variant(ty, name)` instantiates the payload of a `GenericEnum`.

**Generic typing.** Type arguments are inferred at a call from the expected result type and
the arguments; an argument that needs an expected type (`None`, `[]`, `Ok(..)`) is typed
after the others. A parameter left unbound is E042 (``cannot infer type parameter `T` of
`f`; annotate the binding``), conflicting bindings E040. Inside a generic declaration a
type parameter has no methods except `copy` (E041) and no operators (E045), including `==`
and `to_text`.

**Effect mapping.**

- `requires [x.*]` covers every `x.op`; a needed `x.*` is covered only by `x.*`.
- A callee's requirement `q.*` maps through the roots of the argument for `q` like `q.op`.
- A fn value of an effectful fn (declared or inferred) is allowed only where an effectful fn
  type is expected (a field, parameter, annotation or element of that type), and only when
  each of its effects maps onto the type: an effect `p.op` on its `i`-th parameter needs
  `n.op` or `n.*` in the type's `requires`, `n` being the type's `i`-th parameter name; any
  other effect needs itself or `x.*` (`x` its head). Otherwise E017. A pure fn fits any fn
  type of its shape. A value of an effectful fn type passed where a pure fn type is expected
  is E040.
- Calling a value of effectful fn type (local, parameter, `Callee::FieldFn`) is an effectful
  operation needing the type's `requires` mapped through the arguments' roots, like a call of
  a declared fn.
- `Store` methods: `get`, `values` need `r.read`; `next_id`, `put`, `remove`, `update` need
  `r.write` (`r` each root of the receiver). `set_idle_timeout` needs `r.write`.
- E017 is reported by the type checker since slice 7 (it needs the expected type); its
  fixtures are under `crates/ignite-types/tests/fixtures`.
- `ignite-check` finds Store receivers and the fn types of called values from declared types:
  parameter types, `let` annotations, record field types (generic arguments substituted,
  aliases expanded), `x.f`, `x.copy()`, `e?`, `l[i]`, Store results, `for` over a list, a
  literal of a record without type parameters (`let w = World { .. }`, case-study gap G15), a
  call by name of a fn without type parameters that the module itself declares and whose
  declared return type is no fn type (that type: `let d = ends()`, then `d.tx.send(1)`; an
  imported fn's return type names the exporting module's types and stays E042, as does a fn
  value a call returns, 20.4.3), and the names a `match` arm binds from a scrutinee of such a
  type (the value itself, tuple elements, the payload of `Some(x)` of an `Option<T>` and of
  `Ok(x)` and `Err(e)` of a `Result<T, E>`: `match senders.get(id) { Some(tx) => tx.send(1),
  .. }` with `senders: Store<Sender<Int>>`; case-study gap G32). An imported record's field
  types and an imported alias's target are written in the exporting module's namespace: a field
  of such a record has no static type, and such an alias is not expanded, unless every name in
  the declared type is builtin, a type parameter of the declaration, or imported from that same
  module (a name the importer declares, imports from elsewhere or does not know could mean
  another type). Where it cannot, the type
  checker reports E042 at the call (``cannot check the effects of `get`: the type of its
  receiver is not declared; annotate the binding``).

**Systems.** In component property expressions every resource name is a value: a
`TcpListener` resource of type `TcpListener`, a `Store<T>` resource of type `Store<T>`
(`application = HttpApp { state = todos, handle }`). A `Client` naming a protocol without
`connect`, or a `Service` naming one without `serve`, was E030 (`ignite-check`) until slice 10
retired both kinds (section 14.5).

**Placeholders for T2.** Every `unimplemented!("slice 7: …")` arm T1 added in
`crates/ignite-rustgen` and `crates/ignite-llvm` so the workspace keeps compiling:

- `crates/ignite-llvm/src/layout.rs`, `TypeEnv::layout`: `Ty::Param`, `Ty::GenericRecord`,
  `Ty::GenericEnum`, `Ty::Store` (`Ty::EffectFn` is implemented: 8 bytes, like `Ty::Fn`).
- `crates/ignite-llvm/src/layout.rs`, `mangle`: `Ty::Param` (the other new variants are
  implemented: `Store<T>`, `module.Name<A,B>`, `EffectFn` like `Fn`).
- `crates/ignite-llvm/src/lower.rs`, `builtin`: `ListMap`, `IntParse`, `ByteToInt`,
  `IntToByte`, `TcpStreamSetIdleTimeout`, `StoreNextId`, `StoreGet`, `StorePut`,
  `StoreRemove`, `StoreValues`, `StoreUpdate` (one arm; applied by the substrate agent).
- `crates/ignite-llvm/src/region.rs`, `mutates`: decided, not a placeholder: `StoreNextId`,
  `StorePut`, `StoreRemove`, `StoreUpdate` mutate; the other new builtins do not.

`crates/ignite-rustgen` compiles unchanged: its matches have catch-all arms, so it has no
placeholder, and a program using a slice-7 feature fails there at the catch-all (T2).

## 12 Slice 8: tests

Tests are a primitive of the language, written inline next to the code they test, like Rust's
`#[test]`. This section is the contract between the frontend (U1: `ignite-syntax`,
`ignite-check`, `ignite-types`), the substrate, backends and driver (U2: `ignite-io`, `ignite-rt`,
`ignite-rustgen`, `ignite-llvm`, `crates/ignite`) and the inline tests of std and the examples
(U3). Sections 2 to 5 and 8 include it.

### 12.1 Syntax

```
decl     := ... | test | 'test' (fn | record | enum | const)
test     := 'test' STRING block
stmt     := ... | 'assert' expr (',' expr)?
```

- `test "split_once splits at the first separator" { .. }` is a top-level declaration, allowed
  in any module, program or std. The name is a plain string literal: empty or interpolated is
  E053. Its escapes are decoded (`TestDecl.name.name`).
- `test fn`, `test type` (a record), `test enum` and `test const` declare test-only items.
  `test type N = T` (an alias) is E001: an alias is no code and needs no marker.
- `assert cond` and `assert cond, message` are statements, allowed in every block, in tests and
  outside. `message` is any `Text` expression, usually an interpolated string.
- `test` and `assert` are keywords. Like every keyword they still work as package and import
  path segments, property names and record-literal field names.

AST (add-only): `Decl::Test(TestDecl { name: Ident, body: Block, span })`, where `name.name`
is the decoded text and `name.span` the string literal's span; `test_only: bool` on `FnDecl`,
`RecordDecl`, `EnumDecl` and `ConstDecl`; `StmtKind::Assert { cond: Expr, message:
Option<Expr> }`. `Decl::is_test_only()` is true for `Decl::Test` and every item marked `test`.
`Decl::name()` of a test is its name text, which is not an identifier: code that collects
module-level names skips `Decl::Test`.

### 12.2 Names and visibility

- Tests take no part in the module namespace. Two tests of one module with the same name are
  E050; a test may share its name with any fn or type.
- Test-only items share the module namespace like any declaration (E004 on a clash).
- A test-only item is in scope only inside tests and other test-only items of its own module.
  Anywhere else naming it is E051: as a value, call, UFCS method, type, record literal,
  variant (of a test-only enum) or pattern. Test-only items are not exported: importing one is
  E051 too.
- So normal code can never reach a test-only item, and dropping all of them in a normal build
  is safe.

### 12.3 Typing

- A test body is typed like the body of `fn() -> Unit` with no parameters. `return` without a
  value ends it (the test passes).
- `assert cond`: `cond` must be `Bool`, E052 otherwise. `message` must be `Text` (E040).
- `assert a == b` and `assert a != b`, with the comparison at the top of the condition: both
  sides have one type `T` (E040 otherwise), typed like the operands of `==`. `T` may be any
  type with structural equality, not only the `==` types of section 6: `Int`, `Byte`, `Bool`,
  `Text`, `Unit`, `Input`, `IoError`, and `List`, `Option`, `Result`, tuples, records and enums
  (generic ones too) built only of such types. Two `IoError`s are equal when their messages are
  (the exit code is not compared), on both targets and in the evaluator, so `assert port("x")
  == Err(IoError.new("bad port"))` checks. A `T` holding a fn, `TcpStream`, `TcpListener`,
  `Store` or a type parameter is E045. Outside an assert `==` is unchanged (section 13.5: E045
  on any type holding an `IoError`). `assert (a == b) && c` is an ordinary `Bool` condition.
- `?` anywhere in a test body takes any `Result<T, E>` or `Option<T>` (E040 for another type)
  and has type `T`. An `Err` or `None` fails the test; it does not return. `?` in fns is
  unchanged.
- A test is an effect root (section 5): its effect set is inferred and never checked (no
  E016), the runner grants every ambient effect, and the body may do IO and `spawn`.
- `Store.new()` (builtin `StoreNew`) is a new, empty `Store<T>`, `T` from the expected type
  like `List.empty()` (E042 without one, E040 for another type). It is allowed only in tests
  and test-only items (E051 elsewhere): a program's stores stay system resources. Such a store is
  never freed; the test's process ends. It has no roots (section 5.1), so its methods need no
  effect. Rust: `ignite_io::Store::new()`; LLVM: `ignite_rt_store_new` with the glue of `T`.
- A value is shown by its `to_text`: the builtin one of `Int`, `Byte`, `Bool`, `Text` and
  `Bytes`; the message of an `IoError`; else a pure, non-generic module-level fn
  `to_text(value: T) -> Text` in scope (own or imported; a test-only one only inside tests).
  A value with none of these is shown as `<T>`, the type in Ignite syntax (`<Option<(Text,
  Text)>>`).

### 12.4 Typed API (add-only)

| item | meaning |
|---|---|
| `TypedProgram::tests() -> &[TestInfo]` | the module's tests in declaration order |
| `TestInfo { module, name, full_name, decl }` | `full_name` is `"<module>/<name>"` (`std.text/split_once splits ..`, `main/..` for a program without `package`); `decl` is the index of the `Decl::Test` in `Program::decls` |
| `TypedProgram::assert_compare(cond) -> Option<&AssertCompare>` | for the condition of `assert a == b` / `a != b`: `AssertCompare { operand: Ty, show: Option<Show> }`. `None` for every other condition |
| `TypedProgram::test_try(expr) -> Option<&TestTry>` | for each `?` (`ExprKind::Try`) in a test body: `TestTry { error: Option<Ty>, show: Option<Show> }`; `error` is `E` of a `Result`, `None` for an `Option`. `None` for a `?` in a fn |
| `Show::Builtin(b)` | call the builtin `to_text` `b` (`IntToTextMethod`, `ByteToText`, `BoolToText`, `TextToText`, `BytesToText`) |
| `Show::Fn(q)` | call the fn `q` with the value |
| `Show::IoErrorMessage` | the `IoError`'s message |

`type_of` covers every expression of a test body and of an assert, including the condition and
the message. A test-only item gets types and signatures like any other (`exports()` lists it;
no importer can name it).

### 12.5 Semantics of `assert` and `?`

`assert` evaluates `cond` once; for `a == b` and `a != b` it evaluates `a`, then `b`, each
once, and compares them structurally (field by field, variant and payload, element by
element). When the condition is false it writes to stderr, flushes stdout, restores the
terminal and ends the process with exit code 101, in a test and outside one alike:

```
assertion failed: <source text of cond>
  message: <message>
  left: <left shown>
  right: <right shown>
  at <path>:<line>:<col>
```

`message:` only with a message; `left:` and `right:` only for `==` and `!=`. `<source text
of cond>` is the source between `cond.span.start` and `cond.span.end`, unchanged. `<line>:<col>`
is the position of the `assert` keyword (`Stmt.span.start`, `SourceFile::line_col`). `<path>`
is the `SourceFile.path` the driver read the module from. The backends get both from the
driver; the frontend records neither.

A `?` in a test body that meets `Err(e)` or `None` writes, flushes, restores and exits 101 the
same way:

```
`?` failed on Err: <e shown>
  at <path>:<line>:<col>
```

or `` `?` failed on None`` for an `Option`; `<line>:<col>` is the position of the `?`
character (`expr.span.end - 1` of the `Try` expression).

A panic (`match` not exhaustive, index out of range) in a test exits 101 as before. Its
message on stderr is the target's own: `panic: index out of bounds: the len is 2 but the
index is 5` (Rust), `panic: index out of range` (LLVM). Only the exit status is the same.

Both targets end the process through the substrate's `assert_failed` and `try_failed`
(`ignite_io`, section 7; the LLVM target through `ignite_rt_assert_failed` and
`ignite_rt_try_failed`), which write the reports above and exit 101.

On the Rust target `a == b` in an assert compares `Int`, `Byte`, `Bool`, `Text` and `Unit`
with Rust's `==` and every other `T` by its `Debug` text (`format!("{:?}", ..)` of both
sides). The derived `Debug` of generated types and of `Vec`, `Option`, `Result` and tuples is
structural, so this is structural equality; generated types need no `PartialEq`. The LLVM
target compares field by field.

### 12.6 Builds

- `ignite check` types tests and test-only items of the file and of every std module it
  loads; they cannot rot. The driver needs no change for this: `check_module` and
  `type_module` cover them. `check_with_std` also runs the Rust generator, which drops
  test-only declarations (done by U1 in `crates/ignite-rustgen/src/lib.rs`, `module_items`), so
  `ignite check` on a file with tests works now; it reaches the `assert` placeholder below only
  for an `assert` outside tests until U2 lowers it.
- `ignite build` and `ignite emit` drop every declaration with `is_test_only()` after checking.
  Generated Rust, IR, goldens, `terminald` and `tn` stay byte-identical.
- A test build includes the tests and test-only items of the files named on the command line
  only; every other module (the std modules a program loads) is built normally, with its tests
  dropped.

### 12.7 Runner ABI (U2)

A test build produces test executables (Rust: a `[[bin]] ignite-test`; LLVM: a main over a
table of fn pointers), whose main is `ignite_io::test_main(tests)` (LLVM:
`ignite_rt_test_main(table, count)`, forwarding to it). Since slice 9b every file named shares
one test executable per target (`vision.md` invariant 9: one compile and one link, not
one per file): the std modules named and every program, each program in a namespace of its
own. Rust: one crate `ignite-tests` whose `lib.rs` holds each program as the module of its
module path (`examples.todo` is `pub mod examples { pub mod todo { .. } }`), beside the
`ignite-std` crate; generator API `ignite_rustgen::generate_tests`. LLVM: every program in one
`program.ll`, separated by the module path that prefixes every symbol (`"<module>.<name>"`);
`ignite_llvm::generate_tests`. The one exception: two programs with one module path (two files
without a `package` line are both `main`) cannot share an executable, because the runner names
a test `<module>/<name>`; the later one goes into a further executable (directories `tests`,
`tests-2`, … under `--out`). An executable holds its modules' tests in the command-line order
of their files, then declaration order. `ignite test` runs the executables one after another;
each prints its own report (so one `test result:` line per executable), and `ignite test`
exits with the largest status of them.

The LLVM target links a runtime archive that carries `ignite_rt_test_main`: `ignite-rt` built
with `--features runner` (`cargo build -p ignite-rt --features runner`), passed with
`--runtime`. `task test-ignite` builds it into `<target dir>/ignite-test/rt` (`target`, or
`$CARGO_TARGET_DIR` when set) and runs every file of
`std/` and `examples/` on both targets, except `*.design.ignite` (a declaration with no tests,
checked by `cli::check_design_file_is_silent` in `crates/ignite`), which the task's file list
leaves out.

- `<exe> --run-one <full name>`: run that one test in this process under the substrate's
  `run`: exit 0 when the body returns; 101 on an assert failure, a failed `?` or a panic; the
  code passed to `Process.exit` when the body calls it. Spawned tasks end with the process.
  An unknown name exits 2 with `no test named <full name>` on stderr.
- `<exe> [FILTER] [--exact] [--list] [--jobs N] [--timeout SECS]`: the runner. It selects the
  tests whose full name contains `FILTER` (equals it with `--exact`; all without a filter) and
  runs each as a child process `<exe> --run-one <full name>`, at most `N` at a time (default:
  the CPU count), each with a timeout (default 60 s; the child is killed when it expires).
  It captures each child's stdout and stderr and shows them only for failures.
- Per test: exit 0 passes; 101 fails; a timeout fails as `timed out after <SECS> s`; any other
  code `N` fails as `exited with code N`; death by a signal fails as `killed by signal N`.

`ignite test FILE.. [--target rust|llvm] [--release] [--triple TRIPLE] [--std DIR] [--out DIR] [FILTER] [--exact] [--list]
[--jobs N] [--timeout SECS]` builds the test executable for the files and runs it with the
runner flags. Its exit code: 0 when every selected test passed or none exist (or with
`--list`), 1 when a test failed, 2 for a compile error (diagnostics as for `ignite check`). `--release` builds the test executable with the release profile (Rust) or clang `-O2` (LLVM; dev builds use `-O0`); `--triple` pins the LLVM target triple, as for `ignite build --triple`, and a runtime archive for another triple is refused with exit 3 before clang runs (slice 9c).

Output on stdout, modelled on `cargo test`:

```
running 3 tests
test std.text/split_once splits at the first separator ... ok
test std.text/trim drops spaces ... FAILED
test std.text/slow ... FAILED (timed out after 60 s)

failures:

---- std.text/trim drops spaces stdout ----
<captured stdout>
---- std.text/trim drops spaces stderr ----
assertion failed: trim(" a ") == "a "
  left: a
  right: a 
  at std/text.ignite:120:3

failures:
    std.text/trim drops spaces
    std.text/slow

test result: FAILED. 1 passed; 2 failed; 4 filtered out; finished in 0.12s
```

- `running N tests` counts the selected tests (`1 test` for one). Result lines come in
  module and declaration order, each printed once it and every test before it have finished,
  so both targets print the same lines apart from the time.
- A result is `ok`, `FAILED`, or `FAILED (<reason>)` with a reason from the list above.
- The failures block appears only when a test failed. Each failed test gets its stdout and
  stderr sections, each only when that stream is not empty, then the list of failed names.
- The summary is `test result: ok.` or `test result: FAILED.`, then `P passed; F failed; K
  filtered out; finished in S.SSs`. With no tests: `running 0 tests`, a blank line and
  `test result: ok. 0 passed; 0 failed; 0 filtered out; finished in 0.00s`.
- `--list` prints `<full name>: test` per selected test, then `N tests` (`1 test`), and runs
  nothing.

### 12.8 Diagnostics

| code | stage | meaning |
|---|---|---|
| E050 | `ignite-check` | two tests of one module have the same name |
| E051 | `ignite-check` (a UFCS call: `ignite-types`) | code that is not a test or test-only item uses a test-only item or `Store.new()`, or an import names a test-only item |
| E052 | `ignite-types` | the condition of `assert` is not `Bool` |
| E053 | `ignite-syntax` | a test name is empty or interpolated |

### 12.9 Placeholders for U2

Every `unimplemented!("slice 8: …")` arm U1 added so the workspace builds. U2 implemented all
of them; none is left:

- `crates/ignite-rustgen/src/lib.rs`, `Generator::stmt`: `StmtKind::Assert` (lowering).
- `crates/ignite-rustgen/src/lib.rs`, `mutated_names`: `StmtKind::Assert`.
- `crates/ignite-llvm/src/lib.rs`, `lower_decls`: `Decl::Test`.
- `crates/ignite-llvm/src/lower.rs`, `Lower::stmt`: `StmtKind::Assert`.
- `crates/ignite-llvm/src/region.rs`, `block`: `StmtKind::Assert`.

Not a placeholder: `crates/ignite-rustgen/src/lib.rs`, `module_items` skips every declaration
with `is_test_only()` (the normal build of 12.6). The LLVM decl loop and every other
place that iterates `Program::decls` must do the same in normal builds (U2).
`crates/ignite-rustgen/tests/units.rs` builds AST nodes with `test_only: false`.

## 13 Slice 9: friction fixes

The coordinator's decisions of 2026-11-01 for the 17 rows of `docs/friction.md`, as built.
N1 (frontend: `ignite-syntax`, `ignite-check`, `ignite-types`) settled this section; N2 (Rust
target) and N3 (LLVM target and runtime) build against it. The slice-6 to slice-8 rules stay
unless a row below changes them.

### 13.1 Rows

| row | syntax | typing and checking | lowering obligation |
|---|---|---|---|
| 1 | `"a {f("x")} b"`: an interpolation holds any expression, string literals (themselves interpolated, raw or byte) included; still no line break inside `{ }` | nesting deeper than 32 string literals is E054 | none: the parser desugars as before |
| 2 | `r"…"`, `r#"…"#` (any number of `#`, closed by `"` and as many `#`), `br"…"` | no escapes, no interpolation, line breaks kept; a raw byte string is ASCII (E001) | none: `ExprKind::Str` and the byte-string desugaring |
| 3 | none | `[]`, `List.empty()`, `Store.new()`, `None`, `Ok(x)` and `Err(e)` bound by a `let`/`var` without annotation take the type later uses in the same body give them: an argument, a `return`, an assignment, a pushed element, any position with an expected type. The typer walks such a body once more with inference holes and then types the `let` as if annotated; E042 only when neither the expected type nor a use decides | none: every `Ty` the backends read is resolved |
| 4 | `break` and `continue` in expression position (`None => break`) | `ExprKind::Break`/`Continue`, type `Never`; `break` marks the innermost loop as broken | as the statements |
| 5 | none | `x.f(..)` is the builtin `f` when it applies to `x`'s type, else a UFCS call of the user fn `f` (`Callee::Fn`). A user fn may share the name of a builtin method of its first parameter's type: method syntax on that type calls the builtin, a plain call `f(x, ..)` the user fn (case-study gap G14; until 0.13 it was E041 at the declaration) | none: `callee()` says which |
| 6 | `import std.text.*` | every non-test public item of the module; an explicit import and an own declaration win; a name two globs provide is E041 at its use | `Import::is_glob()`, `Import::module()`; a Rust `use` of every item (or of the names `Symbols::imported` lists from that module) |
| 7 | none | `==`/`!=` on every comparable type (13.5); E045 otherwise | general equality glue, the slice-8 assert glue generalised |
| 8 | `b'0'..=b'9' => …`, `LIMIT..=99` | 13.7 | `range_bounds(pattern)`: `lo <= v && v <= hi` |
| 9, 13 | `\|x, y\| expr`, `\|x\| { … }`, `\|x: Int\| …`, `\|\| e` | 13.3 | 13.3 |
| 9 | `Todo { done = flag, ..t }` | `ExprKind::RecordUpdate`: `base` has the record's type, each field its field type; E047 for an unknown or repeated field | a copy of `base` with the fields replaced; fields evaluated in source order, then `base` |
| 10 | `t.0`, `t.1` | `ExprKind::TupleField`; an index past the tuple is E041 | the element |
| 11 | `x.debug()` | `Callee::Builtin(Builtin::Debug)`, `Text`, on every type without a type parameter anywhere inside (`List<T>` in a generic fn has none: E041); `Show::Debug` likewise | 13.6; a failed `assert a == b` shows both values by `AssertCompare.show`, which is `Show::Debug` where no `to_text` applies |
| 12 | `"\u{1F600}"` | 1 to 6 hex digits, a Unicode scalar value (E001 otherwise); Text literals only | none |
| 14 | a left-deep chain of 100,000 `+` | checker and typer walk the left spine of `Binary` iteratively | every backend pass must too, or prove its depth bound |
| 15 | `enum Tree<T> { Leaf, Node(T, Tree<T>) }`, `type Node { next: Option<Node> }` | E055 only for a record holding itself through records and tuples alone | 13.4 |
| 16 | none | none | generic instances emitted once per program (`linkonce_odr` or one module) |
| 17 | `fn Response.json(status: Int, body: Json) -> Response = …` | 13.8 | 13.8 |

### 13.2 AST and typed API (add-only)

| item | meaning |
|---|---|
| `ExprKind::Closure(Closure { params: Vec<ClosureParam { name, ty: Option<Type>, span }>, body: Box<Expr> })` | a closure literal |
| `ExprKind::RecordUpdate { name, fields, base }` | `Name { f = e, ..base }` |
| `ExprKind::TupleField { receiver, index: usize }` | `receiver.index` |
| `ExprKind::Break`, `ExprKind::Continue` | in expression position; statement position stays `StmtKind::Break`/`Continue` |
| `PatternKind::Range { start, end }` | bounds are `PatternKind::Int` or a payload-less `PatternKind::Variant` naming a constant |
| `FnDecl.owner: Option<Ident>`, `FnDecl::qualified_name()` | `Some(Response)` for `fn Response.json`; `qualified_name()` is `Response.json` |
| `Import::is_glob()`, `Import::module()` | `import a.b.*` is `path` `a.b.*`, `names` empty |
| `TokenKind::DotDot`, `TokenKind::DotDotEq` | `..`, `..=` |
| `TypedProgram::closure(expr) -> Option<&ClosureInfo>` | `ClosureInfo { params: Vec<(String, Ty)>, ret, captures: Vec<Capture>, effects: Option<FnEffects> }` |
| `Capture { name, binding: Span, ty, effects: Vec<String> }` | `binding` is the captured binding's name span (the key of `binding_type`); `effects` are the closure's effects whose head is `name` |
| `TypedProgram::equality(&Ty) -> Option<Equality>` | `Scalar`, `Text`, `Structural`, or `None` when not comparable (13.5) |
| `TypedProgram::boxed_payload(&QualName, variant, index) -> bool`, `boxed_field(&QualName, field) -> bool` | 13.4 |
| `TypedProgram::range_bounds(pattern) -> Option<(i64, i64)>` | both bounds of a range pattern, inclusive, constants evaluated |
| `TypedProgram::place_owner(&Place) -> Option<&Ty>` | the record type that owns the last field of an assignment target; the Rust target boxes the assigned value when `boxed_field` holds for that owner (13.4) |
| `Builtin::Debug`, `Show::Debug` | 13.6 |
| `ModuleTypes.const_values: BTreeMap<String, i64>` | values of `Int` and `Byte` constants whose value is a literal, a negated literal or such a constant |
| `Symbols.closure_effects: BTreeMap<Span, EffectSig>`, `Symbols.glob_conflicts` | checker results the typer reads |
| `ignite_types::{equality, boxed_payload, boxed_field}(.., &TypedModules)` | the same answers as the `TypedProgram` methods, for a layout pass that holds only the modules |
| `ignite_check::AMBIENT_HEADS` | `stdin`, `stdout`, `process`, `net`: the heads of ambient effects (E056) |
| `impl Drop for Expr` | drops binary and unary chains from a work list; an owned `Expr` cannot be destructured by move (match on `&e.kind`) |

### 13.3 Closures

**Typing.** A parameter's type is its annotation, else the expected fn type's parameter type;
neither is E042. A closure passed where the parameter is a fn type with an open return (the
builtin `map`'s `fn(T) -> U`, a generic `fn(T) -> U` whose `T` an earlier argument decided)
gets that fn type's closed parameter types, and its synthesised result decides `U`:
`names.map(|n| prefix + n)` needs no annotation. The body is checked against the expected
return type, else synthesised (a `return e` inside a closure without an expected return
type is checked against `Unit`).
`return`, `?`, `break` and `continue` inside the body belong to the closure: `return` leaves
the closure, a loop of the enclosing fn is not visible, and a `?` in a closure inside a test
body returns from the closure. The closure's type is `Ty::Fn(params, ret)`, or
`Ty::EffectFn(params, ret, effects)` when its body performs an effectful operation.

**Captures.** A closure captures every binding of an enclosing scope its body reads (a local,
a parameter, a pattern or loop binding; never a module-level fn or constant), in order of
first use, including those only a nested closure reads. Captures are **by copy**: the value at
creation; a later assignment to the outer binding does not change the closure. Assigning to a
captured binding, or `push`/`append`/`clear` on one, inside the closure is E006.

**Effects.** `ignite-check` walks the body as a fn without `requires`, parameters rooted in
themselves. Each effect is `p.op` on a parameter `p`, `r.op` on a root `r` of a captured
value (the enclosing fn's parameter it comes from), or ambient (`stdout.write`). Every effect
that is not on a parameter is also needed where the closure is created: a closure carries the
authority it captured, and its creator must hold it. `ClosureInfo.effects` lists all of them
(`FnEffects.names` are the closure's parameter names). An effectful closure where a pure fn
type is expected, or where an effectful fn type does not cover its effects, is E017. A
`let f = |..| ..` binding of an effectful closure may be called; the call needs the effects on
the arguments' roots plus the others (`Symbols::effect_sites` lists it).

**Rust target (N2).** Every fn value, closure or named fn, is
`std::sync::Arc<dyn Fn(P1, ..) -> R + Send + Sync>` (generics where cheaper and equivalent). A
closure lowers to `Arc::new({ let c1 = c1.clone(); .. move |p1, ..| body })`, cloning each
capture at creation. A named fn used as a value is wrapped the same way.
A closure literal passed straight to `Store.update`, `Store.upsert` or `List.map`, which call
it before they return, lowers to a plain `|p1, ..| body` that borrows its captures: no `Arc`,
no clone per capture, and the same values, since no binding of the enclosing fn changes during
the call.

**LLVM target (N3).** A fn value is a pair `{ ptr code, ptr env }`. `code` takes `env` first,
then the parameters. `env` points to a struct of the captures in `ClosureInfo.captures` order,
copied at creation; copying a fn value runs the env's copy glue (each capture's copy glue). A
named fn used as a value has `env` null and a thunk that ignores it.

### 13.4 Boxing

A record field or an enum payload position is **boxed** when its declared type holds a value
of the record or enum that owns the position inline: through records, tuples, `Option`,
`Result` and enums (type arguments substituted), not through `List`, `Store` or a fn type.
`boxed_field`/`boxed_payload` answer per position, for every instance of a generic
declaration alike. Examples: `Tree.Node` payload 1 in `enum Tree<T> { Leaf, Node(T, Tree<T>) }`;
field `next` in `type Node { value: Int, next: Option<Node> }` (the whole `Option<Node>` is
boxed); both `A.b` and `B.X` payload 0 for `type A { b: B }`, `enum B { X(A), Y }`.

Rust: the position's type is `Box<T>`; constructing boxes, matching and reading deref.
LLVM: the position holds a pointer to a heap copy; copy glue copies the pointee, equality and
debug glue follow the pointer. A record that holds itself through records and tuples alone is
E055 and never reaches a backend.

### 13.5 Equality

`==` and `!=` take two operands of one type; the type is **comparable** when it holds no fn,
`TcpStream`, `TcpListener`, `IoError`, `Store` or type parameter anywhere inside (fields,
payloads, elements, type arguments; a recursive type is assumed comparable where it recurs).
Otherwise E045. The operand type is `type_of(left)`, or `type_of(right)` when the left is
`Never`. Both operands are evaluated, left first, each once.

| `equality(ty)` | types | equal when |
|---|---|---|
| `Scalar` | `Int`, `Byte`, `Bool`, `Unit` | same value |
| `Text` | `Text` | same bytes |
| `Structural` | `List` (`Bytes` too), `Option`, `Result`, `Input`, tuples, records, enums | lists: same length and elements pairwise; tuples and records: every element or field in order; enums: same variant and payloads pairwise; boxed positions compare the pointees |

`!=` is the negation. Constant patterns stay limited to `Int`, `Byte`, `Bool` and `Text`.

### 13.6 Debug text

`x.debug()` and a failed `assert` with `Show::Debug` produce this text, the same on both
targets, nested values formatted by the same rules:

| type | text |
|---|---|
| `Int`, `Byte` | decimal, `-` for a negative `Int`: `-3`, `255` |
| `Bool` | `true`, `false` |
| `Unit` | `()` |
| `Text` | in `"`; `"` as `\"`, `\` as `\\`, newline `\n`, carriage return `\r`, tab `\t`, NUL `\0`, any other character below U+0020 and U+007F as `\u{hex}` (lowercase, no padding: `\u{1b}`); every other character as itself |
| `List<T>`, `Bytes` | `[a, b]`, `[]` |
| tuple | `(a, b)` |
| `Option` | `Some(x)`, `None` |
| `Result` | `Ok(x)`, `Err(e)` |
| `Input` | `Data([104, 105])`, `Eof` |
| record | `Todo { id: 1, title: "milk", done: false }`: the record's unqualified name, no type arguments, fields in declaration order; `Empty {}` without fields |
| user enum | `Command.Quit`, `Command.Echo("x")`: unqualified enum name, `.`, variant, payloads in `( )` |
| `IoError` | `IoError("connection refused")`, the message as `Text` |
| fn value | `<fn>` |
| `TcpStream`, `TcpListener`, `Store<T>` | `<TcpStream>`, `<TcpListener>`, `<Store>` |

### 13.7 Range patterns

`lo..=hi` matches a value `v` of type `Int` or `Byte` with `lo <= v <= hi`. Each bound is an
integer literal (`-5`, `0x7F`), a byte literal (`b'a'`, an integer) or a constant of the
scrutinee's type whose value is a literal, a negated literal or such a constant
(`ModuleTypes.const_values`); anything else is E045. A range on another type is E040, a bound
out of `Byte` range E044, `lo > hi` E043. Exhaustiveness splits `Int` and `Byte` columns into
intervals: a `match` on `Byte` covering `0..=127` and `128..=255` needs no `_`; an uncovered
interval is shown as `lo..=hi`.

### 13.8 Associated fns

`fn Owner.name(..)` declares a fn of the record or enum `Owner`, which the same module must
declare (E003 otherwise); a second `Owner.name` or a variant `Owner.name` is E004. It is called
`Owner.name(..)` (a path call) and may be used as a value `Owner.name`. Its `QualName` is
`{ module, name: "Owner.name" }` and its key in `ModuleTypes.fns`, `Symbols.functions` and
`Symbols.effects` is `Owner.name`; the call's callee is `Callee::Fn` of that `QualName`.
Importing `Owner` (explicitly or by glob) brings its associated fns. Backends turn the `.` into
a valid symbol of their own (Rust: an inherent `impl Owner { fn name }` or a mangled free fn;
LLVM: a mangled symbol); a free fn `name` of the same module is a different fn. The Rust
target's symbol writes every `_` of owner and name as `_0` and joins them with `__`
(`fn A_._b` is `A_0___0b`, `fn A.b__c` is `A__b_0_0c`); every other Ignite name that contains
`__` gets a trailing `_` in Rust (a free `fn A__b` is `A__b_`), so no two fns share a symbol
(story `rustgen-assoc-symbol-collisions`).

### 13.9 Diagnostics changed in slice 9

| code | change |
|---|---|
| E006 | also: assignment to, or `push`/`append`/`clear` on, a captured binding inside a closure; `push`/`append`/`clear` through a `let`, a `for` element or a pattern binding, or on `x.to_bytes()` (13.11) |
| E016 | also: a closure's effects on captures and ambient effects, charged where it is created; an effect reached through `map`, `push` or `append` on a field or index, or through a call of a local fn value (13.11) |
| E046 | `Int` and `Byte` columns with a range pattern split into intervals (13.7) |
| E056 | new: a parameter named like an ambient capability (13.11) |
| E057 | new: an unused type parameter of a generic record, enum or alias (13.11) |
| E017 | also: an effectful closure where a pure fn type, or an effectful fn type not covering its effects, is expected |
| E041 | also: a name two glob imports provide; a tuple index past the tuple |
| E042 | also: a closure parameter without annotation or expected type; no longer for a generic value later uses decide (row 3) |
| E043 | also: a range pattern whose start is above its end |
| E045 | `==`/`!=` on a type that is not comparable; a range bound that is neither a literal nor a constant with a literal value |
| E054 | also: string literals nested deeper than 32 interpolations |
| E055 | narrowed: only a record holding itself through records and tuples alone |
| E003, E004 | also: an associated fn whose owner is no record or enum of the module; one declared twice or named like a variant of its owner |

### 13.10 Placeholders for N2 and N3

Every `unimplemented!("slice 9: …")` arm N1 added so the workspace builds:

- `crates/ignite-rustgen/src/lib.rs`: three `match &expr.kind` (closures, record update, tuple
  fields, `break`/`continue` expressions), four `match &pattern.kind` (range patterns), one
  `match show` (`Show::Debug`).
- `crates/ignite-llvm/src/lower.rs`: `Lower::expr` (`match &e.kind`), `Lower::pattern` (range
  patterns), the show match (`Show::Debug`), the builtin match (`Builtin::Debug`).
- `crates/ignite-llvm/src/region.rs`: the builtin match (`Builtin::Debug`), the expression
  walk (`match &expr.kind`).

`grep -rn 'slice 9:' crates/ignite-rustgen crates/ignite-llvm` lists them. Not placeholders but
gaps every backend has to close: `FnDecl.owner` (13.8), glob imports (row 6), boxing (13.4),
equality on every comparable type (13.5), and `Builtin::Debug` in the Rust target's builtin
lowering if it matches without a wildcard. `crates/ignite-rustgen/tests/units.rs` builds
`FnDecl` with `owner: None`.

### 13.11 Soundness fixes of wave A0 (frontend)

From `vision.md` sections 10.1 and 10.2. The probes of `docs/audit/2026-11-01/` are copied
to `crates/ignite-types/tests/audit/` with their expected diagnostics (the first failing stage,
empty for a probe that must check clean); `effects_closure_variants.ignite` repeats the effect
probes with the handle captured by a closure instead of passed as an argument.

| item | rule | probes |
|---|---|---|
| a | `push`, `append` and `clear` (the builtins) need a receiver place rooted in a `var` or a parameter (a changed parameter is the callee's copy): through a `let`, a `for` element or a pattern binding it is E006, and so is a receiver `x.to_bytes()`, which shares `x`'s buffer on the LLVM target | `llvm/let_push`, `for_push`, `for_push2`, `match_push`, `match_push2`, `to_bytes_push` |
| b | a parameter of a fn, a role, a fn type or a closure may not be named `stdin`, `stdout`, `process` or `net` (E056), so a `requires` head names a parameter only when the fn declares one of that name, and an ambient effect is never charged to an argument's roots | `effects/a01` to `a04`, `b08`, `c01` |
| c | the roots of `l.map(f)` are the roots of `l` plus the roots of `f`; a closure's roots are those of its body (its captures), and a call of a local fn value `g(..)` has the roots of `g` plus its arguments' | `effects/a05`, `a06`, `b09`, `b11` |
| d | `push` and `append` on a field, index or tuple-field receiver extend the roots of the receiver's place root (`h.items.push(s)` roots `h` in `s`), and the call changes that element in place on every target and in the evaluator: `lists[0].push(9)` on `var lists = [[1], [2]]` gives `[[1, 9], [2]]`, also through a field of an indexed element, a nested index and for `clear`, `put` and `remove` (slice 11b; the Rust target goes through `ignite_io::checked::at_mut`) | `effects/a07`, `b10`; `fixtures/tests/targets.ignite` row 32 |
| e | a type parameter of a generic record, enum or alias that no field, payload or aliased type uses is E057 | `io/p2_unused` |

Every other effects probe keeps the diagnostic `vision.md` section 10.2 names. `io/p1`
now checks clean: row 15 allows recursion through `Option` and enum payloads.

## 14 Slice 10: the system declaration (F1)

Contract reviewed and accepted by the steering session on 2026-11-01. Sources: vision 4.1
row 10, T1, T4, T5, T6, section 6 (X5, X21, X22), 10.3 (R2 to R5); design 3.2, 3.3, 4, 5,
11.3; `examples/terminal/terminal.design.ignite`. Decisions O1 to O12 are recorded in 14.13.

### 14.0 Acceptance

1. `ignite check examples/terminal/terminal.design.ignite` exits 0. Edits to that file are
   allowed only as listed in O5, mirrored in design §11.3 in the same change.
2. `examples/terminal/terminal.ignite` and `examples/todo/todo.ignite` are in the component form
   (14.4). `ignite build` emits `terminald`, `tn` and `todod`. The terminal suite (64 cases
   across both targets), the todo suite and the 78 inline tests per target pass unchanged.
3. T1: a failure ends its task. `terminald` survives a session whose handler fails an
   `assert`, overflows or indexes out of range, on both targets. Every failure report names
   the Ignite file, line, column and operation, identical on both targets.
4. R2 to R5 are fixed, and `docs/audit/2026-11-01/io/iotest` `drop` and `cross` behave as the
   audit README's "expected after the fix" column says.
5. Invariant 9: the `task gate` wall time and the `todod` build time (both targets) are
   measured before and after, and recorded in the `AGENTS.md` "Gate time" section. A rise of
   more than 10 % is a deviation and has to be explained.
6. Every new diagnostic (E100 to E109) has a fixture.

### 14.1 Quantity literals and quantity types

- Syntax: an Int literal or a decimal literal, then a unit name on the same line:
  `4096 bytes`, `16 KiB`, `15 minutes`, `100 ms`, `99.9%`.
  - A unit name is an identifier from the table below, not a keyword.
  - The pair binds tighter than any binary operator.
  - A decimal literal (`99.9`) exists only directly before `%` or a unit. There is no
    `Decimal` type in slice 10.

| type | units (singular and plural accepted) | representation |
|---|---|---|
| `Duration` | `ns`, `us`, `ms`, `second(s)`, `minute(s)`, `hour(s)`, `day(s)` | Int nanoseconds |
| `ByteSize` | `byte(s)`, `KiB`, `MiB`, `GiB` (1024-based), `KB`, `MB`, `GB` (1000-based) | Int bytes |
| `Ratio` | `%` | Int parts per million; at most 4 decimals, so `99.9%` is 999,000 |

- The name is `ByteSize`, not design §3.3's `Bytes`, because `Bytes` is `List<Byte>` today.
  Design §3.3 gets a one-line note.
- Operations:
  - same-type `+`, `-` and comparison;
  - `* Int` and `/ Int`;
  - `Duration.to_ms()`, `.to_ns()` and `ByteSize.to_int()`;
  - `Ratio.ppm()`;
  - `debug()` prints the literal form in the largest exact unit.
- Overflow follows T6. A literal out of range is E107.
- A unit applied to the wrong kind of value, or mixed units (`1 ms + 1 bytes`), is E101.
- `Rate<Event>`, `CPU`, `Money` and `Decimal` are not in this slice.

### 14.2 Named arguments (T5 a)

- `f(a, b, name = e)`: positional arguments first, then named ones.
- Allowed only for a direct call of a fn declared in Ignite, including associated fns and
  UFCS. Not through fn values, closures or protocol roles.
- Each named argument must name a parameter that no positional argument covers.
- An unknown name, a duplicate, a positional argument after a named one, or use through a fn
  value is E103.
- Lowering: the arguments are reordered at the call, and evaluation order stays source order
  (each argument goes into a temporary first when any argument has an effect).

### 14.3 Brace literals and `Map<K, V>` (T5 b, c)

- A brace literal `{ k = e, … }` without a type name takes its type from the expected type:
  - a record type makes it a record literal (the slice-9 rules, defaults and `..base`);
  - `Map<K, V>` makes it a map literal.
    - Identifier keys give `K = Text`.
    - Qualified component references (`Terminal.ingress`) give `K = Ref<C>` for a common
      kind, or the kind-erased `Ref` when the kinds differ (O2).
- There is no type named `Any`. `Ref<K>` coerces to `Ref` wherever a `Ref` is expected.
- A brace literal with no expected type is E100.
- `Map<K, V>`: `Map.empty()`, `get(k) -> Option<V>`, `put(k, v)`, `remove(k)`, `keys()`,
  `len()`, `contains(k)`, and `for (k, v) in m` in key order.
  - Value semantics, as for `List`.
  - Structural `==` and debug text are `{k: v, …}` in key order.
- Key types: `Int`, `Byte`, `Text`, `Bool`, enums without payloads (in declaration order), and
  `Ref<C>`. Any other key type is E106 (O11).
- Rust: `std::collections::BTreeMap`. LLVM: a sorted array of pairs with binary search, plus
  copy and drop glue (O11).

### 14.4 System declarations: resources, components, references

The grammar is unchanged from `slice.md` section 3. What changes is what a kind is: a component
kind is an ordinary Ignite record type (D6), declared in std or in the program.

- `component name: K { props }`: `K` is a record type. `props` is a record literal of `K` with
  the slice-9 rules, so fields with defaults may be left out. A field without a default that is
  left out is E104.
- `resource name: K { props }`: in slice 10 the resource kinds stay the two compiler-known
  ones, `TcpListener` and `Store<T>`. `TcpListener` gains `exposure: Exposure`
  (`Loopback | Private | Public`, default `Loopback`) and `backlog: Int` (default 128).
  `exposure = Loopback` with a non-loopback address is E104.
- A qualified enum variant `Type.Variant` (`Exposure.Loopback`) is accepted wherever an enum
  value is expected, when `Type` names an enum. The bare form stays valid.
- Names inside a `system` (O2):
  - A bare resource or component name used in a property expression is a value of its
    declared type, as resources are today. `application = console` passes the console's
    record value.
  - The qualified form `System.name` is a reference, `Ref<K>`, and is used as an assembly key
    and as a probe `target`.
  - A cycle of bare-name uses among components (`a` uses `b`, `b` uses `a`) is E105.
- Output fields (O3): a field of a kind whose type is `Signal<T>` is an output.
  - Setting it in a declaration is E104.
  - `promptProbe.attempts` is a value of type `Signal<T>` that names the component and the
    field.
  - Signals carry no runtime behaviour in slice 10; slice 12 gives them one.
- `module("p")` (O4): its type is `Module<I>`, where `I` is the expected record type whose
  fields are all fn types.
  - `p` names a module of the program's package (`examples.terminal.commands` is
    `examples/terminal/commands.ignite`) or a std module.
  - The literal becomes a record `I` whose fields are filled by name from the module's public
    fns, and their types must match.
  - A module that is not found, or a missing or mismatched fn, is E102.
  - This adds sibling modules of the program's package to the module loader. Package
    manifests and lockfiles stay wave H.

### 14.5 Executable components (O1)

- A kind `K` produces an executable when it has the associated fn
  `fn K.main(self: K) -> Result<(), IoError>`, declared in K's module, with an inferred
  `requires`.
- The generated `main`, on both targets:
  1. runs under `ignite_io::run`;
  2. creates the `Store` resources;
  3. binds each `TcpListener` resource the component uses, with the `--port` override and the
     `listening on` log as today (section 6.1);
  4. builds the component's record value, with resource handles and the values of embedded
     components substituted;
  5. calls `K.main(value)`;
  6. maps an `Err` to its exit code.
- The accept loop and the per-connection `spawn` move from generated code into Ignite:
  - `fn TelnetServer.main` and `fn TelnetClient.main` in `std/protocol/telnet.ignite`;
  - `fn HttpServer.main` in `std/protocol/http.ignite`.

  The compiler-known kinds `Service` and `Client` are removed.
- Exit statuses stay those of section 6.1 (1 to 4). New builtin `IoError.with_exit_code(n)`
  (O12). Bind failure (1) and `--port` errors (2) stay in the generated part.
- `executable` is a field of each executable kind, defaulting to the component name. The
  examples set `executable = "terminald"`, `"tn"` and `"todod"`.
- Components without `K.main` (`CommandConsole`, `TelnetPrompt` and `Application` in slice
  10) produce no executable. They are values, embedded where a bare name uses them.

Example, the buildable terminal (`examples/terminal/terminal.ignite`):

```ignite
system Terminal {
  resource ingress: TcpListener { address = "127.0.0.1", port = 2323 }
  component console: CommandConsole {
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
  }
  component server: TelnetServer { listener = ingress, application = console, executable = "terminald" }
  component client: TelnetClient { target = ingress, executable = "tn" }
}
```

Whether the buildable example keeps its fns in one file (with `module("examples.terminal")`
naming itself) or splits out `commands.ignite` is the implementing agent's choice. Both must
work.

### 14.6 Std types (Ignite, no runtime behaviour beyond 14.5)

| module | types and fns |
|---|---|
| `std/console.ignite` | `CommandConsole { implementation: Module<ConsoleCommands>, prompt, maximum_line: ByteSize, command_schema: Option<Module<CommandSchema>> = None }`, `ConsoleCommands { respond: fn(Text) -> Reply }`; the existing `Console` stays as the runtime value built from it |
| `std/protocol/telnet.ignite` | `TelnetServer`, `TelnetClient`, `TelnetProfile`, `SessionLimits { maximum, idle_timeout: Duration, input_buffer: ByteSize, output_buffer: ByteSize, slow_reader_deadline: Duration }`; `idle_timeout` replaces `IDLE_TIMEOUT_MS` |
| `std/protocol/http.ignite` | `HttpServer { listener, application: HttpApp<S>, executable }` |
| `std/contract.ignite` | `Availability`, `ThresholdCompliance`, `SLA`, `Window` (`rolling(Duration)`, `calendar_month(Text)`), `Consequence` (`event(Text)`), `MissingData` |
| `std/policy.ignite` | `Restart { backoff: Backoff, maximum_attempts, interval: Duration }`, `Backoff` (`exponential(Duration, Duration)`, `fixed(Duration)`), `DrainRollout { deadline: Duration, on_deadline: Text }` |
| `std/probe.ignite` (since moved: `TelnetPrompt` is in `std/protocol/telnet.ignite`, `ProbeAttempt` in `std/sli.ignite`; `docs/layering.md`) | `TelnetPrompt { target: Ref<TelnetServer>, interval, timeout, expected_prompt, close_after_probe, attempts: Signal<ProbeAttempt> }`, `ProbeAttempt` |
| `std/deploy.ignite` | `Deployment { system: SystemRef, bindings: Map<Ref, Binding> }`, `Binding` (`localSocket()`, `nativeProcess(includes: List<Ref>)`, `workstationExecutable()`, `scheduledProbeHost()`) |
| `std/cli.ignite` | `Application { executable, commands: Map<Text, Command> }`, `Command`, `Argument { name, kind: ArgKind }`, `ArgKind` (`endpoint(Protocol)`, `text()`, …) (O6, O9) |

- Every closed-set field that the design writes as a string stays `Text` in slice 10, with no
  exceptions: `good`, `missing_data`, `on_deadline`, `terminal`, `baseline`, `negotiation`,
  `unsupported_options` and `character_encoding`. Slice 12 turns each into an enum when it
  gives it behaviour.

### 14.7 `contract`, `policy`, `assembly` declarations

- `contract name: K { … }` and `policy name: K { … }` are declarations inside a `system`, with
  the same rules as `component` but never executable.
- `assembly name: K { … }` is a top-level declaration whose kind is a record type
  (`Deployment`). `system = Terminal` is a `SystemRef`.
- Every `bindings` key must be a component or resource of that system, otherwise E108.
- `ignite build` does not read assemblies in slice 10 (O8); slice 11 resolves them.
- `system`, `component` and `resource` are keywords already. `contract`, `policy` and
  `assembly` become keywords, which takes the keyword count from 29 to 32. Check `std/` and
  `examples/` for identifiers with these names.

### 14.8 T1: a failure ends its task

- A panic, a failed `assert` or a failed `?` outside a test ends the task that ran it, on
  every target. So does T6 overflow, an index out of range, a division by zero or a
  non-exhaustive match.
- The substrate logs `task ended with error: <report>`.
- The process exits 101 only when the failing task is the main task or a test.
- Report text, identical on both targets: `<operation> failed at <file>:<line>:<column>: <detail>`.
  The main task and a test print it on stderr as `error: <report>`; a spawned task logs it as
  `task ended with error: <report>`, without a second `error:` (coordinator, 22:40).
  - Example: `+ failed at examples/todo/todo.ignite:41:17: 9223372036854775807 + 1`.
  - The operations are `assert`, `?`, `+`, `-`, `*`, `/`, `%`, `index`, `match`, `unwrap` and
    `slice`.
  - For `assert a == b`, the detail is the slice-9 left and right debug text.
- Rust target:
  - Each failure site calls `ignite_io::fail(op, file, line, col, detail)`. Arithmetic uses
    `checked_*`, so rustc's panic text is gone.
  - `ignite_io::spawn` wraps the task in `catch_unwind`. The main task's panic exits 101.
- LLVM target (O10):
  - `setjmp` never sits in a Rust frame, because `returns_twice` is undefined in Rust. It
    lives either in the emitted IR's task-entry wrapper (a `setjmp` call with the
    `returns_twice` attribute, then the task body) or in a small C file built by `ignite-rt`.
  - `ignite_rt_fail` prints the report, releases the task's arena regions and handles, then
    `longjmp`s to that wrapper, which calls `task_end`.
  - No landing pads in the emitted IR.
  - A Rust panic inside `ignite-io` or `ignite-rt` is caught by `catch_unwind` at the task
    thread and reported in the same format. It is never `longjmp`ed over.
- `slice.md` §7 and §12.5 and `llvm.md` §4 change to match. X5 and X22 are resolved.

### 14.9 Substrate items R2 to R5 (vision 10.3)

| item | change | acceptance |
|---|---|---|
| R2 | a weak reference in the pending list; send plus close in the stream's `Drop` | `iotest drop`: the peer reads the bytes without waiting for a flush |
| R3 | `close` sets a `closed` flag, and `write_all` after it returns `Err` on the writer's side | `iotest cross`: the error reaches the writer and not the other task's `read` |
| R4 | `transient` also covers EPROTO, ENETDOWN, EHOSTUNREACH, ENETUNREACH and ENONET | a unit test of `transient` |
| R5 | remove the flush in `exit` | a unit test: exit with a stalled writer returns |

### 14.10 Typed API (add-only)

- `TypedProgram::quantity(expr) -> Option<(QuantityKind, i64)>`: kind and value in base
  units of a quantity literal.
- `TypedProgram::named_args(call) -> Option<&[usize]>`: present only when the call has a
  named argument; entry `i` is the index in the call's `args` of the argument for parameter
  `i` (a UFCS receiver is not counted). Arguments are evaluated in `args` order.
- `TypedProgram::brace_kind(expr) -> Option<&BraceKind>`, `BraceKind { Record(Ty), Map(Ty,
  Ty) }`; `brace_key(&Path) -> Option<&Reference>` for a qualified map key.
- `TypedProgram::reference(expr) -> Option<&Reference>`: the constant a `Ref`, `Signal`,
  `SystemRef` or `Protocol` value denotes, `Reference { System, Member, Signal, Protocol }`,
  lowered as the text `Reference::text()`.
- `TypedProgram::executables() -> &[ExecutableInfo]`: system, component, kind, main fn,
  executable name, resources used and embedded components.
- `TypedProgram::module_record(expr) -> Option<&ModuleRecord>`: module path, record type and
  the fn per field.
- `TypedProgram::failure_site(expr) -> Option<FailureSite { op, offset }>` for every
  expression that can fail; `FailureSite::position(&SourceFile)` gives file, line and column,
  since the typed program does not know its source files.
- New types `Ty::Map`, `Ty::Quantity(QuantityKind)`, `Ty::Ref(Option<Box<Ty>>)` (`None` is
  the kind-erased `Ref`), `Ty::Signal`, `Ty::SystemRef`, `Ty::Protocol`. `Module<I>` is the
  record `I` itself; `Exposure` is the builtin enum `Ty::Enum(QualName::builtin("Exposure"))`.
- New builtins: `Map*`, `DurationToMs`, `DurationToNs`, `ByteSizeToInt`, `RatioPpm`,
  `IoErrorWithExitCode`, `ModuleRecord`.
- `ModuleTypes::param_names`: parameter names per fn, for named arguments across modules.
- AST: `ExprKind::Quantity`, `NamedArg`, `Brace`; `StmtKind::ForPattern`;
  `Decl::Assembly`; `SystemDecl::contracts` and `policies`.
- `Symbols`: `contracts`, `policies`, `assemblies` and `systems` (members per system), kept
  for slice 11.

### 14.11 Diagnostics E100 to E109

| code | rule |
|---|---|
| E100 | brace literal with no expected type |
| E101 | unit on the wrong kind of value, or unit mismatch in an operation |
| E102 | `module("…")`: module not found, or a fn missing or of the wrong type |
| E103 | named argument: unknown name, duplicate, positional after named, or used through a fn value |
| E104 | component or resource property; three distinct message texts and three fixtures: `required field <f> of <K> is missing`, `output field <f> of <K> cannot be set`, `exposure Loopback needs a loopback address, got <a>` |
| E105 | cycle among components through bare-name uses |
| E106 | `Map` key type not orderable |
| E107 | quantity literal out of range, or more than 4 decimals |
| E108 | assembly binding key not a member of the named system |
| E109 | executable kind's `main` has the wrong signature; since slice 12 also a `machine` or `validate` hook with the wrong signature or an effect (16.17) |

E109 is the last code of this slice. Nothing from E110 on (T4).

### 14.12 Work split

| agent | owns |
|---|---|
| P1 frontend | syntax, check and types: 14.1 to 14.4, 14.7, 14.10, 14.11, `docs/slice.md` §14 |
| P2 Rust target and substrate | `ignite-rustgen`, `ignite-io`: Map, quantities, the 14.5 mains, the T1 Rust half, R2 to R5 |
| P3 LLVM target | `ignite-llvm`, `ignite-rt`, `docs/llvm.md`: Map, quantities, the 14.5 mains, the T1 LLVM half (setjmp, per-task release) |
| P4 std and examples (after P1 to P3) | 14.6 std modules, both examples in the component form, the design-file edits of O5, design §11.3 and §3.3 notes, friction rows, gate-time record |

### 14.13 Decisions taken in the review

| # | decision |
|---|---|
| O1 | A component kind is a record type; it is executable when it has `fn K.main(self: K) -> Result<(), IoError>` in Ignite. The accept loop moves to std, and `Service`/`Client` are removed (14.5). |
| O2 | A bare name inside a `system` is a value of its type; `System.name` is `Ref<K>`. |
| O3 | Fields of type `Signal<T>` are outputs and cannot be set (E104). Refined in 14.14 item 8: a contract's `signal = probe.attempts` sets one to another member's signal. |
| O4 | `module("p")` resolves to a sibling module of the program's package or a std module, and gives a record of fns filled by name. |
| O5 | The design file gets the minimal edits it needs: the package prefix in `module("…")`, the stale `std.telnet` handler path, and `type =` changed to `kind =`. Amended 2026-11-01: `workstationExecutable` and `scheduledProbeHost` join the `std.deploy` import, and `import std.protocol.telnet.Telnet` is added. They are mirrored in design §11.3. |
| O6 | `kind = endpoint(Telnet)`; no types as values. |
| O7 | `Duration` is Int ns, `ByteSize` Int bytes, `Ratio` Int ppm. No `Decimal`. |
| O8 | `ignite build` does not read assemblies in slice 10. |
| O9 | `std/cli` types are checked only; the buildable client is `TelnetClient`. |
| O10 | LLVM T1 uses `setjmp` in the IR task-entry wrapper or in a C file of `ignite-rt`, never in a Rust frame, and `longjmp` from `ignite_rt_fail` (14.8). |
| O11 | `Map` keys are scalars, payload-free enums and `Ref`. LLVM stores a sorted array; a B-tree is a labelled later milestone. There is no `Any` type: the kind-erased reference is `Ref`. |
| O12 | New builtin `IoError.with_exit_code(n)`. |

### 14.14 Implementation record

Ownership, added by the coordinator on 2026-11-01: P1 also owns `crates/ignite/src/lib.rs`
(module loader, front end, check path) and `crates/ignite/src/main.rs` for slice 10; P3 keeps
`crates/ignite/src/llvm_build.rs`. `ignite check` types the program without generating Rust, so
it does not reach a backend's placeholder arms. `Service`/`Client` kept their E020 rules until
P4 moved the examples to the component form (items 17 to 19 below).

Decisions accepted by the steering session on 2026-11-01:

1. A bare member name where `Ref<K>` is expected gives the reference, as `System.name` does
   (`target = server` in a probe).
2. Keywords are allowed as record field names, in record and brace literal keys and after
   `.`. The rule is contextual: a keyword is still refused as a variable, fn or type name
   with the existing diagnostic.
3. `Module<I>` is the record `I`; there is no module type.
4. `Protocol` (`kind = endpoint(Telnet)`) and `SystemRef` (`system = Terminal`) are builtin
   value types, lowered as the text of the name they denote.
5. Client targets (P2's question, 21:12). `std/net.ignite` holds `record Endpoint { address:
   Text, port: Int }`, written by P1 (three lines; P4 may extend the module). A bare
   `TcpListener` resource name where `Endpoint` is expected gives `Endpoint { address, port }`
   of that resource. `TelnetClient.target` has type `Endpoint`. `ExecutableInfo.resources`
   keeps the field's type: an entry typed `Ty::TcpListener` is bound by the generated main
   (14.5 step 3); an entry typed `Endpoint` is built with the `--port` override applied and
   is not bound. No `bound` flag.
6. Report text (14.8) is locked as P2 wrote it to P3: `error: <op> failed at
   <file>:<line>:<col>: <detail>`; `assert` keeps the `message:`, `left:` and `right:`
   continuation lines; `?` details `Err(<shown>)` or `None`; a Rust panic is `error: panic at
   <file>:<line>:<col>: <message>`. P2 updates the expectations in
   `crates/ignite/tests/ignite_test.rs` (text only) and tells P1 which lines changed.

Ref, Signal, SystemRef and Protocol lower to Text in slice 10; slice 12 revisits the representation when probes and controllers resolve references at run time.

Landed by P1 on 2026-11-01 (frontend, driver, `std/net.ignite`); where the implementation
differs from 14.1 to 14.11 or refines them:

7. `module("p")` where the expected type is `Option<I>` (a kind's optional module field)
   has type `Option<I>`; the backends wrap the record in `Some`. `ModuleRecord::record` is
   `I`. Without an expected record type the call is E042, with another expected type E040.
8. A `Signal<T>` field of a declaration may be set to another member's signal
   (`signal = probe.attempts`, a `Reference::Signal`); any other value is E104 `output field
   <f> of <K> cannot be set`. A missing `Signal` field is never E104.
9. `Int * Quantity` is allowed beside `Quantity * Int`; `Quantity / Int`; `%` on a quantity
   is E045, every other mix E101 (comparisons still type as `Bool`). A quantity literal's
   unit decides its kind; the literal's value is checked by E107 as 14.11 says.
10. Failure sites: `assert` at its keyword, `?` at the `?` character, binary `+ - * / %` and
    unary `-` at the expression start for `Int` and `Byte` operands and quantity arithmetic,
    `x[i]` and `.at(i)` as `Index`, `.slice(a, b)` as `Slice`, every `match` as `Match`.
    `FailureOp::Unwrap` is declared and never produced in slice 10.
11. `Loopback`, `Private` and `Public` are builtin variants of the builtin enum `Exposure`
    (bare or `Exposure.Loopback`). The loopback check of E104 runs only when `exposure` is
    written as `Loopback`; `backlog: Int` is the fourth `TcpListener` property; `address`
    and `port` are required. A `Store<T>` resource takes no properties (E047).
12. The executable's name is the string literal of the component's `executable` property,
    else the component's name. `ExecutableInfo.resources` lists the resources the component
    and its embedded components use through bare names, `embedded` the components in an
    order where each follows the components it uses. A kind without `K.main` is a plain
    component. Unknown kinds, `Service` without `application`, two components with one
    executable name and a `system` in a `std.*` module are E020 from the type checker, so
    `ignite check` reports them without a backend.
13. Sibling modules (14.4): the files `<name>.ignite` beside the program whose first
    declaration is `package <program package>.<name>`, loaded, checked and typed in import
    order before the program; they may import `std.*` and each other, and the program may
    import from them (`import examples.todo.model.Todo`). `module("p")` sees them and every
    std module. `ignite build` passes them to `generate_package` and, on the LLVM target, as
    `ignite_llvm::Options.siblings`; `ignite test` adds them to the test crate as modules
    (deduplicated by module path); `ignite check` generates nothing.
14. `Ref` takes zero or one type argument; `Map<K, V>` reports E106 at `K`'s type
    expression, also for `Map.empty()`'s expected type. Map keys written as names are `Text`
    (E040 against another key type), keys written `System.member` are `Ref` values recorded
    in `brace_key`. Named arguments with too many positional arguments are E003 from
    `ignite-check` (arity) before the type checker's E043.
15. `std.net` is both a substrate module (builtin names) and the file `std/net.ignite`
    (`Endpoint`): `import std.net.{Endpoint}` takes the record from the file, and a name
    that is neither is E031 as before.
16. Compiler fix: `SourceFile` keeps its line table (`OnceLock`, computed on the first
    `line_col`), with the `line_col` signature unchanged; `FailureSite::position` over the
    100,000-term chain and 100,000 `line_col` calls on a 100,000-line file each finish under
    1 s (tests `limits::failure_site_positions_of_the_long_chain_are_cheap` and
    `robustness::line_col_computes_the_line_table_once`). Two `SourceFile`s are equal when
    their paths and texts are.
17. A bare fn name that is not in scope, called where a record or enum of another module
    `M` is expected, resolves to `M`'s fn of that name, as a bare variant resolves to the
    expected enum (`typer::expected_module_fn`); the Rust target qualifies such a call
    through `module_fn_path` when the name is not imported. The design file's
    `window = rolling(30 days)`, `backoff = exponential(..)`, `on_breach = event(..)` and
    `kind = endpoint(Telnet)` rely on it with only the record types imported.
18. The cutover (14.5): `Service` and `Client` are gone from the type checker, `ignite-check`
    and both backends; a kind is any record type. The Rust target lowers executables only
    from `TypedProgram::executables`; run without the typed program it reports E020 at every
    component's kind (``unsupported component kind `K`: a component is an executable only
    through the type checker``), and E020 for a malformed `TcpListener` resource as before.
    Two components with one executable name are the type checker's E020. `write_project`
    removes a `src/bin/<name>.rs` whose executable is gone, and `src/bin` when none is left.
    The generated Rust also allows `non_snake_case` (`std.deploy`'s `localSocket`).
19. LLVM target, `spawn`: `ignite_rt_spawn(ptr fn, ptr env)` returns `i8` (1 started, 0 no
    thread) and `ignite_rt_spawn_copy` is removed with the generated accept loop. A
    `TcpStream` argument moves to the task when it is a plain name that a `let` earlier in
    the same block bound to a fresh value and no later statement of the block mentions
    (`region::moved_to_task`): the task owns it (`tcp_own`) and releases it when its callee
    returns, or the parent releases it at once when no thread starts. Other stream
    arguments stay the parent's (`docs/llvm.md` section 4, `docs/friction.md` row 30). Since
    story `llvm-stream-ownership` (2026-11-05) a handle counts its owners: every stream
    argument makes the task an owner (`tcp_retain` in the parent unless it moves), and a
    block owns and releases the streams it creates and does not let escape (`docs/llvm.md`
    section 4).
20. Tests moved with the examples: `cli::check_design_file_is_silent` checks the design
    file; `syntax.rs` builds a copy of the golden with the test listener's port written
    into `Runner.port` (the golden lowers without a standard library, so it cannot name
    `Endpoint`); `terminal.rs`'s short-idle build rewrites `idle_timeout: Duration = 10
    minutes` in `SessionLimits`; `llvm.rs` checks `TelnetServer.main` and `TelnetClient.main`
    in the right executable and `examples.terminal.commands.respond` only in `terminald`;
    `llvm_build.rs` edits the sibling `commands.ignite` and expects `program.ll` alone
    recompiled; `failover.rs` keeps its fixture `Server` kind because the example has no
    failing command; `slice10::crashd_survives_failing_handlers` runs the real
    `TelnetServer` and `CommandConsole`.

Placeholder arms the frontend skeleton added so that the workspace builds with
`--all-targets`. Each is `unimplemented!("slice 10: …")` and is replaced by its owner:

| owner | file | fn | covers |
|---|---|---|---|
| P2 | `ignite-rustgen/src/lib.rs` | `is_atomic`, `expr`, `mutated_in` | `Quantity`, `NamedArg`, `Brace` |
| P2 | `ignite-rustgen/src/lib.rs` | `stmt`, `mutated_names` | `ForPattern` |
| P2 | `ignite-rustgen/src/lib.rs` | `rust_ty` | new `Ty` variants |
| P3 | `ignite-llvm/src/lower.rs` | `expr_kind` | `Quantity`, `NamedArg`, `Brace` |
| P3 | `ignite-llvm/src/lower.rs` | `stmt` | `ForPattern` |
| P3 | `ignite-llvm/src/lower.rs` | `builtin` | new builtins |
| P3 | `ignite-llvm/src/region.rs` | `expr`, `block`, `mutates` | expressions, `ForPattern`, builtins |
| P3 | `ignite-llvm/src/layout.rs` | `variants`, `memory_type` | new `Ty` variants |
| P3 | `ignite-llvm/src/glue.rs` | `debug_function` | new `Ty` variants |

That is 15 arms, 6 in `ignite-rustgen` and 9 in `ignite-llvm`. `Decl::Assembly(_) => {}` arms
in `ignite-rustgen/src/lib.rs`, `ignite-rustgen/tests/units.rs` and `ignite-llvm/src/lib.rs` are
final (O8), and `units.rs` sets the new `SystemDecl` fields to empty lists.

## 15 Slice 11: plan and explain (F2)

Contract reviewed and accepted by the steering session on 2026-11-02, with seven amendments
(telos-steer, 01:58) applied. Sources: vision 4.1 row 11, invariant 9, T4, D5, D7, section 6
(X2, X3), 10.2 (a21); design 3.1, 3.4, 3.5, 5, 6.1 to 6.3, 10.1, 11.3; `docs/slice.md` 14.4 to
14.7 and 14.14; `examples/terminal/terminal.design.ignite`; `std/contract.ignite`,
`std/policy.ignite`, `std/deploy.ignite`, `std/probe.ignite`. Decisions O13 to O26 (numbering
continues slice 10's O1 to O12) are recorded in 15.12.

### 15.0 Acceptance

Carry-overs from slice 10 first. The measuring agent P0 measures and attributes them on the
committed tree `60aef38`, in its own copy of that tree with its own target dir, before any
slice 11 edit lands and in parallel with P1's skeleton; P0 fixes nothing. The coordinator
assigns the fixes by P0's numbers when P2 and P3 are dispatched (15.1, 15.11). Each item ends
in a fix or in a new baseline the steering session accepts in writing.

1. Gate warm time: `task gate` after `touch crates/ignite-check/src/lib.rs`, re-measured at
   load average under 5, attributed per test file of `crates/ignite/tests/`. Slice 10 recorded
   45.5 s against 25.9 s after slice 9b (+76 %; 39.0 s of e2e test execution at load 21).
2. Program build time: `todod` re-measured at load average under 5 on both targets, attributed
   per compile step. Slice 10 recorded LLVM release 0.9 s to 1.2 s and Rust dev 3.0 s to 3.3 to
   4.0 s.
3. The flake `failover::a_failing_session_does_not_end_the_service` is reproduced and fixed, or
   bounded: 0 failures in 30 runs alone and 30 runs beside a full `cargo test --workspace`.

Slice 11 proper:

4. `ignite plan examples/terminal/terminal.design.ignite` exits 0 and prints the system model
   (15.5). Two runs in separate processes, started from two different working directories,
   give byte-identical output, equal to the golden `crates/ignite/tests/golden/terminal.design.model.json`.
5. Every property value in that model is evaluated: the golden contains no Ignite expression
   text (none of `exponential(`, `rolling(`, `calendar_month(`, `event(`, `module(`,
   `localSocket(`, `nativeProcess(`, `endpoint(`, and no quantity written with a unit), and a
   test walks the JSON and checks every value against the encoding of 15.4.
6. The assembly `local` is resolved: each of the five resources and components of `Terminal`
   appears exactly once in `assembly.resolution` (15.6). `ignite check` on the design file
   still exits 0, now with the evaluation and resolution rules of this slice applied.
7. `ignite explain` on the design file prints the golden texts for `Terminal.reachable`,
   `Terminal.server.recovery` and `local` (15.8).
8. The evaluator agrees with both targets: every fn of the fixture
   `crates/ignite/tests/fixtures/eval_equivalence.ignite` gives the same debug text in the
   evaluator, in a Rust-target build and in an LLVM-target build (15.3).
9. Builds are unchanged: every Rust golden and every LLVM golden is byte-identical, except
   LLVM goldens changed by the reachable-only std emission if carry-over 2 adopts it; each
   such golden is re-blessed with the diff explained in P3's report. The only behaviour change
   of an existing program is E116 for effectful property expressions (O16).
10. Invariant 9: `task gate` wall time (cold and warm), the `todod` build time on both targets,
    and `ignite check` and `ignite plan` of the design file are measured before and after and
    recorded in `AGENTS.md`. A rise of more than 10 % is a deviation and has to be explained.
11. Every diagnostic E110 to E119 has a fixture.

### 15.1 Carry-overs: how they are measured

- Who and when: P0 measures on its own copy of commit `60aef38` (`git archive 60aef38 | tar
  -x`), with its own target dir, while P1 builds the skeleton; it reports numbers and
  attributions to the coordinator and changes no file of the repository. The fixes below are
  candidates; the coordinator assigns them by P0's numbers: the LLVM reachable-only std
  emission to P3 only if `clang` on `std.ll` is confirmed as the cause; the e2e shared-build
  or parallel-test fix and the flake test rewrite to P3, the owner of `crates/ignite/tests/`.
  P2 keeps the evaluator only.
- Load: other sessions share the machine, so before and after are measured interleaved on
  the same machine state. For each timed unit, one run on a `git archive 60aef38` copy, then
  one run on the slice-11 tree, alternating, at least 3 pairs. The 1-minute load is recorded
  per run; a pair whose two loads differ by more than 5 is discarded; the median of the pair
  ratios is reported with the absolute times. P0 reports slice 10's carry-overs as shares (%
  per test file, % per compile step) with load per run. P4 runs the interleaved pairs at the
  close of the slice.
- Item 1: each file of `crates/ignite/tests/` runs alone as `cargo test -p ignite --test main
  <file>::` after the warm touch, timed; the sum is compared with the whole step. The fix
  candidates are named in the order to try: one shared build of each example program per test
  binary run instead of one per test, then parallel e2e tests where they now serialise on a
  lock, then removal of a test that repeats another's coverage (named and agreed). If the warm
  gate stays above 28.5 s (25.9 s + 10 %) after the fix, the measured number becomes the new
  baseline only with the steering session's written yes.
- Item 2: LLVM release split into emit, `clang -O2` of `std.ll`, `clang -O2` of `program.ll`
  and link; Rust dev into the `ignite-std` crate and the program crate. If `clang` on `std.ll`
  is the cause (inferred in slice 10: 18,120 lines, 0.84 s), the LLVM target emits only the std
  fns reachable from the program's executables and tests (a call-graph walk over the typed
  modules before emission; glue for types no reachable fn uses is dropped too). The Rust-target
  part is measured and attributed; a fix there is a labelled later milestone unless the
  low-load number is more than 10 % over 3.0 s.
- Item 3: P0 runs the test 30 times alone and 30 times beside a full workspace test run.
  Slice 10's failure lacked the third `task ended with error` line in a log read after `kill`.
  The rewrite (P3, whether or not P0 reproduces the failure): the test waits for the line with
  a deadline instead of reading the log once after `kill`.

P0's result (2026-11-02): warm gate 25.98 s at load under 5, equal to slice 9b's 25.9 s, so no
fix; `clang -O2 std.ll` is 70.5 % of the todod LLVM release build, so reachable-only std
emission is assigned to P3; the flake did not reproduce in 30 + 30 runs, so the deadline
rewrite is preventive.

### 15.2 Property expressions are pure (E116)

- A property expression is the right-hand side of a property in a `resource`, `component`,
  `contract`, `policy` or `assembly` declaration, and the default expression of every kind
  field a declaration leaves out.
- Its effect set must be empty. A call of a fn with a non-empty `requires`, `spawn`, an
  operation on a handle (`TcpListener`, `TcpStream`, `Store<T>`), or an ambient name (`stdin`,
  `stdout`, `process`) inside it is E116, reported by the type checker at the expression.
- A bare member name stays a value of its type (14.4), and a resource name stays legal: naming
  a handle is not an operation on it.
- Audit probe `docs/audit/2026-11-01/effects/a21_system_property_effect.ignite` changes from
  "accepted" to E116; the audit README row and the `.expected` file change with it (O16).
  Vision 10.2 names a21 a hole (property expressions not walked by the checker), and E116
  closes it.
- The generated `main` of 14.5 still evaluates property expressions at run time on both
  targets; this slice does not fold them into the binaries (O19).

### 15.3 The evaluator (D5)

- New crate `crates/ignite-eval`: an interpreter over the typed program, used by `ignite
  check`, `ignite plan` and `ignite explain`. Compile time only: no backend and no compiled
  program depends on it. Both targets share it because neither links it.
- It evaluates every property expression of every declaration of a program, with the fns,
  constants and default field values it reaches in any module (program, siblings, std): `let`,
  `var`, assignment, `if`, `match`, `while`, `loop`, `for`, `break`, `return`, closures applied
  inside the evaluation, records, enums, tuples, lists, maps, quantities, text, string
  interpolation, `debug()`, named arguments (in source order, 14.2), and every builtin whose
  effect set is empty.
- Semantics are those of the targets: T6 overflow, 14.8 failures, 14.3 map order, 14.1
  quantity arithmetic and debug text, slice-9 structural `==`.
- Fuel (O14): one unit per expression evaluated, per loop iteration and per call. The limit is
  1,000,000 units per property expression, and `--fuel N` on `plan` and `explain` changes it;
  `check` always uses the default, and `--fuel N` on `run` and `replay` bounds the reducer
  steps only (16.3). A value larger than 1,000,000 nodes (list items, map entries, record fields,
  counted recursively) also stops the evaluation. Either is E115 at the property expression,
  naming the limit and the fn that was running.
- A failure during evaluation (`assert`, overflow, index, `match`, `?` on `Err`, `slice`) is
  E117 at the property expression, with the 14.8 report text as its detail:
  `E117 evaluating Terminal.server.sessions: + failed at <file>:<line>:<column>: <detail>`.
- A result that the model cannot encode is E118 (15.4): a closure value, a fn value that is not
  a named top-level fn, a handle produced by anything other than a member name.
- Equivalence (acceptance 8): the fixture holds at least 40 pure fns, at least one per builtin
  family (Int, Byte, Text, List, Map, Option, Result, quantities, records, enums, closures,
  interpolation, `debug`). The test evaluates each with `ignite-eval` and runs one compiled
  program per target that prints `debug(f())` for each; the three texts must be equal.
- Pure `test` blocks at `ignite check` and constant folding in the backends are not in this
  slice (labelled later milestone, D5 option A's remaining parts).

### 15.4 Value encoding in the model

Every value is a JSON value; where JSON alone is ambiguous it is an object tagged by `"type"`.

| Ignite value | JSON |
|---|---|
| `Unit` | `null` |
| `Bool` | `true`, `false` |
| `Int` | a JSON integer, the exact i64 (O17) |
| `Byte` | `{"type": "Byte", "value": 7}` |
| `Text` | a JSON string |
| `List<T>` | an array, in list order |
| tuple | `{"items": [...], "type": "Tuple"}` |
| record | `{"fields": {...}, "type": "<qualified type name>"}`, every field present |
| enum variant | `{"type": "<qualified enum name>", "variant": "<name>"}`, plus `"fields": [...]` when it has a payload; `Option` and `Result` are enums named `Option` and `Result` |
| `Map<K, V>` | `{"entries": [[k, v], ...], "type": "Map"}`, in the map's key order (14.3) |
| `Duration` | `{"ns": 100000000, "type": "Duration"}` |
| `ByteSize` | `{"bytes": 16384, "type": "ByteSize"}` |
| `Ratio` | `{"ppm": 999000, "type": "Ratio"}` |
| `Ref<K>`, `Ref` | `{"ref": "Terminal.server", "type": "Ref"}` |
| `Signal<T>` | `{"ref": "Terminal.promptProbe.attempts", "type": "Signal"}` |
| `SystemRef` | `{"ref": "Terminal", "type": "SystemRef"}` |
| `Protocol` | `{"ref": "std.protocol.telnet.Telnet", "type": "Protocol"}` |
| `module("p")` record (14.4) | `{"fns": {"respond": "examples.terminal.commands.respond"}, "module": "examples.terminal.commands", "type": "Module"}` |
| named top-level fn | `{"fn": "std.text.trim", "type": "Fn"}` |
| a member name used as a value (`listener = ingress`, `application = console`) | `{"ref": "Terminal.ingress", "type": "Ref"}`, plus a `uses` edge (15.5); the embedded value is the member's own entry |
| a `TcpListener` name where `Endpoint` is expected (14.14 item 5) | the evaluated `std.net.Endpoint` record |

Qualified names are `module.Name` as the typer prints them; builtin types use their bare name.

### 15.5 The system model (D7)

`ignite plan` prints one JSON document, the system model of one system and at most one
assembly. Canonical form, so that equal inputs give equal bytes:

- UTF-8, object keys in byte order at every level, two-space indentation, one key or array item
  per line, `[]` and `{}` for empty containers, `\n` line ends and a final newline.
- Strings escape only `"`, `\` and control characters (`\n`, `\t`, `\r`, `\b`, `\f`, others as
  `\u00XX`); other characters are written as UTF-8.
- Arrays of members, edges and resolution entries are sorted by `id` (or by `from`, `kind`,
  `to`); declaration order in the source does not show (O15).
- Paths are relative: a program file and its siblings relative to the program file's
  directory, std files as `std/<path>`. No absolute path, timestamp, host name or compiler
  version appears (design 6.3).

Top-level keys. Where design 6.2's envelope has a key, the model keeps its name; `assembly` and
`edges` are additions (O13):

| key | content |
|---|---|
| `assembly` | the selected assembly (15.6), or `null` when the program declares none; design 6.2's `bindings` is `assembly.resolution` |
| `components` | every `component` of the system |
| `contracts` | every `contract` |
| `edges` | `{"from", "kind", "to"}`: `uses` (a bare member name in a property), `references` (a `Ref` value), `observes` (a `Signal` value), `hosts` (from the resolution) |
| `formatVersion` | `"ignite-model/0.1"`; a change that removes or reinterprets a key bumps it, an added key does not |
| `policies` | every `policy` |
| `resources` | every `resource` |
| `sourceMap` | object from every member id and property path (`Terminal.server.recovery`) to `file:line:column` of its declaration or expression; a defaulted property maps to the default's position in the kind's module. Slice 13's plan diff compares the model without `sourceMap`; a reformat of the source changes only that key |
| `systemId` | the system's name |

Each member entry:

| key | content |
|---|---|
| `declaration` | `"resource"`, `"component"`, `"contract"` or `"policy"` |
| `defaults` | names of the properties that took the kind's default, sorted |
| `executable` | the executable name when the kind has `K.main` (14.5), else `null` |
| `id` | `<System>.<name>`, the stable logical identity (design 3.4) |
| `kind` | qualified name of the kind |
| `outputs` | object from each `Signal<T>` field the declaration does not set to its element type's name |
| `properties` | object from every other field of the kind to its evaluated value (15.4), set or defaulted |

Excerpt for `Terminal.promptProbe` (the emitted form puts every key on its own line):

```json
{
  "declaration": "component", "defaults": [], "executable": null,
  "id": "Terminal.promptProbe", "kind": "std.protocol.telnet.TelnetPrompt",
  "outputs": {"attempts": "std.sli.ProbeAttempt"},
  "properties": {
    "close_after_probe": true,
    "expected_prompt": "terminal> ",
    "interval": {"ns": 10000000000, "type": "Duration"},
    "target": {"ref": "Terminal.server", "type": "Ref"},
    "timeout": {"ns": 2000000000, "type": "Duration"}
  }
}
```

The design 6.2 envelope's `machines`, `observations`, `artifacts`, `stateSchema` and
`admissionChecks` are not emitted; slices 12 and 13 add them as new keys under the same
`formatVersion` rule. P4 adds a note to design 6.2 (15.11) saying:

- the executable specification is the system model of `docs/slice.md` §15 (D7's rename, X3);
- its `bindings` is the model's `assembly.resolution`;
- `machines`, `observations`, `artifacts`, `stateSchema` and `admissionChecks` arrive in slices
  12 and 13;
- until wave H the public schema is the 15.4 table plus the golden
  `crates/ignite/tests/golden/terminal.design.model.json`; there is no JSON Schema file in this
  slice.

### 15.6 Assembly resolution

- `ignite plan` selects one assembly: the one named by `--assembly NAME`, else the only one in
  the program. Several assemblies and no `--assembly`, or a name that is not an assembly, is
  E114. Without assemblies, the model's system is the program's only `system`; several systems
  and no assembly is E114 as well.
- An assembly kind is resolvable when it has a field `bindings: Map<Ref, B>` and `B`'s module
  declares `fn admits(binding: B, member: std.deploy.Member) -> Option<Text>` and `fn
  includes(binding: B) -> List<Ref>` (O18). The compiler knows these two fn names and the
  record `std.deploy.Member`, not the variants of `std.deploy.Binding`. An assembly of another
  kind is evaluated and emitted with `"resolution": null`.
- `std/deploy.ignite` gains `type Member { declaration: Text, kind: Text, executable: Bool,
  outputs: List<Text> }` and `admits`: `LocalSocket` admits a `TcpListener` resource;
  `NativeProcess` a component that is executable; `WorkstationExecutable` any component;
  `ScheduledProbeHost` a component with at least one output. Each refusal returns the reason
  text that E112 prints. Inline tests cover each variant.
- Rules, checked by `ignite check` for every assembly and by `ignite plan` for the selected
  one, after evaluation:

| rule | diagnostic |
|---|---|
| every resource and component of the system is a `bindings` key, in exactly one binding's `includes`, or a capability resource hosted implicitly (below) | E110 when none of the three |
| no member is both a key and included, or included by two bindings | E111 |
| `admits(binding, member)` returns `Some(reason)` for a key | E112 with that reason |
| a binding's `includes` equals the components its member embeds (`ExecutableInfo.embedded`, 14.14 item 12) | E113, naming the missing or extra member |
| contracts and policies are never keys | E112 (`admits` is not consulted) |

- Implicit hosting (slice 17, 21.3): a capability resource (`Directory`, `Launcher`, `Entropy`,
  `Setting`) belongs to the process of the member that creates it and needs no binding. One
  that is neither a key nor included is hosted by the first declared executable component
  whose `ExecutableInfo.resources` name it (its embedded components' included), with that
  component's binding and `via` (the component itself when it is a key, else the key that
  includes it); its entry is `{"binding": <the server's binding>, "member": "Studio.entropy",
  "via": "Studio.server"}` and the model gains a `hosts` edge from `via` to it. `admits` is
  not consulted for it, and E113 does not count it (`includes` names components only). A
  capability resource no hosted executable creates is E110. A capability resource may still
  be a key; `admits` then decides as for any key (`std.deploy.Binding` admits none, E112).
  `TcpListener` and `Store` resources keep needing a key or an `includes`.
- `assembly.resolution` lists one entry per resource and component, sorted by member id:
  `{"binding": <value>, "member": "Terminal.console", "via": "Terminal.server"}`, where `via`
  is the key whose binding hosts the member (the member itself when it is a key).
- The assembly entry otherwise follows the member form: `id` (the assembly's name), `kind`,
  `properties` (evaluated, so `system` and `bindings` appear as values), `defaults`.
- `ignite build` still ignores assemblies (O8 stays); slice 12's `ignite run` reads the
  resolution.

### 15.7 `ignite plan`

```text
ignite plan <file.ignite> [--assembly NAME] [--out FILE] [--fuel N] [--std PATH]
```

- Prints the model on stdout, or writes it to `FILE` (overwriting it).
- Exit statuses as `ignite check`: 0 model written, 1 diagnostics, 2 invalid arguments, 3 a
  file could not be read or written.
- Nothing is planned against a live system in this slice: no admission, no diff, no effects
  (slice 13).

### 15.8 `ignite explain`

```text
ignite explain <file.ignite> <path> [--assembly NAME] [--fuel N] [--std PATH]
```

- `path` is a system (`Terminal`), a member (`Terminal.reachable`), a property path
  (`Terminal.server.recovery`, `Terminal.server.sessions.idle_timeout`) or an assembly
  (`local`). A path that names none of these is E119.
- Output on stdout, plain text, deterministic like the model:
  - line 1: `<path>: <declaration> <kind> (<file:line:column>)`;
  - then one line per property in the kind's field order: `  <name> = <value>`, with
    ` (default)` after a defaulted one. Values print as slice-9 debug text with quantities in
    their literal form (14.1); `Ref`, `Signal`, `SystemRef` and `Protocol` print their name
    unquoted;
  - then the member's edges, one per line (`uses Terminal.ingress`, `observed by
    Terminal.reachable`), and its resolution line when an assembly is selected;
  - a contract ends with `standing: not evaluated before slice 12`.
- Exit statuses as `ignite plan`.

Golden for `Terminal.reachable`:

```text
Terminal.reachable: contract std.contract.Availability (terminal.design.ignite:81:3)
  signal = Terminal.promptProbe.attempts
  good = "prompt_received_before_timeout"
  target = 99.9%
  window = Rolling(30 days)
  missing_data = "insufficient_evidence"
observes Terminal.promptProbe.attempts
standing: not evaluated before slice 12
```

Golden for `local`:

```text
local: assembly std.deploy.Deployment of Terminal (terminal.design.ignite:108:1)
  Terminal.client = WorkstationExecutable
  Terminal.console via Terminal.server = NativeProcess([Terminal.console])
  Terminal.ingress = LocalSocket
  Terminal.promptProbe = ScheduledProbeHost
  Terminal.server = NativeProcess([Terminal.console]), executable terminald
```

### 15.9 Typed API (add-only)

As landed by P1's skeleton on 2026-11-02; what differs from the reviewed draft is in 15.14.

- `TypedProgram::systems() -> &[SystemInfo]`: per system `SystemInfo { name, decl, members,
  offset }` (`decl` is the index of the `Decl::System` in `Program::decls`), each member a
  `MemberInfo { id, name, declaration: MemberDeclaration, index, kind: Ty, properties:
  Vec<(field, PropertySource)>, outputs: Vec<(field, Ty)>, offset }`. `properties` is in the
  kind's field order and holds every field that is not an unset `Signal<T>` output;
  `outputs` the unset `Signal<T>` fields with their element type. `PropertySource::Set(i)`
  is the index into the declaration's `properties` (`MemberInfo::property(&SystemDecl,
  field)` gives the expression); `Default(QualName)` names the default's constant
  (`<Record>.<field>`, slice 9 item 6; `ignite_types::field_default(&Program, record, field)`
  finds its expression; the builtin `TcpListener`'s two defaults are `TcpListener.exposure`
  and `TcpListener.backlog` in the builtin module `""`, values
  `TCP_LISTENER_DEFAULT_EXPOSURE` and `TCP_LISTENER_DEFAULT_BACKLOG`); `ComponentName` is
  an `executable` field left out, whose value is the component's name (14.5).
- `TypedProgram::assemblies() -> &[AssemblyInfo]`: `AssemblyInfo { name, decl, kind: Ty,
  system: Option<String>, properties, offset }`; `AssemblyInfo::property(&Program, field)`
  gives a property expression.
- `TypedProgram::effects(expr) -> Option<&EffectSet>`: the effect set of a property
  expression of a resource, component, contract, policy or assembly, of a constant's value,
  of a field default, and of every sub-expression of those (closure bodies excluded);
  `None` inside fn, role and test bodies, whose effects `ignite-check` accounts to the fn.
  `EffectSet::entries()` are sorted `requires` entries: `r.op` on a member `r` the
  expression names (`ingress.accept`), an ambient effect (`stdin.read`), a callee's
  requirement as written when the argument for its parameter names no member, the bare
  operation (`accept`) on a handle that is neither, and `spawn`. Used by E116 and asserted
  empty by the evaluator.
- `TypedModules` stays the exports map every crate uses. The evaluator's view of every
  module (path, `SourceFile`, AST, `TypedProgram`) is `ignite_eval::Module<'p>`, collected in
  `ignite_eval::Modules<'p>`; the driver builds it from the standard library, the siblings
  and the program, as it builds `ignite_llvm::ModuleInput`.
- `ignite-eval`: `Evaluator::new(&Modules, Fuel)`, `Evaluator::evaluate(&mut self,
  &TypedProgram, &Expr) -> Result<Value, EvalError>` with `EvalError::{Limit { limit, running
  }, Failed { report }, Unencodable { what }}` for E115, E117 and E118; `Fuel { units, nodes
  }`, both 1,000,000 by default, `Fuel::with_units(n)` for `--fuel`; `Value` (15.4);
  `model(&mut Evaluator, &TypedProgram, assembly: Option<&str>) -> Result<Model,
  Vec<Diagnostic>>`; `Model::to_json() -> String`; `resolve(&mut Evaluator, &TypedProgram,
  &SystemInfo, &AssemblyInfo, bindings: &Value) -> Result<Vec<ResolutionEntry>,
  Vec<Diagnostic>>`; `explain(&Model, path) -> Result<String, Diagnostic>`.
- `ignite` library: `PlanOptions { assembly, fuel, std }`, `plan(file, &PlanOptions) ->
  Result<String, Error>` and `explain(file, path, &PlanOptions) -> Result<String, Error>`;
  `--out FILE` is written by `main.rs`.

### 15.10 Diagnostics E110 to E119

| code | rule | fixture |
|---|---|---|
| E110 | a resource or component the assembly neither binds nor includes | `e110_unbound_member` |
| E111 | a member bound twice: key and included, or included by two bindings | `e111_bound_twice` |
| E112 | binding not admitted for the member (reason from `admits`), or a contract or policy as a key | `e112_binding_not_admitted` |
| E113 | a binding's `includes` differs from the components its member embeds | `e113_includes_mismatch` |
| E114 | no assembly or system selectable: several and no `--assembly`, or an unknown name | `e114_assembly_selection` |
| E115 | evaluation limit exceeded: fuel or value size | `e115_fuel_exhausted` |
| E116 | effectful property expression | `e116_effectful_property` |
| E117 | failure during evaluation, with the 14.8 report text | `e117_evaluation_failed` |
| E118 | value the model cannot encode | `e118_unencodable_value` |
| E119 | `ignite explain` path names no system, member, property or assembly | `e119_unknown_path` |

Fixtures live in `crates/ignite-types/tests/fixtures/` (E116) and
`crates/ignite/tests/fixtures/` (the rest, run through `ignite check`, `plan` or `explain`).
E119 is the last code of this slice; nothing from E120 on (T4).

### 15.11 Work split

| agent | owns |
|---|---|
| P0 measuring | its own copy of commit `60aef38` and its own target dir; measures and attributes 15.0 items 1 to 3 per 15.1 and reports the numbers to the coordinator; edits no file of the repository and fixes nothing |
| P1 frontend | `ignite-syntax`, `ignite-check`, `ignite-types`: 15.2 (E116), 15.9 typed API, the `crates/ignite-eval` skeleton with its placeholders and the `plan` and `explain` subcommand stubs; `docs/slice.md` for the whole slice |
| P2 evaluator | `crates/ignite-eval` `eval.rs` and `value.rs`: 15.3, E115, E117, E118, the equivalence fixture and test (acceptance 8); nothing else |
| P3 model, resolution, CLI, tests | `crates/ignite-eval` `model.rs`, `resolve.rs`, `explain.rs`; `crates/ignite` `plan`, `explain` and their CLI; `crates/ignite/tests/`, including `main.rs` and `common/mod.rs`; 15.4 to 15.8, E110 to E114, E119; goldens for the design file; the carry-over fixes the coordinator assigns by P0's numbers (15.1), including the flake rewrite |
| P4 std, examples, acceptance | `std/deploy.ignite` `Member`, `admits` and `includes` with inline tests (15.6, first, so that P3 can resolve); model goldens for `examples/terminal/terminal.ignite` and `examples/todo/todo.ignite`; the design 6.2 note of 15.5 and a 10.1 note on `plan` and `explain`; audit README row a21; 15.0 items 4 to 11 verified with commands; `AGENTS.md` layout row for `ignite-eval` and the slice-11 gate-time row; `docs/friction.md`. P4 does not edit `docs/vision.md` |

- Order: P0 and P1 in parallel; then P2, P3 and P4 (P4 starting with `std/deploy.ignite`);
  P4's acceptance last.
- Shared files: `crates/ignite/tests/main.rs` and `tests/common/mod.rs` are P3's. P2's
  equivalence test gets its `mod` line by message to P3, as in slice 10.

### 15.12 Decisions taken in the review

| # | decision |
|---|---|
| O13 | Design 6.2 key names are kept where they exist (`formatVersion` with the value `"ignite-model/0.1"`, `systemId`, `resources`, `components`, `contracts`, `policies`, `sourceMap`); `assembly` and `edges` are additions; additive keys keep the version (steering amendment 3) |
| O14 | Fuel: 1,000,000 units per property expression (expression, iteration, call), value size 1,000,000 nodes; `--fuel` overrides |
| O15 | Member arrays sorted by id, not in declaration order, so that reordering declarations changes nothing (slice 13's plan diff) |
| O16 | Property expressions must be pure (E116); audit probe a21 flips from accepted to E116 |
| O17 | `Int` is a JSON integer even beyond 2^53; consumers parse it as i64. No string fallback |
| O18 | Resolution is defined by `admits` and `includes` in the binding type's module, not by compiler-known variants of `std.deploy.Binding` (invariant 6: domain nouns are library symbols) |
| O19 | Builds do not use the evaluator: constants and property values stay run-time code; goldens unchanged |
| O20 | `ignite check` evaluates every property expression and resolves every assembly, so E110 to E118 appear at check time |
| O21 | Paths in the model and in `explain` are relative to the program file's directory; std files are `std/<path>` |
| O22 | No source hashes and no compiler version in the model, so the golden changes only when the model does; provenance beyond `sourceMap` waits for packages (wave H) |
| O23 | `ScheduledProbeHost` admits a component with at least one `Signal` output; `WorkstationExecutable` any component (`Application` has no `main`, O9) |
| O24 | Explain prints properties in the kind's field order; the model uses byte order |
| O25 | Edge kinds are the syntactic `uses`, `references`, `observes`, `hosts`; design 3.4's provisioning, readiness and recovery classes are slice 12's |
| O26 | Carry-over fixes that leave a number more than 10 % over its baseline need the steering session's written yes before the baseline moves |

### 15.13 Placeholder arms

P1's skeleton adds `crates/ignite-eval` and the `plan` and `explain` subcommands so that the
workspace builds with `--all-targets`. Each placeholder is `unimplemented!("slice 11: …")`,
replaced by its owner:

| owner | file | fn | covers |
|---|---|---|---|
| P2 | `ignite-eval/src/eval.rs` | `Evaluator::evaluate` | 15.3 |
| P2 | `ignite-eval/src/value.rs` | `Value::debug_text` | equivalence, explain values |
| P3 | `ignite-eval/src/model.rs` | `model`, `Model::to_json` | 15.4, 15.5 |
| P3 | `ignite-eval/src/resolve.rs` | `resolve` | 15.6 |
| P3 | `ignite-eval/src/explain.rs` | `explain` | 15.8 |
| P3 | `ignite/src/lib.rs` | `plan`, `explain` | 15.7, 15.8 |

That is 8 placeholders. `TypedProgram::systems`, `assemblies` and `effects` and the `plan`
and `explain` subcommands of `main.rs` (which writes `--out FILE`) are not placeholders; the
skeleton implements them. `crates/ignite-eval/tests/main.rs` holds `fuel.rs` (the O14
defaults); P2 and P3 add their modules there.

### 15.14 Implementation record

Landed by P1 on 2026-11-02 (the skeleton of 15.13 and 15.9); where the implementation differs
from the reviewed contract or refines it:

1. `PropertySource::Set` holds the property's index in the declaration, not `&Expr`:
   `TypedProgram` has no lifetime (its expression keys are addresses), so the expression is
   reached through `MemberInfo::property(&SystemDecl, field)` and
   `AssemblyInfo::property(&Program, field)`. A third source, `ComponentName`, covers an
   `executable` field left out (14.5), which no module's default supplies. The builtin
   `TcpListener`'s defaults are `Default(QualName::builtin("TcpListener.exposure"))` and
   `..backlog`, since no module declares them; their values are the two constants of 15.9.
2. `TypedProgram::effects` returns `Option<&EffectSet>`: sets are recorded for the pure
   contexts the type checker owns (property expressions, constants, field defaults, each
   with its sub-expressions), not for fn bodies, so a 100,000-term chain inside a fn costs
   nothing more. A requirement on a callee parameter whose argument names no member (a
   local bound from a member, an owned handle) stays as written, and an operation on such a
   handle is the bare operation, so every effectful operation leaves at least one entry.
3. `TypedModules` is unchanged (the exports alias of every crate). `ignite_eval::Module` and
   `Modules` carry path, `SourceFile`, AST and `TypedProgram` per module; the driver builds
   them (P3), as it builds `ignite_llvm::ModuleInput`.
4. `model` and `resolve` take the `Evaluator`, which carries the modules and the fuel: the
   draft's `model(&TypedProgram, assembly)` had no way to reach the other modules' ASTs or
   the `--fuel` value. `Model`, `ModelMember`, `ModelAssembly`, `Edge`, `EdgeKind`,
   `ResolutionEntry` and `Value` are starting shapes in P3's and P2's files, theirs to reshape.
5. E116 is reported at the first effectful operation of the property expression, as E010 is
   at the call, with the text ``effectful call `noisy` in the property `application` of
   component `svc`: a property expression is pure`` (``effectful operation `accept` `` for a
   handle operation, `` `spawn` `` for a spawn). A field default is pure by E010 where it is
   declared, so E116 never arises for a defaulted property; its effect set is recorded all
   the same, for the evaluator's assertion.
6. New workspace crate `crates/ignite-eval` (dependencies `ignite-syntax`, `ignite-types`) and
   the dependency `ignite -> ignite-eval`; no external crate was added.
7. E042 "cannot check the effects of `..`" (11.7) is not reported inside a `system` or
   `assembly`: there every effectful operation is E116, which names the property, so the two
   never stack on one expression (`todos.values()` in a property was both before this
   refinement). Outside systems E042 is unchanged.
8. Tests: fixture `e116_effectful_property` (four E116: an inferred-effectful fn call, a
   handle operation, a `Store` method, `spawn`); the audit probe `effects/a21` flips to E116
   (`.expected` and the README row); `valid_system_describes_its_system_and_assembly` in
   `crates/ignite-types/tests/fixtures.rs` reads `systems()`, `assemblies()`, every
   `PropertySource` variant, `outputs`, `property(..)` and empty `effects(..)` of the
   `valid_system` fixture. `ignite check` of every std file, both examples and the design file
   stays clean; frontend time of `check examples/todo/todo.ignite` and of the design file was
   0.056 to 0.059 s and 0.055 to 0.057 s after (5 runs each, load 6.5), 0.081 to 0.136 s and
   0.053 to 0.061 s before (load 10 to 11): no regression measurable above load noise.
9. P3's refinements of 15.4 to 15.8 (model, resolution, `plan`, `explain`, check), landed
   2026-11-02 with 11 `plan::` tests and the fixtures E110 to E114 and E119 under
   `crates/ignite/tests/fixtures/`:
   - `explain` of a system prints `<System>: system (pos)`, then one line per member sorted by
     id, `  <declaration> <id>: <kind>[, executable X]`, then `assembly <name>: <kind>` per
     assembly; of an assembly, the 15.8 form when it resolves, else its properties; of a
     property path, `<path>: property <type>[ (default)] (pos)` and then the record's fields
     as `  f = v` (`  value = v` for a non-record), nested paths allowed
     (`Terminal.server.sessions.idle_timeout` gives `value = 15 minutes`); a member's explain
     adds `  <name>: output of <T>` per output after the properties and
     `resolution: <binding>[ via <key>]` when an assembly is selected. Enum variants print
     without the enum name (`Rolling(30 days)`), as the 15.8 golden.
   - `sourceMap` also holds the system id, the assembly id and the assembly's property paths
     (`local.bindings`). Edges come from the system members' values (`uses`, `references`,
     `observes`) plus `hosts` from the resolution, not from assembly properties.
     `ModelAssembly` has `system: Option<String>`, the `SystemRef`'s name.
   - E114 texts: ``no assembly named `x`: the program declares a, b``; several assemblies and
     no `--assembly`: ``several assemblies (a, b) and no `--assembly`: select one`` at the
     second assembly; several systems and no assembly likewise. E110 is at the `bindings`
     expression, E111 to E113 at the bindings key. The E119 fixture `e119_unknown_path` runs
     `ignite explain <file> S.w.nowhere` through a `// ignite explain S.w.nowhere` first line.
   - `ignite check` of a program without systems evaluates nothing, so every std file stays
     clean. The 15.8 `local` golden keeps `executable terminald`: the design file's
     `component server` now sets `executable = "terminald"` (P4), as 14.5 would otherwise
     name it `server`.
10. Slice 11b (2026-11-02): fixed the four target bugs of friction rows 32 to 35; tests:
    `fixtures/tests/targets.ignite` (one inline test per row, run on both targets by
    `ignite_test::target_bugs_of_slice_11b_are_fixed`), the rustgen golden `targets` with
    `golden::target_bugs_of_slice_11b`, and the restored equivalence fns `list_index_push`,
    `map_enum_key`, `record_named_box`, `enum_exposure` (`eval::` tests, both targets against
    the evaluator). Fixes, all in the targets and the substrate, none in the language: row 32,
    a mutating method (`push`, `append`, `clear`, map `put`, `remove`) on an indexed receiver
    is lowered through `ignite_io::checked::at_mut` on the place instead of a clone
    (`rustgen::indexed_mutation`, `mutable_place`; 13.11 row d); row 33, a generated enum
    without payloads derives `PartialEq, Eq, PartialOrd, Ord` (`rustgen::enumeration`), which
    re-blessed five derive lines in the goldens `consts.std.rs`, `modules.std.rs`, `syntax.rs`
    and nothing else; row 34, `Exposure` is `ignite_io::Exposure` on the Rust target
    (`rust_type`, `enum_name`, `variant_path`, `variants_of`) and a builtin enum of layout
    `Loopback` 0, `Private` 1, `Public` 2 on the LLVM target (`layout::variants`, `mangle`),
    `TcpListener` and `TcpStream` as parameter and return types were already lowered; row 35,
    a user type or variant named after the Rust prelude (`Box`, `Vec`, `String`, `Some`, `Ok`,
    …) is emitted with a trailing underscore (`rustgen::ident`, `RUST_PRELUDE_NAMES`);
    `Option` and `Result` are not covered because they are builtins (E004). LLVM goldens are
    unchanged. `ignite_test.rs` runs its two targets on two threads with one output directory
    and one lock per target, which took the file from 8.6 s to 4.6 s warm (a variant with one
    output directory per invocation and no LLVM lock measured no better at load 24 to 31 and
    was not kept).
    Design §11.2 now says `ignite check`, `plan` and `explain` accept the design file and
    `ignite build` does not. Measurements (15.1 method, 3 interleaved pairs against the
    60aef38 copy and the slice-11 copy, load 6 to 9, other sessions building): warm `cargo
    test --workspace` 10.8 s median (10.3 to 17.8; 60aef38 24.0 s, slice 11 16.1 s in the
    same passes; the steering target of 8 s at load under 5 could not be checked at that load,
    the remaining time is the end-to-end binary 5.6 s, the rebuild 1.8 s, rustgen's tests
    1.7 s); todod Rust dev 3.35 s (pair ratio 1.00), Rust release 4.74 s (1.03), LLVM dev
    0.35 s (0.95), LLVM release 0.88 s (0.70). Against slice 11's absolute numbers in
    `AGENTS.md` (load under 5) Rust release reads +13 % and LLVM release +10 %: load, since
    the 60aef38 copy rose the same way in the same passes (4.46 and 1.16 s against its 4.12
    and 1.10 s) and the pair ratios hold. Gate at the close (load 24 to 32): fmt, clippy,
    `cargo test --workspace --no-fail-fast` (582 passed, 0 failed, 4 ignored; slice 11's 784
    counted the 204 inline tests in, its cargo count was 580), `task test-ignite` (102 inline
    tests per target), `task gate`, `task compile`, `compile-llvm`, `compile-todo`,
    `compile-todo-llvm`: all exit 0.

## 16 Slice 12: run and control (F3)

Contract reviewed and accepted by the steering session on 2026-11-02, with fifteen
amendments (review of 04:10, A1 to A15) applied. Sources: vision 4.1 row 12, invariants 5, 6
and 9, T1, T4, D1, D5, D7; actors.md R8; design 2, 3.4, 4.1 to 4.3, 6.2, 7, 7.1, 7.4, 10.1,
11.7, 11.8, 11.10 (Rollout, Recovery, Contracts); `docs/slice.md` 6.1, 14.5, 14.8, 15.4 to
15.8 and 15.12 (O18, O19, O25); `std/contract.ignite`, `std/policy.ignite`,
`std/probe.ignite`, `std/deploy.ignite`, `std/protocol/telnet.ignite`;
`examples/terminal/terminal.design.ignite`. Decisions O27 to O44 are recorded in 16.19.

The slice adds two commands:

- `ignite run` starts the processes an assembly hosts, supervises them, runs the probes,
  evaluates the contracts and writes every decision to a journal.
- `ignite replay` re-derives every decision from that journal.

Vision 4.1 (amended by steering, A1): the reducers are stepped by the evaluator inside
`ignite run`, and are compiled on both targets in the trace fixture of item 10. The
controller is written as reducers in the shape of actors.md R8, so that wave C replaces the
runtime under them, not the reducers.

### 16.0 Acceptance

Baseline first (16.1). Then the row-12 items.

1. The terminal system runs under `ignite run`.
   - The command: `ignite run examples/terminal/terminal.design.ignite --assembly local
     --port Terminal.ingress=0 --clock manual`.
   - It builds `terminald` and `promptProbe`, starts `terminald`, and journals `started`,
     then `ready` (16.7).
   - A TCP client on the reported port reads `terminal> `, and `echo hello` answers `hello`.
   - The `client` member is listed as declared, not buildable (16.3, A10).
   - The control line `stop` ends the run with exit 0. No process the run started is left,
     checked through `/proc`.
   - One test runs the same with `--target llvm`.
2. Killing `terminald` restarts it within the policy's backoff.
   - Manual clock: after `ready`, the test sends SIGKILL to the journaled pid.
   - The journal shows `exited`, then a `restart` effect with `attempt 1` and `delay 100 ms`.
   - `advance 99 ms` starts nothing. `advance 1 ms` journals `started` at exactly
     `exited.at + 100 ms`, the instant of the `Spawn` dispatch.
   - A second kill gives `delay 200 ms`.
   - Real clock (one test): `started.at - exited.at` lies in `[100 ms, 500 ms]`.
   - A fixture with `maximum_attempts = 2` journals `gave_up` on the third failure inside
     `interval`. The run goes on, and `stop` then exits 1.
3. A probe timeout is visible in the journal and in the contract's standing.
   - Manual clock: one good attempt is journaled.
   - The test sends SIGSTOP to `terminald`, so connects queue in the backlog and no prompt
     arrives.
   - The next scheduled attempt is dispatched. `advance 2 seconds` journals `probe timeout`,
     a bad attempt (as landed, P3: the test accepts the bad attempt at 10 s or at 12 s,
     depending on whether the slot or its deadline falls on the advance).
   - The next `standing` record of `Terminal.reachable` reads `violated` with `good 1` and
     `executed 2`.
   - The control line `status` prints the same standing.
   - After SIGCONT, attempts are good again. The 30-day window keeps `violated`.
   - This is the only test that pays a settle wait (16.4, A2).

Contract semantics (design 4.1, 11.7):

4. Zero executed attempts give `insufficient_evidence`, never `compliant`.
   - `std.contract.meets(_, 0, 0)` is `false` (16.10, O32).
   - A probe that cannot run (its executable is missing, or its output does not parse) is
     missing evidence, not a bad attempt.
   - Completeness below `minimum_completeness` gives `insufficient_evidence`.
   - Each case has an inline test over synthetic events. The zero-attempt case also has an
     end-to-end test: a run stopped before the first slot.
5. `ThresholdCompliance` and `SLA` evaluate from the same signal.
   - The test of item 3 starts with `--epoch 2026-10-31T22:50:00Z`. After the timeout it
     advances across the Europe/Berlin month boundary (2026-10-31T23:00Z).
   - It journals the `standing` of the closed period and one `consequence`,
     `access_commitment_breached`.
   - A second run with only good attempts crosses the same boundary and journals no
     consequence.
   - Inline tests over synthetic events cover the rest:
     - no consequence for `compliant` or `insufficient_evidence`;
     - none twice for one period;
     - period boundaries in months with a DST change (16.11).
6. `DrainRollout` works (A12: the last unit of P2 and P4).
   - The control line `rollout Terminal.server` rebuilds `terminald` and drains the old
     process: its listener closes and prints `released`, and its sessions keep working.
   - The run then starts the new process.
   - At `deadline` (manual clock), the old process gets SIGKILL if sessions remain
     (`disconnect_remaining`).
   - The journal records the declared disruption with its measured interval (16.12).
   - An open session on the old process answers `echo` until the deadline.

Determinism and time:

7. Every journal written by the end-to-end tests replays.
   - `ignite replay` re-derives every effect and standing record byte for byte and exits 0
     (16.15).
   - A journal with one effect record altered exits with E127, naming its `seq`.
8. Two manual-clock runs of one scenario give equal journals after masking the `pid` and
   probe `latency` values, the only values a real process contributes.
9. No test sleeps for a policy or probe duration.
   - Probe timeouts, backoff, windows and calendar periods are driven by the manual clock
     (16.4).
   - Tests assert the `at` of `Ready`, `AttemptDone` and `Started` only as "the instant of
     the dispatch" (A2).
   - All `ignite run` end-to-end tests together add at most 6 s of wall time to
     `crates/ignite/tests` (O41).
10. The reducers agree across the evaluator and both targets.
    - A trace fixture steps each reducer of 16.6 over one event list in the evaluator, in a
      Rust-target build and in an LLVM-target build.
    - The debug texts are equal (this extends 15.0 item 8).

Records:

11. `ignite plan` of the design file adds `machines`, `observations` and the edge classes of
    16.14.
    - The version stays `ignite-model/0.1` (O13, additive).
    - The golden is re-blessed, with the diff explained.
    - The terminal and todo model goldens change only by empty `machines` and
      `observations` keys.
    - Every Rust and LLVM build golden is byte-identical, except programs that use the new
      builtins (16.13).
12. Invariant 9 (16.1).
    - Measured with the interleaved pairs of 15.1 against the slice-12 start tree:
      - gate cold and warm;
      - the warm `cargo test --workspace` step;
      - `todod` on both targets, dev and release;
      - `ignite check` and `ignite plan` of the design file.
    - A rise over 10 % is a deviation to explain; O26 applies.
    - The warm test step stays under slice 11b's recorded number: 10.8 s median at load 6
      to 9 (`AGENTS.md`).
    - New absolute numbers:
      - `ignite run` from start to the first `ready` (warm build);
      - `ignite replay` of a 10,000-record journal;
      - the evaluator test of A7 (16.10): 10,000 emissions into a 30-day window.
13. Every diagnostic E120 to E129 has a fixture (16.17).

### 16.1 Baseline and measurement

- The baseline tree is the coordinator's copy `s12-before`, kept as branch `bench/11b-recovered`
  (all 741 files identical, `docs/performance/evidence/README.md`).
  - It is an rsync copy of `git ls-files -co --exclude-standard` of the working tree at
    slice-12 start (741 files), made by the coordinator.
  - It is not a `git archive`, because the slice-11 and 11b state is uncommitted. The copy
    replaces the slice-11 close commit.
  - It has its own target dir.
- No P0: the slice-11 and 11b rows in `AGENTS.md` are the reference.
- P4 re-measures both trees in pairs, as 15.1 prescribes:
  - one run per tree, alternating, at least 3 pairs;
  - the 1-minute load recorded for every run;
  - a pair dropped when its two loads differ by more than 5;
  - the median of the pair ratios reported next to the absolute times.
- New end-to-end test files of this slice are timed per file in the same pairs. The before
  tree has none, so their absolute time is reported.

### 16.2 Run-time architecture

| part | where | language | role |
|---|---|---|---|
| reducers | `std/control.ignite` (as landed, P4: the reducers share the module of `Event`, `Effect` and `Step`, because a module below `std.control` could not import them and one above would be an E032 cycle, O45; the contract's `std/control/*.ignite` is void) | Ignite | supervisor, probe scheduler, contract evaluator, rollout: pure fns `step(machine, event) -> Step` (16.6) |
| shared vocabulary | `std/sli.ignite`, `std/time.ignite` | Ignite | `ProbeAttempt`, `satisfies`, `Window`, `MissingData`, `meets`; calendar arithmetic and the two time zones (O36): the leaves both the reducers and the kinds import (O45) |
| domain kinds and hooks | `std/contract.ignite`, `std/policy.ignite`, `std/protocol/telnet.ignite` (the probe since the layering fix, `docs/layering.md`), `std/time.ignite` | Ignite | `K.machine`, `K.validate`, the probe's `K.main`, calendar arithmetic |
| driver (the supervisor process) | `crates/ignite/src/run/` | Rust | event loop, adapters (process, probe, timer, build, journal, clock, control input), routing; it knows only the `std.control` types |
| evaluator at run time | `crates/ignite-eval` | Rust | steps the reducers (O27); decodes adapter results into values |
| substrate | `crates/ignite-io` (and the `ignite-rt` adapter) | Rust | monotonic clock, reference endpoints, drain on SIGTERM (16.13); nothing about supervision |

The supervisor process:

- The supervisor process is the `ignite run` process itself, in the foreground (design
  10.1). It is not a compiled Ignite program.
- Its children are the built executables: one per `NativeProcess` member, and one
  short-lived probe process per probe attempt.
- Each child is started in the run's process group with `PR_SET_PDEATHSIG(SIGKILL)`, so that
  a crash of `ignite run` leaves no orphan.
- prctl(2) ties that signal to the thread that forked, not to the process (A3). So the
  process adapter spawns every child, probes included, from one spawner thread that lives
  for the whole run. Other threads send it spawn requests.
- The run stops on SIGINT, on SIGTERM and on the control line `stop`:
  1. It journals `stopping`.
  2. It drains every process with SIGTERM.
  3. It waits up to each member's `DrainRollout.deadline` (10 s when the member has none).
  4. It sends SIGKILL to what is left.
  5. It journals `stopped` and exits.

The decision engine (design 2):

- The driver keeps one ordered queue of events.
- Each event is journaled, then stepped through the reducers it is routed to (16.9).
- Each effect a reducer returns is journaled, then dispatched (design 7.1).
- Adapters run on their own threads and return observations to the queue.
- Reducers never do IO, and adapters never decide.

Synchronous spawn acknowledgements use the driver's existing turn boundary:
after completing every record, effect dispatch and lifecycle recipient of the
current turn, deliver its already-known `Started` or `SpawnFailed` acknowledgement
before taking an unrelated queued observation. Keep the existing due-timer
precedence, acknowledgement order, and `Started` before that child's asynchronous
`Ready`/exit. Timestamp delivery with the current foreground clock; do not backdate
it to spawn time. Acknowledgements must drain at the current manual-clock instant
before settling/moving time, and before entering stop. Complete the initial `Start`
fanout to every machine before draining startup acknowledgements. The adapter's ordinary
channel API retains its existing behaviour. Every effect still synchronizes the
journal before dispatch (O30); processing a probe result cannot postpone publication
of an already completed spawn behind that probe's effect fanout. The existing driver
owns this scheduling; reducers retain policy ownership.

Library boundaries:

- Invariant 6: the driver knows the `std.control` types (`Machine`, `Event`, `Effect`,
  `Step`) and the hook names `machine` and `validate`. It knows no domain noun.
- `Restart`, `Availability` and `TelnetPrompt` are library symbols, read only by Ignite code
  (O18 carried over).
- Nothing named after telnet lives under `crates/`. The probe's protocol work is Ignite
  code: `TelnetPrompt.main` over a new telnet client-role fn (16.8).
- Layering of std (O45): `K.machine` returns `std.control.Machine` and must be declared in
  `K`'s module (E003, 13.8), so `std.contract` and `std.protocol.telnet` import
  `std.control`. `std.control` and every reducer module therefore import no kind's module
  (E032 otherwise): only the leaves `std.sli`, `std.policy` and `std.time`. The vocabulary
  the reducers consume (`ProbeAttempt`, `satisfies`, `Window`, `MissingData`, `meets`) moved
  from `std.probe` and `std.contract` to `std.sli`; `std.contract` and `std.protocol.telnet`
  (which holds the probe since `std/probe.ignite` was removed, `docs/layering.md`) import it
  from there, and the model prints `std.sli.ProbeAttempt` and `std.sli.Window`.

### 16.3 `ignite run` and `ignite replay`

```
ignite run <file> [--assembly NAME] [--state DIR] [--build DIR] [--target rust|llvm]
                  [--release] [--clock real|manual] [--epoch RFC3339] [--settle DURATION]
                  [--port RESOURCE=PORT]... [--json] [--fresh] [--std PATH] [--fuel N]
ignite replay <file> [--assembly NAME] [--state DIR] [--std PATH] [--fuel N]
```

Steps of `ignite run`:

1. Plan as `ignite plan` does: evaluate, then resolve (E110 to E119). Then check E120 to
   E125 and E129 (16.17).
2. Build every buildable member:
   - the build project lives in `<build>`, default `<state>/build/`;
   - the executables are copied into `<state>/bin/<generation>/`;
   - generation 1 is the start, and a rollout increments it.
3. Open the journal (16.5; E126, E127).
4. Construct the machines and start them.

As landed (P3): steps 2 and 3 run in the other order, the journal open and the E126 check
before the build, so a held lock or a foreign journal is reported before any cargo or clang
run. `ignite run` also takes `--runtime PATH` and `--target-dir DIR` (as `ignite build`),
fields `runtime` and `target_dir` of `RunOptions`. `--fuel` of `run` and `replay` bounds the
reducer steps only; property expressions keep the default fuel (15.3).

Paths and builds:

- `<state>` defaults to `target/ignite-run/<assembly>/` under the working directory.
- Builds with unchanged sources are a no-op in `<build>`. Tests pass one shared `--build`
  per target and never rebuild; each test has its own `--state`, because the journal lock
  (E126) admits one run per state directory (A2).

Ports:

- `--port Terminal.ingress=0` binds a free port.
- The driver reads the port from the child's `listening on <address>:<port>` line (6.1) and
  pins it for every restart of that member.
- A member's listener, the resource whose `--port RESOURCE=PORT`, pinned port and endpoint
  the driver uses, is the first `TcpListener` its executable declares
  (`ExecutableInfo.resources`), the one the generated `main` applies `--port` to on both
  targets, not the first by resource id (slice 17).

`WorkstationExecutable` members (A10):

- A member whose kind has `main` is built, not started, and the run prints its path.
- A member whose kind has no `main` is listed as "declared, not buildable" with the reason
  "its kind has no main" in the `status` output and on stderr, and in the `declared` field of
  the journal's first `segment`. This is not E129. The design file's `client: Application`
  is such a member.

Control input, one command per line on stdin:

| line | effect |
|---|---|
| `status` | prints the current standing of every contract and the state of every machine (one JSON line with `--json`); as landed (P3) also the `ports` in use and, per contract, the standing of the open window through `std.control.standing_at` |
| `rollout <member>` | starts the rollout machine of the member (16.12) |
| `stop` | the same as SIGINT |
| `advance <quantity>` | manual clock only: moves time (16.4); `<quantity>` is a Duration literal such as `100 ms` or `2 seconds` |

Output and exit status:

- One human line per journal record goes to stderr.
- With `--json`, the journal records themselves go to stdout; the tests synchronize on them.
- Exit 0: a requested stop with no failed machine.
- Exit 1: a machine gave up (`gave_up`) or failed (E128).
- Exit 2: a diagnostic before the start.

`ignite replay`:

- It reads the journal of `<state>` and checks the header against the current plan (E126).
- It re-steps the reducers with the journaled events and compares every effect and standing
  record (E127).
- It then prints the final standings.

### 16.4 Clock and time control

The driver owns the only clock the reducers see. Every event carries `at`, a `Duration` since
the journal's epoch. Reducers read no clock (actors R8).

`--clock real` (the default):

- `at` comes from the monotonic clock, offset by the segment's start.
- The epoch is the wall time at which the journal was created.
- A resumed segment continues from the wall time at which it starts (16.5).

`--clock manual`:

- Time moves only on `advance`.
- The epoch is `--epoch`, default `2026-01-01T00:00:00Z`.
- `advance d` fires every due timer in `(at, seq)` order, setting `now` to each timer's
  instant. Then it sets `now` to the target.
- The settle rule: at each instant, before moving on, the driver waits in real time until
  every in-flight adapter operation has completed, at most `--settle` (2 s by default;
  tests pass `--settle 1 s`, A2).
- An operation still running after the settle wait stays in flight. It completes at a later
  instant or at its deadline.
- A healthy probe completes at its dispatch instant, so its latency in manual time is 0.
- A hung probe (its target is SIGSTOPped) costs one settle wait, then times out when time
  reaches its deadline.

Other time rules:

- Calendar periods read the epoch plus `at` as UTC. `std/time.ignite` converts that to civil
  time in the zone (O36).
- Inline Ignite tests of the reducers use no clock: they step synthetic event lists with
  chosen `at` values.

### 16.5 The journal

- Location: `<state>/journal.jsonl`.
- `<state>/lock` is held for the run's lifetime; E126 when another run holds it.
- One JSON object per line, keys in byte order, values in the model encoding of 15.4, and
  `\n` after every record.

Line 1 is the header:

```
{"epoch":{"ns":…,"type":"Duration"},"formatVersion":"ignite-journal/0.1","kind":"header","model":{…the full model…},"seq":0}
```

Every other record has this shape:

```
{"at":{"ns":…,"type":"Duration"},"kind":"<kind>","machine":"<machine id or null>","seq":<n>,"value":<encoded std.control value>}
```

| kind | value | written when |
|---|---|---|
| `segment` | `std.control.Segment` (start, stop, clock mode, generation, declared members that are not buildable) | at run start, resume and stop |
| `event` | `std.control.Event` | before the reducers step it |
| `effect` | `std.control.Effect` | after the step and before dispatch, with `sync_data` before dispatch (O30) |
| `signal` | `{"ref": …, "value": …}`, an emission routed to subscribers (16.9) | when a reducer emits |
| `standing` | `std.control.Standing` (16.10) | at every evaluation |
| `consequence` | `std.control.Fired` (16.11) | at a period close with `violated` |
| `error` | the 14.8 report text plus the code (E128) | at a reducer failure |

Rules:

- `seq` is gap-free from 0.
- A run on an existing journal with the same header model and version appends a `segment`
  record (O31).
- `--fresh` first renames the old file to `journal.<n>.jsonl`.
- A final line without `\n`, from a crash while writing, is dropped on open and reported once
  on stderr. Any other unparsable line is E127.
- There are no snapshots. Resume and replay re-step from `seq` 1 (O30).

### 16.6 Machines: the reducer interface (`std/control.ignite`)

Slice 18 (22.3): `AttemptDone(Int, Text)`, `Emitted(Text, Verdict)` and `Emit(Text, Sample)`
replace the `ProbeAttempt` payloads below; `Evaluator` loses `good` and `threshold`; the hooks
the driver knows are `machine`, `validate` and `classify`.

```
enum Machine { Supervise(Supervisor), Schedule(ProbeScheduler), Evaluate(Evaluator), Roll(Rollout) }
type Step { machine: Machine, effects: List<Effect> }
type Event { at: Duration, kind: EventKind }
fn step(machine: Machine, event: Event) -> Step
```

`Event` is a record (as landed, O46): `at` is the Duration since the journal's epoch (16.4)
and `kind` an `EventKind`. Members and signals are named by their path text
(`Terminal.server`, `Terminal.promptProbe.attempts`), POSIX signals and pids are `Int`, and
a signal's element is `std.sli.ProbeAttempt`, the one element type of slice 12.

`EventKind` (final shapes, P4; the skeleton's lifecycle variants gained the pid, and
`Start` is new):

- `Start(Duration)`: the epoch, sent to every machine at segment start; it triggers the
  first `Spawn` and the first period timer;
- process lifecycle: `Started(Int, Int)` (pid, generation), `Ready(Int)` (pid),
  `Exited(Int, Int)` (pid, code), `Signalled(Int, Int)` (pid, signal), `SpawnFailed(Text)`;
- probe: `AttemptDone(Int, ProbeAttempt)`, `AttemptFailed(Int, Text)`;
- `Timer(Int)`;
- `Emitted(Text, ProbeAttempt)` (signal, value);
- build: `Built(Int)` (generation), `BuildFailed(Text)`;
- `Released`;
- control: `Control(Text)`.

`Effect` (final shapes, P4; `Kill` gained the pid, `Abort` is new):

- process: `Spawn(Text, Int)` (member, generation), `Kill(Text, Int, Int)` (member, pid,
  signal);
- probe: `Attempt(Text, Int)` (member, id), `Abort(Text, Int)` (member, id: kill the probe
  process of an attempt at its deadline);
- timers: `SetTimer(Int, Duration)` (id, the absolute instant), `CancelTimer(Int)`;
- outputs: `Emit(Text, ProbeAttempt)`, `Publish(Standing)`, `Fire(Fired)`;
- `Build(Text)`;
- `AwaitRelease(Text)`;
- `Note(Text)`, a journal line with no adapter.

The `Evaluator` machine carries `interval: Duration = 0 ms` (the driver fills it from the
observed probe), `epoch`, `current: Option<Bucket>`, `sum_good` and `sum_executed`;
`Standing.version` and `Fired.standing_seq` are 0 from the reducer and filled by the driver
with the record's `seq`. All four reducers live in `std/control.ignite` (16.2, O45).

The machine records (`Supervisor`, `ProbeScheduler`, `Evaluator`, `Rollout`), `Standing`,
`Fired`, `Segment` and their state enums are declared in `std/control.ignite` with defaults
for every field a reducer owns, so `K.machine` constructs them from the kind's fields only.
Hook signatures are checked by the typer: `K.machine(self, id: Text) -> std.control.Machine`
and `K.validate(self) -> List<Text>`, both pure; any other shape is E109 at the component's
kind (fixture `crates/ignite-types/tests/fixtures/e109_hook_signature.ignite`).

Construction:

- The evaluator calls `K.machine(self, id: Text) -> Machine` for every member and contract
  the assembly hosts whose kind defines `machine`.
- This happens at plan time, so the initial machines are part of the model (16.14).
- Machine ids are `<member path>.<role>`: `Terminal.server.supervisor`,
  `Terminal.promptProbe.scheduler`, `Terminal.reachable.evaluator`,
  `Terminal.server.rollout`.
- `Machine` is a closed std enum in slice 12. A user kind cannot define a new machine (O39).
- As landed (P3, P4): `K.machine` returns one `Machine`, and `Terminal.server` needs two
  (supervisor and rollout). The driver passes the member path as `id` (`Terminal.server`);
  the hook forms `<path>.<role>` in its payload's `id` field (`"{path}.supervisor"`), and the
  driver reads the machine id from there. The second machine comes from
  `std.control.companions(machine: Machine) -> List<Machine>`, which `machines()` calls after
  each hook (a supervisor whose `Restart` member carries a rollout policy yields the `Roll`
  machine); a nested `DrainRollout.machine` was not possible, because `std.policy` cannot
  import `std.control` (O45, E032). The hook check (E109) still covers every `fn K.machine`
  and `fn K.validate` of a module, records and enums alike, not only the kinds of components.

Failures:

- A reducer failure (assert, overflow, fuel per step O14) ends that machine only (T1).
- The run journals `error` with E128 and continues. The exit status becomes 1.

### 16.7 Supervisor and `Restart`

`TelnetServer.machine` returns `Supervise` with the member's `recovery: Restart`. The states
follow design 11.8: `Starting`, `Running`, `Backoff`, `Failed`, `Draining`, `Stopped`. The
supervisor also does the work of design 11.8's listener lifecycle (O44).

Readiness:

- `Ready` is the adapter's event for the child's `listening on` line. Generated `main` binds
  the listener before `K.main` (6.1).
- A member with no listener is ready at `Started`.
- With no `Ready` before `readiness_timeout`, the supervisor sends `Kill` and counts a
  failure.
- The new field is `Restart.readiness_timeout: Duration = 10 seconds` (O37).

Failures:

- An exit or signal that no `Kill` of this machine requested is a failure, exit 0 included,
  because a server does not end by itself.
- The attempt number `n` is the number of failures within the trailing `interval`, plus 1.
- If `allows(restart, n)`: the state becomes `Backoff`, with
  `SetTimer(at + delay(backoff, n))`. This is journaled as the effect `restart` with
  `attempt` and `delay`. At the timer comes `Spawn`.
- Otherwise: the state becomes `Failed`, with `Note("gave_up")`. Other machines go on (O38).
- `delay` and `allows` are the existing std fns, and their tests stay.

### 16.8 Probe scheduler and the probe process

Slice 18 (22.3): `AttemptDone` carries the probe's line as `Text`, decoded first by the
element type of the scheduler's signal; the scheduler emits `Sample { slot, line }` (the
deadline `line = None`) instead of a `ProbeAttempt` with the timeout as latency.

The probe executable:

- `TelnetPrompt` gains `executable: Text` (defaulting to the component name, the 6.1 rule)
  and `fn TelnetPrompt.main(self) -> IoResult<Unit>`.
- `ignite build` emits it as an executable like any kind with `main`. This changes builds of
  the design file only.

`main` runs one attempt:

1. Resolve `self.target` with `Endpoint.of` (16.9).
2. Connect.
3. Read through `Telnet.await_prompt(stream, expected_prompt)`. This is a new client-role fn
   in `std/protocol/telnet.ignite`: it answers option negotiation as `connect` does and
   returns the text before the prompt.
4. Measure `latency` with `Clock.monotonic()`.
5. Close if `close_after_probe`.
6. Print one line, the 15.4 encoding of `std.sli.ProbeAttempt` (O45).
7. Exit 0.

A refused or reset connection gives `received = false`, a bad attempt.

The scheduler: `TelnetPrompt.machine` returns `Schedule`.

- Routing of lifecycle events (A4): the driver sends a member's lifecycle events
  (`Started`, `Ready`, `Exited`, `Signalled`, `SpawnFailed`) to its supervisor. It also sends
  them to every scheduler that has an `edges.readiness` edge to that member (16.14). As
  landed (P3), the order is the supervisor, then the member's rollout machine, then the
  schedulers.
  Readiness is no signal and does not appear in `observations`.
- Slots: one every `interval`, from the target's first `Ready` on. Earlier slots are outside
  the evidence horizon, not missing (O40).
- At each slot the scheduler returns `Attempt(member, id)` and
  `SetTimer(deadline = at + timeout)`.
- The adapter spawns the probe process (through the spawner thread, 16.2) and decodes its
  stdout line by the signal's element type.
- `AttemptDone` before the deadline gives `Emit(attempts, value)`.
- The deadline first gives `Kill` of the probe process and
  `Emit(attempts, ProbeAttempt{received: false, latency: timeout})`. This is journaled as the
  effect `probe timeout`, a bad event (11.7).
- `AttemptFailed` gives no emission and a `Note("missing")`. Its causes: no executable, an
  exit status other than 0, a line that does not decode. The slot counts as expected and not
  executed.
- There is one probe process per attempt (O28). Probes of one member never overlap, because
  E123 guarantees `timeout < interval`.

### 16.9 Signals and references at run time

Slice 18 (22.3): an emission is a `std.sli.Sample` (slot and the probe's line); each
subscriber receives its own `std.sli.Verdict` in `Emitted`, built by the driver through the
subscriber's `classify`.

Signals:

- A `Signal<T>` value is a stream name, its 15.4 `ref` text.
- Only reducers emit, through `Effect.Emit`.
- The driver journals the emission as a `signal` record. It routes it as `Event.Emitted` to
  every machine whose model entry lists that ref in `subscribes`, in machine-id byte order.
- Lifecycle events are routed by 16.8's rule. Every other event goes to the machine it names.
- Compiled programs do not emit signals in slice 12. A probe's stdout line is an adapter
  result, not an emission.
- Pub/sub between processes is wave C (actors.md).

References:

- A `Ref<K>` value in a compiled program is its reference text (6.1).
- The new builtin `Endpoint.of(ref) -> IoResult<Endpoint>` resolves it through the process
  arguments `--ref <path>=<address>:<port>` that the driver passes.
- Outside `ignite run` it fails with "no binding for <path>". This mechanism is new for
  references, and works like the existing `--port`.
- The driver resolves a component ref to the endpoint of the single `TcpListener` resource
  with a `provisioning` edge to that component (16.14). It applies `--port` overrides and
  pinned ports first.
- No such resource, or several, is E122.

### 16.10 Contract evaluation

Slice 18 (22.3, D1, D14): `good` is a predicate `fn(T) -> Bool` the contract kind's `classify`
hook applies to the decoded line; the evaluator counts the `std.sli.Verdict` the driver builds,
in the bucket of the slot the scheduler dispatched the attempt for. `satisfies` and the
latency-derived slot below are the slice-12 form.

- `Availability.machine`, `ThresholdCompliance.machine` and `SLA.machine` return `Evaluate`.
  Each subscribes to its `signal`.
- A good event satisfies `satisfies(attempt, good, threshold)` (std/sli). `threshold` is 0
  for `Availability` and `SLA`.
- Population: the executed attempts in the window (11.7).
- Missing: the expected slots in the window that are not executed.

The `Standing` record:

- `contract`;
- `status`: `Compliant`, `Violated` or `InsufficientEvidence`;
- `good`, `executed`, `expected`;
- `completeness` (a Ratio);
- `from`, `to`;
- `coverage`: the Ratio of the window inside the evidence horizon;
- `calculation = "ignite-sli/0.1"`;
- `version`: the record's `seq`.

The status, decided in this order:

1. Apply `missing_data`: `counts_as_good` and `counts_as_bad` fill the missing slots.
2. `executed == 0` gives `InsufficientEvidence`.
3. `completeness < minimum_completeness`, when `missing_data` is `insufficient_evidence`,
   gives `InsufficientEvidence`.
4. `meets(target, good, executed)` gives `Compliant`; otherwise `Violated`.

Std changes:

- The three kinds gain the field `minimum_completeness: Ratio = 90%` (O32).
- `meets` changes: `total == 0` now gives `false`.
- Its inline test `meets(100%, 0, 0)` flips. The flipped test cites design 4.1: "Zero traffic
  does not automatically prove compliance" (A15).
- The new fn `standing(...)` is the evaluation entry.

Windows:

- `Rolling(d)` keeps counts in buckets of `max(signal interval, d / 10,000)` (O33). A 30-day
  window has 10,000 buckets.
- `CalendarMonth(zone)` keeps the current period's counts. The evaluator sets a timer at the
  next month boundary in `zone`.
- Evaluation runs on every emission and at every period boundary.
- Fuel (A7): an inline test steps an `Evaluate` machine with a 30-day `Rolling` window and
  `interval = 5 seconds` through 10,000 emissions in the evaluator, under the default fuel
  per step (O14). Item 12 records its time.
- If the fuel does not hold, O33 becomes `window / 1,000`, and that test is the evidence.
- As landed (P3): the fuel held, so O33 stays `window / 10,000`. The 10,000-step evaluator
  test takes 12.5 s and is `#[ignore]`; the gate runs the same test with 1,000 steps.

### 16.11 Consequences: `event("…")`

At an `SLA` period boundary, the evaluator publishes the closed period's standing.

- If the standing is `Violated`, the evaluator returns `Fire(Fired{contract, consequence,
  period_from, period_to, standing_seq})`.
- The journal record is the consequence's dispatch. Slice 12 has no external sink. Design 7.1
  requires the intent to be persisted; an adapter for delivery comes later.
- There is one `Fired` per contract and period, and replay and resume never fire it twice.
  The key is `(contract, period_from)`, checked against the journal.
- `InsufficientEvidence` fires nothing. It journals `Note("consequence withheld: insufficient
  evidence")` (invariant 5: no silent loss).
- `exclusions` must be empty in slice 12; anything else is E120. They are not evaluated, so
  accepting them would ignore them without a word.

### 16.12 `DrainRollout`

Design 11.8: "Until it does, planning rejects that rollout binding or requires an explicit
stop-and-start strategy with declared disruption. It must not simulate zero-downtime
behavior in its plan." Slice 12 declares the disruption (O34).

The declaration:

- The model's rollout machine entry carries `"disruption": "connections refused between
  listener close and rebind"`.
- `ignite explain Terminal.server.rollout` prints it.

The sequence, triggered by `rollout <member>`:

1. `Build` writes generation n+1 (a no-op build when the sources are unchanged, 16.3).
2. `Kill(old, SIGTERM)`: the old process drains (16.13).
3. `AwaitRelease`: the adapter reads the old process's stderr line `released
   <address>:<port>` (as landed, P2: printed on and read from stderr, not stdout). The drain
   prints it after the listener is closed, the `listening on` rule in reverse (A5). The
   adapter then sends `Released`. It does not connect-poll.
4. `Spawn(new)`, then await `Ready`.
5. The old process exits when its sessions end, and the journal records `retired`. If it
   still runs at `deadline`, `on_deadline` applies (`DisconnectRemaining` sends SIGKILL),
   and the journal records `retired forced`.

Rules:

- The journal records `disruption` with the measured interval from `Released.at` to
  `Ready.at`.
- The old process's exit during `Draining` ends the drain, and the supervisor counts it as a
  failure (16.7): it restarts the member. Corrected in slice 18 (22.16 Q8): this rule said the
  exit was not a failure, but the supervisor has always counted it, and the rollout relies on
  that restart since `fix/rollout-drain-stuck` (PR 46, the next rule). A failure of the new
  process follows 16.7.
- An old process that exits during `Draining` before `Released` ends the drain: the journal
  records `retired`, the supervisor restarts the member (16.7) and that restart is the new
  process; the rollout spawns none of its own, so no second process races the restart for
  the port. The disruption counts from the exit.
- At `deadline`, a rollout whose new process has not started aborts (`rollout aborted:
  generation <n> did not start by the deadline`) and kills nothing. A rollout past its
  deadline does not refuse the next `rollout <member>`.
- `on_deadline` stays `Text`, with the closed set `{"disconnect_remaining"}` (O35).
  `fn on_deadline(text) -> Option<OnDeadline>` parses it, as `missing_data` does; any other
  value is E120.
- `BuildFailed` aborts the rollout before step 2, and the old process keeps serving.

### 16.13 Substrate additions (`crates/ignite-io`, both targets)

| builtin | behaviour | test |
|---|---|---|
| `Clock.monotonic() -> Duration` | the time since an arbitrary start of the process; it never decreases | inline: two reads are ordered |
| `Endpoint.of(ref: Ref<K>) -> IoResult<Endpoint>` | the `--ref path=address:port` argument for `ref`'s text; otherwise the IoError "no binding for <path>" | end to end (`prober`), as landed (P2): a `Ref<K>` exists only inside a system, so no inline test can call it |
| drain on SIGTERM | see the rules below | e2e: an open session survives SIGTERM; a new connect is refused after `released` |

The drain on SIGTERM:

- A signal thread (`sigwait`, with SIGTERM blocked in all other threads) sets drain mode.
- It shuts down the read side of every listener and prints `released <address>:<port>` on
  stderr for each one once it is closed (A5; as landed, P2: stderr, not stdout).
- A pending `accept` fails with "draining".
- After `K.main` returns, `run` sends what `main`'s task wrote, waits for the live tasks to
  end, then exits 0.
- `Process.exit(code)` during the drain sends what the calling task wrote and exits with
  `code` at once; live tasks are not waited for (story `drain-exit`, the case study's G16). On
  the LLVM target it lowers to the runtime entry `ignite_rt_process_exit`, not to the
  `ignite_rt_exit` the generated main calls when `K.main` returns.
- `Process.exit`'s send is bounded, draining or not: it waits for a write lock another task
  holds and for room in a socket's buffers, in all at most 100 ms after the call. A stream
  whose lock is still held then (a task stalled in a send to a peer that does not read) is
  skipped, and what a socket still has no room for is lost. When `K.main` returns, `Ok` or
  `Err`, both targets send `main`'s writes without that bound, as before.
- A second SIGTERM exits at once.
- A listener kept through the drain (story `drain-keeps-listeners`, the case study's G6): a
  `TcpListener` resource with the optional fifth property `keep_through_drain: Bool` (no
  default; the model shows it only where it is given) set to `true`. The drain does not
  release it and prints no `released` line for it; its `accept` keeps accepting until the
  process exits. Every other listener is released as above. The Rust target needs the value
  as a literal (E020 otherwise, as for `address` and `port`); the generated main binds it with `ignite_io::TcpListener::bind_kept`, on the LLVM target through the runtime
  entry `ignite_rt_tcp_bind_kept(address, port, kept, out, err)`, declared only in a main that
  binds a listener with the property, so other programs' IR is unchanged.
- A listener bound at run time is kept the same way with the static `TcpListener.bind_kept(address:
  Text, port: Int) -> Result<TcpListener, IoError>` (story `drain-keeps-bound-listeners`, the
  case study's G31), effect `net.listen` as `bind`: its address and port are any values, read from
  the environment or configuration, not literals. Rust lowers it to
  `ignite_io::TcpListener::bind_kept`, LLVM to `ignite_rt_tcp_bind_kept(address, port, 1, out,
  err)`, declared only in a module that calls it. `TcpListener.bind` is released as before. Test:
  `drain::a_listener_bound_kept_at_run_time_accepts_through_the_drain`, both targets.
- The wait for live tasks after `K.main` returns does not wait for a task that is blocked in
  `accept` on a kept listener or in a read of a stream a kept listener accepted: the process
  exits 0 once every other task has ended, so a kept admin or internal listener never holds
  up the drain's end. A task that is serving a kept connection (not blocked in its read) is
  waited for as any other. `main` itself is no task: `main` blocked in a kept `accept` never
  returns, as before.
- As landed (P2): a SIGTERM's in-flight guard ends at `released`; a SIGTERM to a program
  without a listener (`tn`) drains too: it waits for `main`, and a second SIGTERM ends it.
- Outside `ignite run`, the wait for live tasks has no bound. Only the driver's deadline
  bounds it (A6).
- Existing tests end servers with `Child::kill` (SIGKILL), so the SIGTERM change touches none
  of them.

Other rules:

- The substrate has no process spawning and no journal API. Both are driver code; a compiled
  program does neither.
- Lowering: rustgen calls the builtins directly; ignite-llvm calls them through C ABI entries
  in `ignite-rt`. The type checker knows their signatures.
- Builds of programs that use none of them stay byte-identical (16.0 item 11).
- As landed in the skeleton: `Clock.monotonic` carries the ambient effect `clock.read` and
  `Endpoint.of` the effect `process.args` (its source is the process arguments), both in
  `ignite-check`'s `EFFECTFUL_STATICS` and the typer's `builtin_effect`. `Endpoint` is both
  a builtin static owner and the record `std.net.Endpoint`; `ignite-check`'s
  `STD_RECORD_STATIC_OWNERS` exempts it from E004, so `std/net.ignite` keeps `type Endpoint`
  and must not declare `fn Endpoint.of` (it would shadow the builtin; its declaration there
  is a doc comment). `Clock` is a static owner with no record: std declares no `type Clock`.

### 16.14 Model additions (additive, `ignite-model/0.1`)

| key | content |
|---|---|
| `machines` | array sorted by id: `{"id", "machine": <encoded initial Machine value>, "member", "subscribes": [refs], "disruption"?}` |
| `observations` | array sorted by ref: `{"ref", "producer", "element": "<qualified type>", "interval"}` |
| `edges.provisioning` | `TcpListener` resource → the component that binds it (`Terminal.ingress` → `Terminal.server`); no other resource kind provisions its component (a `Store`, the capabilities of 21.3; slice 17) |
| `edges.readiness` | probe → its target; evaluator → its signal's producer; the routing of 16.8 reads it |
| `edges.recovery` | member → its supervisor machine |

- O25's deferred edge classes land here.
- `ignite explain` accepts machine ids as paths. It prints the machine's state fields and its
  `disruption`.

### 16.15 Replay

- `ignite replay` and the end-to-end tests share one fn. It takes the model and the journal's
  `event` records and re-steps the machines. It produces the `effect`, `signal`, `standing`
  and `consequence` records.
- They must equal the journaled ones byte for byte. The first difference is E127, with its
  `seq`.
- The values a real process contributes (`pid`, `latency`, `code`) enter only through `event`
  records, so replay is exact.
- Design 7 is the source: "Interpreter and native execution share semantic tests and replay
  traces" (A13).

### 16.16 Typed API (add-only)

- `ignite_eval`:
  - `Value::decode(json: &str, ty: &Type) -> Result<Value, DecodeError>`, the inverse of
    15.4;
  - `Evaluator::call(&mut self, fn path, args) -> Result<Value, EvalError>`;
  - `Model.machines` and `Model.observations`.
- `ignite`:
  - `RunOptions{assembly, state, build, target, release, clock, epoch, settle, ports, json,
    fresh, std, fuel}`;
  - `run(&Path, &RunOptions) -> Result<i32, Diagnostics>`;
  - `replay(&Path, &ReplayOptions) -> Result<Vec<Standing>, Diagnostics>`.
- `ignite-types`: the signatures of the three builtins of 16.13.

As landed in the skeleton (P1, 2026-11-02), where it differs from the list above:

- `ignite_eval`: `Value::decode(json: &str, ty: &Ty)` (the typer's type is `Ty`);
  `DecodeError { message }`; `machines(&mut Evaluator, &TypedProgram, &SystemInfo,
  &[ModelMember], &[ResolutionEntry]) -> Result<Vec<ModelMachine>, Vec<Diagnostic>>` and
  `observations(&TypedProgram, &SystemInfo, &[ModelMember], &[ResolutionEntry]) ->
  Result<Vec<ModelObservation>, Vec<Diagnostic>>` are free fns beside `model`, which
  `Builder` calls to fill `Model.machines` and `Model.observations`; `ModelMachine { id,
  machine: Value, member, subscribes, disruption }`, `ModelObservation { reference, producer,
  element, interval }` (16.14); `validate(&mut Evaluator, &TypedProgram, &SystemInfo,
  &[ModelMember], &AssemblyInfo, &[ResolutionEntry]) -> Result<(), Vec<Diagnostic>>` (16.17).
- `ignite`: `RunOptions { assembly, state, build, target: Target, release, clock: ClockMode,
  epoch: Option<String>, settle: Option<String>, ports: Vec<(String, u16)>, json, fresh, std,
  fuel }`, `ClockMode { Real, Manual }`, `ReplayOptions { assembly, state, std, fuel }`,
  `Standing { contract: String, value: Value }`; `run(&Path, &RunOptions) -> Result<i32,
  Error>` and `replay(&Path, &ReplayOptions) -> Result<Vec<Standing>, Error>`: `Error` is the
  crate's existing error (`Diagnostics`, `Cargo`, `Clang`, `Io`); no `Diagnostics` type
  exists. `main.rs` maps `Error::Diagnostics` of `run` to exit 2 (16.3) and of `replay` to
  exit 1 like `plan`; a malformed `--port` is exit 2.
- `ignite-types`: `Builtin::ClockMonotonic` and `Builtin::EndpointOf` with the signatures
  `Clock.monotonic() -> Duration` and `Endpoint.of(Ref<K>) -> IoResult<Endpoint>`
  (`builtins::static_fn`), their effects in `typer/effects.rs`, and the hook signature
  checks of 16.6 in `typer/system.rs` (E109, `hook_signature`, one pass per module over
  every `fn K.machine` and `fn K.validate`).
- `ignite-types`, for the hooks (P4's request): `Ref<K>.to_text()` and `Signal<T>.to_text()`
  are `Builtin::TextToText -> Text`, and a `Ref` or `Signal` interpolates as its path text
  (`"{self.target}"`). Both types lower to `Text` on both targets (14.14), so rustgen needs
  no change; ignite-llvm's interpolation arm admits the two types (P2); the evaluator's
  `TextToText` arm accepts a member or signal reference and gives its path (P3).
- `ignite_eval` (step 2): `validate_system(&mut Evaluator, &TypedProgram, &SystemInfo,
  &[ModelMember]) -> Result<(), Vec<Diagnostic>>` beside `validate`, split so that E120,
  E123 and E124 are reported once per system and E121, E122 and E129 once per assembly
  (16.17); `resolve::entries_fn` beside `binding_fns`.
- As landed by P2 and P3: `Value::decode(json: &str, ty: &Ty, program: &TypedProgram)` takes
  the typed program (record field order and enum payloads come from it), so its tests live in
  `crates/ignite/tests/decode.rs`, not in `ignite-eval`; `RunOptions` gains `runtime:
  Option<PathBuf>` and `target_dir: Option<PathBuf>` (`--runtime`, `--target-dir`, 16.3);
  the adapters of `crates/ignite/src/run/` are `await_release(..) -> Bool` (whether the
  `released` line arrived before the deadline), `pids(member) -> Vec<i32>`, `kill(pid,
  signal)` and `kill_attempt(..)` for a probe's deadline, in place of the skeleton's
  `spawn`/`kill` pair.

### 16.17 Diagnostics E120 to E129

| code | rule | phase | fixture |
|---|---|---|---|
| E120 | a declaration its kind's `validate` refuses, with the reason text from std: an unknown `missing_data`, `on_deadline` or time zone (an unknown `good` outcome until slice 18; an unknown `good` predicate is E003 since, 22.10); `maximum_attempts` < 1; `target` not in (0 %, 100 %]; non-empty `exclusions`; a backoff initial greater than its maximum | check | `e120_validate_refused` |
| E121 | a contract's `signal` has no producer the assembly runs: its member is not bound to a host that runs it, or the ref is no output | check | `e121_signal_without_producer` |
| E122 | a run-time `Ref` (a probe's `target`) names a member the assembly does not run as a process, or resolves to no endpoint or to several | check | `e122_unresolvable_ref` |
| E123 | probe timing: `interval` or `timeout` not positive, or `timeout >= interval` | check | `e123_probe_timing` |
| E124 | a contract window shorter than its signal's `interval`, so it can hold no expected attempt | check | `e124_window_too_short` |
| E125 | `ignite run` on a file without an assembly, or on an assembly that runs nothing (no member with a machine) | run | `e125_nothing_to_run` |
| E126 | the state directory's journal has another model or `formatVersion`, or another `ignite run` holds its lock | run, replay | `e126_journal_mismatch` |
| E127 | a journal that does not replay: an unparsable line before the last, or a re-derived record that differs (first `seq`) | run (resume), replay | `e127_replay_diverged` |
| E128 | a reducer failed at run time (14.8 report text; fuel per step); the machine stops, and the run continues and exits 1 | run | `e128_reducer_failed` (a run with `--fuel 1`) |
| E129 | a member the assembly hosts whose kind lacks the entry its host needs: `ScheduledProbeHost` without `K.main` or `K.machine`, `NativeProcess` without `K.machine` | check | `e129_missing_entry` |

- Phase "check": `ignite check` and `ignite plan` report these codes for every assembly (O20
  carried over).
- `ignite check` of the design file still exits 0 with no output, which
  `cli::check_design_file_is_silent` asserts (A14).
- `validate` is a hook like `machine`: `fn K.validate(self) -> List<Text>`, evaluated purely.
  The compiler knows no outcome names and no zone names (invariant 6).
- Fixtures live in `crates/ignite/tests/fixtures/`.
- E129 is the last code of this slice (T4).
- A `machine` or `validate` fn with another signature, or an effectful one, is E109 (the
  slice-10 code for the `K.main` signature, 14.11, extended to the two hooks), reported by
  the typer at the fn's name, for every `fn K.machine` and `fn K.validate` of a module
  (records and enums alike, so the kinds of property values such as `DrainRollout` are
  covered): fixture `crates/ignite-types/tests/fixtures/e109_hook_signature.ignite` (O47).

As landed (P1, `crates/ignite-eval/src/validate.rs`), the check-phase codes read the
program as follows; the compiler knows no kind, binding variant, outcome or zone name:

- `validate_system` runs per system (E120, E123, E124), so a program without an assembly
  gets them too; `validate` runs per resolved assembly (E121, E122, E129), after `resolve`
  succeeded. `Builder` calls both (P3's `model.rs`).
- E120: every member whose kind's module declares `fn K.validate` is validated as the record
  value of its declaration (an unset `Signal<T>` output filled with its own ref), and so is
  every record value set in a property whose type's module declares `validate`, recursively
  (`recovery = Restart { .. }` is a declaration of a `Restart`); each text the hook returns is
  one line, ``declaration `S.m[.property]` of kind `K` is refused: <text>``, at the member or
  the property. A hook that fails is E115 or E117 as any evaluation.
- E121, E122 and E129 rest on a third fn of the binding type's module, found by name like
  `admits` and `includes` (O18): `fn entries(binding: B) -> List<Text>`, the fns the hosted
  kind must declare for the binding to run it (`std.deploy`: `["main", "machine"]` for
  `NativeProcess` and `ScheduledProbeHost`, `[]` for `LocalSocket` and
  `WorkstationExecutable`, the A10 rule). A binding with a non-empty list runs its member;
  an included member runs with its host. Without `entries` the three codes are not checked.
- E121: every signal ref `S.p.f` in a property, at any depth (since slice 18 the collection
  `ModelMachine::subscribes` reads, `signal_refs` in `crates/ignite-eval/src/model.rs`): `f`
  must be an unset output of `S.p` (else "is no output"), and `S.p` must run.
- E122: for every member that runs, every property whose field type is `Ref<_>` and whose
  value names a member `T`: `T` must run, and exactly one `TcpListener` resource must appear
  among `T`'s property values (the `provisioning` edge of 16.14); zero or several is E122.
- E123: a member with an output (a producer) whose `Duration` properties `interval` or
  `timeout` are not positive, or `timeout >= interval`; the names are the convention of
  `observations.interval` (16.14).
- E124: a member with a signal-ref property reading producer `S.p` with a `Duration`
  `interval`, and a property of type `std.sli.Window` with variant `Rolling(d)`: `d <
  interval` is E124 (`CalendarMonth` is never too short).
- E129: for every bindings key whose binding has entries: `main` is declared when the kind is
  an executable (14.5), any other entry when the kind's module declares `fn K.<entry>`.
- The fixtures carry their own binding type with `admits`, `includes` and `entries` and their
  own kinds with `validate`, so they hold whatever `std/deploy.ignite` and the std kinds
  declare; `e124` imports `std.sli.Window`.

### 16.18 Work split

| agent | owns |
|---|---|
| P0 | none (16.1) |
| P1 frontend and skeleton | `ignite-syntax`, `ignite-check`, `ignite-types` (the builtin signatures of 16.13, the hook signature checks); `crates/ignite-eval/src/resolve.rs` and the new `validate.rs` (E120 to E124, E129); `std/control.ignite` type declarations only (`Machine`, `Event`, `Effect`, `Step`, `Standing`, `Fired`, `Segment`), written first, then handed to P4; the `run` and `replay` CLI stubs and the `crates/ignite/src/run/` module stubs with their placeholders (16.20); `docs/slice.md` for the whole slice |
| P2 substrate and adapters | `crates/ignite-io` (16.13), the `crates/ignite-rt` entries, and the lowering of the three builtins in `ignite-rustgen` and `ignite-llvm`; `crates/ignite/src/run/{process,probe,clock,journal,control}.rs` (the adapters with the spawner thread, the manual clock with the settle rule, the journal file and lock); substrate tests in `crates/ignite-io/tests/` |
| P3 driver loop, model, replay, tests | `crates/ignite-eval/src/{model,value,eval,explain}.rs` (16.14, `decode`, `call`); `crates/ignite/src/run/{mod,engine,replay}.rs`, `crates/ignite/src/lib.rs` and `main.rs`; `crates/ignite/tests/**`, including `main.rs` and `common/mod.rs`; E125 to E128; the model and explain goldens; the acceptance tests of items 1 to 3 and 5 to 9 |
| P4 std, examples, records, acceptance | the `std/control/*.ignite` reducers with inline tests (the A7 fuel test among them), `std/time.ignite`, `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite` (`main`, `machine`, `validate`), `std/protocol/telnet.ignite` (`await_prompt`), `std/net.ignite` (the `Endpoint.of` declaration); the reducer trace fixture of item 10 (the file goes in `crates/ignite/tests/fixtures/`; its test comes by message to P3); `examples/**`; `AGENTS.md` (layout rows, the slice-12 gate-time row); `docs/friction.md`; design notes in 10.1 (`run` and `replay` exist) and 11.8; acceptance items 4 and 11 to 13, verified with commands |

Order:

- P1 comes first: the skeleton and the `std/control.ignite` types.
- Then P2, P3 and P4 run in parallel. P4 starts with `std/control` and `std/probe`, because
  P3's engine steps them.
- 16.12 (`DrainRollout`) and the drain row of 16.13 are the last units of P2 and P4. They
  start after acceptance items 1 to 5 and 7 to 10 pass (A12).
- If the slice runs long, the close reports the rollout as the carried item, and the rest
  closes. This is no cut: vision row 12 keeps `DrainRollout` in the slice.
- P4's acceptance comes last.

Shared files:

- `crates/ignite/tests/main.rs` and `tests/common/mod.rs` are P3's. P2 and P4 ask P3 for
  their `mod` lines.
- After the skeleton, `std/control.ignite` changes only by message between P3 and P4.

Rules for tests and dependencies:

- One integration-test binary per crate: new files are modules of the existing `main.rs` of
  `crates/ignite`, `crates/ignite-io` and `crates/ignite-eval`. No `[[test]]` entry is added.
- `nix` moves from a dev-dependency to a dependency of `crates/ignite`, for pdeathsig,
  process groups and signals (O43). It is already a workspace dependency of `ignite-io`, so
  no new crate is added.

### 16.19 Decisions taken in the review

| # | decision |
|---|---|
| O27 | Reducers run in the evaluator inside `ignite run`. The trace fixture of item 10 also compiles them on both targets. Steering amended vision 4.1 to say so (A1). Wave C revisits a compiled controller with actors |
| O28 | One probe process per attempt. `latency` is the probe's own connect-to-prompt time. A resident probe host is a later milestone |
| O29 | Tests control time with the manual clock, driven by `advance` on stdin, and the settle rule of 16.4. The default is `--settle 2 s`; tests pass `--settle 1 s` (A2) |
| O30 | `sync_data` after every `effect` record, before dispatch. No snapshots; resume re-steps from `seq` 1 |
| O31 | A run appends to a journal with the same model. The gap between segments is missing evidence. `--fresh` renames the old journal |
| O32 | A new field `minimum_completeness: Ratio = 90%` on `Availability`, `ThresholdCompliance` and `SLA`. Zero executed attempts give `insufficient_evidence`, and `meets(_, 0, 0)` becomes `false`. Source: design 4.1, "Zero traffic does not automatically prove compliance" (A15) |
| O33 | Buckets of `max(interval, window / 10,000)`, calculation version `ignite-sli/0.1`. They become `window / 1,000` if the fuel test of 16.10 fails (A7). Outcome: the fuel held, `window / 10,000` stays; the 10,000-step test is `#[ignore]` at 12.5 s and the gate runs 1,000 steps |
| O34 | `DrainRollout` without a handoff is declared by the plan (`machines[].disruption`). No user field is added, and the rollout is not rejected. Design 11.8: "Until it does, planning rejects that rollout binding or requires an explicit stop-and-start strategy with declared disruption." (A9) |
| O35 | `on_deadline` stays `Text`, with a closed set checked by `validate`. This reverses the std comment that announced an enum, and the design file stays unchanged |
| O36 | Time zones are a closed set in `std/time.ignite`: `UTC` and `Europe/Berlin` (the EU DST rule, in Ignite). Any other zone is E120 |
| O37 | Readiness is the child's `listening on` line. `Restart.readiness_timeout = 10 seconds`; a timeout counts as a failure |
| O38 | A member with exhausted restarts stays `Failed`. The run continues and exits 1 at stop |
| O39 | No user-defined machines in slice 12: `Machine` is a closed std enum |
| O40 | Expected slots start at the target's first `Ready`. Partial windows are evaluated and report `coverage` |
| O41 | All `ignite run` end-to-end tests together add at most 6 s of wall time to `crates/ignite/tests`. Only the hung-probe scenario (one test, one target) pays a settle wait. Tests build the design file once per target into one shared `--build` and never rebuild. They never use port 2323 (A2) |
| O42 | `TelnetPrompt` gains `executable` (defaulting to the component name) and `main`, so the design file builds a `promptProbe` binary |
| O43 | `nix` becomes a normal dependency of `crates/ignite`; no new crate |
| O44 | Design 11.8 lists five machines, and slice 12 has four (A8). It folds the listener lifecycle into the supervisor: `listening on` gives `Ready`, and the `released` line gives `Released`. The design row is "Listener lifecycle: Observe, bind or adopt, report ready, release according to ownership". Adopt and bind ownership are slice 13's |
| O45 | std layering (P1, skeleton; steering confirmed): `K.machine` returns `std.control.Machine` and must live in `K`'s module (E003), so the kinds `std.probe`, `std.contract` and `std.protocol.telnet` import `std.control`, and `std.control` cannot import them back (E032). The shared vocabulary (`ProbeAttempt`, `satisfies`, `Window`, `MissingData`, `meets`) therefore moved to the new leaf `std/sli.ignite`, which `std.contract`, `std.probe` and `std.control` import. Slice 13 must not move these types back. Consequence for P4: reducers that `step` dispatches to live in `std.control` itself or in modules `std.control` imports, never in a module that imports `std.control` |
| O46 | `Event` is a record `{ at: Duration, kind: EventKind }`, not an enum with `at` in some payloads: every event has a time, and the engine fills it once. The one signal element type of slice 12 is `std.sli.ProbeAttempt`, so `AttemptDone`, `Emitted` and `Emit` carry it directly; a generic `Value` payload needs generics on `Machine`, which is closed (O39) |
| O47 | Hook signature errors reuse E109 (the `K.main` code of 14.11) instead of a code from E120 to E129: one code for "a hook of the driver has the wrong signature", and the slice's range stays for the semantic rules of 16.17 |

### 16.20 Placeholder arms

P1's skeleton makes the workspace build with `--all-targets`. Each placeholder is
`unimplemented!("slice 12: …")`, and its owner replaces it:

| owner | file | fn | covers |
|---|---|---|---|
| P2 | `ignite-io/src/clock.rs` | `monotonic` | 16.13 |
| P2 | `ignite-io/src/net.rs` | `endpoint_of` | 16.9, 16.13 |
| P2 | `ignite-io/src/lib.rs` | `drain` (signal thread) | 16.13 |
| P2 | `ignite/src/run/process.rs` | `spawn`, `kill` | 16.2, 16.7 |
| P2 | `ignite/src/run/probe.rs` | `attempt` | 16.8 |
| P2 | `ignite/src/run/clock.rs` | `Manual::advance` | 16.4 |
| P2 | `ignite/src/run/journal.rs` | `Journal::open`, `Journal::append` | 16.5 |
| P3 | `ignite/src/run/engine.rs` | `Engine::step` | 16.2, 16.6 |
| P3 | `ignite/src/run/replay.rs` | `replay` | 16.15 |
| P3 | `ignite/src/lib.rs` | `run`, `replay` | 16.3 |
| P3 | `ignite-eval/src/value.rs` | `Value::decode` | 16.16 |
| P3 | `ignite-eval/src/model.rs` | `machines`, `observations` | 16.14 |
| P1 | `ignite-eval/src/validate.rs` | `validate` | 16.17 |
| P2 | `ignite-rustgen/src/lib.rs` | the `("Clock", "monotonic")` and `("Endpoint", "of")` arm of `static_call` | 16.13 |
| P2 | `ignite-llvm/src/lower.rs` | the `ClockMonotonic | EndpointOf` arm of `builtin` | 16.13 |

- That is 19 placeholders in 15 rows, as landed: the contract's 17 plus the two compiler
  arms, which the contract's "lowering" row of 16.18 covers but its table did not list.
  `grep -rn 'slice 12: ' crates/ | wc -l` counts them (every placeholder text starts so;
  no doc comment does).
- The std fns have no placeholders: P4 writes them on top of P1's type declarations.
  `std/control.ignite` has one body, `step`, which returns the machine unchanged with no
  effects; P4 replaces it.
- The placeholder signatures of `crates/ignite/src/run/*.rs` are provisional; the owner
  replaces the fn, not only its body. `crates/ignite/src/run/mod.rs` carries
  `#![allow(dead_code)]` until the engine calls the adapters; P3 removes it.
- `Builtin::ClockMonotonic` and `Builtin::EndpointOf` are also in `ignite-llvm/src/region.rs`
  (`mutates`: neither mutates an argument) and in `ignite-eval/src/eval.rs` (`is_static`), with
  no placeholder: the typer keeps them out of what the evaluator runs (E116 in a property
  expression, E109 in a hook), and `builtin_static`'s fallback arm reports an internal error
  for any effectful static that reaches it.

### 16.21 Implementation record

Landed by P1 on 2026-11-02 (the skeleton of 16.18 and 16.20); where the implementation
differs from the reviewed contract or refines it:

1. `std/sli.ignite` is a new leaf module (O45, 16.2): `ProbeAttempt`, `satisfies`,
   `Window`, `rolling`, `calendar_month`, `MissingData`, `missing_data`, `meets` and their
   inline tests moved there from `std/probe.ignite` and `std/contract.ignite`, which import
   them. The model golden `crates/ignite/tests/golden/terminal.design.model.json` changed in
   four lines (`std.probe.ProbeAttempt` to `std.sli.ProbeAttempt`, three
   `std.contract.Window` to `std.sli.Window`), re-blessed with `IGNITE_BLESS=1`. Inline tests:
   103 per target (102 before; the `Window` asserts became their own test).
2. `std/control.ignite` holds every type of 16.6 with field defaults, `Event` as a record
   (O46), `EventKind`, `SupervisorState`, `Bucket`, `Status`, `RolloutState`, and a `step`
   that returns the machine unchanged. It imports `std.sli` and `std.policy` only.
3. Hook signatures are E109 (O47), checked in `ignite-types` (`typer/system.rs`, `hooks`)
   for every component whose kind declares `machine` or `validate`; fixture
   `e109_hook_signature` (three diagnostics: wrong return, extra parameter, effectful).
4. The builtins `Clock.monotonic` and `Endpoint.of` are statics in `ignite-check` (tables and
   `static_effect`: `clock.read`, `process.args`) and `ignite-types` (`Builtin::ClockMonotonic`,
   `Builtin::EndpointOf`); `STD_RECORD_STATIC_OWNERS` keeps `type Endpoint` in `std/net.ignite`
   free of E004 (16.13). Their lowering arms in `ignite-rustgen` and `ignite-llvm` are
   placeholders, so 16.20 has 19 placeholders in 15 rows, not 17 in 13.
5. `ignite` API as 16.16 "as landed": `run` and `replay` return `Error`, not a `Diagnostics`
   type; `ClockMode`, `Standing { contract, value }`; `main.rs` parses `--port RESOURCE=PORT`
   and maps a diagnostic of `run` to exit 2. `nix` is a dependency of `crates/ignite` (O43).
6. `ignite_eval`: `machines`, `observations` and `validate` are free fns taking the
   `Evaluator`, the `TypedProgram`, the `SystemInfo`, the evaluated `ModelMember`s and the
   `ResolutionEntry`s (and `validate` the `AssemblyInfo`), the shape `model` and `resolve`
   have; `ModelMachine`, `ModelObservation`, `DecodeError` are starting shapes in P3's files.
7. Tests at the skeleton: `cargo test -p ignite-syntax -p ignite-check -p ignite-types -p
   ignite-eval` green (216 tests); `cargo test -p ignite` 163 passed,
   3 ignored after the re-bless; clippy `-D warnings` and `cargo fmt --check` clean on the
   workspace; `ignite check` of every std file, both examples and the design file exits 0
   with no output.

Step 2 of P1 (the validators), landed 2026-11-02 while P2 to P4 worked:

8. `validate` split in two (16.16): `validate_system` for E120, E123 and E124 once per
   system, `validate` for E121, E122 and E129 once per resolved assembly, so a system with
   two assemblies reports a refused declaration once and a program without an assembly is
   still validated. P3 calls both from `Builder` (`system`, `assembly`).
9. The notion "the assembly runs this member" is the binding type's `fn entries(binding) ->
   List<Text>`, found by name as `admits` and `includes` are (O18; `resolve::entries_fn`),
   not a variant name: the compiler keeps knowing no binding variant (invariant 6). P4 adds
   it to `std/deploy.ignite` with the hooks; until then E121, E122 and E129 are not checked,
   and the design file stays silent (A14).
10. E120 also validates record values set in properties (`recovery = Restart { .. }`), since
    the design file declares `Restart` and `DrainRollout` only that way; one line per text
    the hook returns. E123 and E124 read the properties `interval` and `timeout` and the
    `std.sli.Window` value by name and type, with no hook: the rules are arithmetic the
    contract fixes, and a hook would let std weaken them.
11. E109 moved from the component's kind to the hook's declaration and covers every
    `fn K.machine` and `fn K.validate` of a module (records and enums), because the hooks are
    found by name wherever they are declared (`validate` on property kinds such as `Restart`,
    16.6); the fixture's three lines moved to lines 7, 9 and 15. The second machine of a
    member comes from `std.control.companions` (16.6, P3 and P4), not from a nested hook.
12. `Ref<K>.to_text()`, `Signal<T>.to_text()` and their interpolation are `TextToText`
    (16.16), for the hooks that fill `ProbeScheduler.target` and `.signal` with path text.
13. Fixtures `crates/ignite/tests/fixtures/e120_validate_refused`,
    `e121_signal_without_producer`, `e122_unresolvable_ref`, `e123_probe_timing`,
    `e124_window_too_short`, `e129_missing_entry` (`.ignite` and `.expected`; 3, 1, 3, 3, 1,
    3 lines), each with its own binding type and kinds, verified through a scratch harness
    that calls the two fns as `Builder` does; P3's `plan.rs::fixtures_match_expected` runs
    them with the `e11` fixtures.
14. Invariant 9, with P3's wiring in place (15.1 method, 4 pairs, load 1.66 on every run,
    none dropped; `target/debug/ignite` of each tree on its own
    `examples/terminal/terminal.design.ignite`): `ignite check` 0.059 to 0.063 s before,
    0.061 to 0.063 s after, median pair ratio 1.03; `ignite plan` 0.059 to 0.061 s before,
    0.062 to 0.063 s after, median 1.04. `cli::check_design_file_is_silent` and
    `plan::fixtures_match_expected` pass; `ignite check` of every std file, both examples and
    the design file exits 0 with no output; `cargo test -p ignite-syntax -p ignite-check -p
    ignite-types` 207 passed; clippy and fmt clean on P1's files (the workspace gate waits
    for P2 to P4's in-progress files).
15. Parser finding (P4, not fixed in this slice; a `docs/friction.md` row by P4): an integer
    percent literal followed by `||`, `{`, `(`, `[`, `-` or an identifier is read as a
    remainder, because `unit_after` (`ignite-syntax/src/parser.rs`, around line 1885) asks
    `starts_operand` without regard to `no_record` or to `||`: `t > 100% || x` becomes
    `t > 100 % (|| x)` (E101 and E040), `if t <= 0% {` becomes `0 % { .. }` (E001 at the
    block's first statement). `100.0%` and `(t > 100%)` parse. A fix is a grammar decision
    (`%` after an integer literal when the next token is `||` or, under `no_record`, `{`)
    for the slice that owns the parser next; the std reducers use the parenthesised form.

Landed by P2 (substrate and adapters), verified at run time:

16. Thread count. The drain of 16.13 adds one OS thread to every compiled program on both
    targets: `ignite_io::drain()` (called by `ignite_io::run` and by the LLVM runtime's
    `ignite_rt_init`) blocks SIGTERM in the main thread and starts the signal thread `drain`,
    which takes SIGTERM with `sigwait`. An idle program with a listener therefore runs two
    threads (`/proc/<pid>/task/*/comm`: `terminald`, `drain`; verified on both targets), a
    session adds one; the one-thread-per-task invariant of section 7 holds for tasks.
    `crates/ignite/tests/terminal.rs`: `idle_terminald_runs_one_thread` now expects 2 idle
    and 3 with a session, and `a_task_that_cannot_start_closes_only_its_connection` runs
    under systemd `TasksMax=3` (main, drain, one session) instead of 2.
17. The `released` line. 16.12 step 3 said the drained process prints `released
    <address>:<port>` on stdout. As landed it is written with `ignite_io::log`, on stderr,
    the channel of the `listening on` line it mirrors (A5), and the process adapter reads it
    from the child's stderr; 16.12 step 3 and the 16.13 drain rules say so.

### 16.22 Slice 12b: reachable std, the build-time carry-over

Approved by the steering session on 2026-11-02 after item 12 of 16.0 measured rises over
10 % against `s12-before` (O26: the baseline does not move). Slice 12 is done; item 12 is
carried here, as slice 11's was into 11b.

Measured causes (item 12, 15.1 pairs on `s12-p4-after2`):

- every compiler run loads and checks all of `std/`, used or not: 3,033 to 5,135 lines;
- the generated `ignite-std` crate holds every std module: 2,191 to 3,792 lines;
- `crates/ignite/tests/ignite_test.rs` 6.6 to 9.8 s.

Work:

1. Frontend: a program's build, `check`, `plan`, `explain`, `run` and `test` load, check and
   type only the std modules it reaches. The reached set is:
   - the modules its files import, transitively;
   - the modules whose records a builtin signature names, for each type or static owner a
     loaded file spells (since slice 17): `std.process` for `Launcher` and `ChildProcess`
     (`ChildExit`, `Invocation`), `std.net` for `Endpoint` (`Endpoint.of`), derived from
     `ignite_check::CAPABILITY_METHODS`, `BUILTIN_STATICS` and the signatures of
     `ignite-types` (`ignite_types::builtin_std_modules`); `child.wait()` in a module that
     imports nothing from `std.process` reaches it;
   - the modules the compiler itself calls into for that command: `std.deploy` for
     assembly resolution (15.6), the kinds' modules for `K.main`, the reducers' modules for
     `ignite run`.
   `load_std` keeps its signature (public APIs: add, do not rename); the reachable variant is
   a new fn beside it. `ignite test` of a std file reaches that file's imports.
2. Rust target: `ignite-std` holds only the items the program and its mains reach, as the
   LLVM target's std module does since slice 11 (`docs/llvm.md` section 1).
3. `ignite_test.rs` back to 4.6 s or under (11b), with one test per behaviour kept.

Rules:

- No diagnostic, golden or program behaviour changes. If reachable-only loading changes
  which diagnostics a program sees (a diagnostic in an unreached std module), the change is
  reported as a finding, not silenced: `ignite check` of every std file and of both examples
  stays silent, and the test of the whole std (`task test-ignite`) still checks every module.
- Generated Rust of a program that reaches every std module is unchanged in content; the
  rustgen goldens of programs that reach a subset change only by dropped items, and each
  diff is listed.

Acceptance, measured with the 15.1 pairs against two trees:

- `s12-before` (the 11b baseline): every unit of item 12's table within 1.10, the warm
  `cargo test --workspace` step included. A unit still over 1.10 gets a measured cause in
  the `AGENTS.md` row, not an inferred one.
- `s12-p4-after2` (the slice-12 close): the gain per unit.
- The gate of 16.0 green: fmt, clippy, `cargo test --workspace --no-fail-fast`,
  `task test-ignite`, `task gate`, the four compile tasks.

Implementation record (12b, 2026-11-02): the frontend resolves the import closure plus
literal `module("p")` dependencies of the program and its sibling modules. `check`, `plan`
and `explain` additionally root `std.deploy` when the main program declares an assembly
(assembly admission, inclusion and entries);
`run` and `replay` root `std.control` and `std.sli`, with `std.time` reached transitively.
Checking a std file roots that module, and the whole-std inline test roots every input.
Diagnostics in an unrelated std module are intentionally absent from a program's check;
checking that module directly and the whole-std test still reports them.

The Rust generator retains lowered items referenced by generated program/test text and
all executable mains, transitively; same-name items in different modules are conservatively
retained together. Types retain their defaults, associated functions and derived glue. The
unpruned public API is preserved. Generated std package versions carry a content hash, and
both `build` and `test` claim a shared Cargo target directory before compiling: alternating
projects must rebuild their own program even when their std content is identical. The
`build_cache` integration regression builds A, B and A again in the same target directory,
with identical std content but different program output; it checks that the last A runs A.

12b runtime finding: redundant syncs of `signal`, `standing` and `consequence` records
serialized probe fanout ahead of real-clock restart timers. These records are deterministic
projections of their preceding durable effects. They are still appended in the same order,
but no longer add a separate sync; O30's sync after every effect, before its dispatch, is
unchanged. The real-clock restart assertion remains 100 to 500 ms, and every test journal
still replays. The syscall diagnostic is retained separately from readiness checks because
tracing interfered with unrelated HTTP concurrency tests.

### 16.23 Slice 12c: restore the original performance budget

The implementation plan is `docs/performance-plan.md`. The 11b baseline is the exact
recovered tree published as `bench/11b-recovered` (`df7ccb9`); `9273349` is the published
12b comparison. Closure requires every required paired timing ratio at most 1.10, the
10.8 s warm workspace-test limit under comparable load, unchanged diagnostic/golden
behaviour, and all existing tests on both targets. A measured cause does not waive a miss.

Run/drain test setup includes all initial build and Cargo-lock waiting inside
the measured workspace test. The first stdout `segment` marks completion of
build/copy and entry into the live controller; the on-disk header alone does
not. Helpers bound that setup phase at 60 seconds, retain the first segment
for normal consumption, and then use the existing 15-second live record/event/
exit waits. The real restart assertion remains 100–500 ms. This introduces no
extra warmup command, skipped build, or work outside the measured gate.

Additive test preparation contract: `ignite test --target=all` prepares the source and
standard-library type graph once, then builds and runs every supported target using that
same immutable graph, in native CLI target order (currently Rust then LLVM). `all` enumerates
the native CLI's supported target variants rather than maintaining a separate pair.
Existing `--target rust` and `--target llvm` retain their behaviour;
`ignite build` and `ignite run` keep their two native target choices. The public driver API
adds `test_targets(files, options, targets)` beside `test`/`build_tests`, without adding a
selection variant to the native `Target` type. Each target retains its own generated directory,
Cargo/LLVM artifacts and isolated test children. The same filter, list, jobs, timeout,
profile and tool options reach each target. Both suites run even if the first has failing
tests, and the exit status is the maximum test status; a preparation/build diagnostic uses
the existing diagnostic exit. Tests, reports and group ordering remain unchanged within
each target. The standard-library and program ASTs and their pointer-keyed type maps stay
alive together throughout both builds, and no persisted frontend cache is introduced.
`task test-ignite` uses this combined path while continuing to run every std/example test
on both native targets.

The Rust generator may expose a reusable immutable lowered std template. Preparing it uses
the existing typed emitter and fixed std source/test-selection options; every program still
gets its own transitive pruning, rendering and substrate-specific Cargo metadata. Existing
generation APIs retain their results. Mutable/custom test fixture roots always reload.
LLVM layout memoization is private to an immutable emitter environment; public mutable
`TypeEnv` instances and their clones continue to observe edits. Frontend address-map keys
remain internal AST addresses; untrusted identifier maps retain their existing hashing.
Evaluator worker reuse is scoped to one check/model operation, with fuel, allocations and
frames reset separately for every property/hook call as before. O30 effect durability,
manual-clock backoff, real-clock restart limits and journal replay remain unchanged.

Cold validation exposed a shutdown race: retiring a member before publishing its completion
allowed the closing segment to omit exit events. The process adapter now publishes completion
before exposing quiescence. Stop processes the finite queued prefix through an internal fence
before closing the segment, using the ordinary reducer/journal path. This preserves every
completion without letting later control input extend shutdown; effect syncs remain unchanged.

The remaining 12c hot paths keep the same ownership. Rust reachability borrows identifiers
from immutable generated text and deduplicates before walking; its lexical rules and rendered
order are unchanged. A Rust generator prepares its fixed escaped failure-site filename once,
while every site's line and column still come from the current source. LLVM pruning likewise
borrows temporary symbol names while function text is immutable; the public symbol query keeps
its owned result. LLVM debug-location interning uses a private integer-key hasher and preserves
metadata IDs and emitted IR. Source-identifier maps retain their existing hashing.

The format-check workflow may partition the complete workspace package set into at most
four concurrent CargoFmt groups. CargoFmt remains responsible for every target, recursive
module, edition, configuration and toolchain choice; no source is excluded. Every group runs
with the same inherited environment and `-- --check`, and all groups finish before their
statuses are combined. Successful checks are silent; failure diagnostics are retained in
deterministic group order. A local path dependency outside the workspace uses the original
serial `cargo fmt --all -- --check`, preserving its wider scope. The writing `task fmt` is
unchanged. Performance comparisons run this complete candidate workflow against the original
serial 11b and 12b workflows, recording the actual command without changing any budget.

Six targets with no source-side tests or test-only code omit their empty test and benchmark
harnesses: the syntax, types, rustgen, eval and LLVM libraries, and the CLI binary. Their
existing integration targets exercise the normal libraries and CLI; source unit tests,
doctests and integration targets elsewhere remain enabled. Clippy still checks the workspace
with `--all-targets -D warnings`. The removed six executables contain zero cases: exact
case-name and ignored-name inventories must match before and after. Re-enable these manifest
flags before adding source unit tests or benchmarks to those targets.

The CLI enables the existing `nix/poll` feature consistently with the workspace runner so
the measured gate does not alternate dependency feature sets and rebuild the CLI. No external
dependency is added. Test-only work also shares the checked AST/type pair for immutable std
fixtures, and the giant-chain assertion avoids a pattern search only when its required marker
is absent; every assertion, case, ignore and failure condition is preserved.

## 17 Slice 13: local apply

**Status: approved for implementation; implementation and acceptance checks in progress.**
The user's instruction to implement the plan authorizes this work without another
contract approval. The checks below remain obligations until verified. Slice 12c's
remaining performance work stays deferred.

### 17.1 Scope and command surface

`apply` submits a reviewed desired revision to an explicit foreground local host. The
host owns supervision, probes, the journal and recovery after the client exits. There is
no automatic daemon, cloud provider, portable bundle, in-place update or stateful migration.
Changes use replacement and the existing drain protocol (16.12).

```
ignite host --state DIR
ignite plan FILE --for-apply --state DIR --out PLAN [--assembly NAME]
            [--target rust|llvm] [--release] [--std DIR] [--runtime FILE] [--fuel N]
ignite apply PLAN --state DIR
ignite inspect --state DIR [--operation ID] [--json]
```

A new host state starts empty. An apply revision with zero hosted machines is valid: removing the
last workload leaves the host idle and available for later applies. Existing `run` E125
behavior is unchanged.

`host` creates or opens one state namespace, runs until SIGINT, SIGTERM or stdin `stop`,
and accepts stdin `status`. It uses the real clock. Explicit startup is required; `apply`
never launches a host. Restarting the same host resumes accepted work and reconciles its
last committed desired revision. Normal shutdown drains owned children; a crash kills them
under the existing parent-death rule. Neither promises workload survival during host outage.

Ordinary `plan` keeps its existing model JSON and options. `--for-apply` requires a live
host and explicit output file; it obtains a consistent base revision and ownership snapshot,
checks the source, and prepares immutable local artifacts without changing running resources.
Target defaults to LLVM (Rust until 2026-11-05, `story:llvm-default-target`); profile defaults
to dev. LLVM runtime selection follows `build`.
`--fuel` still bounds planning evaluation. Artifact preparation may build; ordinary planning
continues to generate no executables. No native build may occur after destructive cutover.

`apply` waits for completion; disconnecting its client cancels neither accepted intent nor
host work. Exit 0 means completed or already completed; 1 means failed, outcome unknown or
recovery required; 2 means rejected plan, admission or revision; 3 means host/storage failure.
A disconnected caller must inspect or retry, rather than infer rejection. `inspect` reports
host/state identity, committed and desired revisions, operation id/state, action progress,
observed ownership, readiness and any recovery reason/next action. Its successful exit means
the status was read, not that an operation succeeded. With no host, it may read a durable
status explicitly labelled offline; it must not infer live health.

### 17.2 Host ownership and transport

The host alone holds `<state>/lock` for its lifetime and writes the journal. A second host
or `run` using that state is rejected before builds or resource changes. Clients use
`<state>/control.sock`, a local Unix-domain socket restricted to the host user. Requests and
replies are versioned, length-framed UTF-8 JSON with a request id; unsupported versions,
malformed frames and oversized requests fail before admission. A client cannot supply an
arbitrary journal or executable path as a dispatch instruction.

The state has a durable unique identity, distinct from its directory spelling. Every plan
and request targets that identity. A stale socket is replaced only by a process holding the
state lock; neither socket presence nor a PID file proves ownership. Requests enter the
same serialized controller queue as adapter observations. There is one active apply per
state; another distinct apply is rejected as busy. Inspection and retries remain available.
No remote listener or multi-user authorization model is added.

As landed (U2): a frame is a 4-byte big-endian length and that many bytes of UTF-8 JSON, at
most 16 MiB. A request is an object with `version` (`ignite-host-control/0.1`), `requestId`
(1 to 128 bytes), `stateIdentity`, `operation` and `payload`; a reply carries `version`,
the `requestId`, `status` (the 17.1 categories), `reason` and `payload`. An oversized,
non-UTF-8, non-JSON or incomplete envelope, or another version, is answered with status 2
before the host queue; the reply carries the request id when it could be read and the empty
id otherwise, and the client reports such a reply as the host's rejection (exit 2); a frame
cut short by the client's end of file is answered the same way, with the empty id. The host
takes the lock before it changes anything, then restricts the state directory to mode 0700;
a held lock or a state with a `run` journal is exit 2. After locking, the host checks that
the locked file is still the one at `<state>/lock` and that no host answers on
`control.sock`, so deleting the lock file of a live host does not admit a second one (exit 2).
The socket is bound in a fresh 0700
directory, set to 0600 and renamed over `control.sock`. The identity is 32 random bytes in
hex, written to a temporary file, synced, renamed and the directory synced; an identity file
that is not one is exit 3 and is never replaced, and a missing identity on a state that holds
`host-journal.jsonl` or `status.json` is exit 3 with nothing written. An apply of a plan with neither a receipt
nor `plans/<id>.json` is exit 2. `inspect` adds `host` (PID and start time), `currentOperation`,
`actionProgress` (per member action of the desired plan: `done`, `pending` or `blocked`),
`readiness` (expected and ready machines, `converged`) and `nextAction`, which asks for a
corrective plan only while admission takes one; offline it sets
`online` false, `host` null, `health` and `readiness` to `unknown: host offline`, and renames
every `ready` flag at any depth, `retiringInstances` included, to `lastRecordedReady`.

### 17.3 Saved plan and identity

Apply accepts a separate versioned `ignite-apply-plan/0.1` envelope, not
`ignite-model/0.1`. It records:

- state identity, base revision and relevant observed ownership/resource revisions;
- selected assembly, complete canonical desired model, change actions and dependency order;
- source roots and cryptographic content digests of every reached source, configuration,
  compiler, standard-library/reducer input and build/runtime input;
- target/profile, immutable artifact references and content digests, controller ABI and
  reducer revision, ownership and removal authority; and
- a plan id hashing the canonical envelope, excluding its own id field, including admission assumptions.

Digests use SHA-256 with explicit format/domain separation. Paths and the existing private
`build_stamp` are not identities. The plan is source-bound: referenced inputs must match
before first acceptance; it contains no source or executable bundle. The host verifies and
retains the admitted artifacts in its own immutable generation paths, not mutable build
outputs. Missing/mismatched inputs, artifacts, required capabilities or controller versions
are rejected before resource mutation. An accepted operation recovers from its retained
inputs; later workspace edits must never silently rebuild it as different code.

Raw source/admission identity and operational identity are different. Source locations,
comments and system-member declaration order do not cause deployment changes after
replanning. Operational identity covers canonical model semantics plus canonical resolved
implementation bodies, types/constants and dependencies, target/profile, runtime contract
and versioned compiler/backend code-generation semantics. That codegen revision changes whenever
generation behavior may change; exact admitted compiler bytes remain separately pinned.
The implementation projection excludes locations/trivia and orders module/declaration sets
by qualified name, but preserves statement, parameter, record-field, enum-variant and other
meaningful order. Artifact bytes remain separately pinned and verified: identical model JSON
with changed implementation is not a no-op. Semantic model comparison excludes `sourceMap`
and normalizes member/edge collections already defined as unordered; it never sorts arbitrary
lists. Stale raw inputs still invalidate an unaccepted plan even when its operational diff
would be empty. Replanning is required, not automatic substitution at apply time.

Canonical implementation form (story `structural-apply-identity`, 2026-11-06). The
implementation projection is written from the syntax tree by an encoder of its own
(`crates/ignite/src/apply/canonical.rs`), no longer from the AST's `Debug` text with spans
scrubbed by a string scan. Per module it holds the sorted imports and one canonical text per
non-test declaration other than systems and assemblies (the model carries those). Spans,
comments, layout and declaration order are absent. Names are written by what they denote:

- local bindings (`let`, `var`, `for`, pattern bindings, closure parameters) are numbered per
  declaration in binding order, type parameters by position; a reference names its binding,
  so a rename that a local would capture is a change;
- a private module-level declaration is numbered by first reference, walking the public
  declarations in name order; private declarations no public one reaches follow, ordered by
  their text with the unnumbered names masked;
- every other name stays as written: public names, imports, builtins, fn parameters (named
  arguments call them by name), record fields, variants, methods and associated-fn members.

Ignite has no visibility keyword (4.1 exports every declaration), so public is decided over
the loaded program: every declaration of a std module; a name another module imports, by name
or by a glob; a fn of a module named by a `module("p")` literal whose name is a field of some
record (14.4 fills the interface record by name); a name a system or assembly of its module
references; a free fn `main`. Everything else is private: renaming it, a local or a type
parameter keeps the plan identity, as does any edit of spans, comments or order. A change of
a body, a literal, a type, a public name or a parameter name changes it. The projection covers
the whole loaded program, so a helper's change reaches the identity of every member that
calls it; identity is per plan, not per member.

The identity domains are `ignite-implementation/0.2` and `ignite-operational/0.2`; `0.1` was
the `Debug` scheme. A change of the canonical encoding bumps both. The plan format stays
`ignite-apply-plan/0.1`, so a host keeps reading the plan it committed under `0.1`; that
plan's identities differ from every `0.2` plan, so the first apply after the upgrade replaces
each hosted executable once with the reason "implementation, target, runtime contract or
codegen revision changed", as a `CODEGEN_REVISION` step does. A plan prepared under `0.1` and
applied after the upgrade is still admitted (its codegen revision matches), and the next
replanning compares against it as above. `CODEGEN_REVISION` remains the pin of generated
code; it no longer versions the AST's `Debug` layout, so an AST change that leaves generation
alone needs no step.

### 17.4 Changes and authority

The plan shows `add`, `replace`, `remove`, `detach` or `unchanged` per stable member id,
with reasons and the exact affected dependent closure. Unsupported changes are rejected,
not labelled updates. A change of implementation or operational properties replaces the
owned executable and any dependent whose configuration/endpoint changes. There is no generic
in-place mutation hook in this slice. Stateful members requiring migration are inadmissible.

Prepare all additions/replacements first: build and verify artifacts without starting a
listener that conflicts with an owned one. Independent additions start providers before
readiness-dependent consumers. For an occupied listener, first drain/release the old listener,
then start its replacement, require `Ready`, and only then start affected dependents. Old
accepted connections continue under the existing drain/deadline protocol (16.12); use its
explicit deadline policy without silently changing it. Stop/remove dependents before their
providers. Reject cycles in required action ordering; signal/dataflow edges are not
automatically startup dependencies. Failure before cutover retains the old generation where
possible; failure after destructive progress reports actual state and forward recovery,
not an invented rollback guarantee.

Ownership records distinguish host-created processes/listeners, included members and external
bindings. Host creation grants drain/stop authority over that exact process instance and release
of its owned listener after dependents drain: this is the supported `nativeProcess` ownership default.
Journal/history and inactive artifacts are retained by default; further deletion requires
explicit recorded authority. The client envelope cannot manufacture authority absent the source
and host ownership records. External resources are observed and detached, never killed or
deleted; a reference, matching address or historical PID does not grant authority. A binding whose vocabulary cannot express the required authority
is unsupported. Listener adoption requires matching durable ownership and OS process identity;
port availability alone is insufficient. Resource adoption that cannot be proved is blocked.

As landed (U4): the order is the pure reducer `Transition` in `std.control`, stepped by the
host over the plan's member actions (`ignite-apply-actions/0.1`); each step is journaled as a
`transition` record before the host acts on it. Owned members to remove, and owned members
replaced stop-first, stop one at a time in `stopOrder`; a provider still serving a dependent
that cuts over is stopped only after that dependent has cut over or stopped. `SwitchRevision`
hands the staying instances to the desired revision; members to add or replace then start in
`startOrder`, each once the providers it depends on are ready. A replacement on the port its
old instance holds (an occupied listener) drains and releases the old listener first, then
starts the new one and requires `Ready`; a failure after that step reports actual state with
`recovery_required`. A replacement with no listener conflict starts first: the old instance
keeps serving until the new one is `Ready`, and a replacement that never becomes ready fails
the operation: everything the failed operation started stops, and the old instance and the
other committed instances are handed to a new run of the committed plan (`restoreCommitted`),
whose supervisor takes them back with their policy, generation, port and readiness. Before any destructive step
the host checks that each new listener's port can be bound, and refuses with
`listener_unavailable` otherwise. An effect the host cannot carry out (spawn failure, artifact
mismatch, an ended controller) is reported to the reducer as `EffectFailed` and journaled as an
`Abandon` step before the operation fails; a desired member that gives up gives
`Abandon("gave_up", member)`. A corrective apply (D3) is admitted only while no operation other
than the gave-up one is active; a no-op corrective back to the committed revision clears
`gave_up`, journals `recoveryResolved` and retires the failed revision's instances.

### 17.5 Durability, revision and recovery

This needs a distinct versioned host journal, not an altered interpretation of
`ignite-journal/0.1`. Existing `run`/`replay`, full-model header checks, record order and O30
remain unchanged. The host journal records admitted plans, base/new revisions, immutable input
identities, per-action intents and observed completions, operation outcomes and revision
commit. Historical model/configuration and controller provenance remain replayable; unsupported
schema/reducer upgrades are rejected. Contract evidence retains the definition under which it
was accumulated, rather than being reinterpreted by a changed contract.

Persist and sync acceptance before acknowledging it. Persist and sync every effect intent
before dispatch, and its observed completion before advancing dependent destructive work.
The desired revision is committed only when all required actions converge. A failed operation
leaves its prior committed revision and explicit partial observed state; it does not pretend
that resources reverted. No unrelated operation may overwrite unresolved recovery.

The retry key is state identity plus plan id. Validate envelope integrity and payload/id
agreement first; a mismatched payload is rejected. A repeated accepted request returns the
same operation/result without a second dispatch. Receipt lookup precedes stale-base rejection
for that known operation; an unknown plan still requires admission. Retrying an old completed
operation reports its historical revision and the current one without rolling back. A completed
receipt describes that operation, not present workload health. A newly planned operational no-op causes no replacement or
revision increment. Normal probes and supervision may continue; no-op does not mean freezing
all controller activity.

Recovery inspects each durable intent without a recorded completion. It reconciles effects
using stable action/generation identity and provable ownership before retrying. A missing
owned child may be restarted after verifying the old instance ended; a historical PID must
never be signalled after reuse. If an effect may have succeeded but its outcome/authority
cannot be established, mark `outcome_unknown`/`recovery_required`, block dependent mutation,
and expose the evidence needed to resolve it. Never blind-spawn a duplicate or adopt another
process to make recovery appear successful. Losing durable storage stops new mutations.

Host restart resumes pending operations automatically when reconciliation proves it safe;
retrying `apply` attaches to that work. Client disconnection needs no operator recovery.
Stopping/restarting a host is not a new apply. The current replay implementation only verifies
recorded transitions and tolerates an incomplete final step; it does not already implement
this intent reconciliation. Recovery completeness is an acceptance obligation below.

As landed (U5): every host journal record is written and `sync_data`ed before the host acts on
it: the acceptance before the reply, a run record (an effect intent among them) before its
acknowledgement lets the run driver dispatch, a `transition` step before its effects. When a run
record changes the owned or retiring instances, an `ownership` record (pid, OS start time,
generation, operation) is synced before that acknowledgement too. A failed write or sync
truncates the journal to its last synced record, fails the request with exit 3 and stops the
host before anything else is handled; its children end by the parent-death rule. At the next
start the retained context of a plan without a receipt (an admission that never became durable)
is removed and journaled as `discarded`, so the plan can be admitted again. Recovery takes
the last `state` snapshot and the `ownership` records after it, and the pending intents: `Spawn`
effects with no later `Started` or `SpawnFailed` of that member and no later `reconciled`
record. A recorded instance whose pid is absent or held by a process with another start time
ended and is never signalled; one whose pid and start time match is killed through a pidfd that
re-checks the start time, and recovery waits until it ended. Any other live process whose
executable lies under `<state>/revisions/` is unaccounted for: it is neither signalled nor
adopted, and the active (else committed) operation becomes `recovery_required` with the reason
`outcome_unknown` and the processes and pending intents as `recovery.evidence`; the other
applies are refused and no effect runs. Otherwise a `reconciled` record settles the intents and
the active, else committed, revision starts afresh (after `gave_up` or `listener_unavailable`
the committed one). A fixed listener port of that revision that another process holds blocks it
as `foreign_listener`, without a signal. `status` on the host's stdin, or the next start, lifts
either block and examines the evidence again. A block never overwrites an unresolved recovery:
while one holds (the active operation is `recovery_required`, such as `gave_up`, or the committed
revision's controller ended), the block lives in `recovery` alone, naming that operation as
`recovery.unresolved`; the operation keeps its receipt and reason, every apply but a retry is
refused, and lifting the block leaves that recovery as it was. Admitting a corrective apply
resolves the failed operation (`recoveryResolved`) and clears its reason at once, so a host that
crashes after the acceptance resumes the corrective. Each revision state directory gets
a name no earlier host of the state used, and `revisions/` and the state root are synced once it
is created; a temporary that a crash left behind never blocks the next write. Decision D2: a revision whose reducer revision
differs from the host's is neither re-stepped nor driven; the snapshot's `history` lists each
revision state with `restepped`, and `recovery` is `recovery_required`, `reducer_revision_changed`,
next action "replan and apply", without blocking an apply; the next commit clears it. A start
before that commit keeps a report that names an operation other than the committed one, which no
start examines again, and finds the report of a committed revision still not driven again. Debug
builds read `IGNITE_DEBUG_HOST_FAULT` (`after_intent`, `before_dispatch`, `after_dispatch`,
`mid_transition` crash the host with SIGKILL there; `sync_intent`, `sync_effect` fail that
sync) and `IGNITE_DEBUG_HOST_REDUCER_REVISION` (the reducer revision the host claims); release
builds read neither.

Precedence of recovery reasons: the first reason recorded stands as the reported one, and a later
one is kept beside it without replacing it.

- While an unresolved recovery holds, a later failure (a reducer failure of the committed
  revision's run, that run's controller ending, a process that failed to start, a ready artifact
  that differs), whichever operation the run's records carry, leaves the active operation,
  `recoveryReason` and the receipt's `reason` as they were; it is kept in that receipt's
  `laterReasons` (`operationId`, `reason`). A later reason that a corrective plan does not resolve
  (any but `gave_up` and `listener_unavailable`) still governs what the host does, because the
  corrective's premise, the committed revision's instances as they were, no longer holds: the
  receipt's next action becomes "inspect retained ownership and journal before recovery", the
  running revision dispatches no further effect, no corrective is admitted and no start resumes
  the committed revision. Since slice 18 a refused effect does not end the run: the host answers
  its record "refused" (`Admission::Refused`, `crates/ignite/src/run/live.rs`), the record stays
  in the run's journal and the host journal as the effect's intent, the run dispatches nothing
  for it, prints `effect not dispatched: <reason>` and goes on. So the instances the run still
  supervises, the committed revision's among them, keep serving until `ignite recover` (22.9)
  stops them; before slice 18 the refusal ended the run and its children with it (parent-death
  rule), the healthy committed instances included. Keeping the first reason never weakens what
  the later one blocks. While
  the unresolved recovery is the committed revision's own (its receipt stays `completed`, and its
  reason is never resolvable), nothing is kept beside it.
- In `recovery` the first entry recorded stands; a later one is kept in `recovery.laterReasons`
  with its own `reason`, `planId`, `nextAction`, `blocking` and `evidence`, and a later block makes
  `recovery.blocking` true. A `foreign_listener` block of the committed revision found after the
  `reducer_revision_changed` report of a not-driven operation therefore leaves `recovery.reason`
  and `recoveryReason` at `reducer_revision_changed`, refuses every apply but a retry, and changes
  neither the active operation nor the host's recovery reason. Lifting the block keeps the report,
  no longer blocking, until the next commit; a block found again at the lift or the next start
  goes beside it again.
- A transition abandoned before the running revision realizes its plan gives the run back the
  operation it carried before: `runtimeOperation` and the run's records name the committed
  revision's operation again, not the refused plan.

Since slice 18 `ignite recover` (22.9) is the operator's way back to accepting plans.

### 17.6 Terminal walkthrough

Before: no resources are owned and revision is zero. Keep the host terminal open:

```
# terminal A
ignite host --state .ignite/terminal
# terminal B
ignite plan examples/terminal/terminal.design.ignite --assembly local \
  --for-apply --state .ignite/terminal --out terminal.apply.json
ignite apply terminal.apply.json --state .ignite/terminal
ignite inspect --state .ignite/terminal --json
```

After: revision 1 is committed, `terminald` is ready, probes/standings continue in terminal A,
and the client has exited. Reapply the same file: same completed operation, no replacement.
Change the command implementation, then replan/apply: the diff shows replacement even if
properties are identical; existing connections drain and the new implementation becomes ready.
For removal, remove server, ingress, included console, promptProbe and its three dependent
contracts and their bindings together; retain the workstation client. Replan/apply drains
the server and stops its probes, leaving no hosted machines while the host stays running.
Its dependent removals and release order are explicit. External references produce detach only. Disconnect terminal B after acceptance: inspect/retry
finds the same operation. Kill terminal A during a dispatch, restart the same host, then inspect:
reconciliation resumes safely or reports recovery required, with no duplicate owned process.

### 17.7 Acceptance and implementation owners

These are required future checks, not results. Run the local process scenarios with both Rust
and LLVM artifacts; compare semantic actions/results while masking only physical PID/time.

| Input / interruption | Action | Required observation |
|---|---|---|
| Empty state, terminal design | plan/apply | Revision 1; one owned server; probe/standing and status work |
| Same accepted plan | retry, including concurrent retry | Same operation; no second dispatch/revision |
| Reformatted/reordered system members | replan/apply | Operational no-op; fresh admission pins |
| Same model, changed command body | replan/apply | Replacement and new observable command behavior |
| Stale base, changed source/artifact, wrong state/version | apply | Rejected before mutation; original service remains |
| Changed compiler/backend semantics with unchanged model | replan/apply | Replacement; exact artifact pin checked |
| Add/remove dependent, external reference | apply | Provider/start and dependent/stop order; external untouched |
| Remove the final hosted workload | apply, then inspect/reapply | Host remains idle/running; no owned children; same plan is a no-op |
| Connection active during replacement | apply | Existing drain/deadline behavior; replacement readiness |
| Client disconnect before/after durable acceptance | retry/inspect | Rejected or same durable operation, never duplicate |
| Crash after intent, after dispatch, before completion/commit | restart host/retry | Reconciled single effect or explicit blocked unknown outcome |
| Reused PID, foreign listener, second host | recover/start | No foreign signal/adoption; lock/ownership refusal |
| Storage sync failure, unsupported migration or action cycle | apply/recover | No unsafe subsequent effect; clear recovery/rejection |
| Multiple committed/pending host revisions | replay/recover history | Every action/result uses its original model/input/reducer provenance; divergence is rejected |
| Old run journal and existing commands | run/replay/ordinary plan | Existing contracts and goldens unchanged |

Coverage (U6): the tests that exercise each row, by harness name of `crates/ignite/tests/main.rs`
unless the name says otherwise; `{,_llvm}` is the Rust test and its LLVM twin. Each target builds
its workload in the one shared build directory of the apply suites (`tests/common/apply.rs`). The
pass/fail record is `STATUS.md`.

| Row | Covered by | Targets |
|---|---|---|
| 1 Empty state, terminal design | `apply::{rust,llvm}_saved_apply_retry_source_admission_and_retained_restart` (revision 1, one owned ready server, the probe's `signal` and the contract's `standing` journaled, `lastStandings` in inspect, the console's `status`); `apply_transition::add_remove_dependent_orders_provider_first_external_untouched{,_llvm}` (emission and standing before `Commit`) | Rust, LLVM |
| 2 Same accepted plan, concurrent retry | `apply::concurrent_retry_attaches_to_one_operation_{rust,llvm}` (two clients started together while the port is held, one operation, one `StartMember Terminal.server` and one `Commit`, both receipts); sequential retries in `apply::{rust,llvm}_saved_apply_…` | Rust, LLVM |
| 3 Reformatted/reordered members | `apply_admission::reordered_members_are_operational_noop_with_fresh_pins`; the comment-only replan of `apply::{rust,llvm}_saved_apply_…` | Rust, LLVM (the reorder case has no executable, so its target builds nothing) |
| 4 Same model, changed command body | `apply::changed_command_body_replaces_ready_server_while_old_sessions_drain{,_llvm}` | Rust, LLVM |
| 5 Stale base, changed source/artifact, wrong state/version | `apply_admission::stale_base_changed_source_wrong_state_and_version_reject_before_mutation`; the changed-source refusal of `apply::{rust,llvm}_saved_apply_…`; the tampered artifact of `apply_admission::changed_codegen_revision_…{,_llvm}` | Rust, LLVM (the stale-base, state and version refusals are admission checks before any artifact is read, on a system without executables) |
| 6 Changed codegen, unchanged model | `apply_admission::changed_codegen_revision_with_an_unchanged_model_replaces_the_hosted_server{,_llvm}` (replace action, tampered artifact refused, pinned bytes replace the server); `apply_admission::codegen_revision_and_runtime_bytes_are_separate_identity_and_admission_inputs` | Rust, LLVM |
| 7 Add/remove dependent, external reference | `apply_transition::add_remove_dependent_orders_provider_first_external_untouched{,_llvm}` | Rust, LLVM |
| 8 Remove the final hosted workload | `apply_transition::add_remove_dependent_orders_provider_first_external_untouched{,_llvm}` (revision 3 owns nothing, host online, the same desired state replanned is a no-op); `apply_host::inspect_reports_identity_revisions_operation_progress_readiness_and_next_action` (zero hosted machines); `apply_transition::a_revert_after_gave_up_stops_what_the_failed_operation_started` | Rust, LLVM |
| 9 Connection active during replacement | `apply::changed_command_body_replaces_ready_server_while_old_sessions_drain{,_llvm}` (occupied listener: the old session is served until it quits, the apply waits for it); `apply_transition::a_port_change_starts_the_replacement_before_the_old_one_drains`, `a_replacement_that_never_becomes_ready_keeps_the_old_serving_and_fails`, `a_failed_start_first_replacement_leaves_the_old_instance_supervised`, `a_corrective_after_a_failed_replacement_completes_with_its_receipt` | Rust, LLVM (the start-first cases on Rust: the ordering is the host's reducer, not the target's) |
| 10 Client disconnect before/after durable acceptance | after: `apply::accepted_apply_recovers_after_host_crash_during_failed_dispatch{,_llvm}`, `apply::concurrent_retry_attaches_to_one_operation_{rust,llvm}`; before: `apply_recovery::a_failed_journal_sync_stops_the_host_before_any_dependent_effect` (an acceptance that is not durable is not in the journal and is admitted afresh), `apply_host::adversary_a_frame_cut_short_by_a_half_close_is_answered_with_status_2` | Rust, LLVM (the before half on Rust: the journal and the frame are the host's, not the target's) |
| 11 Crash after intent, after dispatch, before commit | `apply_recovery::crash_after_intent_before_dispatch_yields_one_server_or_outcome_unknown{,_llvm}`, `apply_recovery::crash_mid_transition_resumes_once_and_a_foreign_listener_is_never_adopted{,_llvm}`, `apply::accepted_apply_recovers_after_host_crash_during_failed_dispatch{,_llvm}`, `apply_recovery::a_crash_after_accepting_a_corrective_resumes_the_corrective` | Rust, LLVM |
| 12 Reused PID, foreign listener, second host | foreign listener and an unaccounted generation process: `apply_recovery::crash_mid_transition_…{,_llvm}`, `apply_recovery::crash_after_intent_…{,_llvm}`; second host: `apply_host::second_host_and_run_on_a_host_state_are_refused_before_any_change`, `adversary_a_second_host_is_refused_after_the_lock_file_is_deleted`, `adversary_two_hosts_started_at_once_leave_exactly_one`; reused PID: the unit test `apply::host::recovery::tests::a_reused_or_absent_pid_is_ended_and_a_matching_one_runs` (`cargo test -p ignite --lib`), because an end-to-end test cannot make the kernel give a recorded PID to another process | Rust, LLVM (second host and reused PID target-free) |
| 13 Storage sync failure, migration, action cycle | `apply_recovery::a_failed_journal_sync_stops_the_host_before_any_dependent_effect`; unit tests `apply::plan::tests::replacing_a_stateful_member_is_an_unsupported_migration`, `readiness_cycles_reject_but_signal_cycles_do_not_impose_start_order`, `adversary_self_and_indirect_readiness_cycles_are_rejected`; inline `std/control.ignite` test "members whose providers never become ready are an ordering cycle" | Rust (the journal, the action diff and the reducer are the host's; no artifact is involved) |
| 14 Multiple host revisions, history | `apply_recovery::a_changed_reducer_revision_starts_the_host_without_driving_and_asks_to_replan` (revisions 1 and 2, `restepped` per revision, D2); `apply::tampered_host_history_is_rejected_before_startup` (divergence); `apply_recovery::crash_mid_transition_…{,_llvm}` (a committed and a pending revision across a crash) | Rust (D2 and history are the host's; the LLVM twin of the crash case covers two revisions) |
| 15 Old run journal, existing commands | `apply_host::second_host_and_run_on_a_host_state_are_refused_before_any_change` and `run_on_a_stopped_host_state_is_refused_before_any_build` (`run` on a host state, E126), `apply_host::a_host_refuses_a_state_that_holds_a_run_journal`; unchanged: `run::*` (`run::terminal_runs_under_ignite_run{,_on_the_llvm_target}`, `run::e126_journal_mismatch`, `run::e127_replay_diverged`), `plan::terminal_example_model_matches_golden`, `plan::todo_example_model_matches_golden`, `plan::design_model_is_byte_identical_across_processes_and_directories` | Rust, LLVM (`run`) |

The following are implementation tasks authorized by the approved plan:

| Owner | Deliverable | Dependencies | Acceptance |
|---|---|---|---|
| Evaluator/frontend and existing build driver | Canonical operational projection, saved-plan admission and pinned artifact manifest | Reviewed contract | Source-only no-op; changed code/codegen replacement; exact input validation |
| Ignite control/kind modules | Pure ordering, replacement and reconciliation transitions (`std.control` `Transition`, stepped by the host); the diff is computed by the driver as the plan's `actions` (decision D1) | Plan/action contracts | Reducer traces agree in evaluator and both backends; preserve std layering (O45) |
| Driver host/control/journal | Socket client/host, owned revision context, durable receipts and revision replay | Projection and reducer interfaces | Single owner; retry precedence; multi-revision crash recovery |
| Existing process/probe/build adapters | Instance identity, immutable generations and existing drain/release dispatch | Durable action interface | No foreign mutation/duplicate spawn; occupied-port cutover |
| Existing integration/inline test owners | Walkthrough and fault-injection matrix above | Host and adapter implementation | Both targets; unchanged run/replay/ordinary-plan coverage |

`Live` currently borrows one `Loaded` model/evaluator context, and `Core` owns a fixed machine
set. The host task must introduce an owned revision context and explicit transition boundary;
it cannot hot-swap borrowed AST/type tables or treat a new segment as plan replacement.
The user approved implementation of the plan. Independent review and the acceptance
checks above remain required verification, without reopening that approval.

## 18 Slice 14: in-process streams and generic protocol roles (protocols P1)

**Status: contract written 2026-11-03 and approved in advance by the user. The independent
review (`docs/slice14-review.md`, 22 findings) is resolved in this text. Implementation has
not started.** The rows of 18.0 are required checks, not results.

Sources:

- `docs/protocols.md` §1.2, §2.2, §3.3, §3.6, §5.1, §5.3, §6.1 to §6.3, §7 row P1 and §8.
- `docs/vision.md` §4 row "B protocols P1", §6 (X16, X17, X18), §7 (T2, T3, T4), invariants
  3, 5, 6, 7 and 9.
- This file: sections 3, 4.2, 5, 7, 8, 9 item 4, 11 (generics), 12.3, 12.7, 13.3, 14.4, 14.8,
  and 16.18 to 16.20 (the skeleton pattern). Also `docs/layering.md`.

The base of the slice is commit `340f782` (`main`, which adds `tools/ignite-perf`). Every
`git diff` in 18.0 and the before tree of 18.18 are taken from it. The line numbers in 18.2 are
those of `340f782`. They are not maintained (vision T3). Each row also names the enclosing
symbol, and everything else in this section cites symbols. Between `c33442a` and `340f782`,
none of the cited files under `crates/`, `std/` or `examples/` changed. If `main` moves before
the slice closes, the base becomes the merge base of the slice branch and `main` at the close,
for rows 9 and 15 and for 18.18.

By T2, the P1 row of `protocols.md` becomes a contract here. Where `protocols.md` disagrees
with the code, this section follows the code and records the point in 18.14.

For fns and roles, this section supersedes the type-parameter grammar of section 3, the role
arities of 4.2 and the E030 rule, the substrate surface of section 7, and the generics rules
of 11.1. Each of those sections gets a one-line forward reference to 18 from the unit that
owns the change (18.16).

The slice adds:

- a bound on a type parameter of a fn or role, `<S: ByteStream>`, with one compiler-known
  interface;
- an in-memory stream pair, `Pipe.pair()`, whose ends coalesce writes as `TcpStream` does and
  count the batches they receive;
- `Telnet.serve`, `Telnet.connect` and `Http.serve` generic over the stream; each server role
  takes its protocol's limits record;
- inline protocol tests over pipes on both targets.

### 18.0 Acceptance

- `ignite` is `target/debug/ignite`.
- "Both targets" is `--target all` (12.7).
- Test names in quotes are the inline tests of 18.12. A name ending in `::` is a harness name
  of the crate's `tests/main.rs`.

| # | check | command | required observation | unit |
|---|---|---|---|---|
| 1 | Telnet inline suite | `ignite test std/protocol/telnet.ignite --target all` | exit 0; every test passes on `rust` and on `llvm`; `--list` shows the pipe tests of 18.12 | U5 |
| 2 | HTTP inline suite | `ignite test std/protocol/http.ignite --target all` | the same | U5 |
| 3 | Slice 9 item 4's transcript over a pipe | `ignite test examples/terminal/commands.ignite --target all` | "the terminal transcript over a pipe" passes. Input `help\r\necho hello\r\nstatus\r\nfrobnicate x\r\nquit\r\n`; the client reads exactly `terminal> `, `help\r\necho <text>\r\nstatus\r\nquit\r\nterminal> `, `hello\r\nterminal> `, `ready\r\nterminal> `, `unknown command: frobnicate\r\nterminal> ` and `bye\r\n`, then end of stream. These are the replies of the table in `crates/ignite/tests/terminal.rs` | U5 |
| 4 | `IAC DO ECHO` over a pipe | row 1 | "a pipe session answers IAC DO ECHO with IAC WONT ECHO": the bytes 255 252 1 follow the prompt, before the reply to the next line | U5 |
| 5 | `IAC IAC` over a pipe | row 1 | "over a pipe IAC IAC is one data byte", using the echo console with `maximum_line = 2`. Input: 255 255 255 255 13 10, then 255 255 255 255 255 255 13 10. Output: exactly `> `, EF BF BD EF BF BD 0D 0A (two data bytes 255, which `to_text` decodes lossily as two U+FFFD on both targets), `> `, `line too long\r\n`, then end of stream | U5 |
| 6 | Oversized subnegotiation over a pipe | row 1 | "the server closes on an overlong line, IAC IP and an overlong subnegotiation", now over a pipe: `IAC SB` with 1,025 data bytes ends the session after the prompt. "a session limit bounds the subnegotiation", under `SessionLimits { maximum_subnegotiation = 16 bytes }`. The count includes the IAC of `IAC SE`, as `decode` does today. `IAC SB`, 15 data bytes, `IAC SE`, `bye\r\n` answers `bye\r\n`. `IAC SB`, 16 data bytes, `IAC SE`, `bye\r\n` ends the session after the prompt | U5 |
| 7 | Pipelined requests in one batch | row 2 | "three pipelined requests over a pipe are answered in one batch". The client sends three requests in one `write_all`, the third with `Connection: close`. It reads three responses, then end of stream, and `client.received_batches() == 1` | U5 |
| 8 | 400, 413 and 431 over a pipe | row 2 | "a pipe client gets 400, 413 and 431": each case is the status line and `Connection: close`, then end of stream. "the limits record bounds the body": `HttpLimits { body = 4 bytes }` answers a 5-byte body with 413 and a 4-byte body with 200 | U5 |
| 9 | TCP behaviour unchanged | `cargo test -p ignite --test main -- terminal:: todo::` and `git diff --exit-code 340f782 -- crates/ignite/tests/terminal.rs crates/ignite/tests/todo.rs` | both suites pass on both targets (their per-target cases), and neither file has changed | U5 |
| 10 | Rust static dispatch | `cargo test -p ignite-rustgen --test main streams::` | the reachable `ignite-std` of `examples/terminal/terminal.ignite` (`generate_std_reachable` over the repository std) contains `Telnet::serve::<ignite_io::TcpStream>(` in `TelnetServer__main` and `Telnet::connect::<ignite_io::TcpStream>(` in `TelnetClient__main`. Neither it nor the program crate contains `dyn ignite_io::ByteStream`, and every `dyn` in either is a `dyn Fn` of a fn value (13.3) | U5 |
| 11 | LLVM: each instance calls its own entries | `cargo test -p ignite-llvm --test main streams::` | the std IR of the terminal program defines the `TcpStream` instance of `Telnet.serve` (named by `instance_symbol`), whose callees include `@ignite_rt_tcp_set_idle_timeout` and `@ignite_rt_tcp_close`. The test IR of `std/protocol/telnet.ignite` defines the `PipeStream` instance of `session`, which calls `@ignite_rt_pipe_read`. No instance of a bounded fn calls an entry of the other stream kind | U5 |
| 12 | Diagnostics | `cargo test -p ignite-syntax --test main fixtures::`, the same for `ignite-check` and `ignite-types` | the fixtures of 18.7 match. `every_checker_code_has_a_fixture` lists `e099`, and `every_type_code_has_a_fixture` lists `e096` | U1 |
| 13 | Substrate | `cargo test -p ignite-io --test main pipe::` and `cargo test -p ignite-rt --test main pipe::` | the tests of 18.8 and 18.9 pass | U2 |
| 14 | Generators on a bounded fixture | `cargo test -p ignite-rustgen --test main golden::bounded_streams` and `cargo test -p ignite-llvm --test main golden::bounded_streams` | the goldens of 18.10 and 18.11 match. The generated Rust builds with cargo, and the IR compiles with clang, as the existing compile checks of each crate do | U3, U4 |
| 15 | No new dependency | `git diff 340f782 -- Cargo.lock` and every `Cargo.toml` | `Cargo.lock` gains no `[[package]]`, and no `Cargo.toml` gains a dependency line | all |
| 16 | Goldens move only as named | `cargo test -p ignite --test main -- plan:: eval::` | `terminal.model.json`, `terminal.design.model.json`, `todo.model.json` and the explain goldens differ from `340f782` in these ways only: (a) source positions inside `std/protocol/*.ignite`; (b) the field `maximum_subnegotiation` of the server's `sessions`; (c) the property `limits` of the todo server, in its three places: `defaults`, a `sources` entry pointing into `std/protocol/http.ignite`, and `properties` | U5 |
| 17 | Codegen revision | `grep -n 'CODEGEN_REVISION: &str' crates/ignite/src/apply/plan.rs`, and `cargo test -p ignite --test main apply_admission::` | one match. At the slice-14 close its value is `ignite-codegen/0.1.0-2`; slice 15 bumps it further (`0.1.0-3` for the type-parameter shadowing fix, 19.16), so a later tree checks only that the value is past `0.1.0-1`. The admission tests that read `CODEGEN_REVISION` pass | U3 |
| 18 | Gate | `task gate` | exit 0; the measurements of 18.18 are recorded in `AGENTS.md` | U5 |

### 18.1 Scope and non-goals

In scope:

- the bound syntax;
- `ByteStream` with `read`, `write_all`, `close` and `set_idle_timeout`;
- `Pipe.pair()` and `PipeStream` with `received_batches`;
- the limits records: Telnet's existing `SessionLimits`, extended, and the new `HttpLimits`;
- the generic roles;
- the pipe tests on both targets;
- E096 and E099.

Not in this slice:

- From later protocol phases:
  - user `interface` and `impl` declarations, associated types and `Eq` (P4);
  - a second interface in a bound (`A + B`, P4);
  - `ByteStream.info` from protocols.md §1.2 (P4, the transports);
  - `std/transport/*`, `Listener { transport = .. }`, TLS and Unix sockets (P4);
  - the split of HTTP's head limit into request line and field section (P3);
  - catalogues, `conforms` and `satisfies` (P2);
  - the Q method (P5).
- Bound and interface forms:
  - bounds on record, enum or alias type parameters;
  - an interface used as a type (`dyn`);
  - an interface as a value.
- Callers that stay on `TcpStream`: `await_prompt`, `TelnetPrompt.main`, `TelnetClient.main`,
  and the `connect` field of `std.cli`'s session handler.
- Pipe features:
  - backpressure on a pipe: a full queue fails the writer and never blocks it (D6);
  - a pipe between processes;
  - `Pipe.pair()` outside tests.
- No change to the model format, `ignite run`, `ignite apply`, the journal or any TCP
  behaviour. The new fields reach the model as ordinary record properties.

### 18.2 Current state (at `340f782`)

| fact | where |
|---|---|
| A type parameter is a bare identifier, `('<' ident (',' ident)* '>')?`, stored in `FnDecl.type_params: Vec<Ident>` | `crates/ignite-syntax/src/parser.rs:800` (`type_params`), `crates/ignite-syntax/src/ast.rs:276` |
| The Rust target gives every type parameter `Clone + Send + Sync + 'static` | `crates/ignite-rustgen/src/lib.rs:1479` (`PARAM_BOUNDS`), `:1483` (`bounded_params`) |
| A role call is emitted without a turbofish (`Lines::serve(__spawn_0, __spawn_1)`). A turbofish appears only on fn values, associated calls and record literals, from `TypedProgram::instance` | `crates/ignite-rustgen/tests/golden/generics.rs:160`; `crates/ignite-rustgen/src/lib.rs:1935` (`turbofish`), used at `:1849`, `:2675`, `:2875` and `:4376`; `crates/ignite-types/src/lib.rs:1292` |
| A fn value is `Arc<dyn Fn(..) + Send + Sync>` | 13.3; `crates/ignite-rustgen/src/lib.rs:1848` (`fn_value`) |
| The LLVM target monomorphises generics and names an instance `"<module>.<name><A,B>"`, joining `mangle` of each type argument. A `TcpStream` is an `i64` handle | `crates/ignite-llvm/src/lower.rs:4165` (`instance_symbol`); `crates/ignite-llvm/src/layout.rs:588` (`mangle`), `:601`; the `i64` layout at `layout.rs:339` and `:571` |
| A task owns a `TcpStream` moved into `spawn` and releases it when the task ends. No other type is treated so | `crates/ignite-llvm/src/lower.rs:4019` (`spawn_with`), `:4073`, `:4110` |
| `read`, `write_all`, `close` and `set_idle_timeout` exist only on `TcpStream`. On a type parameter they are E041 from `ignite-types`: `builtins::method` has no `Ty::Param` arm, and gives such a receiver only `copy`. `ignite-check`'s `builtin_applies` decides only builtin against UFCS | `crates/ignite-types/src/builtins.rs:30` (`method`), `:57` to `:72`; `crates/ignite-check/src/lib.rs:3598` (`builtin_applies`); fixture `crates/ignite-types/tests/fixtures/e041_param_method` |
| `can_hold_handle` already holds for every type parameter | `crates/ignite-check/src/lib.rs:1958` (`can_hold_handle`) |
| The effect of a substrate method follows from its name: `read` needs `<root>.read`, the other three need `<root>.write` | `crates/ignite-check/src/lib.rs:292` (`method_effect`) |
| `Store.new()` outside a test is E051, and the message names only `Store.new()` | `crates/ignite-check/src/lib.rs:3156` (in `path_call`); 12.3 |
| A declaration named like a builtin type, builtin function or static owner is E004. An import from `std.net` accepts a name that `is_builtin_name` knows: builtin types, variants and functions. The fixture `e016_effects` declares `protocol Pipe` | `crates/ignite-check/src/lib.rs:931` (in `collect`), `:582` (`is_builtin_name`); `crates/ignite-check/tests/fixtures/e016_effects.ignite:1` |
| The builtin tables `BUILTIN_METHODS`, `EFFECTFUL_METHODS` and `BUILTIN_STATIC_ARITIES` list each method and static by name | `crates/ignite-check/src/lib.rs:163`, `:149`, `:226` |
| E030 checks only the number of a role's parameters, from the public `PROTOCOL_ROLES: &[(&str, usize)] = &[("serve", 2), ("connect", 1)]` | `crates/ignite-check/src/lib.rs:210` (`PROTOCOL_ROLES`), `:1346` (in `protocol_roles`) |
| `ModuleTypes::query_snapshot` keeps only records, enums, defaults and generics for the retained queries. A new query must extend it | `crates/ignite-types/src/lib.rs:654` (`query_snapshot`) |
| Write coalescing (section 7): `write_all` appends to the stream's buffer and registers the stream in the task's `PENDING`. `flush()` sends every pending buffer. It runs before every read, on `close`, at task end and at the end of `run`. A buffer of `SEND_BUFFER` (64 KiB) is sent at once, and dropping the last handle sends and closes | `crates/ignite-io/src/net.rs:32`, `:143` to `:169`, `:223`, `:325` to `:351`, `:372`, `:404`; `crates/ignite-io/src/task.rs:132` (in `work`, task end); `crates/ignite-io/src/lib.rs:144` (in `exit`), `:248` and `:258` (in `run`) |
| An idle timeout makes a waiting read return `Eof`. A value of 0 or less removes it, and clones share it | `crates/ignite-io/src/idle.rs:13` |
| `ignite-io` has no pipe, no `received_batches` and no stream trait | `crates/ignite-io/src/lib.rs:25` (the exports) |
| The runtime handle table holds a `Listener` or a `Stream`. `tcp_own` and `tcp_release` take any handle | `crates/ignite-rt/src/net.rs:57` (`enum Handle`), `:41` (`ignite_rt_tcp_own`), `:326` (`ignite_rt_tcp_release`) |
| `Telnet.serve(stream: TcpStream, application: Console)` calls `serve_with(.., SessionLimits {})`. `TelnetServer.main` spawns `serve_with` directly, so outside the tests no std code calls `Telnet.serve` | `std/protocol/telnet.ignite:21` to `:27`, `:96`, `:144` |
| `SessionLimits` (slice 10) has the fields `maximum`, `idle_timeout`, `input_buffer`, `output_buffer` and `slow_reader_deadline`. Only `idle_timeout` has behaviour | `std/protocol/telnet.ignite:118` (`SessionLimits`) |
| The subnegotiation bound is the constant `MAXIMUM_SUBNEGOTIATION = 1024`. `decode` uses it for both server and client, and counts the IAC of `IAC SE` toward it | `std/protocol/telnet.ignite:89` (`MAXIMUM_SUBNEGOTIATION`), `:390` (`decode`); the test "a subnegotiation is limited to 1024 bytes including the IAC of IAC SE" |
| `BytesToText` is lossy on both targets: an invalid byte becomes U+FFFD | `String::from_utf8_lossy` in `ignite-rustgen`, `text_from_bytes_lossy` in `ignite-llvm`; `crates/ignite/tests/terminal.rs` expects `\u{FFFD}` |
| `examples/terminal/commands.ignite` is copied and edited by tests. `llvm_build::rebuild_compiles_only_what_changed` replaces every `lines = ["ready"]` with `lines = ["ready now"]` and asserts that the text changed. Tests in `apply::` and `apply_recovery::` replace `Status => Reply { lines = ["ready"] }` | `crates/ignite/tests/llvm_build.rs` (`rebuild_compiles_only_what_changed`), `crates/ignite/tests/apply.rs`, `crates/ignite/tests/apply_recovery.rs` |
| A sibling module is a file beside the program whose package is `<program package>.<stem>`. So a file with its own package below `examples.terminal` cannot import `examples.terminal.commands` (E031) | `crates/ignite/src/lib.rs:877` (`parse_siblings`); 14.4 |
| `maximum_line` is a field of `Console` and of `CommandConsole`, not of a limits record | `std/console.ignite:13`, `:40` |
| HTTP's limits are the module constants `MAXIMUM_HEAD` 8192, `MAXIMUM_BODY` 1048576, `IDLE_TIMEOUT_MS` 30000, `MAXIMUM_DRAIN` 65536 and `DRAIN_TIMEOUT_MS` 1000. The role is `Http.serve<S>(stream: TcpStream, application: HttpApp<S>)`. One head limit covers both the request line and the fields (431) | `std/protocol/http.ignite:52` to `:69`, `:412` |
| Every inline Telnet and HTTP session test runs over loopback TCP | `std/protocol/telnet.ignite:831` (`exchange`), `:873` (`exchange_with`); `std/protocol/http.ignite:557` (`connected`), `:657` (`served`) |
| Three tests pin the shape of `Telnet.serve` | `crates/ignite-syntax/tests/parse_terminal.rs:111`, `crates/ignite-check/tests/terminal.rs:59`, `crates/ignite-types/tests/terminal.rs:131` |
| Saved plans pin the backend generation as `CODEGEN_REVISION = "ignite-codegen/0.1.0-1"` | `crates/ignite/src/apply/plan.rs:17` |
| No code from E090 to E099 is emitted | section 8 |

### 18.3 Bounds and `ByteStream`

Grammar. The rule replaces section 3's type parameters for fns; records, enums and aliases
keep theirs:

```
fn_type_params := '<' fn_type_param (',' fn_type_param)* '>'
fn_type_param  := ident (':' ident ('+' ident)*)?
```

- It applies to `fn` declarations, associated fns and protocol roles.
- A `:` in the type-parameter list of a record, enum or alias is E001 with the message "a
  bound is allowed on the type parameters of a fn or role only".
- The parser accepts `+`, so the shape is protocols.md §2.2's. The checker refuses a second
  interface (E099).
- No keyword is added (invariant 6): `ByteStream` is a builtin name.

`ByteStream` is the only interface. Its methods, for `S` the bounded parameter:

| method | signature | effect on the receiver's root |
|---|---|---|
| `read` | `(stream: S, maximum: Int) -> IoResult<Input>` | `read` |
| `write_all` | `(stream: S, data: Bytes) -> IoResult<Unit>` | `write` |
| `close` | `(stream: S) -> IoResult<Unit>` | `write` |
| `set_idle_timeout` | `(stream: S, ms: Int) -> IoResult<Unit>` | `write` |
| `write_urgent` | `(stream: S, data: Bytes) -> IoResult<Unit>`, added by 19.21 | `write` |
| `urgent_pending` | `(stream: S) -> IoResult<Bool>`, added by 19.21 | `read` |

The signatures and effects are those of `TcpStream` today. The methods are called as methods
(`stream.read(n)`) or by UFCS, as `TcpStream`'s are.

`ByteStream` is implemented by `TcpStream`, by `PipeStream`, and by a type parameter bounded
by `ByteStream`. Nothing else implements it.

`ByteStream`, `Pipe` and `PipeStream` are builtin names, visible in every module as `TcpStream`
is:

- `import std.net.{ByteStream, Pipe, PipeStream}` is accepted. Section 8's E031 rule names the
  builtins of `std.net`, and this slice adds the three. `is_builtin_name` therefore also accepts
  `Pipe` and the names in `BUILTIN_INTERFACES` (18.13).
- A program that declares one of these names gets E004 (D15).

Rules:

- In the body of a fn with `<S: ByteStream>`, a value of type `S` has these four methods and
  `copy`, which every type but `Never` has. Any other method or field is E041, and `==` on it
  is E045, as today.
- A use that instantiates `S` with a type that does not implement `ByteStream` is E096. Uses
  are:
  - a direct call, a role call, a UFCS call and a `spawn`;
  - a fn value of the generic fn.

  The type is the one the typer infers for `S` at that use (11.7).
- `ByteStream` in a type position is E099: a parameter, a field, a return type or a type
  argument.
- A bound that names anything else, or names two interfaces, is E099.
- An unbounded type parameter keeps today's rules: `s.read(..)` on it is E041.

Effects (section 5): the rules are unchanged. The root of `stream.read` is the parameter
`stream`, so `requires [stream.read, stream.write]` covers a bounded stream as it covers a
`TcpStream`. `ignite-check` runs before types. It therefore treats a receiver whose declared
type is a `ByteStream`-bounded parameter like `TcpStream` in `builtin_applies`, so the four
methods resolve as builtins and not by UFCS. `can_hold_handle` needs no change, because it
already holds for every type parameter (18.2). The E041 on an unbounded parameter comes from
`ignite-types`' `builtins::method`, which gains the `ByteStream` arms for a bounded
`Ty::Param`.

As landed (U1): the E001 of a bound on a record, enum or alias is at the bound; the E099 of
two interfaces is at the second, that of an unknown interface at it, and that of `ByteStream`
in a type position at the type name. The checker resolves the four methods (and `copy`,
`debug`) of a bounded receiver in `builtin_method`, from the fn's set of bounded parameters,
before `builtin_applies` is asked; the typer looks a bounded `Ty::Param`'s method up in
`builtins::stream_method` (the `Stream*` builtins) before `builtins::method`. E096 is checked
where the typer records a generic instance: calls, role calls, UFCS calls and `spawn` through
`generic_call_refs`, fn values through the name path, at the use's span.

### 18.4 `Pipe` and `PipeStream`

| builtin | signature | effect on the receiver's root | where it may be called |
|---|---|---|---|
| `Pipe.pair()` | `() -> (PipeStream, PipeStream)` | none | in tests and test-only items; elsewhere E051, as `Store.new()` (12.3) |
| `PipeStream.read` | as in `ByteStream` | `read` | anywhere |
| `PipeStream.write_all`, `.close`, `.set_idle_timeout` | as in `ByteStream` | `write` | anywhere |
| `PipeStream.received_batches` | `(stream: PipeStream) -> Int` | `read` | anywhere |
| `PipeStream.write_urgent`, `.urgent_pending` | as in `ByteStream` (19.21) | `write`, `read` | anywhere |

What is written on one end is read on the other. Each end works as follows.

- **Handles.**
  - An end is a handle, and its clones share it, as a `TcpStream`'s do (section 7).
  - `PipeStream` is a handle type wherever `TcpStream` is one: in `can_hold_handle`, for E045,
    for `debug` (which prints `<PipeStream>`) and in the region rules of the LLVM target.
- **Writing.**
  - `write_all(data)` appends to the end's send buffer and registers the end in the current
    task's pending list, exactly as for `TcpStream`.
  - The buffer is delivered by the writing task's next flush: before any of its reads (on any
    stream), on `close`, at task end, at the end of `run`, at exit, or by
    `ignite_io::flush()`.
  - It is also delivered at once when it reaches `SEND_BUFFER`, or when the end's last handle
    is dropped.
- **Delivery.**
  - A delivery moves the whole buffer to the peer's receive queue in one step and adds one to
    the peer's batch counter.
  - An empty buffer delivers nothing and is not counted.
  - The receive queue is bounded (D6): `PIPE_CAPACITY` is 16 MiB of unread bytes. A delivery
    that would take the queue past it fails as a whole. None of its bytes are queued, no batch
    is counted, and the error `pipe queue full` is kept on the writing end, as a peer that is
    gone is (below). A delivery never blocks.
  - A test that needs more than 16 MiB in flight reads it in parts.
- **Reading.**
  - `read(maximum)` first flushes the current task. `maximum` is clamped to `1..=1 MiB`, as
    for `TcpStream`.
  - It returns up to `maximum` bytes from the receive queue. One read may span several
    deliveries.
  - It blocks while the queue is empty and the peer's write half is open.
  - It returns `Eof` when the queue is empty and the peer's write half is closed (by `close`,
    or when the peer's last handle is dropped). It also returns `Eof` when the idle timeout
    expires.
- **`received_batches()`** is the number of deliveries into this end's queue since
  `Pipe.pair()`. Reads do not change it (D7).
- **Closing.**
  - `close()` delivers the buffer, then closes the end's write half. The peer reads the queue,
    then `Eof`.
  - After `close`, a `write_all` on any handle of the end returns the `TcpStream` error `write
    to a closed stream`. That error is the writer's and is not kept.
  - A second `close` is not an error. Reads on the closed end continue.
- **A peer that is gone.** When every handle of one end is gone, a delivery from the other
  end fails. The error `pipe peer is gone` is kept, and the writing end's next `read`,
  `write_all` or `close` returns it, as a failed `TcpStream` send's error is kept (section 7).
- **Idle timeout.** `set_idle_timeout(ms)` sets the read timeout of this end, shared by its
  clones. A value of 0 or less removes it.
- **Concurrency.** Two tasks reading one end are serialised, as on a `TcpStream`. A read and a
  write on the same end do not wait for each other.
- **When an end is dropped differs between the targets.** On the Rust target a `spawn` clones
  its arguments, so the spawner's binding keeps the end alive until its scope ends. On the LLVM
  target an end moved into a task is released when that task ends, and an end held by a test
  fn lives until the process exits. A pipe test therefore closes every end whose `Eof` it
  expects, explicitly; `Telnet.serve` and `Http.serve` close their stream, so the helpers of
  18.12 do.

As landed (U1): `Pipe.pair()` needs no `requires` entry, as the table says, but the typer's
pure-context set counts it as the ambient operation `pipe.pair`, as `Store.new` is
`store.new`, so a property expression or constant that calls it is E116 (18.17).

### 18.5 Limits records

**Telnet.** The existing `SessionLimits` is Telnet's limits record (D4). It gains one field:

```ignite
maximum_subnegotiation: ByteSize = 1 KiB
```

- `Telnet.serve` reads `idle_timeout` (as `serve_with` does today) and `maximum_subnegotiation`.
- The other fields keep their slice-10 status: declared and validated, without behaviour.
- `maximum_line` stays on `Console`.
- The client role `connect` takes no limits, since its arity is 1 (4.2). It keeps
  `MAXIMUM_SUBNEGOTIATION`, so slice 9 item 8 holds: `tn` exits 4.

**HTTP.** A new record in `std/protocol/http.ignite` (D5):

```ignite
type HttpLimits {
  head: ByteSize = 8 KiB
  body: ByteSize = 1 MiB
  idle_timeout: Duration = 30 seconds
  drain: ByteSize = 64 KiB
  drain_timeout: Duration = 1 second
}
```

- The five constants it replaces are removed: `MAXIMUM_HEAD`, `MAXIMUM_BODY`,
  `IDLE_TIMEOUT_MS`, `MAXIMUM_DRAIN` and `DRAIN_TIMEOUT_MS`.
- `READ_SIZE` and `MAXIMUM_LENGTH_DIGITS` stay constants. They are framing facts, not limits.
- The defaults equal today's constants, so the behaviour of `todod` is unchanged.

### 18.6 Roles and E030

| role | signature after the slice |
|---|---|
| `Telnet.serve` | `serve<S: ByteStream>(stream: S, limits: SessionLimits, application: Console) -> IoResult<Unit> requires [stream.read, stream.write]` |
| `Telnet.connect` | `connect<S: ByteStream>(stream: S) -> IoResult<Unit> requires [stream.read, stream.write, stdin.read, stdin.mode, stdout.write, process.exit]` |
| `Http.serve` | `serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S>) -> IoResult<Unit> requires [stream.read, stream.write, application.*]` |

E030 accepts a `serve` role with 2 parameters `(stream, application)` or with 3 parameters
`(stream, limits, application)` (D12). The message for any other count is "role `P.serve`
takes 2 parameters (stream, application) or 3 (stream, limits, application), found N".
`connect` keeps 1 parameter.

### 18.7 Diagnostics E096 and E099

| code | rule | phase | fixture |
|---|---|---|---|
| E096 | A use instantiates a `ByteStream`-bounded type parameter with a type that does not implement `ByteStream` (18.3). The message is "`T` does not implement `ByteStream`, which `S` of `f` requires; pass a `TcpStream`, a `PipeStream`, a `TlsStream` or a type parameter bounded by `ByteStream`" | types (`ignite-types`) | `crates/ignite-types/tests/fixtures/e096_bound_not_met`, with one case each: `Int`, a record, an unbounded type parameter passed on, a fn value of the generic fn, and a `spawn` |
| E099 | A bound naming an interface other than `ByteStream` ("unknown interface `X`; the only interface is `ByteStream`"). A bound naming two interfaces ("`S` has more than one bound; only `ByteStream` exists"). `ByteStream` in a type position ("`ByteStream` is an interface, not a type; declare a type parameter `<S: ByteStream>`") | check (`ignite-check`) | `crates/ignite-check/tests/fixtures/e099_malformed_bound`, one case per message |

Further fixture obligations:

- `crates/ignite-syntax/tests/fixtures/e001_bound_on_record`: a bound on a record type
  parameter.
- `crates/ignite-check/tests/fixtures/e051_pipe_pair`: `Pipe.pair()` in a fn is E051, and in a
  test it is silent. The message is "`Pipe.pair()` is test-only; a program's streams come from
  a listener or a connect". `Store.new()` keeps its message.
- `e030_protocol_roles` gains a `serve` with 4 parameters.
- `e004_builtin_name` gains `type PipeStream {}`.
- `e016_effects` renames its protocol `Pipe` to `Relay`, together with its `.expected` file.
- `crates/ignite-types/tests/fixtures/valid_bounds` checks silently. It contains a bounded fn
  instantiated with `TcpStream`, with `PipeStream` and with an outer bounded parameter; a
  generic role; UFCS on a bounded value; and a `spawn` of a bounded instance.
- The code lists of `every_checker_code_has_a_fixture` and `every_type_code_has_a_fixture` gain
  `e099` and `e096`.

E097 is not used. See D3.

As landed (U1): beyond the list, `e016_effects` gains a bounded fn missing `stream.write`,
`e030_protocol_roles` a valid 3-parameter `serve`, the check fixture `valid_imports` imports
`ByteStream`, `Pipe` and `PipeStream` from `std.net`, and `e001_bound_on_record` also covers
an enum and an alias. The new message of E030 also changes
`crates/ignite-types/tests/audit/effects_a16_roles.expected`, which no unit owns.

### 18.8 Substrate (`crates/ignite-io`)

- A new module `pipe.rs` provides the following, exported from `lib.rs`:
  - `pub struct Pipe;` with `Pipe::pair() -> (PipeStream, PipeStream)`;
  - `#[derive(Clone, Debug)] pub struct PipeStream`, with the inherent methods `read(&self,
    i64) -> Result<Input, IoError>`, `read_into(&self, &mut [u8]) -> Result<usize, IoError>`,
    `write_all(&self, Vec<u8>)`, `write_from(&self, &[u8])`, `close(&self)`,
    `set_idle_timeout(&self, i64)` and `received_batches(&self) -> i64`. Their semantics are
    those of 18.4.
- `pub trait ByteStream: Clone + Send + Sync + 'static` has the methods `read`, `write_all`,
  `close` and `set_idle_timeout`, over `&self` with the inherent signatures. It is implemented
  for `TcpStream` and for `PipeStream` by delegation, and exported.
- The pending list covers both kinds. `flush()` and every existing flush point deliver pipe
  buffers as well as socket buffers: before a read, on close, at task end, at run end, at
  exit and on drop. One list or two is the implementer's choice; the order of deliveries
  within one flush is not specified.
- Each end has its receive queue behind a mutex and a condvar, and its own batch counter,
  idle timeout, closed flag and kept failure. A clone shares all of them.
- `PIPE_CAPACITY` is a constant of `pipe.rs`, 16 MiB (18.4, D6).
- No TCP behaviour changes. `flush()` and `PENDING` in `net.rs` change to cover pipes; the
  existing `tcp::` and `terminal::` tests pass unchanged.

Tests in `crates/ignite-io/tests/pipe.rs`, a new module of `tests/main.rs`:

- `three_writes_before_a_read_arrive_as_one_batch`
- `a_read_flushes_the_reading_task_first`
- `send_buffer_threshold_delivers_at_once`
- `close_delivers_then_eof`
- `write_after_close_is_the_writers_error`
- `dropping_the_last_handle_delivers_and_closes`
- `a_vanished_peer_fails_the_next_operation`
- `a_full_queue_fails_the_writer_and_queues_nothing`
- `idle_timeout_reads_eof`
- `task_end_delivers`
- `clones_share_the_end`

As landed (U2):

- One pending list: `net.rs`'s `PENDING` holds weak references to a crate-private trait
  `Buffered` (`flush_kept`), implemented by the TCP stream and the pipe end. `ByteStream` is in
  `stream.rs`.
- Only a delivery made by a flush keeps its error. A delivery made by `write_all` at
  `SEND_BUFFER`, or by `close`, returns `pipe queue full` or `pipe peer is gone` to that call,
  as a `TcpStream`'s send at the threshold or on close does. Neither queues a byte or counts a
  batch.
- `close` closes the write half also when it returns a kept or a delivery error, so the peer
  still reads `Eof`. A `TcpStream`'s `close` returns before its shutdown in that case.
- Dropping the last handle of an end also discards the unread bytes of its own queue.
- One more test, `byte_stream_dispatches_to_the_pipe`, calls the trait as `<S as
  ignite_io::ByteStream>::read(..)`, the form of 18.10.

### 18.9 Runtime ABI (`crates/ignite-rt`)

- The handle table gains `Handle::Pipe(PipeStream)`. A pipe handle is a slot like any other,
  with the same generation and reuse rules.
- New entries, add-only:

| entry | C signature |
|---|---|
| `ignite_rt_pipe_pair` | `void (i64* first, i64* second)` |
| `ignite_rt_pipe_read` | `i32 (i64 pipe, i64 maximum, Input* out, IoError* err)`; reads straight into the task's arena, as `tcp_read` does |
| `ignite_rt_pipe_write_all` | `i32 (i64 pipe, List* bytes, IoError* err)` |
| `ignite_rt_pipe_close` | `i32 (i64 pipe, IoError* err)` |
| `ignite_rt_pipe_set_idle_timeout` | `i32 (i64 pipe, i64 ms, IoError* err)` |
| `ignite_rt_pipe_received_batches` | `i64 (i64 pipe)` |

- `ignite_rt_tcp_own` and `ignite_rt_tcp_release` keep their names and take a pipe handle as
  well. Their doc comments say "any handle".
- A `pipe_*` entry given a TCP handle is an error, and the reverse too: "`<h>` is not a pipe"
  and "`<h>` is not a TCP stream".
- Tests in `crates/ignite-rt/tests/pipe.rs`, a new module:
  - `pair_write_read_and_batches_through_the_abi`
  - `release_of_the_last_handle_closes_the_peer`
  - `a_pipe_entry_refuses_a_tcp_handle`

As landed (U2): `ignite_rt_pipe_received_batches` has no error out-parameter; given a handle
that is not a pipe it returns 0, as `ignite_rt_tcp_local_port` does for one that is not a
listener. A TCP entry given a pipe gives "`<h>` is not a TCP stream", and `tcp_accept` "`<h>` is
not a TCP listener". `tcp_read` and `pipe_read` share one arena-read helper.

### 18.10 Rust target

- A bounded parameter is emitted as `S: ignite_io::ByteStream + Clone + Send + Sync +
  'static`. An unbounded parameter keeps `PARAM_BOUNDS`.
- A call of one of the four methods on a receiver of a bounded type is fully qualified with
  the type: `<S as ignite_io::ByteStream>::read(&stream, n)`. Parameters are passed by
  reference (`stream: &S`), so `&stream` is `&&S`. Naming `S` fixes the expected type `&S`,
  and the `&&S` coerces; the unqualified `ignite_io::ByteStream::read(&stream, n)` would infer
  `Self = &S` and fail with E0277. The other arguments follow the convention of the existing
  `TcpStream` builtins.
- A call on a concrete `TcpStream` or `PipeStream` stays an inherent method call.
- Any use of a fn or role with at least one bounded type parameter carries a turbofish of all
  its type arguments: a call, a role call, a `spawn` or a fn value. Examples are
  `Telnet::serve::<ignite_io::TcpStream>(..)` and `Http::serve::<ignite_io::TcpStream, S>(..)`.
  Calls of unbounded generic fns are emitted as today, so no existing golden moves (D8).
- `Ty::PipeStream` is emitted as `ignite_io::PipeStream`, `Pipe.pair()` as
  `ignite_io::Pipe::pair()`, and `debug` gives `<PipeStream>`.
- `apply::plan::CODEGEN_REVISION` becomes `ignite-codegen/0.1.0-2`. This one bump covers both
  backends of the slice.
- Golden: `crates/ignite-rustgen/tests/golden/bounds.ignite` with `bounds.rs`, checked by
  `golden::bounded_streams` and built by cargo. It holds:
  - a bounded `relay<S: ByteStream>` and a role of a local protocol generic over the stream;
  - calls with `TcpStream` and with a `PipeStream` parameter of a non-test fn;
  - a `spawn` of a bounded instance.

As landed (U3): the golden also holds an unbounded `identity<T>` (its call has no turbofish),
UFCS calls of a bounded fn (`once::<T>(inner)`), a system whose `relayd` main spawns the role,
and a test with `Pipe.pair()`, so `bounds.main.rs` exists too. `golden::bounded_streams` builds
both the program and its test build (`ignite_io::Pipe::pair()`) with cargo, through a helper of
its own in `golden.rs`, since `compiles.rs` is not U3's. `PipeStream`'s five methods stay
inherent calls, as `TcpStream`'s do. The Rust copy of `bounds.ignite` is
`crates/ignite-rustgen/tests/golden/bounds.ignite`; no test checks it against the LLVM copy.

### 18.11 LLVM target

- `Ty::PipeStream` has the `i64` layout and mangles as `PipeStream`. Instances are named by
  `instance_symbol`, as today.
- The typer resolves the four methods on a bounded receiver to `Builtin::Stream{Read,
  WriteAll, Close, SetIdleTimeout}`. Lowering an instance takes the substituted receiver type:
  `TcpStream` lowers to the `ignite_rt_tcp_*` entries, and `PipeStream` to the
  `ignite_rt_pipe_*` entries. No run-time dispatch remains (D11).
- `Pipe.pair()` calls `ignite_rt_pipe_pair` and builds the tuple. The `PipeStream*` builtins
  call their entries.
- `spawn_with` owns and releases a moved `PipeStream` as it does a `TcpStream`. The same
  applies to the release in the failed-spawn path.
- `region.rs` `mutates`: none of the new builtins mutates. `glue.rs` `debug` gives
  `<PipeStream>`.
- Golden: `crates/ignite-llvm/tests/golden/bounds.*` from the same `bounds.ignite`, checked by
  `golden::bounded_streams`. It shows two instances of `relay`, `relay<TcpStream>` calling
  `@ignite_rt_tcp_read` and `relay<PipeStream>` calling `@ignite_rt_pipe_read`, and it
  compiles with clang.

As landed (U4): the `ignite_rt_pipe_*` declarations are `ir::PIPE_RUNTIME`, and a module
declares one only when it calls it, so the IR of every module without pipes, and every
earlier golden, is unchanged; `RUNTIME` is untouched. A moved `PipeStream` is owned and
released through `ignite_rt_tcp_own` and `ignite_rt_tcp_release`, which take any handle
(18.9). `instance_symbol` is exported from the crate root for row 11. The LLVM crate keeps
its own copy of `bounds.ignite` (its Hub executable and a test); `golden::bounded_streams`
also links the modules with `libignite_rt.a`, and `golden::bounded_streams_run_over_a_pipe`
lowers its test in a test build, links it with a `main` written in the test (this crate's
archive has no runner) and runs it.

### 18.12 Standard library and examples

`std/protocol/telnet.ignite`:

- `Telnet.serve` takes the signature of 18.6. Its body is today's `serve_with` with
  `limits`, and `serve_with` is removed.
- `TelnetServer.main` spawns `Telnet.serve(stream, self.sessions, console)`.
- `session`, `answer_line`, `write_text`, `forward_input`, `finish_input`, `run_command` and
  `edit_input` become generic over `<S: ByteStream>`. `session` also takes `limits:
  SessionLimits`, which `Telnet.serve` passes on, because its decoding reads
  `limits.maximum_subnegotiation`.
- `Telnet.connect` becomes generic, and `TelnetClient.main` calls its `TcpStream` instance.
  The module fn `connect(stream: TcpStream)` stays, because it fills `std.cli`'s
  `SessionHandler.connect`, whose type is `fn(stream: TcpStream)`.
- The server's session decodes with `decode_within(state, b,
  limits.maximum_subnegotiation.to_int())`. `decode(state, b)` stays as `decode_within(state,
  b, MAXIMUM_SUBNEGOTIATION)`, so the decoder tests and the client are unchanged.
- `exchange` and `exchange_with` use `Pipe.pair()` with a 5000 ms idle timeout on the client.
  The test "await_prompt returns the text before each prompt and fails when the server
  closes" stays on TCP (D13).
- New tests:
  - "a pipe session answers IAC DO ECHO with IAC WONT ECHO"
  - "over a pipe IAC IAC is one data byte"
  - "a session limit bounds the subnegotiation"
  - "a pipe session ends on its idle timeout" (the renamed "a client silent for the idle
    timeout ends the session")
- The test "the profile and the limits default to what Telnet implements" asserts
  `maximum_subnegotiation == 1 KiB`.

`std/protocol/http.ignite`:

- `HttpLimits` is added (18.5), and `Http.serve` takes the signature of 18.6.
- `session`, `next_request`, `read_head`, `read_body` and `reject` take `<T: ByteStream>` and
  `limits`.
- `HttpServer<S>` gains `limits: HttpLimits = HttpLimits {}`. Its `main` spawns
  `Http.serve(stream, self.limits, self.application)`.
- `connected()` returns a `Pipe.pair()`. A helper `served_over_tcp` keeps "the server answers
  pipelined requests until one asks to close" on TCP as well (D13).
- New tests:
  - "three pipelined requests over a pipe are answered in one batch"
  - "a pipe client gets 400, 413 and 431"
  - "the limits record bounds the body"

`examples/terminal/commands.ignite` gains one test at its end, "the terminal transcript over a
pipe" (18.0 row 3, D14). It serves `Console { prompt = "terminal> ", respond = respond,
maximum_line = 4096 }` with `Telnet.serve` over `Pipe.pair()`. It imports only `std.*`, which a
program may (14.4), and calls the module's own `respond`.

- The test must not contain the text `lines = ["ready"]`. The edits of 18.2
  (`llvm_build::rebuild_compiles_only_what_changed`, `apply::`, `apply_recovery::`) replace
  that text, so the expected `ready\r\n` is written as bytes, not as a `Reply`.
- `terminal.ignite` stays unchanged, and so do its model goldens: a model holds no tests, and
  no golden names a position in `commands.ignite`. The terminal's source-bound plan inputs do
  change, since `commands.ignite` is a sibling; no test pins their digest.
- `commands.ignite` already holds three inline tests, so `task test-ignite` runs it today.

Unchanged:

- `examples/terminal/terminal.ignite`, `examples/todo/todo.ignite` and
  `terminal.design.ignite`;
- `std/cli.ignite`;
- `std/net.ignite` apart from its doc comment, which names the new builtins.

Goldens, re-blessed with the diff named in 18.0 row 16:

- `crates/ignite/tests/golden/*.json`;
- `crates/ignite/tests/golden/*.txt`.

Tests that pin the shape of `Telnet.serve` (18.2) expect 3 parameters `[Param(S),
SessionLimits, Console]`.

As landed (U5): `read_body` takes `limits` for the uniform signature but does not read it,
because `next_request` checks the length against `limits.body` before calling it. The pipe
helpers are `exchange_over_pipe(input, limits, maximum_line)` under `exchange_with` and
`exchange` (Telnet), and `next_with(input, limits)`, `served_with(input, limits)` and the
generic `read_to_end<T: ByteStream>` (HTTP). One more test, "the limits default to 8 KiB, 1 MiB,
30 s, 64 KiB and 1 s", pins `HttpLimits {}`. "the server answers pipelined requests until one
asks to close" asserts the same answer from `served` (pipe) and `served_over_tcp`. The 413 and
431 thresholds are read from `HttpLimits`; `READ_SIZE` and `MAXIMUM_LENGTH_DIGITS` stay
constants (framing, not policy). `commands.ignite` adds `Console` to its `std.console` import
and imports `std.protocol.telnet.{Telnet, SessionLimits}`; `Pipe` and `PipeStream` are
builtin names and need no import. Its `read_once` returns the empty text at end
of stream rather than `None`, because `crates/ignite-types/tests/terminal.rs` requires every
`None` of the module to have the type `Option<Command>`. That test (18.2) also expects the
three `stream.read(4096)` of `std.protocol.telnet` as `StreamRead` (`Telnet.connect`),
`TcpStreamRead` (`await_prompt`) and `StreamRead` (`session`), and finds the call
`session(stream, limits, application)`; the test helpers of the module therefore name their
pipe end `client`. The review's boundary probes are inline tests of the two modules (9 Telnet,
14 HTTP): deliveries split where the decoder or the framing keeps state, written through
`exchange_parts` and `served_parts`, which `pause` between parts, and the limits at their exact
value. In "the Telnet idle timeout alone ends a pipe session" and "the HTTP idle timeout alone
ends a pipe connection" the client has no idle timeout, writes its input before the server starts
and never closes, so only `limits.idle_timeout` (20 ms) ends the session.

### 18.13 Typed API (add-only)

| crate | addition |
|---|---|
| `ignite-syntax` | `pub struct Bound { pub param: Ident, pub interfaces: Vec<Ident>, pub span: Span }` and `FnDecl.bounds: Vec<Bound>`, one entry per bounded parameter. Every `FnDecl { .. }` literal in the workspace gets `bounds: Vec::new()` |
| `ignite-check` | `BUILTIN_TYPES` gains `PipeStream`. `BUILTIN_STATICS` gains `("Pipe", "pair")`, and `BUILTIN_STATIC_ARITIES` gains `("Pipe", "pair", 0)`; `Pipe` is a static owner. `BUILTIN_METHODS` and `EFFECTFUL_METHODS` gain `received_batches`, and `method_effect("received_batches")` is `read`. A new `pub const BUILTIN_INTERFACES: &[&str] = &["ByteStream"]`. `is_builtin_name` also accepts `Pipe` and the names in `BUILTIN_INTERFACES` (imports, 18.3), and the E004 check in `collect` also consults `BUILTIN_INTERFACES`. A new `pub const SERVE_ARITIES: &[usize] = &[2, 3]` gives the `serve` arities of D12; `PROTOCOL_ROLES` stays, unchanged, and E030 reads `SERVE_ARITIES` for `serve` |
| `ignite-types` | `Ty::PipeStream`. The `Builtin` variants `PipePair`, `PipeStreamRead`, `PipeStreamWriteAll`, `PipeStreamClose`, `PipeStreamSetIdleTimeout`, `PipeStreamReceivedBatches`, `StreamRead`, `StreamWriteAll`, `StreamClose` and `StreamSetIdleTimeout`. `ModuleTypes.bounds: BTreeMap<String, BTreeMap<String, String>>` (fn key, then parameter, then interface). `TypedProgram::bound(&self, fn_name: &QualName, param: &str) -> Option<&str>`. `ModuleTypes::query_snapshot` keeps `bounds`, because `bound` is a retained query, and the ownership regression named in its doc comment covers it |
| `ignite-io` | `Pipe`, `PipeStream`, `ByteStream` |
| `ignite-rt` | the entries of 18.9 |

As landed (U1): `crates/ignite-types/tests/surface.rs` has no `FnDecl` literal, so only
`crates/ignite-rustgen/tests/units.rs` and the checker's own unit test gained `bounds`. A
role's key in `ModuleTypes.bounds` and in `bound` is `Protocol.role`, as in `generics`.

### 18.14 Stale points in the sources

1. protocols.md §3.3, §5.3 and §7 make E097 "`Store.new()` outside a test". Since slice 8 that
   is E051 (12.3, section 8). E097 stays unused, and `Pipe.pair()` joins E051 (D3).
2. protocols.md §6.1 says "the program crate contains no `dyn`". Since slice 9, fn values are
   `Arc<dyn Fn>` (13.3). Row 10 checks no `dyn ByteStream` and no `dyn` other than `dyn Fn`
   (D9).
3. protocols.md §7's obligation "the Rust golden shows `Telnet::serve::<ignite_io::TcpStream>`"
   does not fit the code. Since slice 10 the role call is in std's `TelnetServer.main`, which
   calls `serve_with`, and the Rust target emits no turbofish on role calls (18.2). This slice
   makes `TelnetServer.main` call `Telnet.serve` and checks the generated `ignite-std` (D10).
4. protocols.md §3.6 has `TelnetLimits` and `HttpLimits` with `Int` and `_ms` fields. Since
   slice 10, `SessionLimits` exists with quantity fields, and `maximum_line` is the
   console's (D4, D5).
5. protocols.md §5.1 writes `assert(..)` as a call, uses `TelnetLimits {}`, and takes
   `read_text` and `read_bytes` from `std/transport/pipe.ignite`. `assert` is a statement
   (12.3), and P1 has no `std/transport`.
6. protocols.md §7 P1 says "`ByteStream` (from `std.net`)". §4 and vision §2's L2 row put
   `Pipe` and the stream interface in `std/transport/`. P1 follows §7 and uses `std.net`. P4
   moves them, which costs one import line, as §7 says.
7. protocols.md §7 "phase 1 is proposed as slice 8.1". It is slice 14 (vision §4).
8. protocols.md §1.2's `ByteStream` also has `info`. P1 has the four methods only.
9. protocols.md §5.3 moves the todo imports to `std.http`. The module is
   `std.protocol.http` and stays so.
10. vision X17 (stale line and keyword counts in protocols.md) and X18 (line citations) are
    still open in protocols.md. U5 corrects only the P1 lines named above.
11. vision T4 gives slice 13 the range E130 to E139 and no range to slice 14. P1's codes come
    from the protocols range E090 to E099, which T4 assigns.
12. This file's title said "slices 1 to 12". It now says "1 to 14" (the X15 precedent).
13. protocols.md §3.6 decides that roles read limits only from the limits record, and rejects
    module constants. This slice keeps three: `MAXIMUM_SUBNEGOTIATION` for the client role,
    whose arity 1 leaves no limits parameter (D17), and HTTP's `READ_SIZE` and
    `MAXIMUM_LENGTH_DIGITS`, which are framing facts, not limits (18.5). Besides
    `maximum_line` (item 4), these are the slice's deviations from §3.6.
14. Section 10's acceptance, as vision X16 resolves it, bounds `std/protocol/telnet.ignite`
    at 400 lines excluding tests. At
    `340f782` it has about 655 non-test lines (an approximate awk count), and this slice adds
    more. The bound is already exceeded; the slice records it and does not restore it.
15. vision invariant 3's list of handle types lacks `PipeStream`, and vision §4.1's row `14+ |
    B, C, D, E, G, H` does not say that slice 14 is B. U5 corrects both.

### 18.15 Decisions

| # | decision | alternative | why |
|---|---|---|---|
| D1 | Bounds only on fns, associated fns and roles: `<S: ByteStream>`. `+` parses, and the checker refuses it | `where` clauses; bounds on records and enums | protocols.md §2.2's shape. P1 needs fn bounds only. Record bounds need Rust where-clauses on structs and LLVM layout rules that no P1 row uses |
| D2 | `ByteStream` is compiler-known and not declared in Ignite | `interface ByteStream` in `std/net.ignite` now | `interface` is P4's keyword (invariant 6). P1 fixes the names, and P4 declares them (protocols.md §7) |
| D3 | E096 means a bound not met, from the typer. E099 means a malformed bound or an interface used as a type, from the checker. E097 is unused, and `Pipe.pair()` outside a test is E051 | reuse E097 for the malformed bound | a code keeps one meaning across documents (T4). E097's meaning is E051's, and E099 was free |
| D4 | Telnet's limits record is the existing `SessionLimits`, plus `maximum_subnegotiation: ByteSize` | protocols.md's `TelnetLimits { maximum_line, maximum_subnegotiation, idle_timeout_ms }` | `SessionLimits` is in the design file, the model and `validate` (slices 10 and 12), so two records would split one concern. Quantities are the slice-10 style. `maximum_line` is the console's |
| D5 | `HttpLimits` has one `head` limit and quantity fields with today's defaults | protocols.md §3.6's six `Int` fields | P1 keeps behaviour, and the split is P3's acceptance |
| D6 | A pipe's receive queue is bounded at `PIPE_CAPACITY` (16 MiB of unread bytes). A delivery that would exceed it fails as a whole with the kept error `pipe queue full`, and it never blocks | an unbounded queue, recorded as an exception to vision invariant 5; or a bounded queue with blocking delivery | vision invariant 5 is "no silent loss: bounded queues". D16 compiles the pipe into every binary, so the bound holds everywhere and no exception is needed. The failure is loud, as a failed TCP send is. Blocking delivery would add a deadlock between a test's two ends in one task, and no P1 row needs backpressure. Slow-reader limits are P4 and wave C |
| D7 | `received_batches` counts deliveries, one per non-empty flush, including the threshold, close and drop sends | count reads, or count `write_all` calls | protocols.md §5.3 defines it as "the deliveries caused by the peer's flushes". It observes coalescing (§6.3), which is what row 7 tests |
| D8 | Rust: the trait `ignite_io::ByteStream`, fully qualified calls on bounded receivers, and a turbofish on uses of bounded generics only | `dyn ByteStream`, or an enum of stream kinds | static dispatch (protocols.md §6.1) and the golden obligation. Turbofish only for bounded generics, so no existing golden moves |
| D9 | "No `dyn`" means no `dyn ByteStream`, and every `dyn` is a fn value's `dyn Fn` | remove `dyn Fn` from fn values | that is out of scope, and protocols.md §6.1 predates 13.3 |
| D10 | The static-dispatch check reads the generated `ignite-std` of the terminal. `TelnetServer.main` calls `Telnet.serve`, and `serve_with` is removed | a program-crate golden with a user role | the obligation names `Telnet::serve`, which exists only in std. With `serve_with` gone, the role is the one server entry |
| D11 | LLVM: one handle table for both kinds; new `ignite_rt_pipe_*` entries; `Stream*` builtins resolved by the instance's receiver type | one `ignite_rt_stream_*` family that dispatches on the handle kind at run time | protocols.md §6.2: the target calls the concrete entry. Run-time dispatch would be a hidden `dyn` |
| D12 | E030 accepts `serve` with 2 or 3 parameters | always 3, migrating every fixture and golden protocol | the existing protocols `Lines`, `Greeting` and `Line` of the goldens stay valid. Whether a protocol has limits is its own decision |
| D13 | Inline session tests move to pipes. One TCP test per protocol stays, so `ignite test` builds both instances | run every session test on both stream kinds | TCP stays covered end to end by `terminal.rs` and `todo.rs`, and duplicates cost gate time |
| D14 | Row 3's transcript is an inline test at the end of `examples/terminal/commands.ignite`, which calls its own `respond` | a new `examples/terminal/transcript.ignite`; std (which cannot see the example's commands); `terminal.ignite` | a `transcript.ignite` with package `examples.terminal.transcript` has no siblings, so its import of `examples.terminal.commands` is E031 (14.4, observed by the review). It would also be a sibling of `terminal.ignite` and change that program's inputs. `terminal.ignite` and its model goldens stay unchanged |
| D15 | `ByteStream`, `Pipe` and `PipeStream` are builtin names: declaring one is E004, and `e016_effects` renames its `protocol Pipe` | names that only `std.net` provides and that need an import | `TcpStream` works that way, so there is one rule |
| D16 | `Pipe` is always compiled into `ignite-io` and `ignite-rt`, with no cargo feature. E051 alone keeps it in tests | a feature such as `runner` | no feature plumbing in generated crates, and the code is small |
| D17 | `decode_within` takes the server's subnegotiation limit. `decode` and the client keep the constant | a limit parameter on `connect` | `connect`'s arity is 1 (4.2), and slice 9 item 8 fixes the client at 1024 |

### 18.16 Work split

| unit | owns | depends on | first failing test |
|---|---|---|---|
| U1 frontend and skeleton | `crates/ignite-syntax/src/**`, `crates/ignite-check/src/**`, `crates/ignite-types/src/**`; the arms for the new variants in `crates/ignite-eval/src/{eval,value}.rs`; the fixtures of 18.7 and the code lists in the three `fixtures.rs`; the `FnDecl` literals in `crates/ignite-types/tests/surface.rs` and `crates/ignite-rustgen/tests/units.rs`; the doc comment of `std/net.ignite`; the placeholder arms of 18.17; the rows of section 8 (E096 and E099 added, E051 and E030 amended per 18.4 and 18.6), written when the fixtures land so that every listed code has a fixture; the one-line forward references to 18 in sections 3, 4.2 and 11.1 | none | `cargo test -p ignite-check --test main fixtures::fixtures_match_expected` with the new `e099_malformed_bound` fixture |
| U2 substrate and runtime ABI | `crates/ignite-io/src/**` (new `pipe.rs`; `net.rs`, `lib.rs`, `idle.rs`), `crates/ignite-io/tests/{pipe.rs,main.rs}`, `crates/ignite-rt/src/**`, `crates/ignite-rt/tests/{pipe.rs,main.rs}`; the one-line forward reference to 18 in section 7 | none (runs beside U1) | `cargo test -p ignite-io --test main pipe::three_writes_before_a_read_arrive_as_one_batch` |
| U3 Rust target | `crates/ignite-rustgen/src/**` (replacing U1's placeholders), `crates/ignite-rustgen/tests/golden.rs`, `crates/ignite-rustgen/tests/golden/bounds.*`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`) | U1; U2 for the cargo build of the golden | `cargo test -p ignite-rustgen --test main golden::bounded_streams` |
| U4 LLVM target | `crates/ignite-llvm/src/**` (replacing U1's placeholders), `crates/ignite-llvm/tests/golden.rs`, `crates/ignite-llvm/tests/golden/bounds.*` | U1; U2 for linking | `cargo test -p ignite-llvm --test main golden::bounded_streams` |
| U5 std, examples, acceptance and records | `std/protocol/telnet.ignite`, `std/protocol/http.ignite`, `examples/terminal/commands.ignite` (the transcript test only), `crates/ignite-rustgen/tests/streams.rs` and `crates/ignite-llvm/tests/streams.rs` with their `mod` lines in those crates' `tests/main.rs`, the role-shape assertions of the three tests in 18.2, `crates/ignite/tests/golden/**`, `AGENTS.md` (the layout rows for `commands.ignite` and the pipe, and the slice-14 gate-time paragraph), `STATUS.md` (the 18.0 rows), `docs/protocols.md` (the corrections of 18.14), `docs/vision.md` (invariant 3 and the §4.1 row, 18.14 item 15), `PLAN.md`, `docs/performance/14/` | U1 to U4 | `ignite test std/protocol/http.ignite --target all` with "three pipelined requests over a pipe are answered in one batch" added first (E040: `PipeStream` passed where `TcpStream` is expected) |

Order:

- U1 and U2 run first, in parallel.
- U3 and U4 start when U1 has merged, and run in parallel. Each closes only after U2 has
  merged, because its first test needs U2: the golden of U3 builds against
  `ignite_io::ByteStream`, and the IR of U4 links against `ignite_rt_pipe_*`.
- U5 starts when U1 to U4 have merged, and it closes the slice: 18.0 and 18.18.

Planning store: the coordinator creates `epic:slice-14-protocols-p1` in `.engineering/` and
one story per unit (U1 to U5), each citing its row of this table, before the first unit is
dispatched. No unit writes to the planning store.

Rules:

- Shared files:
  - `docs/slice.md` §18 takes "As landed (Ux)" lines, each in the subsection its unit owns.
  - U5 owns 18.0 and 18.18.
  - The `tests/main.rs` of `ignite-io` and of `ignite-rt` are U2's. Those of `ignite-rustgen`
    and `ignite-llvm` change only by U5's `mod streams;` lines, and U3 and U4 add no file
    there.
- One integration-test binary per crate: a new test file is a `mod` line, never a `[[test]]`.
- Layering (`docs/layering.md` rule 5): no file under `crates/*/src` names a protocol. The new
  substrate and generator code speaks of streams and pipes only.

### 18.17 Placeholder arms

U1 adds the new `Ty` and `Builtin` variants. The workspace must still build with
`--all-targets`, so U1 adds placeholders wherever a match must cover them. Each placeholder is
`unimplemented!("slice 14: …")`, and `grep -rn 'slice 14: ' crates/` counts them.

| owner | file | covers |
|---|---|---|
| U3 | `crates/ignite-rustgen/src/lib.rs`: the arms for `Pipe.pair()`, the `PipeStream*` and `Stream*` builtins, bounded parameters and the turbofish rule | 18.10 |
| U4 | `crates/ignite-llvm/src/lower.rs`: the `PipePair`, `PipeStream*` and `Stream*` arms of the builtin lowering | 18.11 |

U1 writes these arms in their final form, because this contract fixes them, so they need no
placeholder:

- the `Ty::PipeStream` layout (`i64`) and mangle (`PipeStream`) in
  `crates/ignite-llvm/src/layout.rs`;
- `mutates` in `region.rs` (false for every new builtin);
- `debug` in `glue.rs` and in rustgen (`<PipeStream>`);
- `ignite-eval`: like `TcpStream`, not evaluable; the typer keeps the new builtins out of
  property expressions (E116).

As landed (U1): four placeholders. Rust target three: one in `function_line` for any fn with
a bound (bounded parameters and the turbofish rule together), one for `Pipe.pair()` in
`static_call`, one for the `PipeStream*` and `Stream*` builtins in `method`; its `rust_ty` of
`Ty::PipeStream` is final (`ignite_io::PipeStream`). LLVM one arm for all ten builtins.

### 18.18 Gate time

Measure as slices 12 and 13 did (15.1, 16.1).

Trees:

- Before: a `git archive 340f782` copy. It includes the workspace member `tools/ignite-perf`.
- After: the closed slice-14 tree, which includes `tools/ignite-perf` as well. Both trees
  therefore have the same workspace members, and fmt, clippy and test cover the same crates
  plus the slice's changes.
- Each tree has its own target directory. Every build sets `CARGO_INCREMENTAL=0`.

Method:

- One cold gate per tree, then three interleaved warm passes.
- Record the 1-minute load for every run, and drop a pair whose two loads differ by more than 5.
- Report the median of the pair ratios next to the absolute times.

Units:

- the warm gate with the apply suites skipped (`cargo test --workspace -- --skip apply`), with
  fmt, clippy, test and test-ignite timed separately;
- `task test-ignite`, warm;
- `ignite test std/protocol/telnet.ignite --target all` and the same for `http.ignite`, cold
  (fresh `--out` and target) and warm;
- `ignite check` and `ignite plan` of the design file (15 pairs);
- `todod` and `terminald`, Rust and LLVM, dev and release (3 pairs, fresh `--out` and target,
  `SCCACHE_RECACHE=1`);
- `--no-run` after `touch crates/ignite-check/src/lib.rs`;
- one full `task gate` per tree, recorded as slice 13 recorded it: wall time, exit status,
  counts and the end-to-end binary's time.

The apply suites inside `task gate` are the slice-13 exception (`AGENTS.md`, "Slice 13"). No
budget applies to them, and they are reported apart.

A unit that rises over 10 % gets a measured cause, or the record labels the cause inferred. No
baseline moves.

Records:

- a "Slice 14" paragraph in `AGENTS.md`'s gate-time section;
- the raw logs and scripts under `docs/performance/14/`.

### 18.19 Open questions

None. Every conflict between the sources had a default, recorded in 18.14 and 18.15.

## 19 Slice 15: RFC catalogues and the conformance matrix (protocols P2)

**Status: contract written 2026-11-03. The independent review (`docs/slice15-review.md`, 2
blockers, 10 should-fix, 17 notes) is resolved in this text; its "Resolution" section maps
each finding to the place that resolves it. Implementation has not started.** The rows of
19.0 are required checks, not results.

Sources:

- `docs/protocols.md` §3.1 to §3.3, §3.7, §6.1, §7 row P2 and §8 (Q4).
- `docs/vision.md` §4 row "G protocols P2 to P5", §6, §7 (T2, T3, T4), invariants 3, 5 and 9.
- This file: sections 3, 4.2, 4.3, 8, 12.1, 12.7, 15 (the evaluator), 18.0 to 18.19 (the
  structure this section follows). Also `docs/layering.md`.
- The RFC texts: RFC 854, 855, 1143 (plain text) and RFC 9110, 9112 (RFCXML v3, prepped), read
  from the RFC Editor on 2026-11-03; digests in 19.9. Only the two RFCXML files are vendored
  (D5).

The base of the slice is commit `296f849` (`main`). The before tree of 19.17 is taken from it.
Everything in this section cites files and symbols, not lines (vision T3). If `main` moves
before the slice closes, the base becomes the merge base of the slice branch and `main` at the
close.

By T2, the P2 row of `protocols.md` becomes a contract here. Where `protocols.md` disagrees
with the code or with a decision of 19.15, this section wins and records the point in 19.14.

For protocols, fns, records, fields and tests, this section extends the grammar of section 3
by the clauses of 19.5. Clauses are declarations about the code, not code: no backend reads
them.

The slice adds:

- the vocabulary module `std.rfc` (`Level`, `Party`, `Source`, `Requirement`, `Spec`);
- five catalogue modules, `std.rfc.rfc854`, `rfc855`, `rfc1143`, `rfc9112` and `rfc9110`, whose
  constants are literal data;
- the six clauses `conforms`, `scope`, `satisfies`, `verifies`, `unsupported` and
  `not_applicable`, contextual words, not keywords;
- the claim model in `ignite-eval`, and E090, E091 and E092;
- `ignite rfc import` and `ignite rfc check`, with a hand-written XML reader;
- `ignite conformance FILE [--target rust|llvm]`;
- dropping unreferenced constants in `ignite-llvm`, so that an unreferenced constant leaves no
  text or debug entry (19.13, D2);
- the claims of `Telnet` (RFC 854, RFC 855) and `Http` (RFC 9112, RFC 9110 server subset).

### 19.0 Acceptance

Every row is a required check. "Exit 0, silent" means exit status 0, empty stdout and empty
stderr. A test name is `<file>::<test>` in its crate's single integration-test binary
(`cargo test -p <crate> --test main <file>::<test>`). Rows 1 to 3 are the three acceptance tests
of the `protocols.md` P2 row. `STATUS.md` gets one dated row per row of this table (row 7).

| # | check | command | required observation | unit |
|---|---|---|---|---|
| 1 | P2 test 1: removing one `satisfies` from a MUST is E090 | `cargo test -p ignite --test main conformance::removing_a_satisfies_from_a_must_is_e090` | The test copies `std/` into a fresh directory under `target/`. Unchanged, `ignite check --std <copy> <copy>/protocol/telnet.ignite` exits 0, silent. The test then removes `RFC854.RP14_1` (the IAC-doubling rule, 19.4) from every `satisfies` clause of the copy, deleting a clause it leaves empty. `ignite check` of the copy then exits 1, and stderr is exactly one line, `…/protocol/telnet.ignite:<line>:<col>: error[E090]: …`, naming `RFC854.RP14_1` | U6 |
| 2 | P2 test 2: the HTTP matrix lists the §7.1 and §3.2 gaps and claims nothing (both closed by story `http-rfc9112-9110-gaps`) | `cargo test -p ignite --test main conformance::http_matrix_lists_the_chunked_and_host_gaps` | The test runs `ignite conformance std/protocol/http.ignite --target rust`, then the same with `--target llvm`. The row of `RFC9112.R2_2_1` (§2.2) and the row of `RFC9110.R6_6_1_2` (§6.6.1), each level `MUST`, and the row of `RFC9110.R6_2_5` (§6.2, level `MUST NOT`, unsupported while `R2_2_1` is a gap; 19.22, story `http-chunked-adversary-findings`) are `unsupported` and `gap` (19.14 item 17; story `http-octet-head-and-date`). The rows story `http-rfc9112-9110-gaps` closed (19.22), among them `R7_1_2` (§7.1, "A recipient MUST be able to parse and decode the chunked transfer coding.", satisfied by `read_chunked`) and `R3_2_6` (§3.2, the Host check), print status `satisfied` and verdict `pass`, their evidence naming the satisfying fn and the passed test that names the row. The block ends with `claim conformance of Http to rfc9112: no (1 gaps, 0 failures, 0 inconclusive)`, and stdout holds `claim conformance of Http to rfc9110: no (2 gaps, 0 failures, 0 inconclusive)` (19.10; 15 and 9 at the slice-15 close). Exit 1. Both targets print byte-identical stdout | U6 |
| 3 | P2 test 3: the Telnet matrix claims RFC 854 for an NVT that refuses every option | `cargo test -p ignite --test main conformance::telnet_matrix_claims_rfc854_for_an_nvt_that_refuses_every_option` | Met since story `telnet-rfc854-conformance` (19.21; 19.14 item 16). `ignite conformance std/protocol/telnet.ignite`, once per target. Stdout holds `claim conformance of Telnet to rfc854: yes (0 gaps, 0 failures, 0 inconclusive)` and `claim conformance of Telnet to rfc855: yes (0 gaps, 0 failures, 0 inconclusive)`. No row of the `rfc854` block has verdict `gap`. The rows `RP5_3`, `RP5_4`, `RP10_2` and `RP12_4` each print status `satisfied` and verdict `pass`; their evidence names, in that order, `apply_key`, `answer_line`, `interrupt` and `session` among the satisfying fns, and the passed tests "Ctrl+D on a partial line sends it without a line end", "the server sends IAC GA after each prompt and the AYT answer", "an interrupt is IAC IP followed by the Synch" and "a Synch discards the data before its Data Mark". The RFC 855 option-negotiation rows `RFC855.RP2_3` (a receiver may find the end of a subnegotiation by IAC SE) and `RFC855.RP2_4` (either party may refuse by sending WON'T or DON'T) each print verdict `pass`. The evidence of each of the two rows names `decode_within`, the fn that answers `DO` with `WONT` and `WILL` with `DONT`, and names both tests, "a subnegotiation is skipped up to IAC SE" and "DO is answered WONT and WILL is answered DONT", as passed. Exit 0. Both targets print byte-identical stdout | U6 |
| 4 | the std protocol modules carry complete claims | `cargo test -p ignite --test main conformance::std_protocol_modules_check_silently` | `ignite check std/protocol/telnet.ignite` and `ignite check std/protocol/http.ignite` each exit 0, silent | U6 |
| 5 | catalogues do not reach compiled programs | `cargo test -p ignite --test main conformance::emitted_programs_carry_no_catalogue` | For `examples/todo/todo.ignite` and `examples/terminal/terminal.ignite`: the stdout of `ignite emit` (the Rust target; `ignite emit` has no `--target`) and every `.ll` file that `ignite build --target llvm` writes (`std.ll`, `program.ll`, `main_<executable>.ll`) contain none of the five `Spec` digests of 19.9 and not the text "parse and decode the chunked transfer coding" | U6 |
| 6 | inline tests | `target/debug/ignite test std/protocol/telnet.ignite --target all`; the same for `http.ignite` | at least 37 passed per target for Telnet, at least 41 for HTTP, 0 failed | U6 |
| 7 | records and gate | `task gate` on the slice branch; read `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/vision.md` | exit 0. Every row of this table has a dated `STATUS.md` row that names its command and says pass. `AGENTS.md` has: a "Slice 15" section with the merge gate's time, counts and exit; layout rows for `std/rfc.ignite`, `std/rfc/`, `docs/rfc/` and `crates/ignite/src/rfc/`; the commands `ignite rfc import`, `ignite rfc check` and `ignite conformance` in the commands table and in the `crates/ignite/` layout row. `PLAN.md` row 15 says done. `docs/vision.md` §4.1's row "15+" states that slice 15 is G's P2. `docs/performance/15/` holds the records of 19.17 | U6 |
| 8 | the vocabulary checks | `target/debug/ignite check std/rfc.ignite`; `cargo test -p ignite-check --test main terminal::standard_library_checks_in_import_order` | exit 0, silent; the test passes with `std.rfc` among the modules | U1 |
| 9 | E091, first trigger: a catalogue constant that is not literal data | `cargo test -p ignite-check --test main fixtures::` | fixture `crates/ignite-check/tests/fixtures/e091_non_literal_catalogue` reports exactly its `.expected`: one E091 for each case of 19.4 (a call, an interpolated text, a name of a constant that is no `Requirement`, an alias `const A: Requirement = R2_1_1`, a list built with `+`). `every_checker_code_has_a_fixture` lists `e091`. The fixture std of `crates/ignite-check/tests/fixtures/` gains `std/rfc.ignite` and one catalogue, and the module list `fixtures.rs` asserts gains them | U1 |
| 10 | clause syntax | `cargo test -p ignite-syntax --test main fixtures::` | fixture `valid_clauses` parses, silent; fixture `e001_malformed_clause` reports one E001 per case: `satisfies` without `[`, a status without a reason, `scope` before `conforms`, a `verifies` after a test's block, a requirement reference with one segment, a `conforms` entry with two segments. `e001_protocol_member.expected` gets the new message for a protocol member that is no `fn`, status or `}` | U1 |
| 11 | clause names resolve and type | `cargo test -p ignite-check --test main fixtures::`; `cargo test -p ignite-types --test main fixtures::` | fixtures `e003_unknown_requirement` (E003, unknown requirement in a known spec; unknown spec), `e004_conforms_twice` (E004), and in `ignite-types` `e040_conforms_not_a_spec` (E040, `conforms` naming a constant of another type; E040, a `scope` party that is no `Party`) report exactly their `.expected`. Each crate has its own `valid_clauses` fixture, silent. The fixture std of `crates/ignite-types/tests/fixtures/` gains `std/rfc.ignite` and one catalogue, and its asserted module list gains them | U1 |
| 12 | a clause-bearing program checks and tests, and its clauses change no emitted byte | `cargo test -p ignite --test main clauses::` | `clauses::clause_fixture_checks_silently`: `ignite check` of `crates/ignite/tests/fixtures/clauses/claims.ignite` exits 0, silent. The fixture holds its own catalogue (one `Spec`, its requirements), a protocol with `conforms`, `scope`, `unsupported` and `not_applicable`, `satisfies` on a fn, a type and a limits-record field, and a test with `verifies`, with a status for every MUST-class requirement in scope, so it stays silent after U2. `clauses::clause_fixture_tests_pass_on_both_targets`: `ignite test` of it passes with `--target rust` and with `--target llvm`. `clauses::clauses_emit_nothing_on_either_target`: the test copies the fixture to `claims.ignite` in a fresh directory under `target/` and, with that directory as the current directory, runs `ignite emit claims.ignite` and `ignite build --target llvm claims.ignite --out out`. It then overwrites `claims.ignite` with the blanked copy, in which every character of every clause except a newline is a space, so that every remaining token keeps its line and column, and runs both commands again with the same arguments. `ignite emit`'s stdout and each `.ll` file under `out` are byte-identical between the two runs | U1 |
| 13 | the layer of `std.rfc` | `cargo test -p ignite-check --test main layering::` | passes with the prefix rule of 19.11; `docs/layering.md` and `LAYERS` agree | U1 |
| 14 | E090, missing status | `cargo test -p ignite --test main plan::fixtures_match_expected` | `e090_missing_status.ignite`: a protocol conforming to a fixture catalogue with one requirement at each of MUST, MUST NOT, REQUIRED, SHALL and SHALL NOT, and no status: exit 1, five E090 as its `.expected` says. `e090_scope.ignite`, the same with one more MUST outside the `scope` sections and one more whose party `scope` excludes: exactly five E090 | U2 |
| 15 | E090, two statuses | same | `e090_two_statuses.ignite`: one requirement both satisfied and `unsupported`, one both `unsupported` and `not_applicable`: two E090 with the two-status message of 19.7 | U2 |
| 16 | E092 | same | `e092_satisfies_foreign.ignite`, `e092_verifies_foreign.ignite`, `e092_status_foreign.ignite`, each with first line `// ignite check --std claims_std`: each names a requirement of `RFC2`, a spec the protocol does not conform to; one E092 each | U2 |
| 17 | E091, second trigger | same | `e091_catalogue.ignite`: one E091 per case of 19.4, with its precedence: an `ordinal` under 1, a name that does not match its section and ordinal (which is how a duplicate section and ordinal shows), a requirement not listed in the `Spec`, one listed twice, an `rfc` that differs from the `Spec`'s, two `Spec` constants in one module | U2 |
| 18 | the claim model API | `cargo test -p ignite-eval --test main claims::` | `claims::should_without_status_is_unclassified`: a SHOULD with no status reads as `Status::None` and in scope. `claims::limit_below_minimum_does_not_meet_it`: a field default of `7999` against `minimum = Some(8000)` reads `meets == Some(false)`, `8 KiB` reads `Some(true)`. The tests build their programs from source through `ignite-syntax`, `ignite-check` and `ignite-types` (dev-dependencies of `ignite-eval`) | U2 |
| 19 | import is deterministic and checks | `cargo test -p ignite --test main rfc::` | `rfc::import_writes_the_golden_twice`: `ignite rfc import` of `crates/ignite/tests/fixtures/rfc/sample.xml` writes a module byte-identical to `sample.ignite`, twice in a row. `rfc::imported_golden_checks_silently`: `ignite check` of the golden exits 0, silent | U3 |
| 20 | keyword rule | same | `rfc::lowercase_must_is_no_requirement`: a sentence with a plain "must" yields no requirement. `rfc::marked_must_is_one_requirement`: a sentence with `<bcp14>MUST</bcp14>` and `<bcp14>SHOULD</bcp14>` yields one requirement with `level = Must` | U3 |
| 21 | ids, sections and the filter | same | `rfc::sections_filter_keeps_ordinals`: with `--sections 2`, the fixture's §2.1 and §2.2 requirements appear with the same ids and ordinals as without the filter, and §3 requirements do not | U3 |
| 22 | digest check | same | `rfc::check_matches_the_source`: `ignite rfc check sample.ignite sample.xml` exit 0, silent. `rfc::check_names_the_catalogue_when_a_byte_changes`: after one byte of a copy of the source changes, exit 1, and stderr names the catalogue file and both digests. `rfc::check_without_source_is_unchecked`: `ignite rfc check sample.ignite` prints `sample.ignite: unchecked` on stdout, exit 0 | U3 |
| 23 | reader and file errors | same | `rfc::malformed_xml_is_exit_2`: an unclosed element, an unknown entity and a DOCTYPE with an internal subset each give exit 2 and one line `<file>:<line>:<col>: <message>`. `rfc::missing_file_is_exit_3`: `ignite rfc import` and `ignite rfc check` of a path that does not exist exit 3 | U3 |
| 24 | no network client | `cargo tree -p ignite -e normal` | lists none of `reqwest`, `ureq`, `hyper`, `curl`, `isahc`, `attohttpc` | U3 |
| 25 | catalogues check | `cargo test -p ignite --test main rfc::committed_catalogues_check_silently` | `ignite check` of each of `std/rfc/rfc854.ignite`, `rfc855.ignite`, `rfc1143.ignite`, `rfc9112.ignite`, `rfc9110.ignite` exits 0, silent | U4 |
| 26 | catalogues match their sources | `cargo test -p ignite --test main rfc::committed_catalogues_match_their_sources` | `ignite rfc check std/rfc/rfc9112.ignite docs/rfc/rfc9112.xml` and the same for 9110 exit 0, silent. `ignite rfc check std/rfc/rfcN.ignite` for 854, 855 and 1143 prints `unchecked`, exit 0, and each of the three `Spec` digests equals 19.9's | U4 |
| 27 | the prose catalogues | `cargo test -p ignite --test main rfc::prose_catalogues_quote_their_rfcs` | `rfc854.ignite` holds `RP14_1` with `source = Prose` whose `text` contains "only the IAC need be doubled to be sent as data". `rfc855.ignite` equals the table of 19.4 (ids, levels, parties, texts). `rfc1143.ignite` holds at least one `source = Prose` requirement. Each requirement is listed in its module's `Spec`, and every level follows the word table of 19.4 | U4 |
| 28 | the RFC 9112 extent | `cargo test -p ignite --test main rfc::rfc9112_holds_sections_2_to_9` | `rfc9112.ignite` holds requirements from sections 2 to 9 only, at least one from each. It holds `R7_1_2` and `R3_2_6`, both `level = Must`, with the texts quoted verbatim in row 2 | U4 |
| 29 | the RFC 9110 server subset | `cargo test -p ignite --test main rfc::rfc9110_holds_exactly_the_server_subset` | every requirement's section is one of D4's list or below it, and every listed section with a BCP 14 mark has at least one requirement | U4 |
| 30 | the hand review changes only parties | `cargo test -p ignite --test main rfc::reimport_changes_only_party_lines` | from the repository root, re-running `ignite rfc import` with the source path and flags recorded in each imported catalogue's header (paths relative to the repository root) and diffing against the committed file changes only `party =` lines. The commit that adds a catalogue states how many it changed | U4 |
| 31 | one row per verdict | `cargo test -p ignite --test main conformance::each_verdict_prints_its_row` | one fixture each under `crates/ignite/tests/fixtures/conformance/` for pass, fail, gap, deviation, inconclusive, n/a, out of scope and unclassified. Each prints its row as its `.expected` says | U5 |
| 32 | a failing test | `conformance::a_failing_verifies_test_fails_the_row_and_the_claim` | the row prints `fail`, the claim line `no (0 gaps, 1 failures, 0 inconclusive)`, exit 1 | U5 |
| 33 | satisfied without a passing test | `conformance::satisfied_without_a_passing_test_is_inconclusive`; `cargo test -p ignite --lib conformance::` | the fixture row with no test prints `inconclusive`, evidence `no test`. A unit test in `crates/ignite/src/conformance.rs` gives the verdict function a row with two verifying tests, one passed and one absent from the result lines, and gets `inconclusive` with evidence `… passed, test "<name>" not run` | U5 |
| 34 | a claim that holds | `conformance::a_complete_claim_holds` | every MUST row `pass`; claim line `yes (0 gaps, 0 failures, 0 inconclusive)`; exit 0 | U5 |
| 35 | an inconclusive MUST | `conformance::an_inconclusive_must_breaks_the_claim` | the only non-pass MUST row is `inconclusive`; claim line `no (0 gaps, 0 failures, 1 inconclusive)`; exit 1 | U5 |
| 36 | determinism and targets | `conformance::output_is_deterministic`, `conformance::both_targets_print_the_same_matrix` | two runs print byte-identical stdout; `--target rust` and `--target llvm` print byte-identical stdout | U5 |
| 37 | exit statuses | `conformance::conformance_exit_statuses` | a file with a diagnostic: exit 2, the diagnostic on stderr, no stdout. A file with no conforming protocol: exit 2. A test build that fails: exit 3. A test executable killed by a signal: exit 3 | U5 |
| 38 | the workspace | `cargo test --workspace` after each unit | passes. No golden of `crates/ignite-rustgen/tests/golden/` changes in U1 to U5. In the `.ll` goldens of `crates/ignite-llvm/tests/golden/`, U1's dropping of unreferenced constants (19.13) may remove the definitions, strings and debug nodes of unreferenced constants and renumber the remaining metadata; no other golden change | every unit |
| 39 | unreferenced constants leave no trace on LLVM | `cargo test -p ignite-llvm --test main` (U1 names the test) | a std module with an unreferenced `const DIGEST: Text = "sha256:probe-unreferenced-constant-text"` and a referenced constant: `std.ll` holds the referenced constant's definition and string, and neither the probe text nor a `DISubprogram` named after `DIGEST`. `apply::plan::CODEGEN_REVISION` is past the value the shadowing fix sets | U1 |
| 40 | the claim-fixture std stays the real vocabulary | `cargo test -p ignite --test main plan::claims_std_vocabulary_is_std_rfc` | `crates/ignite/tests/fixtures/claims_std/rfc.ignite` is byte-identical to `std/rfc.ignite` | U2 |
| 41 | a complete claim is silent | `cargo test -p ignite --test main plan::complete_claim_fixture_is_silent` | `ignite check claim_ok_complete.ignite` in `crates/ignite/tests/fixtures/` (the protocol of row 14 with a status on each requirement): exit 0, silent | U2 |

### 19.1 Scope and non-goals

In scope: the vocabulary, five catalogues, the six clauses, literal-data rules, the claim
model with E090 to E092, the importer and the digest check, the conformance command, the two
std claims, and the records.

Not in scope:

- `grammar` blocks in catalogues and everything else of P3 (§3.4 to §3.6 of `protocols.md`,
  E093 to E095). The P2 catalogues hold requirements only.
- Requirement aliases (`const BARE_CR: Requirement = R2_2_3`, §3.1). In P2 an alias is E091
  (19.4).
- A `minimum` in an imported catalogue. The importer never writes one, and P2 adds none by
  hand, so row 30 holds. P3's limits work adds them.
- Claims of RFC 1143. Its catalogue lands; the Telnet Q method that would satisfy it is P5.
- Implementing the HTTP §7.1 chunked coding or the §3.2 Host check (P3). They stay gaps (row 2).
- Fetching RFCs over the network. The importer reads a local file (D6).
- A skip verdict in the test runner. A `verifies` test with no result line counts as not run
  (19.10).
- ESS traces and mutation audits of `verifies` tests (`protocols.md` §8, Q7 and Q4).
- Splitting `std/protocol/telnet.ignite` to meet X16's bound (19.14 item 14, 19.18 Q4).
- RFC 9110 §8.6 and §15 (19.14 item 13), and checking a `minimum` against a system's
  overriding limit (19.14 item 11).

### 19.2 Current state (at `296f849`)

| fact | where |
|---|---|
| A protocol body holds roles only; the grammar is `protocol := 'protocol' ident '{' NL* (fn NL*)* '}'` | section 3; `ProtocolDecl { name, fns, span }` in `crates/ignite-syntax/src/ast.rs` |
| A test is `'test' STRING block`; the AST holds name, body and span | section 3 and 12.1; `TestDecl` in `ast.rs` |
| A fn has `requires` and then a block or `= expr`; a record `type Name tparams? { fields }`; a field `name: Type (= default)?` | section 3; `FnDecl`, `RecordDecl`, `Field` in `ast.rs` |
| Full struct literals of `FnDecl`, `RecordDecl`, `Field` and `ProtocolDecl` are in the parser, in `crates/ignite-rustgen/tests/units.rs` and in the `#[cfg(test)]` module of `crates/ignite-check/src/lib.rs`, so a new AST field breaks them; `crates/ignite-types/tests/surface.rs` only reads them | `parser.rs` in `crates/ignite-syntax/src/` (grep `Ok(FnDecl {`, `Ok(ProtocolDecl {`, `Decl::Record(RecordDecl {`); `units.rs` (grep `FnDecl {`, `RecordDecl {`, `ProtocolDecl {`); `crates/ignite-check/src/lib.rs` (grep `Decl::Fn(FnDecl {`) |
| No std or example file uses `conforms`, `scope`, `verifies`, `unsupported`, `not_applicable`, `parties` or `sections` as a name. `std.sli` declares a fn `satisfies`, so the clause words must stay contextual | `grep -rnw` over `std/` and `examples/`; `satisfies` in `std/sli.ignite` |
| A constant of any type compiles on both targets: Rust emits it as a fn, LLVM defines it through `Lower::define_const` | `constant` in `crates/ignite-rustgen/src/lib.rs`; the `Decl::Const` arm in `crates/ignite-llvm/src/lib.rs`, `Lower::define_const` in `crates/ignite-llvm/src/lower.rs` |
| Std emission is reachable-only for functions on both targets. Rust keeps a std item when a reachable item's text names it (`reachable`, over name tokens; std `use` lines are filtered, but a program's own `use` lines are roots, so a program that imports a constant emits it). LLVM lowers every `Decl::Const` through `Lower::define_const`, whose Text literals go to `Module::string`; `Module::prune` drops unreachable functions only, and `Module::finish` writes every string and debug node. So on LLVM an unreferenced std constant's text and debug entry reach `std.ll` | `reachable` in `crates/ignite-rustgen/src/lib.rs`; `lower_decls`, `Module::prune`, `Module::finish`, `Module::string` in `crates/ignite-llvm/src/`; `AGENTS.md`, slice 12b |
| Rust emission writes the source path as given and the line and column of each failure site into the generated code. `ignite emit` emits Rust only; `ignite::emit_llvm(file, std)` returns the LLVM output, and `ignite build --target llvm` writes `std.ll`, `program.ll` and `main_<executable>.ll` | `escaped_site_file`, `position` in `crates/ignite-rustgen/src/lib.rs`; `emit`, `emit_llvm` in `crates/ignite/src/lib.rs`; `crates/ignite/src/llvm_build.rs` |
| The fixture std of each checker crate is pinned by an asserted module list: `["std.greeting", "std.protocol.hello", "std.salute"]` and `["std.net", "std.shapes", "std.shapes.protocol"]` | `crates/ignite-check/tests/fixtures.rs`, `crates/ignite-types/tests/fixtures.rs` |
| `CODEGEN_REVISION` is `ignite-codegen/0.1.0-2` at `296f849`; the slice branch's shadowing unit sets `0.1.0-3` | `crates/ignite/src/apply/plan.rs`; 19.16 |
| The frontend loads only the std modules a program reaches | `AGENTS.md`, slice 12b; `PLAN.md` row 12b |
| The evaluator evaluates constants | `find_const` in `crates/ignite-eval/src/eval.rs` |
| `Model::check` returns `Ok` before any work when the program has no systems and no assemblies | `Model::check` in `crates/ignite-eval/src/model.rs` |
| `ignite check` runs `Model::check` through `Loaded::check` | `check_with_std`, `Loaded::check` in `crates/ignite/src/lib.rs` |
| `ignite_eval::Module` holds path, source, program and types, and no `Symbols` | `Module` in `crates/ignite-eval/src/lib.rs` |
| A test verdict is `Passed`, `Failed`, `Exited`, `Signal` or `TimedOut`; there is no skip. A result line is `test <module>/<name> ... ok`, `... FAILED` or `... FAILED (<reason>)` | `Verdict`, `Verdict::word` in `crates/ignite-io/src/test.rs` |
| `test_build` builds one test executable per target for the files given; `lib.rs` re-exports `TestGroup`, `TestOptions`, `build_tests`, `test`, `test_groups` and `test_targets` as `pub` | `crates/ignite/src/test_build.rs`; the `pub use test_build::…` line in `crates/ignite/src/lib.rs` |
| The test runner prints `running N tests`, one result line per test, a `failures:` section that echoes each failed test's output, then `test result:`; its exit is 0 or 1 | `crates/ignite-io/src/test.rs` |
| `Error::exit_status` maps a diagnostic to 1, a cargo or clang failure to 2, IO to 3; `ignite run` maps its own statuses in `main.rs` | `Error::exit_status` in `crates/ignite/src/lib.rs`; `crates/ignite/src/main.rs` |
| `ignite check` takes `--std DIR` | `cli::missing_std_root_is_exit_3` in `crates/ignite/tests/cli.rs` |
| `plan::fixtures_match_expected` runs `crates/ignite/tests/fixtures/e11*` and `e12*` except `e125` to `e128` | `fixtures_match_expected` in `crates/ignite/tests/plan.rs` |
| `every_checker_code_has_a_fixture` lists the checker's codes | `crates/ignite-check/tests/fixtures.rs` |
| The layering test knows four layers, a protocol prefix `std.protocol.` and a table of non-protocol modules. Rule 5 fails any line under `crates/*/src` whose lowercase text contains a protocol name (`telnet`, `http`), so it also fails `https` | `Layer`, `LAYERS`, `PROTOCOL_PREFIX`, `compiler_crates_name_no_application_protocol` in `crates/ignite-check/tests/layering.rs`; `docs/layering.md` rules 1 to 5 |
| `sha2` is a normal dependency of `crates/ignite`; no XML crate is in `Cargo.lock` | `crates/ignite/Cargo.toml`; `Cargo.lock` |
| Telnet answers `DO` with `WONT` and `WILL` with `DONT`, ignores `DONT` and `WONT`, and skips a subnegotiation up to `IAC SE` | `decode_within` in `std/protocol/telnet.ignite`; tests "DO is answered WONT and WILL is answered DONT", "a subnegotiation is skipped up to IAC SE" |
| The HTTP server answers any `Transfer-Encoding` with 501 and does not check Host | the module comment and `parse_head` in `std/protocol/http.ignite` |
| Limits are quantities: `HttpLimits.head: ByteSize = 8 KiB`, `SessionLimits.maximum: Int = 1000` | `HttpLimits` in `std/protocol/http.ignite`, `SessionLimits` in `std/protocol/telnet.ignite` |
| Enum variants are written bare in std (`Exponential(initial, maximum)`); no std enum has a variant named like one of 19.3 | `std/policy.ignite`; grep of `std/` |
| Codes E096 and E099 are taken (slice 14). `protocols.md` §3.3 assigns E093 to E095 and E098 to P3 and P4, and keeps E097 unused | section 8; `docs/protocols.md` §3.3 |
| RFC 9112's RFCXML has 149 `<bcp14>` marks: 11 in §1.1 (notation), 137 in §2 to §9, 1 in §10.1. RFC 9110 has 445; the D4 subset has 75 | count of `<bcp14>` per innermost `<section pn>` over the files of 19.9 |

### 19.3 The vocabulary, `std.rfc` (D1)

File `std/rfc.ignite`, module `std.rfc` (the rule "module `std.a.b` is `std/a/b.ignite`").
The catalogues are `std/rfc/rfcN.ignite`, modules `std.rfc.rfcN`. The text, which U1 writes
and the importer targets:

```ignite
package std.rfc

// The requirement keywords of BCP 14 (RFC 2119, RFC 8174).
enum Level { Must, MustNot, Required, Shall, ShallNot, Should, ShouldNot, Recommended,
  NotRecommended, May, Optional }

// Whom a requirement addresses.
enum Party { Any, Sender, Recipient, Client, Server, Proxy }

// Bcp14: extracted from a <bcp14> mark; Prose: written by hand from an RFC that predates BCP 14.
enum Source { Bcp14, Prose }

type Requirement {
  rfc: Int
  section: Text
  ordinal: Int
  level: Level
  party: Party = Any
  source: Source = Bcp14
  minimum: Option<Int> = None
  text: Text
}

type Spec {
  rfc: Int
  title: Text
  url: Text
  digest: Text
  requirements: List<Requirement>
}
```

Level classes, used by 19.7 and 19.10:

| class | levels |
|---|---|
| MUST | `Must`, `MustNot`, `Required`, `Shall`, `ShallNot` |
| SHOULD | `Should`, `ShouldNot`, `Recommended`, `NotRecommended` |
| MAY | `May`, `Optional` |

The module declares no fn. Tests of the module, if any, are inline.

### 19.4 Catalogues, ids and literal data

A catalogue module is a module that declares a constant of type `std.rfc.Spec`. It holds:

- exactly one `Spec` constant, named `RFC<n>` (`RFC9112`);
- one `Requirement` constant per requirement, each listed exactly once, in declaration order, in
  the `Spec`'s `requirements`;
- an `import std.rfc.{Level, Party, Source, Requirement, Spec}` (unused names allowed).

A module under `std/rfc/` holds nothing else (row 25 checks the committed ones). A program
module may also declare one `Spec` and its requirements beside other declarations: the claim
fixtures of rows 12 and 14 to 17 do, and the rules below apply to it as to a catalogue
module.

**Ids.** A requirement's constant name is its id: `R` + its `section` with each `.` replaced by
`_` and letters upper-cased + `_` + its `ordinal`. The ordinal is 1-based within the
requirement's innermost section, in document order. Section 3.2's sixth requirement is
`R3_2_6`.

- Numbered sections (RFC 1143, RFC 9110, RFC 9112): `section` is the section number.
- RFC 854 and RFC 855 number no sections. `section` is `p<page>`, the page of the plain-text
  RFC on which the sentence starts (`p14`), and the id is `RP14_1`.

**Literal data.** The value of every constant whose declared type is `std.rfc.Requirement` or
`std.rfc.Spec`, in any module, is literal data. Literal data is:

- an Int literal, also with a leading `-`;
- a Text literal without interpolation, or a `+` chain of such literals;
- a variant of `Level`, `Party` or `Source`, bare or qualified;
- `None`, or `Some(` Int literal `)`;
- a `Requirement { .. }` literal whose field values are literal data;
- a `Spec { .. }` literal whose fields are literal data and whose `requirements` is a list
  literal of names of `Requirement` constants of the same module.

Anything else is E091 (first trigger, `ignite-check`): a call, a name of any other constant
(an alias), an interpolated text, a list built with `+`, `if`, `match` or a block. The
message names the constant and what was found: "catalogue constant `R2_1_1` must be literal
data, found a call".

E091 (second trigger, `ignite-eval`), from the constants' values, for the `Spec` and
`Requirement` constants declared in the checked file only (a catalogue imported by the
checked file is checked when it is itself the checked file, row 25). One E091 per constant,
the first of these that applies:

1. an `ordinal` under 1;
2. a constant name that differs from the id of its `section` and `ordinal`;
3. a `Requirement` constant not listed in the `Spec`, or listed twice;
4. a requirement whose `rfc` differs from the `Spec`'s.

Two `Spec` constants in one module are one E091 at the second. Two requirements with one
`section` and `ordinal` need no rule of their own: if both have the same name, the second is
E004 (a duplicate name, `ignite-check`); otherwise one name differs from the id and is case 2.
The checker cannot see these: they need the values.

**Level words.** A BCP 14 mark gives its keyword's level. A prose requirement (`source =
Prose`) takes the level of its sentence's strongest word, MUST class over SHOULD class over
MAY class (19.3):

| word in the sentence | level |
|---|---|
| must | `Must` |
| must not, must never | `MustNot` |
| required, will be required | `Required` |
| necessary, need be | `Must` |
| should, desirable | `Should` |
| should not | `ShouldNot` |
| may | `May` |

A sentence that describes the RFC's own standing or what hosts are able to do is no
requirement. Excluded from the RFC 854 and RFC 855 catalogues: "Hosts on the ARPA Internet are
expected to adopt and implement this standard." and "It should be possible for hosts to
invent, test, or discard options at will."

**The RFC 855 catalogue**, fixed here because rows 3 and 27 name its ids. One requirement
per sentence that says what an implementation, an option designer or the option registrar
must, should or may do, all `source = Prose`, texts verbatim with whitespace collapsed:

| id | level | party | text |
|---|---|---|---|
| `RP1_1` | `Should` | `Any` | "Nevertheless, it is envisioned that options which prove to be generally useful will eventually be supported by many hosts; therefore it is desirable that options should be carefully documented and well publicized." |
| `RP1_2` | `Must` | `Any` | "In addition, it is necessary to insure that a single option code is not used for several different options." |
| `RP1_3` | `Required` | `Any` | "The individual responsible for assignment of option codes may waive the requirement for complete documentation for some cases of experimentation, but in general documentation will be required prior to code assignment." |
| `RP1_4` | `May` | `Any` | "Options will be publicized by publishing their documentation as RFCs; inventors of options may, of course, publicize them in other ways as well." |
| `RP1_5` | `Should` | `Any` | "Documentation of options should contain at least the following sections:" |
| `RP1_6` | `Should` | `Any` | "The meaning of each possible TELNET command relevant to this option should be described." |
| `RP2_1` | `Must` | `Any` | "The default assumptions for hosts which do not implement, or use, the option must be described." |
| `RP2_2` | `Should` | `Any` | "Therefore, a more complete description should be furnished in most cases." |
| `RP2_3` | `May` | `Recipient` | "On the other hand, the receiver may locate the end of a parameter string by searching for the SE command (i.e., the string IAC SE), even if the receiver is unable to parse the parameters." |
| `RP2_4` | `May` | `Any` | "Of course, either party may refuse to pursue further subnegotiation at any time by sending a WON'T or DON'T to the other party." |
| `RP3_1` | `Must` | `Any` | "Designers of options requiring \"subnegotiation\" must take great care to avoid unending loops in the subnegotiation process." |
| `RP3_2` | `Must` | `Sender` | "Finally, if parameters in an option \"subnegotiation\" include a byte with a value of 255, it is necessary to double this byte in accordance the general TELNET rules." |

The `Spec`: `rfc = 855`, `title = "Telnet Option Specifications"`,
`url = "https://www.rfc-editor.org/rfc/rfc855"`, `digest` the sha256 of the published text
file of 19.9 (the file is not vendored, D5).
RFC 854 and RFC 1143 follow the same rule; their ids are U4's, except `RFC854.RP14_1`, the
sentence "With the current set-up, only the IAC need be doubled to be sent as data, and the
other 255 codes may be passed transparently." at `Must`, `Any` (rows 1 and 27). U4's commit
lists the MUST-class ids of RFC 854 it wrote and the status each gets in `telnet.ignite`.

### 19.5 Clauses (D3)

The grammar of section 3 changes in these productions. `conforms`, `scope`, `satisfies`,
`verifies`, `unsupported`, `not_applicable`, `parties` and `sections` are identifiers, read as
clause words only in the positions below.

```
protocol   := 'protocol' ident conforms? '{' NL* ((fn | status) NL*)* '}'
conforms   := 'conforms' '[' ident (',' ident)* ','? ']' scope?
scope      := 'scope' '{' (scope_item sep)* scope_item? '}'
scope_item := 'parties' '=' '[' (ident (',' ident)* ','?)? ']'
            | 'sections' '=' '[' (STRING (',' STRING)* ','?)? ']'
status     := ('unsupported' | 'not_applicable') reqref STRING
satisfies  := 'satisfies' '[' reqref (',' reqref)* ','? ']'
verifies   := 'verifies' '[' reqref (',' reqref)* ','? ']'
reqref     := ident '.' ident
fn         := 'fn' ident tparams? '(' params ')' ('->' type)? requires? satisfies? (block | '=' expr)
record     := 'type' ident tparams? satisfies? '{' (field sep)* field? '}'
field      := ident ':' type ('=' expr)? satisfies?
test       := 'test' STRING verifies? block
```

- `conforms` lists names of `Spec` constants, local or imported (`import std.rfc.rfc9112.{RFC9112}`); a qualified path is E001 (row 10).
- A `reqref` is `Spec.Id`: the first segment names a `Spec` constant in scope, the second a
  `Requirement` constant declared in that constant's module. The requirement need not be
  imported. Bare ids are not accepted, because every catalogue reuses ids such as `R2_1_1`.
- `scope` narrows the claim. A requirement is in scope when its `party` is in `parties` and its
  `section` equals an entry of `sections` or starts with that entry followed by `.`. A missing
  `parties` or `sections`, or a missing `scope`, admits everything. Each item at most once
  (E001 otherwise).
- A `satisfies` on a fn or role, a type or a field says that the declaration implements the
  requirements. It counts for every protocol of its module that conforms to the requirement's
  spec, inside or outside the protocol body.
- A `verifies` on a test says that the test is evidence for the requirements. It counts in the
  same way.
- `unsupported REF "reason"` and `not_applicable REF "reason"` are members of a protocol body.
  The reason is a non-empty Text literal without interpolation (E001 otherwise).
- A requirement may be satisfied by several declarations. That is one status, "satisfied".
- A `satisfies` on a field of a record: the field's default is the configured value of 19.7.
- Malformed clauses are E001 (row 10).

Examples, as U6 writes them:

```ignite
import std.rfc.{Party}
import std.rfc.rfc9112.{RFC9112}
import std.rfc.rfc9110.{RFC9110}

protocol Http conforms [RFC9112, RFC9110] scope { parties = [Server, Sender, Recipient, Any] } {
  fn serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S>) -> IoResult<Unit> requires [stream.read, stream.write, application.*] satisfies [RFC9112.R2_2_3] { .. }
  unsupported RFC9112.R7_1_2 "only Content-Length framing; any Transfer-Encoding is 501"
  unsupported RFC9112.R3_2_6 "Host is not checked"
}

test "bare CR is 400" verifies [RFC9112.R2_2_3] { .. }
```

The ids in this example other than `R7_1_2` and `R3_2_6` are illustrative.

### 19.6 Resolution and typing

| rule | crate | diagnostic |
|---|---|---|
| A `conforms` entry or the first segment of a `reqref` that names nothing | `ignite-check` | E003 |
| The second segment of a `reqref` that names no constant of the spec's module | `ignite-check` | E003 "`RFC9112` has no requirement `R9_9_9`" |
| A `conforms` entry that is not a constant of type `Spec`; a `reqref` whose first segment is not one; a `reqref` whose second segment is not a `Requirement` | `ignite-types` | E040 |
| A `parties` entry that is no variant of `Party` | `ignite-types` | E040 |
| A `conforms` that names one spec twice | `ignite-check` | E004 |
| A non-literal catalogue constant | `ignite-check` | E091 (first trigger) |

`ignite-check` resolves the names for E003 and E004 only. The type checker records the
resolution in `TypedProgram`: for every `conforms` entry and every `reqref`, its span and the
`QualName` of the constant it names, read through an add-only accessor (19.13), as
`pattern_consts` already records constant patterns. `ignite_eval::Module` holds the module's
`TypedProgram` (`types`), so the claim model reads the resolutions from there, and neither
`ignite_eval::Module` nor `Loaded::modules` changes.

### 19.7 The claim model and E090 to E092

The claim model is computed by `ignite-eval` (U2) from the typed program of the checked file
and the values of the catalogue constants its protocols name. It covers the protocols of the
checked file only. A program that imports `std.protocol.http` does not re-check `Http`'s claim:
row 4 does that.

`Model::check` computes it first, before its early return for a program without systems and
assemblies, and reports its diagnostics with the model's. No change in `crates/ignite/src/`
is needed for `ignite check` (`Loaded::check` already calls `Model::check`).

For each protocol P that `conforms`, and each spec S in its list, one row per requirement of S,
in `requirements` order:

- **in scope**: by 19.5's `scope` rule;
- **status**: `satisfied` (by one or more declarations), `unsupported` (reason),
  `not_applicable` (reason), or none;
- **satisfiers**: the declarations, named `f` (fn), `Owner.f` (associated fn), `P.role`,
  `Name` (type) or `Record.field`;
- **tests**: the full names (`<module>/<name>`, 12.7) of the tests that verify it;
- **limit**: for a satisfier that is a field and a requirement with `minimum = Some(m)`, the
  configured value, which is the field's default as an Int (an `Int` value, or a `ByteSize` in
  bytes), and whether it is at least `m`. A field with no default, or of another type, has no
  configured value.

Diagnostics:

| code | rule | crate, story |
|---|---|---|
| E090 | A requirement in scope of P, at MUST class, with no status. Message: "requirement `RFC9112.R7_1_2` (MUST, section 7.1) is in scope of `Http` and has no status; add `satisfies`, `unsupported` or `not_applicable`". Position: the spec's entry in P's `conforms`. Several at one position are ordered by `requirements` order | `ignite-eval`, claim-model (U2) |
| E090 | A requirement of P, at any level, with two statuses: satisfied and `unsupported`, satisfied and `not_applicable`, `unsupported` and `not_applicable`, or one status kind twice. Message: "requirement `RFC9112.R3_2_6` has two statuses in `Http`: satisfied by `parse_head` and unsupported". Position: the later clause | `ignite-eval`, U2 |
| E091 | First trigger: 19.4, literal data | `ignite-check`, frontend (U1) |
| E091 | Second trigger: 19.4, the four cases with their precedence, and two `Spec` constants | `ignite-eval`, U2 |
| E092 | A `satisfies` or `verifies` naming a requirement of a spec that no protocol of its module conforms to; a status in P naming a requirement of a spec P does not conform to. Message: "`RFC9110.R9_3_1_1` belongs to RFC 9110, which `Http` does not conform to" or "…, which no protocol of this module conforms to". Position: the `reqref` | `ignite-eval`, U2 |

A status on a requirement out of scope is allowed; its row's verdict is `out of scope`. A
SHOULD or MAY with no status is not an error; its row is `unclassified`.

The second-trigger E091 checks run for the `Spec` and `Requirement` constants declared in the
checked file only, not for a catalogue the file imports, so a broken std catalogue never
surfaces in a program that conforms to it. `ignite check std/rfc/rfc9112.ignite` validates
the catalogue itself; row 25 runs it for every committed catalogue.

### 19.8 `ignite rfc import` and `ignite rfc check` (D6)

```
ignite rfc import FILE [--sections LIST] [--package NAME] [--url URL] [--out FILE]
ignite rfc check CATALOGUE [SOURCE]
```

`import` reads a local RFCXML v3 file (prepped, as the RFC Editor publishes it). It never opens
a network connection. Exit 0 when the module is written; 2 on a usage error, malformed XML or
a root without a `number` attribute; 3 when FILE cannot be read or `--out` cannot be written.

**Reader.** `crates/ignite/src/rfc/xml.rs`, hand-written, no dependency. It reads:

- elements, attributes in single or double quotes, and character data;
- CDATA sections, comments, processing instructions and the XML declaration, which it skips;
- a DOCTYPE without an internal subset, which it skips;
- the five predefined entities and numeric character references.

Any other entity, a DOCTYPE with an internal subset, a mismatched or unclosed element, or a
file over 16 MiB is an error. The error is `<file>:<line>:<col>: <message>` on stderr, exit 2.
There is no validation and no namespace processing. Prefixed names are kept as written.

**Extraction**, in document order:

1. The section of a node is the `pn` attribute of its innermost `<section>`, without the
   `section-` prefix. Front and back matter (`<front>`, `<back>`) are skipped.
2. A text block is a `<t>`, or an `<li>`, `<dd>`, `<td>` or `<th>` with no `<t>` child.
3. A block's text is its character data with the markup removed, plus these renderings:
   - `<bcp14>`, `<em>`, `<strong>`, `<tt>`, `<sub>`, `<sup>` and an `<xref>` or `<eref>` with
     content: the content;
   - an empty `<xref>` with a `section` attribute: `Section <section> of [<derivedContent>]`,
     or `[<derivedContent>], Section <section>` when `sectionFormat="comma"`;
   - an empty `<xref>` without one: `[<derivedContent>]` when its target is a `<reference>`
     anchor, else `derivedContent`;
   - an empty `<eref>`: its `target`;
   - `<iref>`: nothing.

   Runs of whitespace become one space, and the text is trimmed.
4. Sentences end at `.`, `?` or `!`, optionally followed by `)` or `"`, then a space and an
   upper-case ASCII letter, `"` or `(`.
5. A sentence that contains at least one `<bcp14>` mark is one requirement. Its level is the
   first mark's keyword (`MUST NOT` gives `MustNot`, `NOT RECOMMENDED` gives `NotRecommended`).
   Keywords outside `<bcp14>` never count, so a lowercase "must" yields nothing (RFC 8174).
6. Ordinals count requirements per section, over the whole file. `--sections` filters after
   counting, so ids do not depend on the filter.
7. The party is the last party word before the first mark, case-insensitive, whole words:
   `server`, `servers`, `origin server` give `Server`; `client`, `clients`, `user agent`,
   `user agents` give `Client`; `sender`, `senders` give `Sender`; `recipient`, `recipients`,
   `receiver` give `Recipient`; `proxy`, `proxies`, `intermediary`, `intermediaries`,
   `gateway`, `cache`, `caches` give `Proxy`. With none, `Any`. A human reviews every party
   (row 30).

**Flags.**

- `--sections` is a comma-separated list of section numbers. A requirement is kept when its
  section equals one or starts with one followed by `.`. Without the flag, every section is
  kept.
- `--package` defaults to `std.rfc.rfc<number>`, from the root's `number` attribute. A given
  `--package` must be a module path (dotted identifiers); anything else is a usage error.
- `--url` defaults to the `href` of the root's first `<link rel="alternate">`.
- Without `--out`, the module goes to stdout.

**Output.** One file, in this order:

1. A header comment naming the RFC number, the title, the source path as given, and the exact
   flags. Row 30 re-runs the import from it.
2. `package`, then `import std.rfc.{Level, Party, Source, Requirement, Spec}`.
3. The `Spec` constant: `rfc`, `title` from the root `<front><title>`, `url`, `digest`, and
   `requirements` with one name per line.
4. One `Requirement` constant per requirement. Each field is on its own line, in the order
   `rfc`, `section`, `ordinal`, `level`, `party`, `text`. `party` is written even when it is
   `Any`; `source` and `minimum` are never written.

Texts escape `\` as `\\`, `"` as `\"`, `{` as `{{` and `}` as `}}`. Output is deterministic:
no time, no absolute path beyond what was given.

**Digest.** `"sha256:" + lowercase hex` of the source file's bytes, with the `sha2` crate
`crates/ignite` already depends on.

**`rfc check`** parses CATALOGUE with `ignite-syntax`, finds the single `Spec` constant's
`digest` Text literal, and compares it with the digest of SOURCE's bytes. It works for any
source file, a `.txt` too.

- Without SOURCE: one line `<CATALOGUE>: unchecked` on stdout, exit 0. This is how the gate
  reports the prose catalogues, whose sources are not vendored (D5).
- Exit 0, silent: the digests match.
- Exit 1: they differ. One line on stderr names CATALOGUE and both digests.
- Exit 2: CATALOGUE has no single `Spec` constant with a literal `digest`.
- Exit 3: a file cannot be read.

**Layering rule 5.** No file under `crates/*/src` may contain a protocol name, so the importer's
source holds no URL literal (`https` contains `http`) and no RFC title. URLs and titles come
from the source file or from flags.

### 19.9 Sources and licence (D5)

Only the two RFCXML sources are vendored, unmodified, under `docs/rfc/` by U4. They are
post-2008 RFCs under the IETF Trust Legal Provisions. RFC 854, 855 and 1143 predate them and
are not vendored: their prose catalogues quote the sentences they need with page citations,
and their `Spec` digest is the sha256 of the published text file as read on the import day,
recorded in the catalogue and here, and reported `unchecked` by `ignite rfc check` without
SOURCE (row 26; no network in the gate).

| source | vendored as | from | bytes | sha256 |
|---|---|---|---|---|
| RFC 854 text | not vendored | `https://www.rfc-editor.org/rfc/rfc854.txt` | 38,517 | `f9cb6d4a7d7a67e5a1ac5ae3c2962b1183fb26119632a1617f3c8379820dfb10` |
| RFC 855 text | not vendored | `https://www.rfc-editor.org/rfc/rfc855.txt` | 6,049 | `c8c4744c9fa7650c0ae78d140f560b26e26b2935391e1ff624476c5fdbf4e03a` |
| RFC 1143 text | not vendored | `https://www.rfc-editor.org/rfc/rfc1143.txt` | 22,771 | `4d2ef5878b0b88742c860fc777d6d91ad61580633fe277eae06410a6eee04bdf` |
| RFC 9110 RFCXML | `docs/rfc/rfc9110.xml` | `https://www.rfc-editor.org/rfc/rfc9110.xml` | 1,223,686 | `09ec9ad4d1664623adcb298577e3fb9cd5c285f1bbe6b24d8c715b29f218e721` |
| RFC 9112 RFCXML | `docs/rfc/rfc9112.xml` | `https://www.rfc-editor.org/rfc/rfc9112.xml` | 262,821 | `ce82219c272374005115c909244a79e587687df3858edca4112b8563c02ccf19` |

The sizes and digests are those of the downloads of 2026-11-03. If U4's download differs, U4
records the new values in its commit and in this table.

`docs/rfc/README.md` (U4) says that the two files are RFCs published by the RFC Editor,
unmodified, subject to the IETF Trust Legal Provisions (`https://trustee.ietf.org/license-info`),
that they are vendored so that `ignite rfc check` runs offline in the gate, and that the three
older RFCs are cited, not vendored.

### 19.10 `ignite conformance` (D8)

```
ignite conformance FILE [--target rust|llvm] [the build flags of `ignite test`]
```

1. Load and check FILE as `ignite check` does. A diagnostic: print it as `ignite check` does,
   exit 2, no stdout.
2. Read the claim model (19.7). No protocol of FILE conforms: one line on stderr, exit 2.
3. If any row names a test, build FILE's test executable on the target (default `llvm`)
   through `ignite::build_tests`, run it once without arguments, and read its stdout. Result
   lines are read after the `running N tests` line, one entry per test of FILE in the order
   the runner runs them (declaration order, 12.7), each matched against that test's exact
   full name, which may hold `\n`; the first entry that does not match ends them, so a name
   cannot end the section early, and the `failures:` section, which echoes test output, is
   never parsed. An entry `test <module>/<name> ... ok` is a pass, `test <module>/<name> ...
   FAILED…` a failure. A test with more than one result keeps the worst (failed over not run
   over passed), so a forged line never raises a verdict. A named test with no entry is `not
   run`. Test output is not echoed.
   - A build failure is exit 3.
   - A run failure is exit 3: the executable cannot be spawned, ends by a signal, exits with a
     status other than 0 or 1, or prints no `test result:` line.
4. Print one block per (protocol, spec), in protocol then `conforms` order, with a blank line
   between blocks.

Block:

```
rfc<n> <title>, protocol <P>, parties <names|all>, sections <list|all>
<row>
...
claim conformance of <P> to rfc<n>: <yes|no> (<g> gaps, <f> failures, <i> inconclusive)
```

`<names>` is the `parties` list and `<list>` the `sections` list, each entry joined by one
space; `all` when the item is absent, `none` when its list is empty (`parties = []`). A row
has six columns: id, section, level, status,
evidence, verdict. Columns are joined by two spaces and padded with spaces to the widest
entry of their column within the block. The last column is not padded, and no line ends in a
space.

- **level**: `MUST`, `MUST NOT`, `REQUIRED`, `SHALL`, `SHALL NOT`, `SHOULD`, `SHOULD NOT`,
  `RECOMMENDED`, `NOT RECOMMENDED`, `MAY`, `OPTIONAL`.
- **status**: one of
  - `satisfied`;
  - `limit` (satisfied, with a limit satisfier);
  - `unsupported`;
  - `not_applicable`;
  - `none`.
- **evidence**:
  - satisfied: the satisfiers joined by `, `, then `; `, then each test as `test "<name>"
    passed|failed|not run` joined by `, `, or `no test`;
  - limit: `<Record.field> = <value>, minimum <m>`, or `<Record.field> has no value,
    minimum <m>`;
  - unsupported and not_applicable: the reason in double quotes;
  - none: `-`.
- **verdict**, the first rule that applies:
  1. not in scope: `out of scope`;
  2. `not_applicable`: `n/a`;
  3. `unsupported`: `gap` at MUST class, `deviation` at SHOULD class, `n/a` at MAY class;
  4. no status: `unclassified`;
  5. satisfied:
     - `fail` when any verifying test failed or a limit is below its minimum;
     - else `inconclusive` when any verifying test did not run, or a limit has no value
       (`protocols.md` §3.3: a skipped test makes the row inconclusive);
     - else `pass` when at least one verifying test exists and every one passed, or when there
       is no test, at least one limit, and every limit meets its minimum;
     - else `inconclusive` (no test and no limit).

The claim holds (`yes`) when no MUST-class row in scope is `fail`, `gap` or `inconclusive`.
The counts are over all rows in scope, at any level: `gaps` counts `gap`, `failures` counts
`fail`, and `inconclusive` counts `inconclusive`.

Exit statuses, mapped in `crates/ignite/src/main.rs` as `ignite run` maps its own:

| exit | when |
|---|---|
| 0 | every claim in the output holds |
| 1 | a claim does not hold |
| 2 | a diagnostic, a usage error, or no protocol of FILE conforms |
| 3 | a test build or run failure (step 3), or an IO error |

A `not run` test is what `protocols.md` §3.3 calls skipped. The runner at `296f849` has no skip
(19.2), and a run that dies is exit 3, so `not run` is a named test without a result line in a
run that ended normally. Row 33 covers it through the verdict function.

Illustrative, not a golden (ids other than `R7_1_2` and `R3_2_6` are made up):

```
rfc9112 HTTP/1.1, protocol Http, parties Server Sender Recipient Any, sections all
R2_2_3  2.2  MUST         satisfied    parse_head; test "bare CR is 400" passed                        pass
R3_2_6  3.2  MUST         unsupported  "Host is not checked"                                           gap
R6_1_6  6.1  SHOULD       satisfied    next_request; test "unknown coding is 501" not run              inconclusive
R7_1_2  7.1  MUST         unsupported  "only Content-Length framing; any Transfer-Encoding is 501"     gap
claim conformance of Http to rfc9112: no (2 gaps, 0 failures, 1 inconclusive)
```

`crates/ignite/src/conformance.rs` and the CLI dispatch hold no protocol name (rule 5).

### 19.11 Layering (D7)

A prefix rule, as for `std.protocol.*`:

- `std.rfc` and every `std.rfc.*` module are layer **leaf**. They import base and leaf only. A
  catalogue imports `std.rfc`, which is leaf.
- Protocol modules import every layer, so `std.protocol.telnet` and `std.protocol.http` may
  import the catalogues. General and base modules may import leaf modules by the existing
  rules, so nothing forbids them a catalogue; none imports one in this slice.
- `docs/layering.md`: the leaf row reads "`std.sli`, `std.time`, `std.policy` (O45), `std.rfc`
  and every `std.rfc.*`", and rule 4's text names the `std.rfc` prefix beside
  `std.protocol.`. `crates/ignite-check/tests/layering.rs` gets an `RFC_PREFIX` beside
  `PROTOCOL_PREFIX`, with the same effect in `layer_of` and in `StdModule::layer`. U1 makes
  these changes; U4 adds no layering row. U2 adds a paragraph to `docs/layering.md`, after the
  `crates/ignite-load` paragraph: `crates/ignite-eval/src` mirrors the std vocabulary `std.rfc`
  (`Level`, `Party`, `Source`), as it already names `std.sli.Window.Rolling` (the audit row of
  `crates/ignite-eval/src/validate.rs`). No protocol name is involved, so rule 5 holds.
- Rule 3 applies to the catalogues: no item, field or variant name contains a protocol name.
  The spec constants are `RFC854` and `RFC9112`, not names of the protocols. Titles and texts
  are Text values, not names.

### 19.12 Diagnostics E090, E091 and E092

| code | rule | phase | fixture |
|---|---|---|---|
| E090 | 19.7: a MUST-class requirement in scope with no status; a requirement with two statuses (separate message) | eval (`ignite-eval`), through `ignite check` and `ignite conformance` | `crates/ignite/tests/fixtures/e090_missing_status`, `e090_scope`, `e090_two_statuses`; the silent `claim_ok_complete` (row 41) |
| E091 | 19.4: first trigger, a catalogue constant that is not literal data | check (`ignite-check`) | `crates/ignite-check/tests/fixtures/e091_non_literal_catalogue` |
| E091 | 19.4: second trigger, an inconsistent catalogue in the checked file (the four cases, two `Spec` constants) | eval (`ignite-eval`) | `crates/ignite/tests/fixtures/e091_catalogue` |
| E092 | 19.7: a clause naming a requirement of a spec its protocol, or no protocol of its module, conforms to | eval (`ignite-eval`) | `crates/ignite/tests/fixtures/e092_satisfies_foreign`, `e092_verifies_foreign`, `e092_status_foreign`, run with `--std claims_std` |

No other new code (D2, D3). E093 to E095 and E098 stay with P3 and P4, and E097 stays unused.
`plan::fixtures_match_expected`'s filter gains `e090`, `e091` and `e092` (U2); a fixture
whose name starts with `claim_ok` is not in the filter and is checked by row 41.

**The claim-fixture std.** E092 needs a second spec the protocol does not conform to, so its
fixtures run against `crates/ignite/tests/fixtures/claims_std/`: a byte copy of
`std/rfc.ignite` as `rfc.ignite` (row 40 keeps it equal), and two small catalogues
`rfc/rfc1.ignite` and `rfc/rfc2.ignite` (modules `std.rfc.rfc1`, `std.rfc.rfc2`). The fixture's
first line `// ignite check --std claims_std` makes `plan::fixtures_match_expected` pass the
directory. The other claim fixtures declare their catalogue in the fixture itself (19.4) and
use the real std.

### 19.13 Typed API (add-only)

Names may change in review. Shapes are what implementors build against. Nothing existing is
renamed.

`ignite-syntax` (U1):

- `ProtocolDecl` gains `conforms: Vec<Ident>`, `scope: Option<Scope>` and
  `statuses: Vec<StatusDecl>`.
- `pub struct Scope { pub parties: Option<Vec<Ident>>, pub sections: Option<Vec<(String, Span)>>, pub span: Span }`.
- `pub struct StatusDecl { pub kind: StatusKind, pub requirement: Path, pub reason: String, pub span: Span }`,
  with `pub enum StatusKind { Unsupported, NotApplicable }`.
- `FnDecl`, `RecordDecl` and `Field` gain `satisfies: Vec<Path>`.
- `TestDecl` gains `verifies: Vec<Path>`.
- A `reqref` is a `Path` of two segments.

`ignite-check` (U1): resolution of the clause names for E003 and E004, and E091's first
trigger. `Symbols` does not change.

`ignite-types` (U1): `TypedProgram` records the `QualName` of the constant every `conforms`
entry and every `reqref` names, keyed by the reference's span
(`std.rfc.rfc9112.RFC9112`, `std.rfc.rfc9112.R7_1_2`), with an accessor
`pub fn clause_constant(&self, span: Span) -> Option<&QualName>`.

`ignite-llvm` (U1, as built): unreferenced constants are dropped after lowering. `lower_decls`
still lowers every `Decl::Const` in declaration order and marks its symbol
(`Module::mark_const`). `Module::finish` calls `prune`, which keeps a constant only when a kept
function, a global, a root or a symbol named by another module of the build
(`Module::add_const_roots`) references it, in a module with roots (std) and without (the
program). A dropped constant takes with it its definition, every `@.str` no other function or
global names, its `DISubprogram` and its `DILocation`s; the metadata ids of dropped nodes are
left as gaps, not renumbered. `generate_with` and `generate_tests` build the mains before the
program module and pass the symbols the mains name as constant roots. An unreferenced constant
therefore adds no definition, no `@.str` and no `DISubprogram` (row 39), and the string
numbering and function order of every kept item stay as before, so no golden changes. Known
gap: glue, instances and closures lowered for a dropped constant stay in the module (the
ignored test `golden::adversary_an_unreferenced_constant_leaves_no_closure_behind`); a
worklist that lowers constants on first reference would close it at the cost of renumbering
strings.
The Rust target needs no change: `reachable` already drops unreferenced std constants
(19.2). `apply::plan::CODEGEN_REVISION` moves one step past the value of the shadowing fix
(`ignite-codegen/0.1.0-4` if that fix sets `0.1.0-3`), as `AGENTS.md` requires for a backend
generation change.

`ignite-eval` (U2):

```rust
pub struct Claims { pub protocols: Vec<ProtocolClaim> }
impl Claims {
    pub fn of(evaluator: &mut Evaluator<'_>, module: Module<'_>) -> Result<Claims, Vec<Diagnostic>>;
}
pub struct ProtocolClaim { pub protocol: String, pub specs: Vec<SpecClaim> }
pub struct SpecClaim {
    pub constant: String, pub rfc: i64, pub title: String, pub url: String, pub digest: String,
    pub parties: Option<Vec<Party>>, pub sections: Option<Vec<String>>, pub rows: Vec<Row>,
}
pub struct Row {
    pub id: String, pub section: String, pub ordinal: i64, pub level: Level, pub party: Party,
    pub source: Source, pub minimum: Option<i64>, pub text: String,
    pub in_scope: bool, pub status: Status, pub tests: Vec<String>,
}
pub enum Status { None, Satisfied(Vec<Satisfier>), Unsupported(String), NotApplicable(String) }
pub struct Satisfier { pub declaration: String, pub limit: Option<Limit> }
pub struct Limit { pub configured: Option<i64>, pub minimum: i64, pub meets: Option<bool> }
pub enum Level { Must, MustNot, Required, Shall, ShallNot, Should, ShouldNot, Recommended, NotRecommended, May, Optional }
pub enum LevelClass { Must, Should, May }
impl Level { pub fn class(self) -> LevelClass; pub fn keyword(self) -> &'static str; }
pub enum Party { Any, Sender, Recipient, Client, Server, Proxy }
pub enum Source { Bcp14, Prose }
```

`Model::check` calls `Claims::of` first (19.7). `ignite-eval` gains `ignite-check` as a
dev-dependency (it has `ignite-syntax` and `ignite-types` as normal dependencies), so
`crates/ignite-eval/tests/claims.rs` builds its programs from source (row 18); the commit
says what it replaces (nothing; a test-only dependency).

`ignite` (U3, U5): `pub mod rfc;` (`rfc::import`, `rfc::check`) and `pub mod conformance;`
(`conformance::run(file, options) -> Result<i32, Error>`). The verdict rules of 19.10 live in
`conformance`, which calls the public `ignite::build_tests` and `ignite::TestOptions`.
`main.rs` maps `conformance::run`'s statuses and `rfc`'s (19.8, 19.10) as it maps `ignite
run`'s.

### 19.14 Stale points and known deviations

| # | where | stale text | resolution |
|---|---|---|---|
| 1 | `protocols.md` §3.1 | a catalogue holds "the RFC's collected ABNF, verbatim, in a `grammar` block" | phase 3; P2 catalogues hold requirements only (19.1). Marked "(phase 3)" there |
| 2 | `protocols.md` §3.1 | "a module may alias one (`const BARE_CR: Requirement = R2_2_3`)" | deferred; in P2 an alias is E091 (19.4). Noted there |
| 3 | `protocols.md` §3.1 | "`ignite rfc import 9112` reads RFCXML" | the importer takes a local file path (D6). Edited there |
| 4 | `protocols.md` §3.2 | `satisfies [R2_2_3, ..]`, `unsupported R7_1_1 "reason"`: bare ids | references are `Spec.Id` (19.5). Edited there |
| 5 | `protocols.md` §3.3 | the illustrative ids `R3_2_4` (Host) and `R7_1_1` (chunked) | by the id rule of 19.4 they are `R3_2_6` and `R7_1_2`. Inferred from the XML read by hand; row 28 confirms it. Edited there |
| 6 | `protocols.md` §3.3 | the example's status column shows `gap` for an unsupported row, and "skipped" for a test | the status is `unsupported`, the verdict `gap`; a test without a result is `not run` and makes its row `inconclusive` (19.10). The example is replaced by 19.10's format |
| 7 | `protocols.md` §3.3 | claim line `claim conformance to rfc9112: …` | `claim conformance of Http to rfc9112: …`, because a file may hold two protocols conforming to one spec (19.10). Edited there |
| 8 | `protocols.md` §3.1 and §7 row P2 | the vocabulary block shows `package std.rfc` with no file; the stories and the P2 row name `std/rfc/rfc.ignite` | `std/rfc/rfc.ignite` would be module `std.rfc.rfc`; the vocabulary is `std/rfc.ignite` (D1). Noted in §3.1, edited in the P2 row |
| 9 | `protocols.md` §3.1 | ids `R<section>_<ordinal>` | RFC 854 and 855 number no sections; their `section` is `p<page>` (19.4). Noted there |
| 10 | `protocols.md` §3.3, E096 row | "missing impl, an impl method needing more effects than its interface, or a stream-kind mismatch" | slice 14 gives E096 only "does not implement `ByteStream`" (18.7); the other meanings are P4's. Not edited: outside P2 |
| 11 | `protocols.md` §3.1 | the example catalogue constant sets `minimum = Some(8000)` | the importer writes no `minimum`, and P2 adds none (19.1). A `minimum` checked against a system's overriding value, not only the field default, is P3's (`protocols.md` §3.1 "Limits"). Noted there |
| 12 | `protocols.md` §3.3 | "prints one row per requirement in scope" | every requirement of each claimed spec gets a row; those outside `scope` print `out of scope`, the verdict §3.3 lists (19.10). Edited there |
| 13 | D4, `protocols.md` §3.3 | the RFC 9110 subset leaves out §8.6 (Content-Length, 13 marks) and §15 (status codes, 77 marks), which `http.ignite` implements | deferred: §15 alone is more than D4's 75 marks, and §8.6 framing is judged through RFC 9112 §6 in this slice. A later slice widens D4; the `Http` claim of row 2 says nothing about them |
| 14 | `docs/vision.md` X16, slice 10 acceptance | "`std/protocol/telnet.ignite` at most 400 lines", excluding tests | known deviation, not fixed in this slice: at `296f849` the file has 1,086 lines, 762 outside top-level `test` blocks (the review's awk count; slice 14 item 14 recorded about 655). U6 adds clauses to it. Not split in slice 15; Q4 |
| 15 | 19.4 "Level words", `std/rfc/rfc854.ignite` | a prose requirement "takes the level of its sentence's strongest word" | known limits of the mechanical classification, kept as written for this slice (steering, 2026-11-03): `RFC854.RP5_3` reads `Must` from "find it necessary", `RP7_1` `Required` from "not required … may treat", `RP12_1` `May` because "the protocol requires" has no word in the table. Two obligations have no word in the table and are not in the catalogue: "is obliged to provide" (p7) and "shall be capable" (p12, whose "reguired" is the RFC's typo). A later slice may widen the table |
| 16 | `protocols.md` §7 row P2, test 3; 19.0 row 3 | "The Telnet matrix claims RFC 854 for an NVT that refuses every option" | not met in slice 15 (steering, 2026-11-03), met by story `telnet-rfc854-conformance` (2026-11-06, 19.21): the RFC 854 claim prints `yes (0 gaps, …)` on both targets. In slice 15 the four MUST-class rows were `unsupported`, checked against the plain-text RFC 854: `RP5_4` (p5, "the process must transmit the TELNET Go Ahead (GA) command"), `RP10_2` (p10, "must transmit the TELNET SYNCH sequence"), `RP12_4` (p12, "The pair DM-Urgent is to have required meaning") and `RP5_3` (p5, "implementers are cautioned to provide methods of locally signaling that all buffered data should be transmitted immediately", whose `Must` comes from the word-table limit of item 15). Each is now `satisfied` with a passing verifying test on both targets: the server sends IAC GA after every prompt and after its answer to AYT (`RP5_4`); `tn` sends IAC IP followed by the Synch, IAC DM with the DM as TCP urgent data, on Ctrl+C (`RP10_2`); a session that finds urgent data pending discards data up to the Data Mark while still obeying commands (`RP12_4`); Ctrl+D on a partial line in `tn`'s line editor sends the line without a line end (`RP5_3`). The cost slice 15 measured was paid as stated: every server transcript gained IAC GA (the inline tests of `telnet.ignite`, the transcript test of `examples/terminal/commands.ignite`, the raw reads of `crates/ignite/tests/terminal.rs` and the prompt reads of the `run`, `drain`, `apply*`, `rollout_repeat` and rustgen `slice10` suites); the substrate gained `write_urgent` and `urgent_pending` on both streams and both targets, and `CODEGEN_REVISION` stepped to `0.1.0-31`; `tn`'s Ctrl+C and Ctrl+D transcripts in `terminal.rs` changed |
| 17 | U6's claims of `http.ignite` and `telnet.ignite`; 19.0 rows 2 and 4 | statuses the U6 adversary found wrong (`crates/ignite/tests/conformance.rs`, `adversary_*`) | resolved (steering, 2026-11-03). Fixed in code: `RFC9112.R6_3_4` and `R6_3_5` (a field value is trimmed of SP and HTAB only, RFC 9110 OWS, so `Content-Length: 3` beside U+00A0, U+0085 or U+3000 is 400); `R9_6_6` and `R9_6_7` (a response whose application headers list the `close` option ends the connection); the client role prints CR NUL as a carriage return alone, CR LF and a lone CR as LF (`RFC854.RP11_3`, `RP13_3`; no transcript changed, and of the goldens only `terminal.model.json` and `todo.model.json` move, by the std source positions the claims of `1bc293f` shifted). Restated as `unsupported`, so the `rfc9112` claim counts 15 gaps instead of 12: `R6_1_2` and `R6_1_4` (were `not_applicable`: the application's `Transfer-Encoding` is sent as given on a kept connection), `R2_2_1` (was satisfied by `parse_head` and `read_head`: the head is decoded as lossy UTF-8 before it is parsed). Their adversary tests stay ignored with the gap id; story `http-rfc9112-9110-gaps`. Landed (19.22), PR A (2026-11-06): `R6_1_2` and `R6_1_4` are satisfied by `checked` and their adversary test runs; the claims count `rfc9112: no (8 gaps, 0 failures, 0 inconclusive)` and `rfc9110: no (4 gaps, 0 failures, 0 inconclusive)` on both targets. PR B: the Host check and absolute-form (`R3_2_6`, `R3_2_2_5`, `R3_2_2_6`), 414 (`R3_3`) and HEAD (`RFC9110.R9_1_1`, `R9_3_2_1`), so `rfc9112: no (4 gaps, …)` and `rfc9110: no (2 gaps, …)`. PR C: chunked request bodies decoded (`R6_1_1`, `R6_3_3`, `R7_1_2`, `RFC9110.R6_2_5`, and the rows that became applicable, `R7_1_3`, `R7_1_1_1`, `R7_1_2_2`, `R7_1_2_3`, `RFC9110.R6_5_1_2`), so `rfc9112: no (1 gaps, 0 failures, 0 inconclusive)` and `rfc9110: no (1 gaps, 0 failures, 0 inconclusive)` on both targets. `R2_2_1` and `RFC9110.R6_6_1_2` stay gaps (story `http-octet-head-and-date`). Story `http-chunked-adversary-findings` restates `RFC9110.R6_2_5` as `unsupported` while `R2_2_1` is a gap, so `rfc9110: no (2 gaps, 0 failures, 0 inconclusive)` (19.22) |

### 19.15 Decisions

| # | decision | alternative | why |
|---|---|---|---|
| D1 | The vocabulary is 19.3's text, in `std/rfc.ignite`, module `std.rfc`, as `protocols.md` §3.1 gives it. The spec constant is `RFC<n>`. Requirement ids are 19.4's | `std/rfc/rfc.ignite` (the stories' path) | that path is module `std.rfc.rfc` by the module-path rule; §3.1's `package std.rfc` needs `std/rfc.ignite` |
| D2 | Catalogue constants are ordinary constants (section 4.3). Clauses are no expressions: no backend reads them and no reachability walk starts at them. A catalogue reaches generated code only through a name the program imports (Rust: a program's `use` lines are roots) or an expression uses. U1 makes LLVM drop every constant nothing kept references, after lowering (19.13), so that an unreferenced constant leaves no text or debug entry on either target. No runtime-use diagnostic, no new code. Rows 5, 12 and 39 make "clauses emit nothing" observable | (a) compile-time only, with a diagnostic for a runtime use; (b) exclude modules under `std.rfc` from emission by prefix | (a) needs a number outside E090 to E099's free slots (E093 to E095 and E098 are P3's and P4's, E097 is kept unused). (b) is smaller in code but turns a test's legitimate read of a requirement's `minimum` into a link error that needs its own diagnostic. Dropping unreferenced constants is one pass in `ignite-llvm`'s `prune`, needs no new code, and fixes the same leak for every unreferenced constant (as built; a constant's glue, instances and closures stay, 19.13) |
| D3 | Clause grammar as 19.5. A requirement reference is `Spec.Id`; a `conforms` entry is a name. E091's first trigger is in `ignite-check` (U1); its second trigger, E090 and E092 are in `ignite-eval` (U2). Two statuses on one requirement is E090 with its own message. Clause resolutions travel in `TypedProgram` (19.13) | bare ids; E092 for two statuses; resolutions in `Symbols` | ids repeat across catalogues; E090 is "status count is not one", while E092 means "outside the protocol's specs"; `ignite_eval::Module` carries `TypedProgram` and no `Symbols` |
| D4 | RFC 9112 is truncated at import to sections 2 to 9 (`--sections 2,3,4,5,6,7,8,9`). The RFC 9110 server subset is sections 5, 6, 7.2, 9.1, 9.2, 9.3.1, 9.3.2, 9.3.3 and 9.3.5 (fields, message abstraction, Host and `:authority`, method overview and properties, GET, HEAD, POST, DELETE), 75 marks. `Http` narrows parties by `scope { parties = [Server, Sender, Recipient, Any] }`, which leaves out `Client` and `Proxy`. §8.6 and §15 are left out (19.14 item 13) | 9112 whole, narrowed by `scope`; 9110 with §8.6 and §15 | §1.1 is the notation boilerplate (11 marks) and §10.1 is security prose (1). Truncating keeps the catalogue to what the claim judges. The 9110 subset is what an origin server for `todod`'s routes (GET, POST, PATCH, DELETE; PATCH is RFC 5789) must classify |
| D5 | Only the RFCXML sources of RFC 9112 and RFC 9110 are vendored, unmodified, under `docs/rfc/`, with `docs/rfc/README.md` citing the IETF Trust Legal Provisions (19.9). RFC 854, 855 and 1143 are not vendored: their catalogues quote the sentences they need with page or section citations, their digest is recorded from the published text file as read on the import day, and `ignite rfc check` without SOURCE reports them `unchecked` | vendor all five; tests download | steering decision, 2026-11-03: the post-2008 RFCs are under the Trust's licence; the gate runs offline; `rfc check` of the imported catalogues needs their bytes (1.49 MB) |
| D6 | `ignite rfc import FILE [--sections] [--package] [--url] [--out]` reads a local RFCXML v3 file with a hand-written reader (`crates/ignite/src/rfc/xml.rs`, no dependency, so the commit needs no "replaces" line). The digest is `sha256:<hex>` through the existing `sha2` dependency. Extraction as 19.8 | an XML crate | RFCXML uses a small subset of XML; no new dependency |
| D7 | `std.rfc` and `std.rfc.*` are leaf by a prefix rule (19.11), in `docs/layering.md` and `layering.rs`; U1 adds it | one row per module | five catalogues now and more with every protocol; a prefix rule needs no edit per catalogue |
| D8 | `ignite conformance FILE [--target rust\|llvm]`, default `rust` (`llvm` since 2026-11-05, `story:llvm-default-target`). It builds FILE's test executable on that target through `ignite::build_tests`, runs it once, and reads the result lines between `running N tests` and the first blank line. Output and verdicts as 19.10, with the claim line `claim conformance of <P> to rfc<n>: yes\|no (<g> gaps, <f> failures, <i> inconclusive)`. Exit 0 when every claim holds, 1 when one does not, 2 on a diagnostic, usage error or no conforming protocol, 3 on a test build or run failure or IO | run on all targets at once; JSON output | one target per run matches `ignite test --target`; rows 2, 3 and 36 run both and compare. JSON can be added later without changing the text |
| D9 | Seven units as 19.16, one per story, in five waves | merge importer and catalogues | the stories are already cut this way; their file scopes do not overlap within a wave |

### 19.16 Work split

| unit | story | owns | depends on | first failing test | rows |
|---|---|---|---|---|---|
| U0 | `story:slice-15-contract` | this section, the section-8 rows, `docs/protocols.md` stale points, slice 14's row 17 | — | — | — |
| U1 | `story:slice-15-frontend` | `crates/ignite-syntax/` (clauses, AST fields, `e001_malformed_clause`, `valid_clauses`, the message in `e001_protocol_member.expected`); `crates/ignite-check/` (resolution, E091 first trigger, the struct literals of its `#[cfg(test)]` module, `valid_clauses`, the fixture std's `std/rfc.ignite` and catalogue and the module list in `tests/fixtures.rs`); `crates/ignite-types/` (E040 on clauses, the `TypedProgram` resolutions, `valid_clauses`, the fixture std and module list in `tests/fixtures.rs`); `crates/ignite-llvm/src/` (dropping unreferenced constants, 19.13) and its row-39 test; `CODEGEN_REVISION` in `crates/ignite/src/apply/plan.rs`; the struct literals in `crates/ignite-rustgen/tests/units.rs`; `std/rfc.ignite`; the prefix rule in `docs/layering.md` and `layering.rs` (`layer_of`, `StdModule::layer`); `crates/ignite/tests/clauses.rs`, its `mod clauses;` line, `crates/ignite/tests/fixtures/clauses/` | `story:rustgen-type-parameter-shadowing` (shared rustgen test binary, `CODEGEN_REVISION`) | `fixtures::fixtures_match_expected` (ignite-syntax) on `valid_clauses` | 8 to 13, 39 |
| U2 | `story:slice-15-claim-model` | `crates/ignite-eval/` (`Claims`, E090, E092, E091 second trigger, the call in `Model::check`, the `ignite-check` dev-dependency); `crates/ignite-eval/tests/claims.rs` and its `mod` line; `crates/ignite/tests/fixtures/e090_*`, `e091_catalogue`, `e092_*`, `claim_ok_complete`, `claims_std/`; in `plan.rs` the filter of `plan::fixtures_match_expected` and the tests of rows 40 and 41; the `docs/layering.md` paragraph on the eval enums (19.11) | U1 | `plan::fixtures_match_expected` on `e090_missing_status` | 14 to 18, 40, 41 |
| U3 | `story:slice-15-rfc-importer` | `crates/ignite/src/rfc/`; `pub mod rfc;` in `crates/ignite/src/lib.rs`; `rfc` and its exit statuses in `crates/ignite/src/main.rs`; `crates/ignite/tests/rfc.rs`, its `mod rfc;` line, `crates/ignite/tests/fixtures/rfc/` | U1 | `rfc::import_writes_the_golden_twice` | 19 to 24 |
| U4 | `story:slice-15-catalogues` | `std/rfc/rfc854.ignite`, `rfc855.ignite`, `rfc1143.ignite`, `rfc9112.ignite`, `rfc9110.ignite`; `docs/rfc/rfc9112.xml`, `rfc9110.xml`, `README.md`; its tests appended to `crates/ignite/tests/rfc.rs` | U1, U3 | `rfc::committed_catalogues_check_silently` | 25 to 30 |
| U5 | `story:slice-15-conformance-command` | `crates/ignite/src/conformance.rs` and its unit tests; `pub mod conformance;` in `lib.rs`; `conformance` and its exit statuses in `main.rs`; `crates/ignite/tests/conformance.rs`, its `mod conformance;` line, `crates/ignite/tests/fixtures/conformance/` | U2, U3 (shared `main.rs`, `lib.rs`, `tests/main.rs`) | `conformance::each_verdict_prints_its_row` | 31 to 37 |
| U6 | `story:slice-15-protocol-claims` | the clauses in `std/protocol/telnet.ignite` and `std/protocol/http.ignite`; rows 1 to 5 appended to `crates/ignite/tests/conformance.rs`; `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/vision.md` §4.1, `docs/performance/15/` | U4, U5 | `conformance::telnet_matrix_claims_rfc854_for_an_nvt_that_refuses_every_option` | 1 to 7 |

Row 38 belongs to every unit.

Order: W1 U0 (with `story:rustgen-type-parameter-shadowing`); W2 U1; W3 U2 and U3; W4 U4 and U5;
W5 U6.

Shared files:

- W2: U1 edits `crates/ignite/src/apply/plan.rs` after the shadowing fix merged (W1), so
  its `CODEGEN_REVISION` step follows that fix's.
- W3: U2 and U3 both touch `crates/ignite/tests/`. U2 adds fixtures and edits `plan.rs`;
  U3 adds `rfc.rs` and the `mod rfc;` line in `tests/main.rs`, and edits `src/main.rs` and
  `src/lib.rs`, which U2 does not touch. Neither edits the other's file.
- W4: U4 appends to `rfc.rs` and edits nothing in `src/`. U5 edits `lib.rs`, `main.rs` and
  `tests/main.rs`.
- U6 appends to `conformance.rs` after U5 merged.

Rules: one integration-test binary per crate (`AGENTS.md`), so every new test file is a `mod`
line in its crate's `tests/main.rs`. Layering rule 5 holds for `crates/ignite/src/rfc/` and
`conformance.rs`. Each unit's commit names the rows of 19.0 it makes pass.

Planning store: the stories' bodies differ from this split in the points of the contract
report ("Story updates"). The steering session updates them; U0 does not edit the store.

### 19.17 Gate time

Method of 18.18 (`docs/slice.md` 15.1), run through `tools/ignite-perf` (modes `cold`,
`warm`, `programs`, `cli`; records `ignite-performance/1`). "Before" is a `git archive 296f849`
copy and "after" the closed tree, each with its own `CARGO_TARGET_DIR`. One cold gate per tree,
then three interleaved warm passes. A pair whose two 1-minute loads differ by more than 5 is
dropped. Report the medians of the pair ratios next to the absolute times.

Units: those of 18.18, plus:

- `ignite check examples/todo/todo.ignite` (15 pairs). `std.protocol.http` now imports two
  catalogues (about 200 requirement constants), and every program that reaches it loads them.
  The rise is the expected cost of the slice;
- `ignite conformance std/protocol/telnet.ignite` and `…/http.ignite`, per target, cold and
  warm (after only);
- `ignite rfc import docs/rfc/rfc9110.xml` with D4's sections (after only, 5 runs).

Expected causes, to be measured or labelled inferred: the frontend on the catalogues for every
program that reaches a protocol module (`ignite check`, `ignite plan`, program builds), and in
`task test-ignite` the compile of the catalogue constants into the test executables of
`telnet.ignite` and `http.ignite`.

A unit that rises over 10 % gets a measured cause, or the record labels the cause inferred. No
baseline moves. The apply suites stay the slice-13 exception.

Records: a "Slice 15" paragraph in `AGENTS.md`'s gate-time section; raw logs and records under
`docs/performance/15/`.

### 19.18 Open questions

Each has a default, which the units follow unless the review changes it.

| # | question | default |
|---|---|---|
| Q1 | An RFC 854 MUST that the refuse-everything NVT does not meet would make row 3 fail | U6 may narrow `scope` by party only where RFC 854 addresses a party `Telnet` does not implement, with the reason in the commit. It never marks an implemented party's MUST `not_applicable` to make the claim hold. If a MUST stays unmet, U6 implements it when the change fits the story. Otherwise it reports a blocker |
| Q2 | `ignite check` of `todo.ignite` (and every HTTP program) rises over 10 % from loading the catalogues (19.17) | record the measured cause as a slice-15 exception, as 12c and 13 did. A later milestone loads catalogue modules only for `check` and `conformance` |
| Q4 | X16: when `std/protocol/telnet.ignite` is split to meet the 400-line bound (19.14 item 14), and along which seam (codec, session, executable kinds) | not in slice 15. The slice that next changes Telnet beyond clauses proposes the split |

Q3 (licence of vendoring the older RFCs) is closed by D5.

### 19.19 Specifications that are no RFC (story `rfc-spec-id`, 2026-11-05)

JSON-RPC 2.0 and the Language Server Protocol are not RFCs, so their catalogues have no honest
`rfc` value. This subsection adds a text identifier. It is add-only: nothing in 19.3 to 19.18
is renamed, removed or retyped, and every output of an RFC catalogue stays byte-identical
(the Telnet and Http matrices, `ignite rfc check` of the committed catalogues, the model
goldens).

**Vocabulary.** `std/rfc.ignite` changes in three lines; where its text differs from 19.3's
block, the file holds:

- `Requirement.rfc` and `Spec.rfc` are `rfc: Int = 0`. A default only: the type stays `Int`,
  and every existing literal names `rfc`.
- `Spec` gains a last field `id: Text = ""`: the text id of a specification that is no RFC
  (`"jsonrpc-2.0"`, `"lsp-3.17"`), empty for an RFC.

A specification that is no RFC sets `id` and leaves out `rfc` in the `Spec` and in every
requirement, so both are 0 and case 4 of E091's second trigger (19.4) still holds. The claim
model links a requirement to its spec through the `Spec`'s `requirements` and the `Spec.Id`
reference (19.5), not through `rfc`, so a requirement needs no field of its own.

**Label.** A spec's label is its `id` when that is not empty, else `rfc<n>`.

- 19.10's block header is `<label> <title>, protocol …` and its claim line `claim conformance of
  <P> to <label>: …`. For an RFC that is the text of 19.10, unchanged.
- E092 (19.7) names a spec with an id by the id ("`TEXT3.R1_1` belongs to text-3.0, which …"),
  an RFC as before ("belongs to RFC 2").
- E091's case 4 (19.4) names a spec with an id by the id: "requirement `R2_1` has rfc 5, and
  `LINE_ECHO_1` is `line-echo-1`, which is no RFC"; an RFC as before ("… is RFC 7001").
- `ignite_eval::SpecClaim` gains `pub id: String` and `pub fn label(&self) -> String` (19.13).

**E091, labels.** A label names one specification, so E091 (`ignite-eval`) also reports:

| case | position | message |
|---|---|---|
| a `Spec` with neither `rfc` nor `id` | the constant, when the checked file declares it; else the first reference to it | "spec `NOID` has neither `rfc` nor `id`; an RFC sets `rfc`, a specification that is no RFC sets a text `id`" |
| a `Spec` whose `id` is `rfc` followed by digits, in any case | as above | "spec `FAKE7` has the id `rfc7`, the label form of an RFC; an RFC sets `rfc`, and a text id is not `rfc<digits>`" |
| two specs with one label among those the checked file's clauses name | the first reference to the second spec, in source order (`satisfies`, `conforms` entries, statuses, `verifies`) | "specs `TEXT3` and `DUP3` have one label `text-3.0`; a label names one specification" |

The first two run for the checked file's own `Spec` in the second trigger's pass, and for an
imported spec at its first reference, because the label is what the checked file's matrix
prints. E092 does not name a spec these cases report. Fixtures: `e091_no_label`,
`e091_rfc_label`, `e091_one_label` (with the claim-fixture std's `rfc/dup3.ignite`, a second
spec with the id `text-3.0`) and `e091_text_id_rfc` (case 4's wording).

**Catalogues.** Module `std.rfc.<name>`, file `std/rfc/<name>.ignite`. 19.4 applies, with these
differences:

- The `Spec` constant is the upper-case module name (`JSONRPC_2`), not `RFC<n>`.
- Rule 3 of `docs/layering.md` exempts a catalogue `std.rfc.*` from names containing the name of
  a protocol whose module imports it, so `std.rfc.jsonrpc_2` declares `JSONRPC_2`, which
  `std.protocol.jsonrpc` imports. The exemption is explicit, not the effect of an underscore,
  and covers no other module. The LSP catalogue is `std.rfc.language_server_3_17`
  (`LANGUAGE_SERVER_3_17`) until a protocol module imports it.
- `section` is the section number where the specification numbers its sections. Where it does
  not (LSP), `section` is a dotted label of the heading (`base.header`, `lifecycle.shutdown`),
  and the module's header comment maps each label to its anchor on the page. The id rule of
  19.4 is unchanged (`RBASE_HEADER_1`); 19.5's `sections` prefix rule works on the labels.
- `source = Prose`. One requirement per sentence with a word of the table below, case
  insensitive; the level is that of the strongest word (19.4). The table is 19.4's, plus:
  `needs to`, `need to` give `Must`; `not allowed` gives `MustNot`; `allowed`, `free to` give
  `May`; `recommended` gives `Recommended`; `advised` gives `Should`. The module's header
  comment lists the sentences left out as descriptions or for want of a word.
- The party is `Client` or `Server` where the sentence names one, else `Sender` or `Recipient`
  of the message the sentence describes, else `Any`.

| source | catalogue | id | from | bytes | sha256 | requirements |
|---|---|---|---|---|---|---|
| JSON-RPC 2.0, sections 4 to 6 | `std.rfc.jsonrpc_2` (`JSONRPC_2`) | `jsonrpc-2.0` | `https://www.jsonrpc.org/specification` | 24,512 | `8fe1edfdca511d309e712e47447457ea5159b728ec02071a84593aed692aefeb` | 35 |
| LSP 3.17, base protocol and every subsection of the server lifecycle up to exit (initialize, initialized, register and unregister capability, setTrace, logTrace, shutdown, exit) | `std.rfc.language_server_3_17` (`LANGUAGE_SERVER_3_17`) | `lsp-3.17` | `https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/` | 918,678 | `93afdf15adbaa7355cd00d0134de05331b8dcc9361ffdd11d3da0924cab20579` | 37 |

Read on 2026-11-05. Neither page is vendored; `ignite rfc check` reports both catalogues
`unchecked`, as the prose RFC catalogues (19.9).

**The JsonRpc claim.** `protocol JsonRpc conforms [JSONRPC_2] scope { parties = [Server, Any]
}`: the module is the server, so the Client's requirements are `out of scope`, R4_4 among them
(a request with `"id": null` stays refused as an invalid request, which the LSP base protocol
also forbids; it would be a deviation if the Client were claimed).

- Satisfied, each with a passing test on both targets: R4_6, R4_1_1, R5_1 to R5_11, R5_1_1,
  R5_1_2, R5_1_4, R6_7. R4_6, R4_1_1 and R5_9 hold for ids and params that are JSON numbers
  with a fraction, an exponent or beyond `Int` since `story:json-decimals` (19.20); from
  831b72d4 until then they were `unsupported` gaps and the claim was no.
- `unsupported`, batches: R6_2 and R6_3 (SHOULD, `deviation`), R6_4 and R6_5 (MAY, `n/a`).
- `not_applicable`: R4_2 and R4_2_4 (the application names methods and reads their params),
  R5_1_3 (the error's author writes its message), R6_8 (no response array is ever sent).
- The claim holds: `claim conformance of JsonRpc to jsonrpc-2.0: yes (0 gaps, 0 failures, 0
  inconclusive)`, exit 0 on both targets.
- A body that is no message is answered under the id it holds when it is a JSON object whose
  `id` is a number or a string (`detected_id`), and under null only when none can be read
  (R5_10).
- A batch of notifications only, a non-empty array whose every element decodes as a
  notification, is passed to the application in order and not answered (R4_1_1's "including
  those that are within a batch request"). Any other batch is one invalid-request error, or
  the parse error when it is no JSON (R6_7).

The LSP catalogue is claimed by no protocol yet; the later server conforms to it.

**Fixtures and tests.** `crates/ignite/tests/fixtures/e090_text_id` (a MUST of a spec with
only the id and no status is E090) and `e092_text_id_foreign` with the claim-fixture std's
`rfc/text3.ignite` (E092 names the id); the E091 fixtures above;
`claims::a_spec_with_only_a_text_id_is_claimed_under_its_id` in `ignite-eval`;
`rfc::text_id_catalogues_check_and_quote_their_pages` (both catalogues check silently, are
`unchecked`, carry no `rfc`, and every level follows the table above);
`rfc::lsp_catalogue_holds_or_lists_every_server_lifecycle_rule`;
`conformance::json_rpc_matrix_is_labelled_by_its_id_and_holds` on both targets; the
`plan::adversary_*` label cases; `layering::rule_3_exempts_only_catalogues_of_their_own_protocol`.

Not done: a URL per requirement (the anchors are in the header comment).

### 19.20 JSON numbers with a fraction or an exponent (story `json-decimals`, 2026-11-05)

`std.json` read integer numbers of at most 18 digits only, so a JSON-RPC message holding an LSP
`decimal` such as `0.5` was answered with the parse error. `Json` gains the variant
`Decimal(Text)`; every other variant is unchanged.

- `parse` reads every number RFC 8259 section 6 allows: `-`, an integer part without leading
  zero, an optional fraction `.` with one or more digits, an optional exponent `e` or `E` with
  an optional sign and one or more digits.
- A token with no fraction and no exponent that fits the 64-bit `Int` (from
  `-9223372036854775808` to `9223372036854775807`) is a `Number(Int)`, except `-0`. Every
  other token is a `Decimal` holding the token's exact text: `-0`, `9223372036854775808`,
  `-9223372036854775809`. The range check compares the digit text with the bounds, and a
  negative token accumulates below zero, so nothing overflows and nothing converts a number to
  a float. `MAXIMUM_DIGITS` is 19, the digit count of the longest `Int`: an integer token of
  more digits is a `Decimal`, one of exactly 19 is compared with the bound.
- `-0` was `Number(0)` before this story and is `Decimal("-0")` now; `Number(0)` and
  `Decimal("-0")` are different values.
- `decimal(text) -> Result<Json, JsonError>` is the checked constructor: for exactly one number
  token, no whitespace around it, it returns the `Number` or `Decimal` `parse` gives, and the
  error otherwise. `Decimal(text)` built by hand is not checked, and `encode` writes its text
  whatever it is.
- An invalid token is still an error: `01` and `00.5` "leading zero in a number"; `1.`, `1e`,
  `1e+`, `-` and `-.5` "expected a digit" at the offset where a digit is missing; `.5` and `+1`
  "unexpected character". The messages "only integer numbers are supported" and "number longer
  than 18 digits" are gone.
- `encode` writes a `Decimal` as its text, so `encode(parse(t)?) == t` for every number token
  `t`.
- Equality is structural: `Number(1) != Decimal("1.0")`, `Decimal("1.0") != Decimal("1.00")`,
  `Decimal("1e2") == Decimal("1e2")`. Numeric comparison is the caller's business. `as_int`
  and `int` return `None` for a `Decimal`.
- Exhaustive matches over `Json`: `encode` is the only one in `std/`, `examples/`, the crates'
  test fixtures and the book's checked blocks; every other match has a wildcard arm.
- `std.protocol.jsonrpc`: `Id` gains `DecimalId(Text)`. `id_of` reads a `Decimal` id as
  `DecimalId`, `id_json` writes it back as `Decimal`, so a request with id `1.5` or
  `12345678901234567890` is answered under that id (R4_5 discourages a fractional id but
  allows it). R4_6, R4_1_1 and R5_9 are satisfied again (19.19).
- No compiler crate names `std.json` or `Json`: both targets lower the new variant like any
  other enum variant, so no generated code changes shape for the variant.
  `ignite_eval::Json`, the canonical JSON of `ignite plan` and `check --json`, is a separate
  Rust type and is unchanged.
- The Rust target's `std/Cargo.toml` version is `0.0.0-h<content hash>`, no longer
  `0.0.0-<content hash>`: the std library of `std/protocol/jsonrpc.ignite`'s test build hashed
  to `0791203012029406`, all decimal digits with a leading zero, which SemVer forbids in a
  pre-release identifier, so cargo refused the manifest and `ignite conformance` exited 3. The
  `h` makes the identifier alphanumeric. `apply::plan::CODEGEN_REVISION` is
  `ignite-codegen/0.1.0-19`.

Tests: in `std/json.ignite`, "a number with a fraction, an exponent or more than 18 digits is a
decimal of its exact text", "an invalid number token is still an error" and "numbers compare
by their structure, not their value", "decimal builds what parse gives for one number token
and refuses the rest", and the adversary tests "an integer of 19 digits that fits Int is a
number" and "minus zero round trips byte-identical"; in `std/protocol/jsonrpc.ignite`, the four
tests removed in 0b28ae4e and 831b72d4, restored, and "an id of 19 digits that fits Int is a
NumberId and -0 comes back as sent"; `conformance::json_rpc_matrix_is_labelled_by_its_id_and_holds`.

Repeated member names (story `json-duplicate-keys`, 2026-11-08). RFC 8259 section 4 says the
names of an object should be unique and leaves open what a reader does when they are not.
ECMAScript's `JSON.parse` keeps the last member, so a server that read the first one disagreed
with its JavaScript clients: `{"channel":"/a","channel":"/meta/handshake"}` was `/a` in std and
`/meta/handshake` in JavaScript (case study gap G29). The rule now:

- `parse` keeps every member in document order and `encode` writes every one back, in that
  order: for compact `t` (no whitespace between tokens, as `encode` writes) `encode(parse(t)?)
  == t` holds for an object with a repeated name too; for any other `t` the members come back
  in order, compacted (`{"a": 1, "a": 2}` encodes as `{"a":1,"a":2}`).
- `get(value, name)` returns the last member `name`; `text`, `int` and `bool` read through it.
  It returned the first before this story.
- `get_all(value, name) -> List<Json>` returns every member `name` in document order, empty
  when there is none or `value` is not an object.
- `read` and `read_with` (27) still refuse a repeated name with "duplicate member"; the split
  stays: `parse` is the lenient reader that agrees with JavaScript, `read` the strict one that
  gives a `Map`.

Tests: in `std/json.ignite`, "get returns the last duplicated member and get_all every one"
and "the accessors read members of the right kind" (its repeated `id` now gives 8);
`data_json::read_and_write_agree_across_the_evaluator_and_both_targets` evaluates the G29
document's `get`, `get_all` and `encode` in the evaluator and prints them on both targets.

### 19.21 Telnet conforms to RFC 854 (story `telnet-rfc854-conformance`, 2026-11-06)

`ignite conformance std/protocol/telnet.ignite` prints `claim conformance of Telnet to rfc854:
yes (0 gaps, 0 failures, 0 inconclusive)` on both targets (19.0 row 3, 19.14 item 16). The four
rows slice 15 left `unsupported` are satisfied, each with a verifying test that passes on both
targets.

| row | RFC 854 | satisfied by | verified by |
|---|---|---|---|
| `RP5_4` | p5, Go Ahead | `session`, `answer_line` | "the server sends IAC GA after each prompt and the AYT answer" |
| `RP10_2` | p10, the Synch after IP | `interrupt`, `edit_input` | "an interrupt is IAC IP followed by the Synch" |
| `RP12_4` | p12, DM-Urgent | `session`, `decode_within` | "a Synch discards the data before its Data Mark" |
| `RP5_3` | p5, sending buffered data at once | `apply_key`, `edit_input` | "Ctrl+D on a partial line sends it without a line end" |

**Urgent data, a stream capability.** The substrate knows sockets, not Telnet (19.11): it gains
TCP urgent data as two methods of `ByteStream`, on `TcpStream`, `PipeStream` and a bounded
parameter (18.3, 18.4). The Telnet logic stays in `std/protocol/telnet.ignite`.

- `write_urgent(data: Bytes) -> IoResult<Unit>`, effect `write`: what was written before and
  `data` are sent at once, the last byte of `data` as the urgent byte. On TCP that is one
  `send` with `MSG_OOB`, so a short message is one segment and the urgent byte arrives with the
  bytes before it. Empty `data` sends what was written and marks nothing. After `close` it is
  refused as `write_all` is.
- `urgent_pending() -> IoResult<Bool>`, effect `read`: whether an urgent byte has arrived that no
  read has returned yet. It never waits. On TCP it is `POLLPRI`.
- Every TCP stream has `SO_OOBINLINE`, so the urgent byte stays in the stream at its place. A
  read stops before the urgent byte (the kernel's rule); the next read starts with it and
  clears the pending flag. A reader that checks `urgent_pending` after a read therefore knows
  that every byte of that read came before the urgent byte. A later urgent byte replaces an
  earlier one, as a later urgent pointer does.
- A pipe end does the same in memory: `write_urgent` delivers its buffer and `data` as one
  batch and records the last byte's place in the peer's queue; reads stop before it.
- Runtime ABI, add-only, each declared only in a module that calls it: `ignite_rt_tcp_write_urgent`
  and `ignite_rt_pipe_write_urgent` `i32 (i64, List* bytes, IoError* err)`,
  `ignite_rt_tcp_urgent_pending` and `ignite_rt_pipe_urgent_pending`
  `i32 (i64, i8* out, IoError* err)` (`docs/llvm.md` section 5). Each refuses the other kind of
  handle as the 18.9 entries do.
- `Builtin` gains `TcpStreamWriteUrgent`, `TcpStreamUrgentPending`, `PipeStreamWriteUrgent`,
  `PipeStreamUrgentPending`, `StreamWriteUrgent` and `StreamUrgentPending` (add-only, 18.13). The
  Rust target calls the inherent method or `<S as ignite_io::ByteStream>::…`; the LLVM target
  resolves each instance to its `tcp_` or `pipe_` entry (18.11). `CODEGEN_REVISION` is
  `ignite-codegen/0.1.0-31`.

**Telnet.**

- Go Ahead: the server sends IAC GA after every prompt (the first and each after a reply) and
  after its answer to AYT, the outputs after which it waits for input. A reply that closes the
  session has none. Every server transcript changes by these two bytes; the client role and the
  probe read GA as a command and print nothing for it.
- The Synch, sent: `tn`'s Ctrl+C calls `interrupt`, which writes IAC IP and then IAC DM by
  `write_urgent`, so the DM is the urgent byte.
- The Synch, received: `decode_within` reports IAC DM as `DataMark`. After each read the session
  asks `urgent_pending`; when it holds, the session drops the line so far and discards data
  (literals and line ends) until the next `DataMark`, while commands are still obeyed (options
  are refused, AYT answered, IP ends the session). An IAC DM without urgent data is a mark
  only and discards nothing.
- Ctrl+D on a partial line in `tn`'s line editor sends the line as NVT data with no line end and
  starts an empty line, as a terminal in canonical mode does; on an empty line it still closes
  the write half. The edit is `Edit.Send(Bytes)`.

**Known limits.**

- The Synch is honoured for urgent data that is pending when a read returns. Data a session read
  and decoded before the urgent segment arrived is not taken back.
- `SO_OOBINLINE`, `POLLPRI` for pending urgent data and the stop at the mark are Linux
  behaviour; the substrate already depends on Linux (`PR_SET_PDEATHSIG`, `close_range`, 21.5).

### 19.22 Http closes its RFC 9112 and 9110 MUST gaps (story `http-rfc9112-9110-gaps`, 2026-11-06)

At the slice-15 close `ignite conformance std/protocol/http.ignite` counted `rfc9112: no (15
gaps, …)` and `rfc9110: no (9 gaps, …)` (19.0 row 2, 19.14 item 17). The story lands in three
pull requests, each with its own gate; each moves its rows from `unsupported` to `satisfied`
with a verifying test that names the row and passes on both targets.

**PR A, the application's response headers.** `checked` replaces the application's response
by 500 when one of its header fields would break the message. Http frames every response by its
own `Content-Length` and applies no transfer coding, so an application `Transfer-Encoding` or
`Content-Length` is refused rather than sent.

| row | section | refused by `checked` | verified by |
|---|---|---|---|
| `RFC9112.R2_2_8` | 2.2 | a field name that is not a token (one starting with whitespace) | "a response header name that is not a token is 500" |
| `RFC9112.R6_1_2`, `R6_1_4`, `R6_1_7`, `R6_1_8`, `R6_1_11`, `R6_2_1` | 6.1, 6.2 | `Transfer-Encoding` | "an application Transfer-Encoding is 500 and Http applies no transfer coding" |
| `RFC9110.R5_3_3` | 5.3 | `Content-Length`; a repeated name other than a list field of RFC 9110 or 9111 and `Set-Cookie` | "a repeated response field is 500 unless it is a list or Set-Cookie" |
| `RFC9110.R5_6_1_1_1` | 5.6.1.1 | an empty element of a list field (commas inside a quoted-string do not count) | "an empty element of a response list field is 500" |
| `RFC9110.R5_6_3_4` | 5.6.3 | whitespace beside `=` in an authentication field's `auth-param`, the fields whose grammar has BWS | "BWS in a response's auth-param is 500" |
| `RFC9110.R5_6_7_2`, `R5_6_7_3` | 5.6.7 | `Date`, `Last-Modified`, `Expires`, `Retry-After` whose value is not exactly an IMF-fixdate (`Retry-After` may be digits) | "a response timestamp is an IMF-fixdate with no extra whitespace" |

The U6 adversary test of `R6_1_4` runs again. The claims count `rfc9112: no (8 gaps, 0
failures, 0 inconclusive)` and `rfc9110: no (4 gaps, 0 failures, 0 inconclusive)` on both
targets. A list or date field is known by its name; an unknown field the application repeats is
refused, since Http cannot know that its definition allows a list.

**PR B, the request head.**

| row | section | satisfied by | behaviour | verified by |
|---|---|---|---|---|
| `RFC9112.R3_2_6` | 3.2 | `resolve_host` | 400 for an HTTP/1.1 request without `Host`, for two `Host` lines in any version, and for a value that is not `uri-host [ ":" port ]` (a `reg-name`, IPv4 or bracketed IP-literal; empty is valid) | "an HTTP/1.1 request has exactly one valid Host or is 400" |
| `RFC9112.R3_3` | 3 | `target_too_long` | a head over `HttpLimits.head` whose request line alone is over it, and starts with a method, is 414; other heads over the limit stay 431 | "a request-target longer than the head limit is 414" |
| `RFC9112.R3_2_2_5`, `R3_2_2_6` | 3.2.2 | `resolve_host`, `next_request` | an `http` or `https` absolute-form target is accepted; the application sees its path and query, and its authority (not empty, a valid host) as the `host` header in place of the received one, which is still checked | "an absolute-form target is accepted and its authority replaces Host" |
| `RFC9110.R9_1_1`, `R9_3_2_1` | 9.1, 9.3.2 | `session`, `reject` | HEAD is asked of the application as GET and answered with the head of its response, the `Content-Length` of the body included, without the body; a rejected HEAD gets the head of the rejection only (`Next.RejectHead`) | "HEAD is answered as GET without content" |

Inline tests, the adversary fixture and `todo.rs` cases that sent an HTTP/1.1 request without
`Host` gained `Host: h` (or `Host: x`). The claims count `rfc9112: no (4 gaps, 0 failures, 0
inconclusive)` and `rfc9110: no (2 gaps, 0 failures, 0 inconclusive)` on both targets. No limit
was added: the request-target is bounded by `HttpLimits.head`.

**PR C, chunked request bodies.** `next_request` decodes a chunked body instead of answering
501. `coding_status` decides from the request's `Transfer-Encoding` lines, their list elements
in lower case with empty elements ignored (RFC 9110 5.6.1.2): an HTTP/1.0 request with
`Transfer-Encoding`, one with `Content-Length` too, one with no coding, one whose final coding
is not `chunked` (`chunked;a=b` and `identity` included) or that names `chunked` twice is 400; a
coding other than `chunked` before the final `chunked` is 501; each of those closes the
connection lingering, as every rejection does. `read_chunked` then reads chunk-size lines,
chunk data and the trailer section from the stream.

| row | section | satisfied by | behaviour | verified by |
|---|---|---|---|---|
| `RFC9112.R6_1_1`, `R7_1_2` | 6.1, 7.1 | `read_chunked` | the chunks are joined into `Request.body`; the bytes after the trailer section are the next request | "a chunked request body is decoded" |
| `RFC9112.R6_3_3` | 6.3 | `coding_status` | a final coding other than `chunked` is 400 and the connection closed | "a Transfer-Encoding Http cannot frame is 400 and an unknown coding 501" |
| `RFC9112.R7_1_3` | 7.1 | `chunk_size` | a size is compared with what `HttpLimits.body` still admits before each digit is added, so no number of hex digits overflows; over it is 413 | "a chunked body over the body limit is 413 however many digits its size has" |
| `RFC9112.R7_1_1_1` | 7.1.1 | `chunk_size` | after the size, optional SP or HTAB and `;` start the extensions, which are ignored once they hold no control byte but HTAB; anything else after the size is 400 | "chunk extensions are ignored" |
| `RFC9112.R7_1_2_2`, `R7_1_2_3`, `RFC9110.R6_5_1_2` | 7.1.2, 6.5.1 | `read_chunked` | each trailer field line is checked as a header field line (400 otherwise) and discarded: no trailer field is retained, so none is merged into the header section | "trailer fields are checked and discarded and never merged into the header section" |
| `RFC9110.R6_2_5` | 6.2 | `render_message` | every response says `HTTP/1.1`, now that the chunked coding a recipient of that version must decode is decoded | "a chunked request body is decoded" |

Limits: the decoded body is bounded by `HttpLimits.body` (413), each chunk-size line and the
whole trailer section by `HttpLimits.head` (431). The bytes the decoder has consumed are
dropped as it goes, and chunk data its input buffer does not hold yet is appended to the body
read by read (`read_chunk_data`), so a chunk is never held twice: however the body is split
into chunks, the decoder holds the body and, besides it, input of less than three reads
(`READ_SIZE`, 64 KiB) and the chunk-size line being read (at most `HttpLimits.head`): consumed
bytes are dropped only once they reach one read, so as not to copy the buffer per chunk; the
read that completed the chunk-size line and the chunk; and the read that brings the CRLF after
the chunk. The test "one chunk of the whole body limit is decoded holding at most one read
besides the body" decodes one chunk of `HttpLimits.body` bytes and checks that the input kept
after it is at most one read; "adversary: a chunk read whole but for its LF keeps less than
three reads of input and the line" builds the worst case (196,598 bytes) and checks the bound.
The input buffer is moved, never copied, from chunk to chunk:
`http_chunked_adversary::the_chunk_decoder_copies_no_bytes_parameter_on_either_target` reads the
generated Rust and LLVM of the todo example and refuses a borrowed or copied input buffer, and
"adversary: the body limit in one-byte chunks is decoded in under 8 s" catches the copy's cost
on LLVM (on Rust the copy costs 2.4 to 3.8 s, under the bound). The bound counts values in use,
not memory: on LLVM a loop's arena is released only once it has grown past four times what it
keeps (`ignite.arena_due`), and Rust's `extend_from_slice` of a chunk slice builds a temporary
of less than two reads. Not bounded beyond that: the extension bytes over a body's chunks together, at most
`HttpLimits.head` per chunk, so a 1 MiB body in 1-byte chunks may carry up to 8 GiB of
extensions under the default limits, each read still under `idle_timeout`. `todo.rs`'s
`header_framing` case that expected 501 for a chunked POST now creates a todo from a chunked
body with an extension and a trailer (201) and adds a 400 and a 501 case. The claims count
`rfc9112: no (1 gaps, 0 failures, 0 inconclusive)` and `rfc9110: no (1 gaps, 0 failures, 0
inconclusive)` on both targets: `RFC9112.R2_2_1` and `RFC9110.R6_6_1_2` (story
`http-octet-head-and-date`).

**Adversary findings (story `http-chunked-adversary-findings`, 2026-11-07).** Two inline tests
kill the decoder's surviving mutants: "a chunk whose data ends in CR and another byte instead
of CR LF is 400" (the LF half of the check after chunk data) and "an unterminated chunk-size
line or trailer over the head limit is 431, not read until the peer closes" (`read_line`'s
length guard before the next read). PR C held a single large chunk twice, in the input buffer
and in the body, until the next compaction; the bound above is the one the decoder now keeps.
The first `read_chunk_data` read its borrowed input after copying it, so the buffer was copied
once per chunk; it now takes the buffer by move. The body limit in one-byte chunks, median of
three runs at load 21 to 32: 0.79 s Rust and 1.13 s LLVM on `b23f827e`,
2.39 s and 25.8 s with the copy, 0.60 s and 1.58 s after the move.
`RFC9110.R6_2_5` ("A server MUST NOT send a version to which it is not conformant") is restated
as `unsupported`: every response says `HTTP/1.1` while `RFC9112.R2_2_1`, a MUST of that version,
is a gap, so the claim cannot hold until story `http-octet-head-and-date` closes it. The claims
count `rfc9112: no (1 gaps, 0 failures, 0 inconclusive)` and `rfc9110: no (2 gaps, 0 failures,
0 inconclusive)` on both targets.

### 19.23 Streamed and binary responses, every reason phrase (story `http-server-responses`, 2026-11-07)

Before the story every response was a `Text` body framed by `Content-Length`, and `reason`
named 11 codes (a 403 went out as `HTTP/1.1 403 Unknown`). The shape adds fields and fns and
renames none:

```ignite,fragment
type Response {
  status: Int
  headers: List<(Text, Text)> = []
  body: Text = ""
  content: Content = Inline
}

enum Content { Inline, Binary(Bytes), Streamed }

type HttpApp<S> {
  state: S
  handle: fn(state: S, request: Request) -> Response requires [state.*]
  piece: fn(state: S, request: Request) -> IoResult<Option<Bytes>> requires [state.*] = no_piece
}

fn Response.bytes(status: Int, content_type: Text, body: Bytes) -> Response
fn Response.streamed(status: Int, headers: List<(Text, Text)>) -> Response
fn no_piece<S>(state: S, request: Request) -> IoResult<Option<Bytes>>
```

- `Inline` sends `body`, `Binary(bytes)` the bytes in place of `body`, framed by
  `inline_framing`: its own `Content-Length` except where RFC 9110 forbids content or that
  field. A status without content (1xx, 204, 304; `has_no_content`, the rule the streamed path
  uses) gets the head only, without `Content-Length` or body (RFC 9110 sections 8.6, 15.2,
  15.3.5, 15.4.5); a 2xx answer to `CONNECT` gets no `Content-Length`, its body as it is and
  the connection closed after it (`Connection: close`; RFC 9110 section 9.3.6), as its
  streamed form does. Until story `http-inline-no-content` (2026-11-07) only 204 was
  exempt, so `status_response(304)` sent `Content-Length: 13` and `Not Modified`, which a
  client does not read as part of a 304 and misreads as the start of the next response. A
  rejection (`reject`) and `render` go through `render_message`, which applies the rule for
  statuses without content; the CONNECT case needs the request. `Response.plain`,
  `Response.json` and `status_response` are unchanged byte for byte: `content` defaults to
  `Inline`, so a record literal without it still equals theirs.
- `Streamed`: the producer is a field of `HttpApp`, not of `Response`, because a closure that
  reads a captured handle is effectful and a fn type can name effects only on its own
  parameters (13.3, E017): `fn() -> Option<Bytes>` on `Response` could not read a pipe or a
  store. `piece` gets the state and therefore the authority `handle` has. `send_streamed`
  writes the head at once, then calls `piece(state, asked(request))` until `Ok(None)` and
  writes each non-empty piece as it comes. An `Err` of `piece` ends the session and closes
  the connection at once. What the client sees depends on the framing: a chunked body lacks
  its last chunk `0` and the empty trailer section, so the client sees it incomplete (RFC 9112
  section 7.1); a body that ends by closing the connection (HTTP/1.0, a 2xx answer to
  `CONNECT`) cannot be told from a complete one, since the close is its only end (RFC 9112
  section 6.3). Http has no way to signal the failure on such a body.
- Framing of a streamed body, `streamed_framing`: a status without content (1xx, 204, 304)
  gets the head only, without `Transfer-Encoding` or `Content-Length`, and `piece` is not
  called; an HTTP/1.0 request, or a 2xx answer to `CONNECT`, gets the pieces as they are and
  the connection closed after them (`Connection: close`, also when the request asked to keep
  it); otherwise `Transfer-Encoding: chunked`, one chunk per piece (lower-case hexadecimal
  size), the last chunk `0` and an empty trailer section, and the connection is kept as for any
  response. HEAD gets the head GET would get and no body.
- `reason` names every status code of RFC 9110 section 15 and the four of RFC 6585 (428, 429,
  431, 511); `Unknown` for any other, 306 and 418 included.

| row | section | satisfied by | behaviour | verified by |
|---|---|---|---|---|
| `RFC9112.R6_1_7`, `R6_1_8`, `R6_1_11` | 6.1 | `streamed_framing` (and `checked`, which still refuses an application `Transfer-Encoding`) | no `Transfer-Encoding` in a 1xx or 204 response, in a 2xx answer to `CONNECT` or to an HTTP/1.0 request | "Http chunks only an HTTP/1.1 response with content" |
| `RFC9112.R6_1_2` | 6.1 | `checked`, `send_streamed` | the chunked coding is applied once, by Http only | "an application Transfer-Encoding is 500" |
| `RFC9110.R6_5_1_1` | 6.5.1 | not applicable | Http generates no trailer field: a chunked response ends with an empty trailer section | |

Until this story those three rows of 6.1 held only because Http never sent a
`Transfer-Encoding`; the test that named them, "an application Transfer-Encoding is 500 and
Http applies no transfer coding", is renamed "an application Transfer-Encoding is 500" and
still verifies `R6_1_2`, `R6_1_4` and `R6_2_1`. The claims stay `rfc9112: no (1 gaps, 0
failures, 0 inconclusive)` and `rfc9110: no (1 gaps, 0 failures, 0 inconclusive)` on both
targets. Other tests: "a streamed response is sent in chunks as its pieces come" (the
producer waits on a pipe the test controls; the client reads the head and the first chunk
before the second piece exists, then a pipelined request is answered), "a binary body is sent
as its bytes" (0 to 255, `Content-Length: 256`), "every status of RFC 9110 has its reason
phrase" (with `HTTP/1.1 403 Forbidden` on the wire). Not bounded: a streamed body's length and
duration.

Story `http-inline-no-content` (2026-11-07, from the adversary review of PR 84) adds nine
tests. Four decide the inline rule above and failed before it on both targets: "a 304
answered inline sends no content, so a pipelined response is not misread", "a 1xx response
answered inline has no Content-Length", "a 2xx answer to CONNECT answered inline has no
Content-Length" (the adversary's), "a 2xx answer to CONNECT answered inline ends by closing
the connection" (the unit's: no keep-alive, the pipelined request unanswered, a 403 to CONNECT
still framed by `Content-Length`). Five cover branches of the streamed path no test decided:
"an empty piece is skipped and does not end a chunked body", "a streamed 304 is the head only
and the connection goes on", "a streamed 3xx answer to CONNECT is chunked", "a chunk size over
15 has its hexadecimal digits most significant first", "an error of piece ends a chunked body
before its last chunk and the connection with it". No claim changes: the catalogue of RFC 9110 holds no
requirement of sections 8.6, 9.3.6 or 15.

## 20 Slice 16: language gaps (wave A)

**Status: contract written 2026-11-04. The independent review (`docs/slice16-review.md`, 4
blockers, 5 should-fix, 14 notes) is resolved in this text; its "Resolution" section maps each
finding to the place that resolves it. Implemented 2026-11-04 to 2026-11-05 in units U1 to U6
(20.10); every row of 20.0 has a dated row in `STATUS.md`.** The rows of 20.0 are
required checks, not results.

Sources:

- `docs/vision.md` §4 row "A language gaps" and row "C actors P1 to P3" (which needs "A
  (last-use moves), T1"), §4.1, §5, §6, §7 (T1, T2, T3, T4), invariants 3, 7, 8 and 9.
- `docs/friction.md` rows 1, 2, 4, 7, 8, 9, 10, 12, 13, 15 and 17; `docs/actors.md` R1, §3.3
  and the P1 and P4 rows; `docs/protocols.md` §5.2; `docs/syntax.md` P9 and R7; `docs/llvm.md`
  §3, §3.1 and §4.
- This file: sections 3, 6, 8, 13 (slice 9: 13.3 closures, 13.4 boxing, 13.7 ranges), 14.3
  (`Map`), 14.8 (T1), and 19.0 to 19.18 (the structure this section follows).

The base of the slice is commit `7ab3f7c`, the slice-15 branch head; it will be rebased onto the
merge of slice 15 into `main`, and the before tree of 20.12 is then that merge. Everything in
this section cites files and symbols, not lines (vision T3). Where `docs/syntax.md`,
`docs/actors.md` or `docs/vision.md` disagree with this section, this section wins and records
the point in 20.8.

Most of the wave-A row holds already: slice 9 and slice 10 delivered it, and 20.1 cites the test
for each item. The slice adds what is missing:

- last-use moves (actors R1): a frontend analysis in `ignite-types` that marks the arguments,
  initializers and results that may take a place's value without a copy, and the parameters a
  callee consumes; both backends drop the copy there (20.3);
- calls of fn-typed expressions, `adder(1)(2)`, `fs[i](x)`, `(|x: Int| x * 3)(2)` (20.4);
- rest patterns in list patterns, `[_, _, ..]`, `[first, ..rest]`, `[.., last]`, as `docs/syntax.md`
  P9 proposes them, with its code E076 (20.5);
- a both-target fixture for every item that holds, and the records.

### 20.0 Acceptance

Every row is a required check. "Exit 0, silent" means exit status 0, empty stdout and empty
stderr. A test name is `<file>::<test>` in its crate's single integration-test binary
(`cargo test -p <crate> --test main <file>::<test>`). "Both targets" means `--target rust` and
`--target llvm`. The fixtures named `slice16/…` are under `crates/ignite/tests/fixtures/slice16/`.
`STATUS.md` gets one dated row per row of this table (row 27).

| # | check | command | required observation | unit |
|---|---|---|---|---|
| 1 | the moves analysis marks R1's shape | `cargo test -p ignite-types --test main moves::a_var_passed_and_reassigned_moves` | the test types the source of `run` and `step` of 20.1's probe p07 (inline in the test) and collects every expression for which `TypedProgram::moves` is true, as (line, col). The list is exactly the `s` in `s = step(s, i)`, the `state` in `var next = state` and the `next` that is `step`'s result; `consumed(&Callee::Fn(step), 0)` is true, `consumed(&Callee::Fn(step), 1)` false | U1 |
| 2 | a read after the move point keeps the copy | `moves::a_later_read_keeps_the_copy` | for each case of 20.3.2's list "no move" (a read later in the block, a read in the next loop iteration, another argument or the receiver of the same call mentioning an overlapping place, among them `take(s, if c { s = fresh(); 0 } else { 0 })`, a use inside the arms of a `match` whose scrutinee is rooted at the variable, a use in the body of a `for` whose iterable is rooted at it, an in-place change after the call) `moves` is false at the argument. With a local `s`, in `if c { consume(s) } else { s.count }` as a fn body's result, `moves` is true at `consume`'s argument. In `consume(t); if c { var t = a } else { var t = b }; t.count` (inner `t`s shadow) `moves` is false at `consume`'s argument | U1 |
| 3 | positions and places | `moves::every_move_position_moves` and `moves::places_move_through_fields_only` | each move position of 20.3.1 with a dead place is marked; a receiver of a builtin method, a `match` scrutinee, a `for` iterable, an operator operand, a closure capture and the initializer of a pattern `let` are not; a `spawn` argument is marked (`moves::spawn_arguments_move_at_their_last_use`, 20.3.1). `b.f` with `b.f` dead after is marked; `xs[0]`, a boxed field (13.4) and a place rooted at a pattern binding are not. Field paths are disjoint: in `a.state = a.handle(a.state, i)` the argument `a.state` is marked; in `Step { state = st.state, out = publish(st.out) }` with `st` dead after, `st.state` is marked; in `answer(state, reply, state.table.values())` `state` is not. No expression whose type is `Int`, `Byte`, `Bool`, `Unit`, a quantity, a handle type (but for 20.3.1's scope tail and `spawn` argument) or a fn type is marked | U1 |
| 4 | consumed parameters are a fixpoint | `moves::consumed_is_a_fixpoint` | in a module with `a(x)` calling `b(x)` at its last use, `b(y)` changing `y` in place and calling `a`, and `c(z)` only reading `z`: `a` and `b` consume their parameter, `c` does not. `d(m)` whose only change is `m.put(k, v)` consumes `m`; `e(p)` that passes `p.items` to a consumed parameter on every path consumes `p`. `pick(a, b, c) = if c { a } else { b }` consumes neither `a` nor `b` (each is moved on one path only, 20.3.3), and neither does `unwrap_or` in `std/option.ignite` consume `fallback`. A program fn whose last use of a local passes it to a std fn that consumes its parameter moves it; `ModuleTypes::consumed` of the std module lists that fn | U1 |
| 5 | closure bodies are not entered | `moves::closure_bodies_never_move` | no expression inside a closure body is marked, and creating a closure that captures `s` counts as a read of `s` | U1 |
| 6 | the checker accepts calls of fn-typed expressions | `cargo test -p ignite-check --test main fixtures::` | fixture `crates/ignite-check/tests/fixtures/valid_expression_calls` (`adder(1)(2)`, `fs[1](10)`, `(|x: Int| x * 3)(2)`, `(h.run)(2)`, `pick(true)(3)`): exit silent. `nope(1)(2)` in fixture `e003_unknown_variable` gives one E003 `unknown name \`nope\`` and no `not callable` | U2 |
| 7 | the typer types them | `cargo test -p ignite-types --test main fixtures::` | fixture `valid_expression_calls` (the same source with a checked use of each result) is silent. `e040_not_callable`: `(1)(2)`, `xs[0](1)` with `xs: List<Int>` and `"a"(1)` give one E040 each, `type mismatch: this expression has type \`<T>\` and cannot be called`. `e043_expression_call_arity`: `adder(1)(2, 3)` gives one E043. `valid_expression_calls` also holds `hs[0](conn)` with `hs: List<fn(c: PipeStream) -> Int requires [c.write]>` and `(h.send)(conn)` with an effectful fn field, inside a fn that `requires [conn.write]`: silent, as `h.send(conn)` is today. `e042_effectful_expression_callee`: `pick_sender(true)(conn)`, with `pick_sender` returning that effectful fn type, gives one E042, the message of 20.7; the same fn without `conn.write` in `requires` gives the E042 and no E016. `e042_closure.expected` and `e017_closure.expected` are unchanged | U2 |
| 8 | closures already hold, both targets | `cargo test -p ignite --test main slice16::closure_shapes_pass_on_both_targets` | `ignite test slice16/closures.ignite --target all` exits 0, every test passed on both targets, 0 failed. The fixture holds the closure tests of 20.1's probes p02, p08 and p04b: captures by copy, a var captured and then changed (the closure keeps the old value), a generic fn returning a closure that captures a `T`, a closure returning a closure, a closure in a record kept in a `Store`, a closure passed to a spawned task, a closure that writes through a captured `PipeStream` called through its `let` binding, and a record holding `handle: fn(S, M) -> S` stepped in a loop (the actors §5.1 shape) | U2 |
| 9 | Rust: R1 is linear | `cargo test -p ignite-rustgen --test main slice16::state_threading_is_linear` | the test builds `slice16/moves_scale.ignite` once on the Rust target (dev profile) with the helpers `crates/ignite-rustgen/tests/slice9.rs` uses (`common::lower_tests` with the repository's `std/` and cargo), then runs the test executable twice, with the exact filters "50000 steps through a call" and "200000 steps through a call". Both pass. The time of each is the `finished in` figure of the runner's `test result:` line; the 200,000-step time is under 8 times the 50,000-step time (linear is about 4; p07's quadratic growth is about 20 per factor 4) | U3 |
| 10 | LLVM: R1 is linear | `cargo test -p ignite --test main slice16::llvm_state_threading_is_linear` | the same as row 9 on the LLVM target (dev, `-O0`), through `ignite test slice16/moves_scale.ignite --target llvm <filter>`: a warm-up round runs both filters, then 5 rounds run them one after the other, and each filter's fastest `finished in` figure counts; the 50,000-step time counts as at least 0.05 s, five steps of the runner's 0.01 s resolution (story `apply-test-deadlines`: alone the two take 0.00 to 0.02 s, and one slow run under load reached the former bound of 0.08 s) | U4 |
| 11 | Rust: moves keep value semantics | `cargo test -p ignite-rustgen --test main slice16::fixtures_pass` | with the same helpers as row 9, the Rust target builds `slice16/moves.ignite` and `slice16/calls.ignite` (both import std) with rustc without an error (in particular no E0382, E0505 or E0507) and every test of both passes | U3 |
| 12 | LLVM: moves keep value semantics | `cargo test -p ignite --test main slice16::llvm_fixtures_pass` | `ignite test slice16/moves.ignite slice16/calls.ignite --target llvm`: exit 0, every test passed, built against the debug runtime that overwrites released memory with 0xDB (`docs/llvm.md` §4) | U4 |
| 13 | Rust golden | `cargo test -p ignite-rustgen --test main golden::last_use_moves_and_expression_calls` | `crates/ignite-rustgen/tests/golden/slice16.ignite` lowers to exactly `slice16.rs`. In it `step` (which moves its parameter into `var next`, as p07's does) is declared `fn step(state: State, x: &i64) -> State`, the call in `run` is `step(s, &i)` with no `.clone()` on `s`, a read-only aggregate parameter stays `&T`, a call to a consumed parameter whose argument is read later passes `.clone()`, a role called with a moving argument receives it bare, and `adder(1)(2)` is a call of the call's result | U3 |
| 14 | LLVM golden | `cargo test -p ignite-llvm --test main golden::last_use_moves_and_expression_calls` | `crates/ignite-llvm/tests/golden/slice16.ignite` (self-contained, no std, as the other LLVM goldens) lowers to exactly `slice16.program.ll`. The test also asserts on the text: in `run`, no `copy.` call lies between the loop head and the call of `step`, and the call passes the slot of `s` (the loop's release block keeps its `copy.State`, `docs/llvm.md` §4); in `run_read_after`, a `copy.State` call precedes the call of `step`; `read_only_parameters_are_borrowed` and `loops_release_their_region` still pass | U4 |
| 15 | codegen revision | read `apply::plan::CODEGEN_REVISION` in `crates/ignite/src/apply/plan.rs` | after W3 one step past its value at the slice base, one step for U3 and U4 together (U4 does not edit `plan.rs`); after W4 one step further | U3, U5 |
| 16 | rest pattern syntax | `cargo test -p ignite-syntax --test main fixtures::` (`fixtures::fixtures_match_expected_diagnostics`) | fixture `valid_rest_patterns` (`[..]`, `[_, _, ..]`, `[first, ..rest]`, `[.., last]`, `[a, .., z]`, `[[a, ..], ..]`, `[] \| [_, ..]` in one arm, a rest on `Bytes`) parses, silent. `e076_rest`: `[.., x, ..]`, `[..a, ..b]` and `[..init, last]` give one E076 each, with the two messages of 20.7. `e001_rest_outside_list`: `(a, ..)`, `Some(..)` and a bare `..` arm give one E001 `expected pattern, found \`..\`` each | U5 |
| 17 | rest pattern types | `cargo test -p ignite-types --test main fixtures::` | `valid_rest_patterns` is silent, among it `[] => …`, `[_, ..] => …` (exhaustive on `List<Int>` and on `Bytes`) and `[first, ..rest]` passing `rest` where `List<Int>` is expected. `e040_rest_binding`: `rest` of a `List<Int>` passed where `Int` is expected, and `rest` of a `Bytes` where `List<Int>` is expected: one E040 each. `e046_rest`: `[] , [_, _, ..]` gives E046 `match does not cover \`[_]\``; `[x, ..]` alone gives `match does not cover \`[]\``; on `List<Bool>`, `[]`, `[_]`, `[_, _]`, `[_, true, ..]`, `[.., false, _]` gives `match does not cover \`[_, false, true, _]\`` (the case where prefix and suffix overlap below length P + S, 20.5); on `List<Bool>`, `[true]`, `[]`, `[_, _, ..]` gives `match does not cover \`[false]\``. `valid_rest_patterns` holds the same five `List<Bool>` arms plus `[_, _, _, _, ..]`, silent. `e046_patterns.expected`'s list case reads `match does not cover \`[_, _, ..]\`` (20.7); every other line of it is unchanged | U5 |
| 18 | rest patterns run | `cargo test -p ignite-rustgen --test main slice16::fixtures_pass`, `cargo test -p ignite --test main slice16::llvm_fixtures_pass` | both now also build `slice16/rest.ignite` and every test passes: lengths 0 to 4 against `[]`, `[_]`, `[_, _, ..]`; `[first, ..rest]` binding a copy (pushing to a `var` bound to `rest` leaves the scrutinee unchanged); `[.., last]`; `[a, .., z]` on two and on three elements; nested rests; the rest on `Bytes`; protocols §5.2's `([_, _, ..], _, _)` inside a tuple | U5 |
| 19 | rest goldens | `cargo test -p ignite-rustgen --test main golden::rest_patterns`, `cargo test -p ignite-llvm --test main golden::rest_patterns` | `rest.ignite` in each crate's `tests/golden/` lowers to exactly `rest.rs` and `rest.program.ll` | U5 |
| 20 | the evaluator agrees on rest patterns | `cargo test -p ignite --test main eval::` | `eval_equivalence.ignite` gains a `rest_` family (at least four fns); `fixture_covers_every_family` lists it; `evaluator_agrees_with_the_rust_target` and `evaluator_agrees_with_the_llvm_target` pass | U5 |
| 21 | every wave-A fixture on both targets through the CLI | `cargo test -p ignite --test main slice16::fixtures_pass_on_both_targets` | `ignite test` of `slice16/holds.ignite`, `calls.ignite`, `moves.ignite` and `rest.ignite`, each with `--target all`: exit 0, every test passed on both targets, 0 failed | U6 |
| 22 | the items that hold | same, `holds.ignite` | the fixture holds one test per "holds" verdict of 20.1 (unwrap_or and `!=` on `Option`, a range pattern on `Byte`, raw text and raw byte literals, a `\u{..}` escape, an interpolation holding a string literal, tuple fields, `break` as a match arm's value, a boxed recursive enum, `Map` operations in key order, an associated fn), taken from probe p01 | U6 |
| 23 | the evaluator agrees on calls and moves | `cargo test -p ignite --test main eval::` | `eval_equivalence.ignite` gains a `call_` family (expression callees, at least three fns) and a `move_` family (fns whose arguments move, with a read of the moved variable's old value through a copy made before, at least three fns); both agreement tests and `fixture_covers_every_family` pass | U6 |
| 24 | the workspace | `cargo test --workspace` after each unit | passes. Golden changes in U3 and U4 only as 20.3.4 and 20.3.5 allow (a parameter's passing mode, a dropped copy at a move, a `.clone()` or `copy.` at a call to a consumed parameter, renumbered metadata); in U1, U2 and U6 no golden changes; in U5 only the rest goldens are new | every unit |
| 25 | inline tests | `task test-ignite` | passes on both targets; the per-target count equals the base's plus the slice's std additions (none planned) | every unit |
| 26 | end-to-end unchanged | `cargo test -p ignite --test main -- terminal:: todo:: failover:: slice9:: slice10_llvm::` | passes; `crates/ignite/tests/terminal.rs` and `todo.rs` unchanged against the base | U3, U4, U6 |
| 27 | records and gate | `task gate` on the slice branch; read `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/vision.md`, `docs/friction.md`, `docs/actors.md`, `docs/syntax.md` | exit 0. Every row of this table has a dated `STATUS.md` row naming its command. `AGENTS.md` has a "Slice 16" section with the merge gate's time, counts and exit, and its `crates/ignite-types/` layout row names the moves analysis. `PLAN.md` has a row 16 that says done. `docs/vision.md` §4.1 has a row "16" for wave A, and §5's rows of wave A say where each landed. The stale points of 20.8 are edited where 20.8 says, among them the title of this file. `docs/performance/16/` holds the records of 20.12 | U6 |

### 20.1 Current state and verdicts (at `7ab3f7c`)

Probes are `.ignite` files run with the worktree's `target/debug/ignite test FILE --target all`
or `ignite check FILE` on 2026-11-03; their sources are folded into the fixtures of 20.0 (p01 into
`holds.ignite`, p02, p04b and p08 into `closures.ignite`, p03 into `calls.ignite`, p06 into
`rest.ignite`, p07 into `moves_scale.ignite`).

| item (vision §4 row A) | verdict | evidence |
|---|---|---|
| closures capturing locals (`friction` 9, 13; actors P4) | holds; one gap in, two deferred | captures by copy on both targets: tests "closures capture by copy" and "store update with a closure is atomic" of `crates/ignite-rustgen/tests/slice9/features.ignite`, run by `slice9::slice9_features_pass` (Rust) and `slice9::rust_target_features_pass_on_llvm` in `crates/ignite/tests/slice9.rs` (LLVM); the closure in `patch` of `examples/todo/todo.ignite`, exercised by `crud_flow` in `crates/ignite/tests/todo.rs` on both targets. Probes p02, p08 and p04b (20.0 row 8's shapes) pass on both targets. Gaps: (a) a call whose callee is an expression of fn type is E003 `not callable` (p03: `adder(1)(2)`, `fs[1](10)`, `(|x: Int| x * 3)(2)`): **in** (20.4). (b) An unannotated closure parameter whose expected type holds a type parameter only the closure body decides, the shape of actors P4's `call(|r| Get(id, r))`, is E042 twice (p05: `cannot infer type parameter \`T\` of \`call\``, `cannot infer the type of closure parameter \`r\``); annotated, `call(\|r: Reply<Text>\| Get(id, r))` passes (p05b): **deferred** (20.13). (c) A closure that captures a capability and uses it (`\|t\| s.write_all(..)`) passed where a fn type is expected is E017 (p04: `effectful closure (requires [s.write]) where the pure fn type … is expected`); called through its `let` binding it passes (p04b): **deferred**, E017 stays the rule (20.4.3) |
| `Option.unwrap_or`, `!=` on `Option` (7) | holds | std/option.ignite test "unwrap_or gives the value or the fallback" (`task test-ignite`, both targets); `!= None` in `contains` of `std/text.ignite`; p01 |
| rest patterns `..` (protocols §5.2) | missing; **in** | p06: `[_, _, ..] => "many"` is E001 `expected pattern, found \`..\``. A list column is covered only by `_` or a binding: p09's `[] => 0, [x] => x` is E046 `match on \`List<Int>\` does not cover every value; add a \`_\` arm or a binding arm` (`crates/ignite-types/src/exhaustive.rs`) |
| ranges in patterns (8) | holds | test "range patterns" of `slice9/features.ignite` (both targets as above); fixture `e046_ranges` of `crates/ignite-types/tests/fixtures`; p01 |
| raw strings (2; also 1 and 12, vision §5) | holds | the std/json.ignite tests "encode and parse round trip" and "the accessors read members of the right kind" (`task test-ignite`, both targets); p01 (`r#"…"#`, `br"…"`) |
| tuple field access `.0` (10) | holds | test "record update and tuple fields" of `slice9/features.ignite`; p01 |
| `break` as an expression (4) | holds | test "break and continue expressions" of `slice9/features.ignite`; p01 (`None => break` as a `match` arm inside `let`) |
| recursive enums by boxing (15) | holds | test "boxed recursion" of `slice9/features.ignite`; section 13.4; p01 |
| static fns on records (17, vision §5) | holds | test "associated fns" of `slice9/features.ignite`; section 13.8 |
| `Map<K, V>` (design §3.3) | holds | tests "map literal and lookups", "map mutation has value semantics", "for over a map visits pairs in key order", "maps compare structurally" of `crates/ignite-rustgen/tests/slice10/features.ignite`, run by the rustgen `slice10` tests (Rust) and `crates/ignite/tests/slice10_llvm.rs` (LLVM); section 14.3; p01 |
| last-use moves (actors R1) | missing; **in** | p07, state threaded through a call, `s = step(s, i)` with `step` pushing to `state.items`: Rust 0.04 s for 10,000 steps and 0.84 s for 40,000; LLVM 0.36 s and 7.40 s. Four times the steps cost about 21 times the time on both targets: every step copies the whole state. The same loop with the push inline takes 0.01 s for 40,000 steps. Rust parameters are `&T`, copied on entry when the body changes them (section 6, `mutated_names` in `crates/ignite-rustgen/src/lib.rs`); LLVM passes a fresh copy to a parameter the body modifies (`docs/llvm.md` §3, `region::modified` in `crates/ignite-llvm/src/region.rs`) |
| T1 failure rule (vision §7) | holds | section 14.8; `failover::a_failing_session_does_not_end_the_service` (`crates/ignite/tests/failover.rs`), `slice10::crashd_survives_failing_handlers` (`crates/ignite-rustgen/tests/slice10.rs`) |

Facts the design relies on:

| fact | where |
|---|---|
| A call's callee is any postfix expression in the grammar; `ignite-check` reports E003 `not callable` for every callee that is no name and no path head, and `ignite-types` records `Callee::Local` only for a name | `call` in `crates/ignite-check/src/lib.rs` (its `ExprKind::Field` arm without a path head and its `_` arm); the `ExprKind::Name` branch of the call typing in `crates/ignite-types/src/typer.rs` (grep `Callee::Local`) |
| `ignite-check` already gives a declared type to a name, a field, a method result, `?`, a tuple field, a closure and an index of a `List` or `Bytes` (not to a call's result), and reads an effectful fn type's effects from it; so `h.send(conn)` with an effectful fn field, and `let f = hs[0]; f(conn)`, check today | `static_type`, `fn_value_effects` in `crates/ignite-check/src/lib.rs` |
| Rust parameters: a protocol role's are `mut name: T` by value; every other parameter is `&T` (scalars too), cloned on entry when the body changes it or its type is Copy; a role argument is passed as a value (a place with `.clone()`) | `function_line`, `args` in `crates/ignite-rustgen/src/lib.rs`; section 6 |
| The in-place changes that make a parameter owned today: on LLVM `region::mutates` (`push`, `append`, `clear`, `Map.put`, `Map.remove`, the `Store` mutators, assignments); on Rust `mutated_names` with `mutates_receiver` (`push`, `append`, `clear`, `Map.put`, `Map.remove`, assignments) | `crates/ignite-llvm/src/region.rs`, `crates/ignite-rustgen/src/lib.rs` |
| The backends and the evaluator already lower a `Callee::Local` call by evaluating the callee as an expression | the `_` arm of `call` in `crates/ignite-rustgen/src/lib.rs`; the `Some(Callee::Local)` arms in `crates/ignite-llvm/src/lower.rs` and `call_expr` in `crates/ignite-eval/src/eval.rs` |
| The typer's effect pass maps an effectful fn type's `requires` onto a call's arguments for any `Callee::Local` callee; an effectful operation at a span `ignite-check` did not record as an effect site is E042 | `Callee::Local` arm in `crates/ignite-types/src/typer/effects.rs`; `effect_site` in `typer.rs` |
| A closure parameter takes its type from an annotation or from the expected fn type; a type that still holds a type variable is E042 | `closure` in `typer.rs` |
| Pattern grammar: `'[' (pattern (',' pattern)*)? ']'` matches a list (or `Bytes`) of exactly that length | section 3; `PatternKind::List` in `crates/ignite-syntax/src/ast.rs` |
| `PatternKind::List` is matched in `ignite-syntax` (parser), `ignite-check`, `ignite-types` (`typer.rs`), `ignite-eval`, `ignite-rustgen` and `ignite-llvm` (`lower.rs`, `region.rs` `pattern_binds`); `exhaustive.rs` has no list arm, so a list pattern falls to its catch-all rule | grep `PatternKind::List` |
| Fn values: Rust `Arc<dyn Fn(..) -> R + Send + Sync>`; LLVM `{ ptr code, ptr env }`, environment block in the arena, captures copied at creation, glue ops 0 to 2 | section 13.3; `docs/llvm.md` §3.1 |
| A call through a fn value passes copies on LLVM, owned values on Rust | `docs/llvm.md` §3; `fn_value` in `crates/ignite-rustgen/src/lib.rs` |
| `ModuleTypes` derives `Default`; a new query field needs a line in `query_snapshot` | `ModuleTypes`, `query_snapshot` in `crates/ignite-types/src/lib.rs` |
| `CODEGEN_REVISION` is `ignite-codegen/0.1.0-4` at `7ab3f7c` | `crates/ignite/src/apply/plan.rs` |
| E076 is free: `docs/syntax.md` §8.3 proposes it for P9 inside T4's range E070 to E079 for that proposal; no code uses it | `docs/syntax.md` §8.3; grep of `crates/` and `docs/slice.md` |
| The test runner's default timeout is 60 s | `DEFAULT_TIMEOUT` in `crates/ignite-io/src/test.rs` |

### 20.2 Scope and non-goals

In scope: last-use moves (20.3), calls of fn-typed expressions (20.4), rest patterns in list
patterns (20.5), both-target fixtures for every item that holds, and the records.

Not in scope (20.13 gives the reasons): effectful closures passed as fn values; inferring a
closure parameter from the closure body; capture by move; moves of pattern bindings, `for`
elements, closure parameters and anything inside a closure body; rest patterns in tuples and
variant payloads; the open rows of `docs/friction.md` that are not on the wave-A row; a
persistent `Table` (actors R1 "Open").

### 20.3 Last-use moves (D1, D2, D3)

A move is an optimisation that must change no observable value: a program means the same with
and without it, and the evaluator, which copies by construction, stays the reference (row 23).
The analysis runs once in `ignite-types`, after typing a module, in a new module
`crates/ignite-types/src/moves.rs`; both backends read its result through `TypedProgram`
(20.6). It reports no diagnostic.

#### 20.3.1 Tracked variables, move positions and places

Tracked variables: a single-name `let` or `var` binding, and a parameter of a fn, an associated
fn or a protocol role, whose type is an aggregate or a type parameter: not `Int`, `Byte`, `Bool`,
`Unit`, a quantity, a handle type (invariant 3) or a fn type (copying a fn value is a reference
count on Rust; on LLVM its environment copy is left for later, 20.13). Each binding is its own
variable: a shadowing `let` or `var` starts a new one and neither reads nor reassigns the outer
one; the analysis works on resolved bindings, not on names. Pattern bindings, `for` elements and
closure parameters are not tracked.

Move positions, where an aggregate value is taken:

- an argument of a direct call (a named fn, an associated fn, a role, or a UFCS call of a user
  fn, whose receiver is argument 0) to a parameter the callee consumes (20.3.3);
- an argument of a call through a fn value;
- the element argument of the builtins `push` and `put`;
- the initializer of a single-name `let` or `var`, and the right side of an assignment (not the
  initializer of a pattern `let`, whose bindings point into it on LLVM);
- the operand of `return`, and the result expression of a fn body or of a block whose value is
  used;
- a list element, a record literal's field value, the base of a record update, a variant,
  `Some`, `Ok` or `Err` payload, a tuple element;
- an argument of `spawn f(args)`, the receiver of a spawned fn call included (the task takes
  every argument; 2026-11-06, `story:llvm-spawn-pattern-bound-streams`). A handle binding moves
  there at its last use, not only at its scope's tail (the move hands the binding's share to
  the task, which the task drops when it ends); a place rooted at a parameter never moves there,
  so a `spawn` makes no parameter consumed. For 20.3.2 a spawn argument is held like a borrowed
  one, since LLVM's thunk reads a copied aggregate in place after the later arguments ran.
  On Rust a moved argument is emitted bare into `__spawn_N`, so the spawning body keeps no
  clone; on LLVM a moved aggregate goes into the thunk's environment as an owned value instead
  of its place's pointer, and a handle is a scalar, so the mark changes nothing there.

Not move positions: a receiver of a builtin method, a `match` scrutinee, a `for` iterable, an
operator operand (also `==` and `+` on `Text`), an `assert` operand, a closure capture.

A place in a move position moves when it is a tracked variable `v` or a field chain `v.f.g`
rooted at one, and that place is dead after the position (20.3.2). A chain through an index
(`v[i]`), through a boxed field or payload (section 13.4), or rooted at an untracked binding never
moves.

**Owned bindings (2026-11-06, `story:moves-for-pattern-bindings`).** This replaces the sentence
"Pattern bindings, `for` elements and closure parameters are not tracked" above, 20.0 row 5's
"no expression inside a closure body is marked" (its test is now
`moves::closure_bodies_move_only_their_own_bindings`), and those three of 20.13 item 4:

- A binding of a `match` arm, a pattern `let` or a `for` pattern, and a `for` element, is
  tracked when it is owned: the value it is bound from (scrutinee, initializer, iterable) is
  fresh. A value is not fresh when it is a name with a value (a binding or a constant) or reaches
  one through a field, a tuple field, an index, `?`, `await`, the receiver of a builtin method or
  a block's last expression; a call of a fn, a role or a variant, a literal, an `if` or a `match`
  is fresh (their values are copies or moves). The type rule is the variables'. A binding of a
  value that is not fresh is borrowed from that place (on LLVM it points into it, on Rust it is a
  reference or a copy): it never moves, and is copied where a value is taken. A binding inside a
  list pattern is never owned.
- A pattern binds its names afresh each time it runs: seen backward, their places die there,
  so a mention in the next loop iteration does not keep this iteration's last use.
- A closure body is a body of its own, not part of the body that creates it. Its parameters are
  its variables under the parameter rule: one moves only when the closure consumes it, by
  20.3.3 applied to the closure body (changed in place on some path, or moved on every path);
  `TypedProgram::closure_consumed` gives the set. A capture is untracked and never moves (20.13
  item 3). The LLVM target lends a caller's place to a closure parameter only when the closure
  neither modifies nor consumes it (`Lower::known_borrows`, `docs/llvm.md` §3.1); on Rust a
  closure parameter is a value.
- A handle stays shared and is never moved as an aggregate, with two exceptions. A `let`/`var`
  or owned binding of a handle type moves as the value of the block or `match` arm that binds
  it (directly, or as the value of an `if` branch or `match` arm that is that value), where its
  scope ends. The move hands on the share the binding drops there, so nothing closes earlier
  than with a copy; on LLVM a handle is a scalar and the mark changes nothing. Such a binding
  also moves as a `spawn` argument at its last use (above): the task holds the share from then
  on, so the stream closes when the task ends or fails, not when the spawning body ends
  (`stream_ownership::a_spawn_in_a_test_body_hands_the_pattern_bound_end_over`, both targets).
  That holds for a spawn inside an `if` or `else` branch or a `match` arm that is the binding's
  last use, also in a branch that owns streams of its own, and for a spawn followed by a binding
  that shadows the name: a `let` in the same block or in a later branch, a later `match` arm,
  `for` loop or closure parameter
  (`stream_ownership::a_spawn_in_a_branch_or_before_a_shadowing_binding_closes_the_end_on_both_targets`,
  `…_in_a_match_arm_or_else_branch_…`, `adversary_nested_spawns_close_the_end_on_both_targets`,
  `adversary2_later_bindings_of_the_name_agree_on_both_targets`);
  a spawn inside a loop body does not hand over. On LLVM both whether the block owns the binding
  (`own::owned`) and where `own::handovers` finds these spawns count a mention only where the
  name still denotes that binding; the
  hand-over's release leaves a tombstone in the task's owner list, so the marks of the blocks
  in between stay valid and each still closes its own streams when it is left. Named `spawn`
  arguments are evaluated in source order and passed in parameter order on both targets
  (`stream_ownership::named_spawn_arguments_in_another_order_build_and_pass_on_both_targets`).
  A handle parameter is never consumed by a move.

Checks: `moves::owned_pattern_bindings_move`, `moves::owned_for_elements_move`,
`moves::closure_parameters_move_when_consumed`, `moves::handles_move_only_at_their_scope_tail`,
`moves::spawn_arguments_move_at_their_last_use`,
`moves::a_spawned_ufcs_call_moves_its_receiver_at_its_last_use`,
`moves::a_later_spawn_argument_does_not_move_a_place_an_earlier_one_reads`
(`cargo test -p ignite-types --test main moves::`), and `slice16/moves_tails_owned.ignite`
on both targets (`slice16::owned_binding_tails_move_on_both_targets` in `crates/ignite`,
`slice16::fixtures_pass` in `ignite-rustgen`).

#### 20.3.2 Liveness

Liveness is per place: a tracked variable `v` or a field path `v.f.g` rooted at it. Two places
overlap when one is a prefix of the other (`v` and `v.f`, `v.f` and `v.f.g`); `v.f` and `v.g`
do not. A place is dead after a position when no path from the position mentions an overlapping
place before that place, or a prefix of it, is reassigned as a whole (`v.f = e` kills `v.f`;
`v = e` kills every place of `v`) or the fn exits. Evaluation order is section 6's: arguments
left to right, record fields in source order, then the base. Paths follow `if`, `match` arms,
loops with their back-edges, `break`, `continue`, `return` and `?`. A mention is any read of an
overlapping place, an in-place change through one (`v.items.push(x)`, `v.f = e` for a longer
`v.f.g`), and the creation of a closure that captures `v`.

No move, even when the rule above would allow one:

- inside the arms of a `match` whose scrutinee is a place overlapping the moved one, and inside
  the body of a `for` whose iterable overlaps it: on LLVM their pattern bindings point into it
  (`docs/llvm.md` §4), on Rust they borrow it;
- at an argument when another argument of the same call or of an enclosing call, the receiver
  of an enclosing builtin, the left operand of an enclosing operator or the receiver of an
  enclosing index mentions an overlapping place at all: a read, an in-place change, or an
  assignment to it or to a place rooted at it. A borrowed parameter or an operand read in place
  would see the moved value change (`f(s, s)`, `f(s.count, s)`, `look(s, wipe(s))`,
  `s.items.at(wipe(s))`, `s == bump(s)`, `s.items == bump(s).items`, `xs[reset(xs)]`), and on
  LLVM a later argument that assigns `s` would overwrite the slot the callee owns
  (`take(s, if c { s = fresh(); 0 } else { 0 })`). For an enclosing call, operator or index the
  operands that count are those evaluated before the one the move is in and read in place after
  it ran: a borrowed argument, a builtin or borrowed UFCS receiver, the receiver of a fn-field
  call, the left operand of a binary operator when it is an aggregate (an `Int`, `Bool` or
  other scalar left operand is a value once evaluated, and `&&` and `||` branch on theirs before
  the right one runs), the receiver of an index. An argument of an enclosing call in a move
  position is a value once evaluated (moved or copied, 20.3.5), and a later one is kept by
  liveness;
- anywhere inside a closure body (the analysis does not enter closure bodies).

Examples. R1's shape: in `while i < n { s = step(s, i); i = i + 1 }` the `s` argument moves,
because `s` is reassigned before any mention. In `let t = step(s, 1); t.count + s.count` it does
not. A handle loop's `a.state = a.handle(a.state, i)` moves `a.state`: the receiver path
`a.handle` does not overlap it, and `a.state` is reassigned after the call. A
`Step { state = st.state, out = publish(st.out) }` with `st` dead after moves `st.state`, and
`st.out` too when `publish` consumes its parameter.
`answer(state, reply, state.table.values())` does not move `state` (another argument mentions
`state.table`); `let values = state.table.values(); answer(state, reply, values)` does.

#### 20.3.3 Consumed parameters

A parameter `p` is consumed when either holds:

- its fn changes it in place on some path: an assignment to `p` or a place rooted at it, or a
  `push`, `set`, `append`, `clear` (`List`, `Bytes`), `put` or `remove` (`Map`) or a `Store`
  mutator through a place rooted at it. The builtins that change their receiver are one table,
  `ignite_check::IN_PLACE` (receiver type, method, and whether the call changes the value or
  the state behind a `Store` handle), which the checker reads for E006 and
  `ignite_types::ownership` resolves to builtins; the names a body changes
  (`ownership::changed_in_block`) and the variable a receiver is rooted at
  (`ownership::place_root`, `memory_root`) are decided there once, for this analysis and both
  backends (story:shared-ownership-analysis replaced LLVM's `region::modified` and
  `region::root` and Rust's `mutated_names`, which disagreed). The names are counted widely, an
  extra copy and never a missed one: through every expression a backend lowers to a pointer
  into its operand (`?`, `await`, `at`, a block's result), closure bodies included, and for a
  method named `push`, `set`, `append` or `clear` whatever its callee. An in-place builtin
  itself changes a variable only through a place (a name, field, index or tuple field); through
  `xs.at(0)` or `o?` it changes a copy on both targets, and inside a closure it may not change
  a place rooted at a capture (E006: from the checker where it can tell the receiver's type, else
  from the type checker once the callee is resolved, so a user fn named `push` is never refused);
- its fn moves `p` or a place rooted at it (`p.items`) on every path that reaches a `return`, a
  `?` exit or the end of the body (any move position of 20.3.1, including passing it on to a
  consumed parameter and returning it).

A place rooted at a parameter that is not consumed never moves; it is copied where it is taken,
as today (`pick(a, b, c) = if c { a } else { b }` copies the one it returns). The set is the least
fixpoint over each module's fns, computed in import order, so a std fn's set is known before the
program's. Generic fns have one set per declaration, not per instance. A protocol role's set is
its declaration's.

Copy count: a parameter consumed by an in-place change was owned before, and its caller passed or
its entry made one copy, as now. A parameter consumed by a move was copied at the move point on
every path before; now the caller copies once, or not at all when its argument moves. So no
call makes more copies than before.

#### 20.3.4 Rust target (U3)

- A consumed parameter is declared by value, `p: T`, or `mut p: T` when the body changes it; the
  entry copy of section 6 disappears for it. Every other parameter keeps today's form: `&T`,
  scalars and fn types included (`x: &i64`, cloned on entry when Copy; a called-only fn or lent bytes parameter is lent, 20.3.7), and a role's parameters
  stay `mut name: T` by value whether consumed or not.
- A moving place is emitted bare, without `.clone()` or `&`, to a consumed parameter, to any
  role parameter and through a fn value. A non-moving argument to a consumed parameter is
  emitted as a value (a place with `.clone()`).
- Every call site the backend writes itself follows the same rule: the generated mains' calls of
  `K.main`, the test runner's calls, `spawn`'s `__spawn_N` (passed by value to a consumed
  parameter, `&__spawn_N` otherwise), and a named fn wrapped as a fn value (the wrapper passes its
  owned argument by value to a consumed parameter).
- rustc must accept every program the checker accepts (vision X23): a move the analysis allows
  is one rustc's move check allows. Rows 11 and 26 check it on the fixtures, the examples and
  the end-to-end programs.
- Section 6's sentence on parameters gets the consumed rule; its `fn(A, B) -> R` row, the "fn
  types" among the Copy parameters cloned on entry and the `as fn(i64) -> i64` wrapper get 13.3's
  `Arc<dyn Fn>` (20.8 item 4).

#### 20.3.5 LLVM target (U4)

- A parameter is owned exactly when it is consumed, borrowed otherwise (today: owned when
  modified, `region::modified`).
- A moving argument passes the place's own pointer to the owned parameter, with no `copy.` call;
  ownership passes to the callee, which may change the value in place. A non-moving argument to
  an owned parameter is a fresh copy, as today.
- A non-moving operand in a move position is copied when it is evaluated, before the next
  operand runs, in every form that evaluates several: positional and named arguments, variant
  payloads, list, tuple and record literal elements, record update fields. 20.3.2 relies on it:
  a later operand may move the same place into a callee that changes it in place
  (`weigh(first = s, second = bump_count(s))`, `Both(s, bump_count(s))`).
- A moving `let`/`var` initializer, assignment, field value, element or payload takes the value
  with a shallow `memcpy` of its representation, without `copy.` glue.
- The result slot never aliases a moved variable: `s = step(s, i)` writes the result to a
  temporary slot and then moves it into `s`.
- Loop regions keep their rule: a moved variable that the loop reassigns is a carried variable
  (`docs/llvm.md` §4), and a moved argument lies below every mark the callee takes, as a borrowed
  one does. `slice16/moves.ignite` (U1) grows a moved state past 64 KiB inside a loop, so
  releases happen, and row 12 runs it under the debug runtime.
- The call sites the backend writes itself (the `spawn` thunk, the fn-value thunk, the
  `store_update` thunk, the generated mains) pass fresh copies today, which is correct for an
  owned parameter; they need no change. A `spawn` argument the analysis moves (20.3.1) is stored
  in the thunk's environment as an owned value; the thunk still copies it into the task's arena.
- `docs/llvm.md` §3's borrowed/owned sentence and §4's soundness sentence ("Callee arguments are
  fresh copies or the caller's places") get the consumed rule and the moved place.

#### 20.3.6 The evaluator

Unchanged. `ignite-eval` copies values by construction and reads neither `moves` nor
`consumed`.

#### 20.3.7 Lent parameters and fewer copies on the Rust target (story `rustgen-fewer-allocations`)

Two more parameter sets, beside `consumed`, decided once per module after it and read from
the typed modules for imported fns. Names are not resolved: a shadowing binding's uses count
against the parameter, an extra copy and never a missed one.

- *Called only* (`ModuleTypes::called_only`, `TypedProgram::called_only`): a fn-typed
  parameter of a fn or associated fn whose every use is the callee of a call, none inside a
  closure or a `spawn`, and that nothing assigns.
- *Lent bytes* (`ModuleTypes::lent_bytes`, `TypedProgram::lent_bytes`): a `Bytes` parameter that
  is not consumed and that its body reads only as a slice (`TypedProgram::reads_as_slice`):
  every use is the receiver of `len`, `is_empty`, `at`, `slice` or `to_text`, or a positional
  argument to a lent bytes parameter, outside any closure or `spawn`, never assigned. The
  greatest fixpoint over the module's fns, so a recursive call keeps it.

The Rust target (20.3.4 otherwise):

- A called-only parameter is `p: &(dyn Fn(A) -> R + Send + Sync)`, not copied on entry. A
  closure literal passed to it is a plain closure that borrows its captures,
  `&|a: A| -> R { .. }`, with no `Arc` and no copy of a capture; a fn of the module or an
  imported one named there is its fn-value closure without the `Arc`, `&|__a0: A| f(&__a0)`;
  any other argument passes the
  `Arc`'s `dyn Fn` as `&*f` (a generic callee would not get it by deref coercion), and so do
  the calls the generator writes itself (fn-value wrappers, `spawn`, reordered named
  arguments).
- A lent bytes parameter is `p: &[u8]`; the argument `t.to_bytes()` of a `Text` place `t` is
  `t.as_bytes()`, and `let b = t.to_bytes()` without an annotation, whose following statements
  in its block read `b` only as a slice, bind no other `b`, do not change `t`'s variable and
  move it (`TypedProgram::moves`, a late move included) in no statement that reads `b` or comes
  before one that does, is `let b: &[u8] = t.as_bytes();`, read like a borrowed parameter;
  otherwise `b` is a copy (`t.clone().into_bytes()`), as `let r = ctx(2); let b =
  r.label.to_bytes(); let n = consume(r, b.len(), 1); n + b.len()` needs (rustc E0505
  before, with `r` moved into `consume` or moved late after `b.len()`; unit `call-moves`
  correction round). `t.to_bytes().slice(i, j)
  .to_text()` borrows `t.as_bytes()`, and `for x in t.to_bytes()` over a `Text` place whose
  variable the body does not change iterates `t.bytes()`.
- A `Text` literal read by reference (an argument by reference, a `Map` key) is a `static`
  `LazyLock<String>` built once, `""` is `&String::new()`; as an operand of `==` or `!=` on
  `Text` and as the argument of `starts_with`, `strip_prefix` and `join` it is a `&str`. An
  operand `l.at(i)` of `==` or `!=` compares the element in place, `(*checked::at(..))`.
- A `Text` `+` chain is flattened through its right operands too (an interpolation,
  `a + "{b}c"`) into one `format!`. `x = x + ..` on a local `Text` variable that the appended
  parts do not name appends in place: `write!(x, ..)`, or `x.push_str("..")` when every part is
  a literal.
- A fn body's tail `x.to_bytes()` or `x.to_text()` on a variable the fn owns (not a borrowed
  parameter, not a capture) takes its buffer: `x.into_bytes()`,
  `String::from_utf8(x).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())`.
- `l.map(f)` with a closure literal that does not consume its parameter
  (`TypedProgram::closure_consumed`) maps `l.iter()`, the parameter a reference read like a
  borrowed parameter; with a fn of the module or an imported one that does not consume its
  parameter, `l.iter().map(|__a0| f(__a0))`. Neither copies an element.
- The `for` over a list place of section 6 that iterates by reference also takes
  `l.slice(i, j)` of a list place whose bounds change nothing: it iterates
  `(checked::slice_ref(&l, i, j, <site>)).iter()`, with no copy of the slice.
- Element reads are lent (story `std-mysql-client`, unit `mysql-cost`): an element read
  `l[i]`, `l[i][j]` or `l.at(i)` of a list that is a place, whose indexes change no variable,
  is `checked::at(..)`, a `&T` into the list, where it is only read: an argument to a
  parameter passed by reference, a `match` scrutinee whose arms do not change the list's
  variable, the list of a `for` whose body does not change it, the receiver of `len`,
  `is_empty`, `trim`, `starts_with`, `strip_prefix`, `to_text`, `to_bytes`, `join` and `map`,
  and an operand of a comparison (`(*checked::at(..)) == k`). A `for` iterates by reference
  also over such an element and over a field of one, and a `for` with a tuple pattern
  (`for (name, v) in l`, the only pattern besides a name and `_` that `for` takes) iterates
  `(l).iter()` as one with a single binding does when the body does not change the list's
  variable (a binding of a `for` cannot be changed, E006): each binding is a reference
  read like a borrowed parameter, and one of a `Copy` type is copied out at the start of the
  body. A `for` over a list of `Copy` elements (`List<Int>`, `Bytes`) that can be iterated
  by reference reads them out of it, `(l).iter().copied()`. Each of these copied the list or
  the element before (`borrowed_reads::` in `crates/ignite-rustgen`; in `crates/ignite`,
  `borrowed_reads::a_lookup_that_only_reads_allocates_nothing_on_the_rust_target`, 132.5
  allocations per lookup before, none after). Still copied: `let v = l[i]` whose `v` is only
  read, an element returned from a borrowed list (`return e.1`, one copy of the value, never
  of the list). `CODEGEN_REVISION` is `ignite-codegen/0.1.0-51` (one bump for both targets).
- Late moves and tuples of places (story `std-mysql-client`, unit `call-moves`). A place in a
  move position of a fn call (a variable, or a field place the analysis moves) that is dead
  after the call and that only later arguments of the same call read moves into the call
  after them (`TypedProgram::moves_late`, decided by `moves.rs` `Walk::late_moves`): no
  argument before it mentions it, no later argument changes its variable (an assignment, an
  in-place builtin, a block or closure that does either, by name as
  `ownership::changed_in` counts), and no later argument passed by reference has a value
  that may borrow from it (a field or element of it, an `Int` field too, which this target
  passes as `&i64`, a `slice` or `to_bytes` of one, a closure that reads it, a block,
  branch or `match` whose value is one; a call's, an operator's or a scalar builtin's
  result is a value of its own). The place is held while the later arguments run, so none
  of them moves it. `r = set_in(r, "a", read_in(r, "b")?)` is
  `r = { let __ignite_l1 = String::from("a"); let __ignite_l2 = (read_in(&r, ..))?;
  set_in(r, __ignite_l1, __ignite_l2) }`: every other argument bound in source order, the
  place passed last; with reordered named arguments the place is left out of the bindings.
  A `?` in a later argument returns before the move, and a parameter moved late counts as
  moved only on the paths that reach the call. The receiver of a UFCS call and named
  arguments take it alike. Before, the place was copied (`r.clone()`). A later argument
  that borrows keeps that copy of the whole place (`grown(r.clone(), &r.rows)`): copying
  only the borrowed field instead would need the LLVM target to copy that argument too,
  which neither target does. The LLVM target takes a late move as any move (`docs/llvm.md`
  section 4), and the evaluator, which takes a moved variable under a test, takes a late one
  once the call's arguments have run (`Evaluator::late_args`). Only a fn call
  (`Callee::Fn`) has late moves: it is the one call whose later arguments the Rust target's
  `fn_call` and `Evaluator::late_args` bind first, so a variant payload, a role call, a call
  of a fn value and a builtin keep the copy (marked late in `Two(r, r.count + 1)`, the
  evaluator would take the variable once an enclosing fn call's arguments ran, from an outer
  binding of the same name). A late move rooted at a variable this target reads through a
  reference (a borrowed parameter, a capture, a binding of a lent element or of an item of a
  lent tuple, which the analysis tracks as owned: a tuple literal is a fresh value) is
  passed as the copy `Generator::moves` keeps for it, after the same bindings. A parameter
  whose only move is a late one (`fn renamed(r: Ctx) -> Ctx = set_in(r, "k",
  Num(r.count))`) is consumed: taken by value and moved on, not borrowed and copied. The
  analysis runs on every check and plan, for every fn call in every round of its fixpoint:
  `Walk::late_moves` allocates nothing for a call with no late move (an empty result) and
  tests, in this order, an operand after the place, a place, a later operand that names its
  variable (on a reused stack), then liveness, pins and holds. Under callgrind (debug build)
  it takes 0.12 % of the instructions of `ignite check examples/todo/todo.ignite` and 0.16 %
  of `examples/terminal/terminal.ignite`, `ignite plan` alike (0.44 % and 0.59 % with the
  order of the unit's first commit, which allocated per call). `Walk::call` keeps one inline
  test (`may_move_late`: a fn call with a name or field in a move position before another
  operand); the rest, and `Walk::take_late`, are out of line and cold (`Walk::late_call`).
  Release driver, `ignite check` of `examples/terminal/terminal.design.ignite`:
  250,406,443 instructions and 1,385,671 simulated I1 misses (cachegrind, 32 KB 8-way)
  against 249,997,181 and 1,348,654 before the unit and 250,689,071 and 1,442,264 with
  everything inline; `ignite plan` 251,691,445 and 1,386,863 against 251,428,736 and
  1,349,011, and 252,024,280 and 1,442,774.
- A `match` whose scrutinee is a tuple of two or more places, each of a type that is `Copy`
  here (passed as a value) or not (passed by reference, at least one; a binding of a type that
  is not lendable is copied out of its reference at the start of the arm, as the tuple of
  copies copied it; before the correction round only lendable types), whose variables
  no arm changes or moves (`ownership::changed_in`, `ownership::moved_in`) and whose arm
  patterns are all native, matches a tuple of references: `match (a, b)` of borrowed
  parameters, `match (&a, &b)` of owned places, the bindings lent as for a single place and
  those of a `Copy` type copied out at the start of the arm; a lent binding moved into a
  callee is copied there, as any borrowed one. An arm whose pattern is one name binds the
  tuple of references; when its body reads the name it starts with `let pair =
  (pair.0.clone(), pair.1.clone());`, the copy every arm made before, and the other arms
  stay uncopied (rustc E0308 before, `(&Val, &Val)` read as `(Val, Val)`; unit
  correction round). A tuple whose type may hold a handle (`TypedProgram::holds_handles`, a
  type parameter and a fn value included, the latter because it may have captured one, as the
  LLVM target's `own::plain` takes it) is not lent: an item the analysis moves into the tuple
  is dropped with it at the end of the `match`, a lent one only at the end of its scope
  (`rx.closed()` after the `match` read false, the evaluator true; a record with a fn field,
  a list or option of fn values, a variant with a fn payload holding the only sender:
  `call_moves_adversary_pass2::adversary2_a_sender_held_by_a_fn_value_is_dropped_with_a_tuple_match`).
  A tuple with a fn-typed item is therefore matched as before the unit
  (`match (f.clone(), v)`). Before, `match (a.clone(),
  b.clone())`. The LLVM target matches a tuple of shallow copies of the places (`docs/llvm.md`
  section 4). Tests: `borrowed_reads::` in `crates/ignite-rustgen` (the emitted text, the
  guards keeping their copy) and `crates/ignite-llvm` `copies::` (no copy glue of the record
  in either form; 2 `copy.Ctx` and 2 `copy.Val` calls before); in `crates/ignite`,
  `borrowed_reads::an_update_read_by_a_later_argument_and_a_tuple_match_do_not_copy_the_record`
  (allocations per update 1514.5 over records of 10 rows and 1704.5 over 200 before, 1.0
  and 1.0 after; per match 25 and 405 before, 3 and 3 after) and
  `borrowed_reads::each_guard_of_a_call_move_gives_the_evaluator_value_on_both_targets`
  (`fixtures/call_moves.ignite`, one program per guard). Still copied: a place read by an
  earlier argument of the call (`set_in(v = read_in(r, "b")?, r = r)`), a place that later
  elements read in a variant payload, a record, list or tuple literal
  (`Both(r.clone(), size(&r))`, `Pair { c: r.clone(), n: size(&r) }`), a parameter whose
  only move is a late one behind a `?` (it stays borrowed and is copied), and a tuple
  scrutinee with a text, list or constant pattern in an arm (the guard test of such an arm
  reads every item through a reference, which a `Copy` item passed as a value is not: rustc
  E0614 on `match (n, t) { (1, "x") => .. }`). Tests of the correction round: in
  `crates/ignite`, `call_moves_adversary::` (a lent `let` across a move or a late move of
  its root, four forms; a binding of the whole tuple, three forms; one program per guard of
  the adversary pass), `borrowed_reads::each_guard_of_the_correction_round_gives_the_evaluator_value_on_both_targets`
  (the fn-call guard, the text pattern next to a `Copy` item, bindings of a lent tuple
  moved, late or not, into a callee that changes them in place, returned or replaced, items
  of a fn type, a type parameter or holding a channel end) and
  `borrowed_reads::a_record_holding_a_sender_moved_into_a_tuple_match_is_dropped_with_it`
  (evaluator and Rust; the LLVM gap of `docs/llvm.md` section 4 held as it is);
  `borrowed_reads::a_parameter_moved_late_is_taken_by_value` in `crates/ignite-rustgen` and
  `copies::a_parameter_moved_late_is_not_copied` in `crates/ignite-llvm`.
  `CODEGEN_REVISION` is `ignite-codegen/0.1.0-52` (one bump for both targets).

The LLVM target and the evaluator read none of this, except the late moves and the tuple
scrutinees of unit `call-moves`, as the last two items say.

### 20.4 Closures and calls of fn-typed expressions (D4, D5)

#### 20.4.1 What holds, as the contract

Section 13.3 stays the rule: a closure captures by copy at creation; its type is a fn type; it
lowers on Rust to `Arc<dyn Fn(..) -> R + Send + Sync>` and on LLVM to `{ ptr code, ptr env }`
with the environment block of `docs/llvm.md` §3.1 (glue word first, captures in
`ClosureInfo.captures` order with C layout, null for no captures). Row 8 pins the shapes wave C
uses. No change to either lowering in this slice.

#### 20.4.2 Calls of fn-typed expressions

Grammar: unchanged (a call's callee is a postfix expression, section 3).

- `ignite-check` resolves the names inside a callee expression and no longer reports E003 `not
  callable`; a name callee keeps its rules (an unknown name is E003, a constant callee is E003
  `constant … is not callable; bind it first`).
- `ignite-types` types the callee. When its type is `fn(..) -> R` or an effectful fn type, the
  call is a `Callee::Local` call: arguments checked against the parameter types (E040, E043 on
  arity), result `R`. Any other type is E040 (20.7).
- Effects: in its non-name arms, `ignite-check`'s `call` uses `static_type(callee)` and
  `fn_value_effects` as its `Name` branch does, charges the effects (E016 when `requires` lacks
  them) and records the effect site. So `hs[0](conn)` and `(h.send)(conn)` check as `h.send(conn)`
  does today. Only when `static_type` gives no type (a call's result, `pick_sender(true)(conn)`)
  and the typer finds an effectful fn type is the call E042 (20.7), through the existing
  effect-site rule. Binding it first (`let f: fn(c: PipeStream) -> Int requires [c.write] =
  pick_sender(true)`, then `f(conn)`) is the way out.
- Evaluation order: the callee first, then the arguments left to right (as `docs/llvm.md` §3.1
  says for fn values).
- Lowering: the existing `Callee::Local` arms (20.1 facts); U3 and U4 check them with
  `calls.ignite` and fix what fails.

#### 20.4.3 Effects of a captured capability

Unchanged: the effects a closure body performs on captures are charged where the closure is
created (section 13.3, E016), and the closure is effectful. A fn type's `requires` names only its
own parameters (`fn(n: Int) -> Int requires [n.read]`), so no fn type can describe an effect on
a capture; passing such a closure where a fn type is expected is E017 (fixture `e017_closure`).
Calling it through its `let` binding, or passing the capability as a parameter of an effectful
fn type, works today (p04b, row 8). Wave C does not need more: actors P1 says "No closures", and
`call(fn)` is P4 (20.13).

`List.map` is the exception since story `effectful-list-map` (20.13 row 1): its fn may have the
effects of the code that calls `map`. The rule, as landed:

- `xs.map(f)`, `xs` a `List` (`Bytes` included), `f` a closure literal, the name of a
  module-level fn or, since story `effectful-fn-in-local`, a local of effectful fn type (a
  let-bound closure, a parameter `g: fn(id: Int) -> T requires [..]`), may be effectful where
  `map` is called inside a fn body, a role body, a test body or a closure body. In a constant, a
  field default or a property `map` takes a pure fn, as before.
- An effect of `f` on its own parameter (the element: `|s| s.get(1)` over a `List<Store<T>>`,
  or a named fn with `requires [s.read]` on its parameter `s`) is E017 everywhere: ``effectful
  closure passed to `map` needs `s.read` on its parameter `s`, the list's element; `map` passes
  on only its caller's effects`` (for a local, ``effectful fn value `g` passed to …``). The
  one parameter of a closure literal passed to `map` has the list's element type in
  `ignite-check` too when it is unannotated and the receiver's type is declared, so
  `stores.map(|s| s.get(1))` is this E017 and no E042.
- Every other effect (on a capture's roots, or ambient) must be covered by the `requires` of the
  fn or role that calls `map`, when it has one; otherwise E017 at `f`: ``effectful closure passed
  to `map` needs `store.read`, which the `requires` of `names` does not cover`` (for a named fn,
  ``effectful function `stamp` passed to …``). The type checker reports it; `ignite-check` does
  not charge such a closure where it is created, so no E016 comes beside the E017. A let-bound
  closure is charged where it is created (13.3), as before: under a `requires` that lacks its
  effects that is one E016 at the literal, and the type checker, which does not run then, adds
  no E017 at `map` (fixture `e016_list_map_local_fn`, `ignite-check`).
- In a fn without `requires`, `ignite-check` adds those effects to the inferred set: a closure's
  where it is created (13.3), a named fn's and a local's (those of its declared fn type not on
  its parameters) at the `map` call, so a caller whose `requires` lacks them is E016 (fixtures
  `e016_list_map_fn_value`, `e016_list_map_local_fn`). A test body and a closure body accept
  them as they accept any effect.
- A local of effectful fn type whose type `ignite-check` cannot tell (bound from a call
  `let g = make()`, from a field of a call's result, or by a pattern `Some(g)`) is E042 at `f`
  where the caller has no `requires` (a fn without one, a test body, a closure body), as a call
  `g(n)` of it is: ``cannot check the effects of `g` passed to `map`: its type is not declared;
  bind it with its type: `let g: fn(id: Int) -> Int requires [clock.read] = …` `` (story
  `effectful-map-escape`). `ignite-check` lists the argument as an effect site wherever the
  local's declared type is an effectful fn type; under a `requires` the type checker checks the
  local's effects itself, so no E042 there. Tests: `effectful_map_adversary::` (`ignite-types`).
- Both backends and the evaluator lower an effectful fn value as a pure one; nothing changes
  there. Tests: fixtures `valid_effectful_list_map` and `e017_effectful_list_map`
  (`ignite-types`), the inline tests "List.map takes a fn with the effects of its caller" and
  "List.map takes a local holding an effectful fn" in `std/text.ignite` (both targets).
- Not covered: `Option.map`, `and_then`, `Result.map`, `map_err` and every other higher-order
  fn, which take a pure fn (E017); an unannotated closure parameter over a receiver whose type
  `ignite-check` cannot tell (a call's result) that calls a `Store` method stays E042.

### 20.5 Rest patterns (D6)

`docs/syntax.md` P9 and R7 as written, contracted here (vision T2).

Grammar (section 3's `pattern`, one alternative replaced):

```
pattern  += '[' (elem (',' elem)*)? ']'
elem     := pattern | '..' ident?
```

with at most one `..` per list pattern, and `..name` only as the last element. A second `..`, or a
`..name` that is not last, is E076, reported by the parser. `..` outside the brackets of a list
pattern stays E001 `expected pattern, found \`..\``. A list pattern in a `let` stays refutable
(E001), with or without a rest.

AST (add-only): `PatternKind::Rest(Option<Ident>)`, which appears only as an element of
`PatternKind::List`.

Typing: on a `List<T>` scrutinee the elements are `T` and `..name` binds `List<T>`; on `Bytes`
they are `Byte` and `..name` binds `Bytes`. A rest binding is an ordinary pattern binding (E049
in an or-pattern applies; it is immutable, section 13.11's E006 rules apply).

Exhaustiveness (E046): a list column is split by length, as rustc splits slice patterns. Let `m`
be the largest element count of an exact pattern, and `P` and `S` the largest prefix and the
largest suffix (elements before and after the `..`) over the column's rest patterns. Every length
`L` from `0` to `max(m, P + S - 1)` is checked on its own, and one representative length
`max(m + 1, P + S)` stands for every longer length (from `P + S` on, no rest pattern's prefix
and suffix share a position, so all longer lengths behave alike). At a length `L` an exact pattern
of `L` elements applies, and a rest pattern with `p + s <= L` applies with its prefix at
positions `0` to `p - 1`, its suffix at `L - s` to `L - 1` and `_` in between; the `L` element
columns are then checked as a tuple's. A value missing at a checked length is printed as that
length's list pattern, element by element, with `_` for an unconstrained position (`[_]`,
`[false]`, `[_, true]`), in the order of the lengths and then of the element witnesses. A
value missing at the representative length `R` stands for every length from `R` on, so it is
printed as rustc prints a slice witness: its first `R - S` elements, `..`, then its last `S`
(`[None, ..]`, `[.., false]`, `[true, .., false]`, `[_, false, .., true, _]`, `[_, _, ..]`);
adding the printed pattern as an arm covers every length it names. This also applies to list
columns without a rest, so p09's `[] , [x]` now reads `match does not cover \`[_, _, ..]\`` (20.7).

Lowering, both targets and the evaluator: an exact pattern tests `len == n`, a rest pattern
`len >= k`; prefix element `i` is index `i`, suffix element `j` of `s` is index `len - s + j`; a
rest binding is a copy of elements `p` to `len - s`, as `List<T>` or `Bytes`, where `p` is the
prefix length. Rust may use a slice pattern (`[a, rest @ .., z]`, with `rest.to_vec()`) or the
explicit tests. The copy is a value position of section 6.

### 20.6 Typed API (add-only)

In `crates/ignite-types/src/lib.rs`:

```rust
impl TypedProgram {
    /// Whether `expr`, a place in a move position, takes its value without a copy
    /// (`docs/slice.md` 20.3). False for every other expression.
    pub fn moves(&self, expr: &Expr) -> bool;
    /// Whether the callee consumes its parameter `index` (UFCS receiver = 0). Defined for a
    /// declared fn, associated fn or role; false for every other callee (a variant, a builtin,
    /// `Callee::Local`, `Callee::FieldFn`, `Callee::Const`).
    pub fn consumed(&self, callee: &Callee, index: usize) -> bool;
}
pub struct ModuleTypes {
    // …existing fields…
    /// Consumed parameters per fn, associated fn (`Owner.name`) and role (`Protocol.role`).
    pub consumed: BTreeMap<String, Vec<bool>>,
}
```

Both struct literals of `ModuleTypes` end in `..Default::default()`, so the field is add-only.
`query_snapshot` copies it: `TypedProgram::consumed` answers for a callee of an imported module
from that module's snapshot, so a backend asks it about a std fn the program calls. In `crates/ignite-syntax/src/ast.rs`:
`PatternKind::Rest(Option<Ident>)`. Nothing is renamed.

### 20.7 Diagnostics

New code, in section 8's syntax-stage table:

| code | meaning |
|---|---|
| E076 | a second `..` in a list pattern (`a list pattern has at most one \`..\``), or a `..name` that is not the last element (`\`..name\` must be the last element of a list pattern`); reported by the parser |

E076 is the code `docs/syntax.md` §8.3 proposes, in T4's range E070 to E079 for that proposal,
taken over with P9 (D6); slice 16 adds no range.

Changed meanings:

- E003: no longer for a callee that is an expression (`not callable`); a name callee keeps its
  rules.
- E040: also a callee whose type is no fn type: `type mismatch: this expression has type \`<T>\`
  and cannot be called`; a name callee keeps `\`<name>\` has type \`<T>\` and cannot be called`.
- E042: also a call of an effectful fn-typed expression that is no name and to which
  `ignite-check`'s `static_type` gives no type (20.4.2): `cannot check the effects of a call of
  this expression; bind it first with its type: \`let f: <fn type> = …\``.
- E046: list columns are split by length (20.5); the witness of a list column is a list pattern,
  and `match on \`List<…>\` does not cover every value; add a \`_\` arm or a binding arm` no
  longer appears for lists.

Fixtures: as in rows 6, 7, 16 and 17.

### 20.8 Stale points

| # | where | stale text | resolution, by whom |
|---|---|---|---|
| 1 | `epic:slice-16-language-gaps`, acceptance | "`examples/todo` `patch` uses a closure (friction 9 closed)" | holds since slice 9 (20.1). The steering session updates the epic; U0 does not edit the store |
| 2 | `docs/vision.md` §4.1, row "15+" | lists "B (P2 on), C, D, E, G, H", which skips A although C needs it | U6 adds a row "16 \| A language gaps" before it |
| 3 | `docs/vision.md` §4 row A and §5 | the wave-A items read as open | U6 marks each with the slice that landed it (20.1) |
| 4 | section 6, lowering table | `fn(A, B) -> R` lowers to `fn(A, B) -> R` | 13.3's `Arc<dyn Fn(..) -> R + Send + Sync>`; U3 edits it with the parameter rule (20.3.4) |
| 5 | `docs/actors.md` R1 | "P1 adds last-use moves in the generators"; "The LLVM target already passes borrowed parameters without a copy" | slice 16 adds them (20.3); a borrowed parameter is not the case R1 needs (the callee changes the state). U6 edits R1 with what moves and what still copies: `s = handle(s, m)`, `a.state = a.handle(a.state, m)` and `Step { state = st.state, … }` move; an argument beside another argument that mentions the state (`answer(state, reply, state.table.values())`) copies, and binding that argument first moves (20.3.2) |
| 6 | `docs/actors.md` §3.3 | "T3 found that a Ignite fn value cannot capture locals. Once closures land (P4)" | closures capture since slice 9 (13.3); what P4 still needs is 20.13 items 1 and 2. U6 edits it |
| 7 | `docs/syntax.md` P9, §8.3 | P9 proposed, E076 proposed | contracted by 20.5 and 20.7; U6 marks P9 "contracted, `docs/slice.md` 20.5" |
| 8 | `docs/llvm.md` §3, §4 | borrowed iff not modified; callee arguments are fresh copies or the caller's places | U4 (20.3.5) |
| 9 | `docs/friction.md` | no row for `assert` as a match arm body (`Some(h) => assert …` is E001; braces work) | U6 adds a row, open (20.13) |
| 10 | title of this file | "slices 1 to 15" | U6 makes it "slices 1 to 16" (vision X15) |
| 11 | section 6, parameters | "fn types" among the Copy parameters cloned on entry; the `as fn(i64) -> i64` wrapper | U3, with item 4 (20.3.4) |

### 20.9 Decisions

| # | decision | alternative | why |
|---|---|---|---|
| D1 | One frontend analysis in `ignite-types`, read by both backends through `TypedProgram::moves` and `consumed` (20.6) | an analysis per backend, as `mutated_names` (Rust) and `region::modified` (LLVM) are today | both targets must agree on which parameter is owned, because a std fn's mode is part of every caller's code; one analysis is one rule to review and test (rows 1 to 5) |
| D2 | Moves are conservative (20.3.2): tracked single-name bindings and parameters only, resolved per binding; liveness per field path, so disjoint fields move independently; no indexes or boxes; no move inside closure bodies, inside `match` arms or `for` bodies over an overlapping place, or beside another argument that mentions an overlapping place | Rust-like full ownership with moves of pattern bindings and partial moves through indexes; liveness per variable only | R1's shapes (a variable or a field reassigned from a call, a record built from a dead record's fields, 20.3.2's examples) need field paths and nothing more; every excluded case is one where a backend aliases (LLVM pattern bindings point into the scrutinee, a later argument overwrites the moved slot) or rustc's borrow check differs. Widening is additive (20.13) |
| D3 | A parameter is consumed when changed in place on some path or moved on every path; the set is a per-module least fixpoint in import order; no new diagnostic | an annotation on parameters (`own p: T`); consumed when moved on any path | no new keyword (invariant 6); moved-on-every-path keeps the copy count from rising (20.3.3), where moved-on-any-path would copy `unwrap_or`'s `fallback` at every call |
| D4 | Calls of fn-typed expressions are in. Their effects are charged by `ignite-check` from `static_type` where it gives a type, as for a name; E042 only where it gives none | E042 for every effectful non-name callee | `static_type` already types fields and indexes, so `(h.send)(conn)` must check as `h.send(conn)` does; only a call's result is left without a type, and binding it with a type is the way out |
| D5 | Closures keep section 13.3; effectful closures as fn values (E017) and body-directed inference of closure parameters are deferred (20.13) | effects on captures in fn types; inference variables for closure parameters | actors P1 to P3 use no closures; P4's `call` works with an annotated parameter (p05b); either change is a type-system extension larger than the rest of the slice |
| D6 | Rest patterns as `docs/syntax.md` P9 and R7, with its code E076 | `..name` anywhere, a new code in a slice-16 range | the proposal is what protocols §5.2 asked for and its code is reserved by T4 for it; one fewer deviation to record |
| D7 | Seven units in five waves (20.10) | one unit per backend for each feature | the moves analysis and the expression callees share no file; the two backends share no file; rest patterns touch every crate and so get their own wave |

### 20.10 Work split

| unit | story | owns | depends on | first failing test | rows |
|---|---|---|---|---|---|
| U0 | `story:slice-16-contract` | this section, the section-8 rows, `docs/slice16-review.md` | — | — | — |
| U1 | `story:slice-16-moves-analysis` | `crates/ignite-types/src/moves.rs`; in `crates/ignite-types/src/lib.rs` the `moves` and `consumed` accessors, `ModuleTypes::consumed`, its `query_snapshot` line and the call of the analysis after typing; `crates/ignite-types/tests/moves.rs` and its `mod moves;` line; `slice16/moves.ignite` (with a moved state grown past 64 KiB inside a loop, 20.3.5, and the shapes of 20.3.2's examples), `slice16/moves_scale.ignite` | U0 | `moves::a_var_passed_and_reassigned_moves` | 1 to 5 |
| U2 | `story:slice-16-expression-calls` | in `crates/ignite-check/src/lib.rs` the non-name arms of `call` (with `static_type` and `fn_value_effects`); in `crates/ignite-types/src/typer.rs` the call typing of an expression callee; fixtures `valid_expression_calls` (check and types), `e003_unknown_variable` (the added case), `e040_not_callable`, `e043_expression_call_arity`, `e042_effectful_expression_callee`; `crates/ignite/tests/slice16.rs` with row 8's test, its `mod slice16;` line; `slice16/closures.ignite`, `slice16/calls.ignite` | U0 | `fixtures::fixtures_match_expected` (ignite-check) on `valid_expression_calls` | 6 to 8 |
| U3 | `story:slice-16-rust-moves` | `crates/ignite-rustgen/src/lib.rs` (consumed parameters, bare moves, the backend's own call sites, expression calls if `calls.ignite` fails); `crates/ignite-rustgen/tests/slice16.rs` and its `mod` line; `crates/ignite-rustgen/tests/golden/slice16.{ignite,rs}` and the test of row 13; existing rustgen goldens as row 24 allows; `CODEGEN_REVISION`; section 6's parameter sentence and `fn` row | U1, U2 | `golden::last_use_moves_and_expression_calls` (rustgen) | 9, 11, 13, 15 |
| U4 | `story:slice-16-llvm-moves` | `crates/ignite-llvm/src/` (owned iff consumed, moved places, result slots, expression calls if `calls.ignite` fails); rows 10 and 12 appended to `crates/ignite/tests/slice16.rs`; `crates/ignite-llvm/tests/golden/slice16.{ignite,program.ll}` and the test of row 14; existing LLVM goldens as row 24 allows; `docs/llvm.md` §3 and §4 | U1, U2 | `golden::last_use_moves_and_expression_calls` (ignite-llvm) | 10, 12, 14 |
| U5 | `story:slice-16-rest-patterns` | `PatternKind::Rest` and its arms in every crate (`ignite-syntax` parser and AST, `ignite-check`, `ignite-types` `typer.rs`, `exhaustive.rs` with its new list arm, `moves.rs`, `ignite-eval`, `ignite-rustgen`, `ignite-llvm` `lower.rs` and `region.rs`); fixtures of rows 16 and 17 and the list line of `e046_patterns.expected`; `slice16/rest.ignite` added to `slice16::fixtures_pass` (rustgen) and `slice16::llvm_fixtures_pass` (`crates/ignite/tests/slice16.rs`); the rest goldens of row 19; the `rest_` family in `crates/ignite/tests/fixtures/eval_equivalence.ignite` and its line in `fixture_covers_every_family`; `CODEGEN_REVISION` | U3, U4 | `fixtures::fixtures_match_expected_diagnostics` (ignite-syntax) on `valid_rest_patterns` | 16 to 20 |
| U6 | `story:slice-16-records` | `slice16/holds.ignite`; row 21's test appended to `crates/ignite/tests/slice16.rs`; the `call_` and `move_` families of `eval_equivalence.ignite`; `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/vision.md` §4, §4.1 and §5, `docs/friction.md`, `docs/actors.md` R1 and §3.3, `docs/syntax.md` P9, the title of `docs/slice.md`; `docs/performance/16/` | U5 | `slice16::fixtures_pass_on_both_targets` | 21 to 23, 27 |

Rows 24 and 25 belong to every unit; row 26 to U3, U4 and U6.

Order: W1 U0; W2 U1 and U2; W3 U3 and U4; W4 U5; W5 U6.

Shared files:

- W2: U1 and U2 are both in `crates/ignite-types/`. U1 owns `moves.rs`, `lib.rs` and
  `tests/main.rs`; U2 owns `typer.rs` and adds fixture files only. Neither edits the other's
  file. U1 adds fixture files under `crates/ignite/tests/fixtures/slice16/`; U2 adds others there
  and owns `crates/ignite/tests/slice16.rs` and `crates/ignite/tests/main.rs`.
- W3: U3 owns `crates/ignite-rustgen/` and `crates/ignite/src/apply/plan.rs`; U4 owns
  `crates/ignite-llvm/`, `docs/llvm.md` and the appends to `crates/ignite/tests/slice16.rs`
  (U2's file, merged in W2). Both read the W2 fixtures and edit none.
- W4: U5 is alone; it edits files of every earlier unit after they merged.
- W5: U6 appends to `crates/ignite/tests/slice16.rs` and `eval_equivalence.ignite` after U2, U4
  and U5 merged.

Rules: one integration-test binary per crate (`AGENTS.md`), so every new test file is a `mod`
line in its crate's `tests/main.rs`. Each unit's commit names the rows of 20.0 it makes pass.
The story ids above are the contract's names; the steering session creates or updates the
stories, U0 does not edit the store.

### 20.11 Placeholder arms

- After U1 (W2), `moves` and `consumed` exist and no backend reads them: no behaviour or golden
  changes.
- After U2 (W2), expression callees type as `Callee::Local`; the backends' existing arms lower
  them (20.1 facts). No existing test has an expression callee, so the workspace stays green
  even if a backend fails on `calls.ignite`; rows 11 and 12 are where U3 and U4 find out.
- U5 adds `PatternKind::Rest` and every match arm over it in one unit, so no crate sees the
  variant without its arm.

### 20.12 Gate time

Method of 18.18 and 19.17 (section 15.1), through `tools/ignite-perf` (modes `cold`, `warm`,
`programs`, `cli`; records `ignite-performance/1`). "Before" is a `git archive` copy of the slice
base after the rebase (the slice-15 merge), "after" the closed tree, each with its own
`CARGO_TARGET_DIR`. One cold gate per tree, then three interleaved warm passes. A pair whose two
1-minute loads differ by more than 5 is dropped. Report the medians of the pair ratios next to the
absolute times.

Units: those of 18.18, plus:

- `ignite check examples/terminal/terminal.design.ignite` and `ignite check
  examples/todo/todo.ignite` (15 pairs each): the moves analysis runs on every program and on the
  std modules it reaches;
- the program builds of `todod` and `terminald`, both targets, dev and release (3 pairs): fewer
  copies, changed parameter modes;
- `ignite test slice16/moves_scale.ignite`, per target, cold and warm (after only), with the run
  times of its two tests.

Expected causes, to be measured or labelled inferred: the analysis's cost in the frontend
(linear in the fn bodies, with one fixpoint per loop and per module); smaller generated code
where clones disappear.

A unit that rises over 10 % gets a measured cause, or the record labels the cause inferred. No
baseline moves. The apply suites stay the slice-13 exception.

Records: a "Slice 16" paragraph in `AGENTS.md`'s gate-time section; raw logs and records under
`docs/performance/16/`.

### 20.13 Deferred

| # | item | why | where it goes |
|---|---|---|---|
| 1 | an effectful closure (one that uses a captured capability) passed as a fn value; E017 stays | no fn type can name an effect on a capture (20.4.3); wave C P1 to P3 pass no closures | `List.map` landed (story `effectful-list-map`, 20.4.3: its fn may have the effects its caller's `requires` covers); every other fn type: actors P4, or the slice that adds effect polymorphism |
| 2 | inferring an unannotated closure parameter from the closure body (p05) | an annotation works (p05b); needs inference variables the typer does not have | actors P4 |
| 3 | capture by move (a closure taking a dead variable without a copy) | a capture copies once per closure creation, not per loop step; not on R1's path | later, with measurements |
| 4 | moves of pattern bindings, `for` elements, closure parameters, fn-typed variables, inside closure bodies, operator operands (`a + b` on `Text`), and beside another argument that only reads an overlapping place (`answer(state, reply, state.table.values())`; binding the argument first moves) | D2; each needs its own aliasing argument on LLVM | additive, when a measurement asks for one |
| 5 | rest patterns in tuples (`(a, ..)`) and variant payloads | P9 covers lists; protocols §5.2's shape is a list rest inside a tuple, which row 18 covers | `docs/syntax.md`, unscheduled |
| 6 | `docs/friction.md` open rows (19 `for` patterns, 20 match guards, 22, 23, 24, 38, 41 and the others not on the wave-A row) | not on vision §4 row A | their own proposals (`docs/syntax.md` P4, P5) |
| 7 | `assert` as a match arm body (20.8 item 9) | found while probing; braces work | friction row by U6 |
| 8 | a persistent `Table` (actors R1 "Open") | a data-structure question of wave C, not a language gap | actors P1 |

### 20.14 Open questions

Each has a default, which the units follow unless the review changes it.

| # | question | default |
|---|---|---|
| Q1 | actors R1 says "Measure on the 1600-POST test", which exists only once actors P1 lands | rows 9 and 10 stand in for it; actors P1 measures R1 on its own acceptance test |
| Q2 | the rebase onto the slice-15 merge changes the base values this section quotes (`CODEGEN_REVISION`, test counts) | rows 15 and 25 are relative to the base; U3 and U5 read the value at their wave's base |
| Q3 | whether a copy the analysis keeps in a loop (a read after the call) should be reported to the user | no diagnostic in this slice (D3); a later lint can use `moves` |

## 21 Slice 17: processes, files, arguments and random bytes (playground substrate)

**Status: contract written 2026-11-04. The independent review (`docs/slice17-review.md`, 37
findings) is resolved in this text; 21.17 maps each finding to its resolution. Implemented
2026-11-05 on `feat/slice17-processes-files`, units U1 to U6; the standing of each row of 21.0
is in `STATUS.md`, the stale points of the contract found during the slice in 21.18.** The
rows of 21.0 are required checks, not results.

Sections 19 (slice 15) and 20 (slice 16) are on `main` (slice 16 merged in PR 30 before this
slice started). This section is numbered 21 so that no merge order renumbers it.

Sources:

- the planning store at `7dcb08e`: `epic:playground-substrate`, `story:process-spawn` (gap P1),
  `story:file-system` (P2), `story:program-arguments` (P8), `story:random-bytes` (P7), and the
  constraint P3 that the first two stories carry;
- `docs/design.md` §3.5 (effect interfaces for files, processes and the clock), §8.6 (IO is a
  runtime interface), §8.7 (handles across the ABI), §8.8 (process isolation), §9.3 (effects of
  toolchain builds);
- `docs/vision.md` §3 invariants 3 (handles are types), 4 (authority is passed, never ambient),
  5 (nothing lost silently, bounded queues), 7 (both targets), 8 (a fixture per code) and 9
  (compile-time budget); §6 and §7 (T2, T3, T4);
- this file: sections 5 and 5.1 (effects), 6.1 (system lowering), 7 (substrate), 8, 12.3
  (test-only constructors), 14.4 to 14.6 (resources and kinds), 15.5 (the model), 16.3 and 16.9
  (`ignite run`, `--ref`), 17 (`ignite apply`), 18 (the form this section follows);
  `docs/layering.md`; `docs/llvm.md` §4 (handle lifetime).

The base of the slice is commit `651be04c` (`main` when U1 started, with slice 16, release
`v0.5.0` and the LLVM default target of PR 32). `main` moved after it by planning-store commits
only, and U6 merged it, so the merge base at the close is `99647132`, whose code is the same as
`651be04c`'s. Every `git diff` in 21.0 and the before tree of 21.14 are taken from that base.
The contract was written against `c539392`, an older `main`; the probes of 21.1 ran there.
Code is cited by symbol (T3).

Since PR 32 (2026-11-05, before U1) LLVM is the default target of the CLI and of
`BuildOptions::default`. A command in this section without `--target` builds with LLVM and,
for `ignite build`, `ignite run` and `ignite plan --for-apply`, needs the runtime archive
(`--runtime`, or `libignite_rt.a` beside the driver; since PR 33, merged into this slice at its
close, else the archive the driver carries, `docs/llvm.md` section 1); the end-to-end tests pass
the target and the runtime explicitly. Where a row below names one target, it names it.

Out of this slice by the steering session's word: `story:http-server-responses` (gaps P4 to
P6), which follows the HTTP RFC gaps story.

The slice adds four capabilities, each a handle type that a system declares as a resource and
passes to the code that uses it:

- `Directory`: a root directory; read, write, list, create and remove files and directories
  under it, never outside it;
- `Launcher`: start child processes with arguments, a working directory, an environment, a
  timeout and bounded captured output; wait for them and kill them;
- `Entropy`: random bytes from the operating system;
- `Setting`: one command-line argument of the executable, such as `--tool PATH`.

### 21.0 Acceptance

`both` in the command column means the test runs on the Rust and the LLVM target
(`ignite test FILE --target all`, or a `crates/ignite` end-to-end test that builds with both).
Fixture paths without a directory are under `crates/ignite/tests/fixtures/slice17/`.

| # | check | command | exact required observation | unit |
|---|---|---|---|---|
| 1 | files round trip | `ignite test std/fs.ignite --target all` | the test "a file written is read back, listed, nested and removed" passes on both targets: it writes 256 bytes holding every byte value to `a.bin` in `Directory.temporary()`, reads them back equal, `read_directory("")` is `["a.bin"]`, after `create_directory("sub/deeper")` it is `["a.bin", "sub/"]`, after `remove("a.bin")` and `remove_tree("sub")` it is `[]` | U2 |
| 2 | lexical confinement | same file | the test "paths that leave the root are refused" passes on both targets: for each of `../x`, `a/../../x`, `/etc/passwd` and `a\0b`, taken from a `List<Text>` so that E140 (literals only) does not apply, each of the six `Directory` operations of 21.4 that take a path returns `Err` whose message is `path <p>: leaves its root`, `path <p>: absolute` or `path <p>: contains NUL`; the parent of the root holds no entry named `x` afterwards; `read_file("")` returns `Err` starting `path : ` | U2 |
| 3 | symlinks out, non-regular files | `cargo test -p ignite-io --lib fs::` | `fs::symlinks_out_are_refused` passes: a root holding `out -> <a sibling directory with secret>` and `f -> <a file outside>` refuses `read_file("out/secret")`, `write_file("out/new", ..)`, `read_directory("out")`, `create_directory("out/d")`, `read_file("f")` and `write_file("f", ..)` with the message `path <p>: symbolic link <component>`; the outside directory's listing and the outside file's bytes are unchanged; `remove("f")` removes the link and leaves the outside file. `fs::a_fifo_is_not_read` passes: `read_file` of a FIFO in the root returns `Err` `path <p>: not a regular file` within 1 s, with no writer on the FIFO | U2 |
| 4 | symlink race | same | `fs::a_swapped_component_never_escapes` passes: one thread exchanges `d` (a directory holding `inner`) with `e` (a symlink to an outside directory holding `inner`) by `renameat2(RENAME_EXCHANGE)`, 10,000 times; 10,000 `read_file("d/inner")` calls on another thread each return the inside bytes or an `Err`, never the outside bytes, and every `Err` whose cause was the link reads `path d/inner: symbolic link d` | U2 |
| 5 | atomic replace | same | `fs::write_replaces_atomically` passes: 1,000 alternating `write_file` calls of a 64 KiB A-file and a 64 KiB B-file race 1,000 `read_file` calls, every read is all A or all B, and the directory lists only the file afterwards (no temporary left) | U2 |
| 6 | tree removal | `ignite test std/fs.ignite --target all` | "remove_tree removes links, not their targets" passes on both targets: a tree with nested files and a link to a directory outside the root (made by `Launcher.new()` running `/bin/ln -s` as `tree/out`); before the removal `read_directory("tree/out")` returns `Err` `path tree/out: symbolic link tree/out`; the tree is removed and the outside directory keeps its files; `remove_tree("")` returns `Err` `path : the root itself` and `remove_tree(".")` returns `Err` `path .: the root itself` (`<p>` is the path as given) | U2 |
| 7 | read and size bounds | same | "a file over the read bound is refused" passes: a file of 16 MiB + 1 byte, written by `write_file`, makes `read_file` return `Err` whose message contains `larger than 16 MiB`; a file of exactly 16 MiB is read. `fs::bounds_hold` (`cargo test -p ignite-io --lib`): a directory of 65,537 entries makes `read_directory` return `Err` `path <p>: more than 65536 entries`; a chain of 257 nested directories makes `remove_tree` return `Err` `path <p>: deeper than 256 levels` and removes nothing | U2 |
| 8 | file effects checked | `cargo test -p ignite-check --test main fixtures::` | fixture `e016_directory_effect` (`fn save(files: Directory) -> Result<Unit, IoError> requires [files.read]` calling `files.write_file`) gives exactly ``missing effect `files.write` for `files.write_file`; add it to the `requires` of `save` ``; `valid_capabilities` (each capability method called with its effect declared, receivers bound without annotation from the four test-only constructors and from `start`, 21.3) is silent | U1 |
| 9 | process output and status | `ignite test std/process.ignite --target all` | "a child's output and status are captured" passes on both targets: `Launcher.new().execute(Invocation { program = "/bin/sh", arguments = ["-c", "echo out; echo err >&2; exit 3"] })` gives `ending == 0`, `code == 3`, `stdout == "out\n"`, `stderr == "err\n"`, `dropped == 0`, and `outcome(exit)` matches `ExitCode(3)` | U3 |
| 10 | timeout kills the group | same | "a child over its timeout is killed with its group" passes on both targets: `/bin/sh -c "echo $$; sleep 30 & sleep 30"` with `timeout = 200 milliseconds` returns `TimedOut` (`ending == 2`, `code == -1`) within 3 s of the start with the shell's pid as `stdout`, and a follow-up `execute` of `/bin/sh -c "ps -o stat= -g <that pid> \| grep -v Z \|\| echo gone"` prints `gone` (the backgrounded grandchild died with the group; a zombie counts as gone) | U3 |
| 11 | kill | same | "a started child is killed" passes on both targets: `start` of `/bin/sleep 30`, then `kill()` returns `Ok`, `wait()` returns `Killed` (`ending == 3`) within 1 s; then a second `wait()` returns `Err` `child already waited` and `kill()` returns `Ok` | U3 |
| 12 | bounded output | same | "output past the limit is counted, not kept" passes on both targets: a child writing 2 MiB to stdout with `output_limit = 1 KiB` exits 0, `stdout.len() == 1024`, `dropped == 2097152 - 1024`, and the call returns (no deadlock on the full pipe); `output_limit = 17 MiB` returns `Err` `output_limit must be at most 16 MiB` | U3 |
| 13 | SIGTERM reaches a child of a drained server | `cargo test -p ignite --test main slice17::` | `slice17::{rust,llvm}::a_child_of_a_server_gets_sigterm`: the fixture `signal_mask.ignite`, an `HttpServer` whose handler executes `/bin/sh -c "kill -TERM $$; sleep 5"`, answers `GET /` with `ending 1 code 15` within 2 s | U3 |
| 14 | no inherited descriptors | same | `slice17::{rust,llvm}::a_child_inherits_no_descriptors`: the test starts the server with a descriptor 5 open without close-on-exec; the same fixture's `GET /fds` executes `/bin/ls /proc/self/fd` and the body lists exactly `0`, `1`, `2` and `3` (the last is the directory `ls` reads; descriptor 5, the listener, the accepted connection, the root directory and `/dev/urandom` are absent; the fixture declares a `Directory` and an `Entropy` so they exist) | U3 |
| 15 | process effects checked | `cargo test -p ignite-check --test main fixtures::` | fixture `e016_launcher_effect` (`fn build(tools: Launcher) -> Result<ChildExit, IoError> requires []` calling `tools.execute`) gives exactly ``missing effect `tools.launch` for `tools.execute`; add it to the `requires` of `build` ``; `e016_child_effect` (`fn stop(child: ChildProcess) -> Result<Unit, IoError> requires []` calling `child.kill()`) gives exactly ``missing effect `child.kill` for `child.kill`; add it to the `requires` of `stop` `` | U1 |
| 16 | child bound and refusals | `ignite test std/process.ignite --target all` | "the launcher refuses what it cannot run" passes on both targets: with `Launcher.new()` (bound 16) the 17th `start` of `/bin/sleep 30` returns `Err` `too many children (16)`, and after `kill` and `wait` of one of the 16 a further `start` succeeds; `program = "nosuch-ignite-17"` returns `Err` starting `program not found`; `program = "bin/sh"` returns `Err` `program bin/sh: a relative path with /`; `timeout = 0 seconds` returns `Err` `timeout must be positive`; `directory = "rel"` returns `Err` `directory rel: not absolute` | U3 |
| 17 | children end with the program | `cargo test -p ignite --test main slice17::` | `slice17::{rust,llvm}::live_children_die_with_the_program`: the fixture `orphans.ignite` starts `/bin/sleep 30`, prints `child.id()` and returns from `main`; within 1 s of the program's exit the pid is absent or a zombie (`/proc/<pid>/stat` state `Z`). The same holds after one SIGTERM to a drained server (`orphans_after_drain`), after a second SIGTERM during the drain (`orphans_after_second_sigterm`), and for the direct child after SIGKILL of the server (`orphans_after_sigkill`, through `PR_SET_PDEATHSIG`) | U3 |
| 18 | arguments | `cargo test -p ignite --test main slice17::` | `slice17::{rust,llvm}::settings_read_their_flags`: the fixture `arguments.ignite` (a kind declared in the fixture whose `main` prints its state; resources `Setting { flag = "--tool" }`, `Setting { flag = "--mode", default = "fast" }`, `Directory { path = "missing", flag = "--root" }`, a `TcpListener`; `main` also prints `Endpoint.of` of a `Ref`), started as `<exe> --root DIR --tool /bin/true --port 0 --ref Arguments.peer=127.0.0.1:9` (a `Ref` resolves by its path `<System>.<member>`, 16.9), prints `tool /bin/true`, `mode fast`, `root <DIR canonical>`, `listening 127.0.0.1:<port>` and `peer 127.0.0.1:9`; with `--mode=slow` it prints `mode slow`; without `--tool` it exits 2 with stderr `error: --tool: required argument missing` and binds no port; with `--mode --tool /bin/true` it exits 2 with stderr `error: --mode needs a value` | U4 |
| 19 | property checks | `cargo test -p ignite-types --test main fixtures::` | fixtures `e104_setting_flag` (`flag = "tool"`), `e104_directory_path` (`path = ""`), `e104_launcher_bound` (`maximum_children = 0`) each give exactly one E104 with the texts of 21.9; `e141_flag_collision` (two resources with `flag = "--root"`, a third with `flag = "--port"`) gives exactly two E141; `fixtures::every_type_code_has_a_fixture` lists E140 and E141 | U1 |
| 20 | random bytes | `ignite test std/process.ignite --target all` | "two draws differ" passes on both targets: `Entropy.new()` gives two `random_bytes(32)` results of length 32 each that differ; `random_bytes(65537)` and `random_bytes(-1)` return `Err` `random_bytes: count must be in 0..=65536`; `random_bytes(0)` is empty | U5 |
| 21 | random effect checked | `cargo test -p ignite-check --test main fixtures::` | fixture `e016_entropy_effect` (`fn token(entropy: Entropy) -> Result<Bytes, IoError> requires []` calling `entropy.random_bytes(32)`) gives exactly ``missing effect `entropy.draw` for `entropy.random_bytes`; add it to the `requires` of `token` `` | U1 |
| 22 | P3: a handler uses the capabilities through its state, check | `ignite check crates/ignite/tests/fixtures/slice17/studio.ignite` | silent, exit 0. `studio.ignite` is a `system Studio` with `resource files: Directory { path = "workspace", create = true, flag = "--root" }`, `resource tools: Launcher`, `resource entropy: Entropy`, `resource tool: Setting { flag = "--tool" }`, an `HttpServer` with `application = HttpApp { state = Bench { files = files, tools = tools, entropy = entropy, tool = tool }, handle }`, and a handler with no `requires` that uses all four through `state` | U1 |
| 23 | P3: capabilities reached ambiently are refused | `cargo test -p ignite-check --test main fixtures::` and `cargo test -p ignite-types --test main fixtures::` | fixture `e051_capability_outside_state` (ignite-check): a handler that calls `Entropy.new()` and `Directory.temporary()` gives exactly two E051; no other way to name a capability exists outside a resource (21.3). Fixture `e017_ambient_in_handler` (ignite-types; it declares an `HttpApp`-shaped record locally, because the fixture standard library has no `std/protocol`): a handler calling `Clock.monotonic()` gives exactly one E017, as probe p2 | U1 |
| 24 | P3: the handler runs on both targets | `cargo test -p ignite --test main slice17::` | `slice17::{rust,llvm}::studio_serves_through_its_state`: `studio.ignite` built and started with `--port 0 --root <tmp> --tool /bin/sh`; `POST /write` with body `a.ignite\nx` answers 204 and `<tmp>/a.ignite` holds `x`; `POST /read` with body `a.ignite` answers 200 `x`; `POST /read` with body `../x` answers 400 with the body `path ../x: leaves its root`; `POST /run` (the handler executes `state.tool.value()` with `-c "cat a.ignite"` in `state.files.absolute_path()`) answers 200 `x`; `GET /token` answers 64 lowercase hex digits, and a second launch answers a different token | U6 |
| 25 | the model lists the capabilities | `cargo test -p ignite --test main plan::` | `plan::studio_model_matches_golden`: `ignite plan studio.ignite` equals `crates/ignite/tests/golden/studio.model.json`, whose `resources` hold `{"id":"Studio.files","kind":"Directory","properties":{"create":true,"flag":"--root","path":"workspace"}}`, `Studio.tools` with `{"maximum_children":16}`, `Studio.entropy` with `{}` and `Studio.tool` with `{"flag":"--tool"}` | U1 |
| 26 | `ignite run` and `plan --for-apply` refuse an unset setting | `cargo test -p ignite --test main run::` and `apply::` | `run::a_setting_without_default_is_e142`: `ignite run e142_setting_without_default.ignite` exits 2 with exactly one `E142` naming the setting; `apply::plan_refuses_a_setting_without_default`: `ignite plan e142_setting_without_default.ignite --for-apply --state <tmp> --out <tmp>/p` exits 2 with the same diagnostic, reported before the host is contacted (no `ignite host` runs); `run::studio_defaults_runs`: `studio_defaults.ignite` (`default = "/bin/sh"`) run with `--target llvm` and the test runtime in a temporary working directory starts, `status` shows `Studio.server` running, `stop` exits 0, and the repository gains no file | U4 |
| 27 | path literals checked | `cargo test -p ignite-types --test main fixtures::` | fixture `e140_path_leaves_root`: `files.read_file("../x")`, `files.write_file("/etc/x", ..)` and `files.read_directory("a/../..")` give exactly three E140; `files.read_file(name)` with a variable and `files.read_file("a/b")` give none | U1 |
| 28 | layering | `cargo test -p ignite-check --test main layering::` | passes with rows for `std.fs` and `std.process` in `docs/layering.md` and in the test's table, both in the base layer | U1 |
| 29 | no new dependency | `cargo tree -p ignite-io -e normal --prefix none \| sort -u` and `cargo tree -p ignite-io -e features` | the first lists the same crate set as at the base; the second shows `nix` with the added features `dir` and `poll` (the latter now outside `runner` too), recorded in the commit message that adds them | U1 |
| 30 | names stay free | `ignite check` of every `std/` and `examples/` file; `cargo test -p ignite-check --test main fixtures::` | silent; `std/cli.ignite` still declares `Command` and `Argument`; the fixtures that declare a fn `start` (`valid_protocol`, `e013_spawn_not_effectful_call`) give their expected output unchanged; no existing `.expected` file newly mentions `value`, `wait`, `kill` or `execute` | every unit |
| 31 | the old resource error | `git diff --stat 651be04c -- '*.expected'` at U1's merge | only files whose E020 text "expected `TcpListener` or `Store`" became the new text of 21.9, and the new fixtures, appear | U1 |
| 32 | codegen revision | `git diff 651be04c -- crates/ignite/src/apply/plan.rs`; `cargo test -p ignite --test main apply_admission::` | `CODEGEN_REVISION` differs from the base, bumped by U1 and again by U4: `ignite-codegen/0.1.0-16` at `651be04c` (`0.1.0-2` at `c539392`; slice 16 and PRs 27 to 29 stepped it in between), `-17` after U1, `-18` after U4; the admission tests pass | U1, U4 |
| 33 | gate | `task gate` | exit 0 on the slice branch at the close | every unit |
| 34 | gate time | 21.14 | the records of 21.14 exist under `docs/performance/17/` and in `AGENTS.md` | U6 |
| 35 | no laundering through a parameter named `clock` | `cargo test -p ignite-check --test main fixtures::` | fixture `e056_clock_parameter` (probe p1) gives exactly one `E056` at the parameter `clock`; `AMBIENT_HEADS` holds `clock` | U1 |
| 36 | generator goldens | `cargo test -p ignite-rustgen --test main golden::` and `cargo test -p ignite-llvm --test main golden::` | `golden::capabilities` on each backend: `crates/ignite-rustgen/tests/golden/capabilities.{ignite,rs}` and `crates/ignite-llvm/tests/golden/capabilities.{ignite,program.ll}` (the four resources, a flag read in the generated `main`, one call of each method) match, and the Rust output builds with cargo and the LLVM output compiles with clang | U1 |

### 21.1 Current state and verdicts (at `c539392`)

Probes, run 2026-11-04 with `target/debug/ignite check` built from the base (`cargo build -p
ignite`, exit 0):

- p1, `fn helper(clock: Int) -> Duration requires [clock.read] = Clock.monotonic()` called from
  `fn outer() -> Duration requires [] = helper(1)`: silent, exit 0. A parameter named `clock`
  absorbs the ambient `clock.read`, so `outer` reads the clock while declaring nothing. A
  handler can launder an ambient effect this way past `requires [state.*]`, which breaks P3;
  U1 closes it (row 35).
- p2, an `HttpServer` handler over `Store<Int>` that calls `Clock.monotonic()`: exactly one
  ``E017 effectful function `handle` does not fit `fn(state: Store<Int>, request: Request) ->
  Response requires [state.*]`: its effect `clock.read` is not covered by the type's
  `requires` ``, exit 1.
- p3, `resource files: Directory { path = "workspace" }` and `resource tools: Launcher`: two
  ``E020 unsupported resource kind `Directory`; expected `TcpListener` or `Store` `` (and the same
  for `Launcher`), exit 1.
- p4, one state record built from two `Store` resources, `HttpApp { state = Both { counts =
  counts, names = names }, handle }`, the handler using both through `state`: silent, exit 0.
  A state record composes several capabilities today.

| fact | where | verdict |
|---|---|---|
| F1 the substrate reads only `--port` and `--ref` | `ignite_io::args::port_from_args`, `endpoint_from_args` | `Setting` adds flags beside them; both keep their meaning (row 18) |
| F2 no file, process or random operation exists in a builtin table | `BUILTIN_STATICS`, `BUILTIN_METHODS` in `crates/ignite-check/src/lib.rs`; `crates/ignite-types/src/builtins.rs` | all four capabilities are new |
| F3 the resource kinds are `TcpListener` and `Store` | `resource` in `crates/ignite-types/src/typer/system.rs`, `lower_resources` in `crates/ignite-rustgen/src/lib.rs` | four kinds added; the E020 text changes (row 31) |
| F4 a handler may act only on its state | `HttpApp` in `std/protocol/http.ignite` (`requires [state.*]`); probe p2 | capabilities are values in the state, not statics (21.3) |
| F5 `Store` methods are effectful by the receiver's declared type, other substrate methods by name | `store_op` and `store_effect`; `EFFECTFUL_METHODS` and `method_effect` | the new methods follow `store_op`: a by-name table would capture user fns named like them, such as `start` (fixtures `valid_protocol`, `e013_spawn_not_effectful_call`), `value`, `wait`, `kill`, `execute`; a receiver bound without annotation needs `static_type` arms (21.3, R4) |
| F6 the ambient heads are `stdin`, `stdout`, `process`, `net` | `AMBIENT_HEADS` | `clock` is missing (probe p1): U1 adds it, so a parameter named `clock` is E056 (row 35); the capabilities add no head, they are not ambient |
| F7 `ignite run` starts an executable without a working directory of its own | `spawn` in `crates/ignite/src/run/process.rs` | a relative `Directory.path` resolves against `ignite run`'s working directory (21.6) |
| F8 the drain blocks SIGTERM in the main thread and in every thread it starts | `ignite_io::drain::install` | a child must start with an empty signal mask. Resolved by U3: Rust's `Command` keeps the mask of the thread that forks (row 13 failed without the hook), so the `pre_exec` hook of 21.5 empties it |
| F9 sockets are opened close-on-exec by Rust's `std::net` | `ignite_io::net` | covers only what the runtime opens; a descriptor the server inherited without close-on-exec would pass to a child, so the child closes every descriptor from 3 up (21.5, row 14) |
| F10 on LLVM a handle lives until the process exits unless a `spawn` moved it | `docs/llvm.md` §4 | `wait` frees a child's slot and the adapter answers a released child handle as Rust does (21.5); `execute` allocates none |
| F11 `nix` 0.31: the workspace enables `term` and `fs`, `ignite-io` adds `signal` (which implies `process`) and `poll` under `runner` | root `Cargo.toml`, `crates/ignite-io/Cargo.toml` | `openat`, `fstatat`, `unlinkat`, `mkdirat`, `renameat2` and `killpg` are on already; U1 adds `dir` (for `Dir`) and `poll` outside `runner` (row 29) |
| F12 the test runner already captures a child's output with `poll` and kills it at a deadline | `run_child`, `drain_output` in `crates/ignite-io/src/test.rs` | the watcher of 21.5 is the same loop, moved into `ignite_io::child` and shared; the runner keeps its own process-group behaviour (the watcher takes the group as a parameter), so Ctrl-C still reaches test children |
| F13 the model encodes a resource as `{"id","kind","properties"}` | the resource arm of the model in `crates/ignite-eval/src/eval.rs` | no format change; the new kinds appear with their properties (row 25) |
| F14 names in use: `Command`, `Argument` (`std.cli`), `Exited`, `Signalled` (variants of `std.control`'s `Event`), `write_text` (`std.protocol.telnet`) | those modules | the new names avoid them: `Invocation`, `ChildExit`, `Ending` with `ExitCode`, `BySignal`, `TimedOut`, `Killed`, `read_text_file`, `write_text_file` |
| F15 E140 to E149 are unused on `main` and on `feat/slice16-language-gaps`; T4 ends at E139 | section 8; `docs/vision.md` §7 T4 | this slice takes E140 to E149 (stale point S1) |

The facts above are as found at `c539392`; how each stands at the close is in 21.18.

### 21.2 Scope and non-goals

In scope: the four capability types and their resource kinds, their effects, their methods on
both targets over one implementation in `crates/ignite-io`, the std modules `std.fs` and
`std.process`, the test-only constructors, the model entries, `ignite run`'s handling of
settings, and the P3 end-to-end row.

Not in scope:

- sandboxing a child: a child process is not confined to any `Directory`; the launcher is the
  authority to run programs (design §8.8 leaves isolation to a later slice);
- shells, pipes between children, a pty, writing to a child's stdin (stdin is the null device),
  streaming a child's output while it runs;
- file watching, permissions and modes, memory-mapped files, file handles for partial reads,
  symlinks that stay inside the root (they are refused, 21.4), renaming;
- seeded or deterministic random generators;
- an argument-parsing library beyond `Setting` (the stories exclude it; `std.cli` keeps its
  declarations);
- passing setting values from `ignite run` or `ignite apply` (21.15);
- `story:http-server-responses` (P4 to P6);
- killing grandchildren when the program dies without an orderly exit (SIGKILL, a crash):
  direct children are killed (21.5), grandchildren are 21.15's;
- platforms other than Linux; the code uses POSIX calls, but no row runs elsewhere.

### 21.3 Capabilities are values (P3)

A capability is obtained in exactly two ways:

1. A system declares it as a resource, and the generated `main` creates it before the kind's
   `main` runs, as it creates a `Store` and binds a `TcpListener` (section 6.1). A component
   receives it through a property, typically the `state` of an `HttpApp`.
2. A test or test-only item creates one with a test-only constructor: `Directory.temporary()`,
   `Launcher.new()`, `Entropy.new()`, `Setting.of(flag, value)`. Anywhere else these are E051.

A capability resource needs no deployment binding: it belongs to the process of the member
that creates it, and an assembly's resolution hosts it there implicitly (15.6), so the studio
below resolves under the standard `Deployment` with bindings for `Studio.ingress` and
`Studio.server` only. A capability resource provisions nothing: only a `TcpListener` has a
`provisioning` edge (16.14).

There is no static that yields a capability outside tests, and no new ambient head: `process`
stays the head of `process.exit` and `process.args` only. A handler of an `HttpApp<S>` can use a
capability only if `S` holds it, so `requires [state.*]` covers every effect it uses (probe p2
shows that an ambient effect there is E017).

```ignite
type Bench {
  files: Directory
  tools: Launcher
  entropy: Entropy
  tool: Setting
}

fn handle(state: Bench, request: Request) -> Response = ...

system Studio {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8095 }
  resource files: Directory { path = "workspace", create = true, flag = "--root" }
  resource tools: Launcher
  resource entropy: Entropy
  resource tool: Setting { flag = "--tool" }

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = Bench { files = files, tools = tools, entropy = entropy, tool = tool }, handle }
    executable = "studiod"
  }
}
```

Effects. Each method needs `<root>.<op>` on each root of its receiver (section 5.1), checked by
the receiver's declared type as `Store` methods are (F5):

| type | method | op |
|---|---|---|
| `Directory` | `read_file`, `read_directory` | `read` |
| `Directory` | `write_file`, `create_directory`, `remove`, `remove_tree` | `write` |
| `Directory` | `absolute_path` | `read` |
| `Launcher` | `start`, `execute` | `launch` |
| `ChildProcess` | `wait` | `wait` |
| `ChildProcess` | `id` | none (pure; a `ChildProcess` exists only at run time) |
| `ChildProcess` | `kill` | `kill` |
| `Entropy` | `random_bytes` | `draw` |
| `Setting` | `value` | `argument` |
| `Environment` (21.20) | `get` | `variable` |

- The op is `launch`, not `spawn`, because `spawn` is a keyword.
- The ops differ per kind (`read`, `write`, `launch`, `draw`, `argument`), so `requires
  [b.read]` on a record holding a `Directory`, an `Entropy` and a `Setting` grants only the
  directory's reads; `b.*` grants all.
- `Setting.value` and `Directory.absolute_path` are effectful although their values are fixed
  at launch: that keeps them out of compile-time evaluation (a property expression must be pure,
  E116; design §3.5), and keeps reading a launch argument a tracked authority, as
  `Endpoint.of`'s `process.args` is.
- A `ChildProcess` returned by `start` has no roots (vision §6 L4: authority over a listener is
  authority over its connections), so the function that started it waits for and kills it
  without further `requires`. A `ChildProcess` received as a parameter needs `p.wait` and
  `p.kill`.
- All five types are handles (`can_hold_handle`), so a record holding one carries roots, and a
  closure that captures one is charged where it is created (section 13.3).
- `ignite-check`'s `static_type` knows the result types of the four test-only constructors
  (also through `?`) and of `start` on a receiver it knows as `Launcher` (also through `?`), so
  `let files = Directory.temporary()?`, `Launcher.new().execute(..)` and `let child =
  tools.start(..)?` need no annotation. Any other receiver whose type `ignite-check` cannot see
  is E042, as for `Store` (section 11.7).

Concurrency: every capability may be used by several tasks at once. On the Rust target each is
a cheap clone of an `Arc`; on LLVM each is an `i64` handle into the runtime's table, and the
substrate object behind it is shared.

### 21.4 Files (`Directory`, `std.fs`)

Resource kind `Directory`:

| property | type | default | meaning |
|---|---|---|---|
| `path` | `Text` | required (E104) | the root; relative to the executable's working directory at launch |
| `create` | `Bool` | `false` | create the root and its parents at launch when it is missing |
| `flag` | `Text` | `""` | a command-line flag whose value replaces `path` at launch, for example `"--root"` (21.6) |

At launch the generated `main` resolves the path (flag value first, else `path`), creates it if
`create` is true, opens it with `O_DIRECTORY | O_CLOEXEC` and keeps that descriptor for the
process's life. A missing root, a root that is no directory, or a failing open ends the program
with exit status 2 and the line `error: directory <path>: <reason>` on stderr (the form of
`ignite_io::run`), before any listener is bound. Every operation resolves relative to the kept
descriptor, so renaming the root's path later moves nothing.

Methods. Each returns `Result<_, IoError>`, and the result column names the `Ok` type.
`std.fs` and `std.process` declare no `IoResult` alias: `std.protocol.http` and
`std.protocol.telnet` each declare one, and a name two glob imports provide is E041 at use.

| method | result | behaviour |
|---|---|---|
| `read_file(path)` | `Bytes` | the file's bytes. The last component is opened with `O_NONBLOCK`, checked with `fstat` and refused unless it is a regular file (`path <p>: not a regular file`, so a FIFO cannot block the task), then read; a file larger than 16 MiB is the error `path <p>: larger than 16 MiB` |
| `write_file(path, data: Bytes)` | `Unit` | creates or replaces the file atomically: writes a temporary `.ignite-<pid>-<n>` in the same directory with `O_CREAT \| O_EXCL \| O_NOFOLLOW` (its name does not grow with the target's, so every name the OS takes, up to `NAME_MAX`, is written), then renames it over the target; a replaced regular file keeps its permission bits (`mode & 0o777`, given to the temporary with `fchmod` before the rename), a new file is created with mode 0644 under the process's umask; no `fsync` (durability is not promised); the parent must exist |
| `read_directory(path)` | `List<Text>` | the entries' names in byte order, without `.` and `..`; a directory's name ends with `/`; a symlink is listed by its name without following it; more than 65,536 entries is the error `path <p>: more than 65536 entries`; this bound is the listing's only. An entry whose name is not UTF-8 has no `Text` that names it back, so it is neither dropped nor listed under another name: the listing is the error `path <p>: entry <name> is not UTF-8`, `<name>` its bytes as Rust's `escape_ascii` writes them (`a\xff`); `remove_tree` of a directory above it removes it |
| `create_directory(path)` | `Unit` | creates the directory and its missing parents; an existing directory is `Ok` |
| `remove(path)` | `Unit` | removes a file, a symlink (not its target) or an empty directory; a missing entry is an error |
| `remove_tree(path)` | `Unit` | removes a directory and everything under it without following any symlink, iteratively, by the raw names the OS lists (a name that is not UTF-8 is removed with the rest) and with no bound on the entries of a directory; a missing entry is an error; a tree deeper than 256 levels is the error `path <p>: deeper than 256 levels`, found before anything is removed; the root itself (`""`, `.`) is the error `path <p>: the root itself`, where `<p>` is the path as given |
| `absolute_path()` | `Text` | the root's canonical absolute path as resolved at launch; needs `read` (21.3) |

Paths are `Text`, relative, separated by `/`. Empty components and `.` are skipped, so `""` and
`.` name the root (which `read_directory` and `create_directory` accept and `read_file` refuses
as a directory). Refused before any IO, as error values (row 2):

- an absolute path (`/` first): `path <p>: absolute`;
- a `..` component anywhere: `path <p>: leaves its root`;
- a NUL byte: `path <p>: contains NUL`.

Symlinks. Resolution walks the components from the root descriptor with `openat(...,
O_NOFOLLOW | O_DIRECTORY | O_CLOEXEC)` for each directory and `O_NOFOLLOW` for the last
component. A symlink at any component is the error `path <p>: symbolic link <component>`, where
`<component>` is the path up to and including the link,
except that `remove` and `remove_tree` remove a link that is the last component (or lies inside
the tree) as a link. Because no step follows a link and each step opens relative to the
descriptor of the one before, a component swapped for a link between steps is caught at the step
that opens it (row 4). The kernel reports such an open as `ELOOP` or `ENOTDIR`; the substrate
then checks the component with `fstatat(.., AT_SYMLINK_NOFOLLOW)` and reports `symbolic link
<component>` when it is a link, else the OS reason. A walk that loses a race (the component is
missing for an instant) is an ordinary `Err`; it never reaches outside the root. Symlinks that
stay inside the root are refused too; following them is 21.15's.

As landed by story `fs-resolve-beneath` (2026-11-08): where the kernel has `openat2` (Linux
5.6), a path is first opened from the root descriptor in one call with `RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS`, which fails on a symlink at any component, magic links included, and on
any step out of the root, so it confines exactly as the walk does. `read_file` opens the whole
path that way, `read_directory` the listed directory, `create_directory` returns at once when
the path is an existing directory, and `write_file`, `remove` and `remove_tree` open the parent
once and act on the last component relative to it (the root's own descriptor when the parent is
the root). When that call fails for any reason the operation walks as above, which either
succeeds (the path changed in between) or gives the error this section words, naming the
component; the walk is also the whole resolution once `openat2` answered `ENOSYS` or `EPERM` (a
kernel before 5.6, a seccomp filter). A listing reads the directory with `readdir` through
`nix::dir::Dir` (story `unsafe-sites-back`). The calls of a listing, as `strace` shows them:
an `fstat`, an `fcntl(F_GETFL)` and an `fcntl(F_SETFD, FD_CLOEXEC)` (`fdopendir`), the
`getdents64` calls, an `lseek` back to the start (the end of the iteration) and the `close` of
the descriptor. `remove_tree` lists each level through that level's own descriptor. A level it
descends from then duplicates the descriptor (`fcntl(F_DUPFD_CLOEXEC)`) and closes the listing
before it goes deeper. The deepest level keeps its listing until it is emptied. So the walk
holds one descriptor per level and one listing at a time. Level `k` holds `k + 1` descriptors
while it makes its duplicate, and a tree 256 levels deep needs 256 descriptors. Every behaviour above, its errors included, is the same in both resolutions; the substrate's
tests run on both (`fs::tests`, `Scratch::both`). Measured with `strace -c` on the todo example's
patch workload (16 connections on one todo, LLVM, release): 108.5 syscalls per reply before,
45.8 after, the plain-Rust reference 16.0. Of the 45.8, 8.1 are failed `remove`s of versions a
concurrent save already removed (`unlinkat` `ENOENT`, 3 syscalls each with the parent's
`openat2` and `close`); the reference fails 0.3 per reply. That count is set by how many saves
of one todo overlap, not by the resolution.

Errors from the operating system are `IoError` values whose message is `path <p>: <reason>` with
the OS reason text.

`std.fs` (new file `std/fs.ignite`, base layer, a substrate module beside `std.net`) declares
the doc comments of the builtins and:

- `fn read_text_file(files: Directory, path: Text) -> Result<Text, IoError> requires [files.read]`
  (invalid UTF-8 is an error naming the path);
- `fn write_text_file(files: Directory, path: Text, text: Text) -> Result<Unit, IoError> requires
  [files.write]`;
- `fn join_path(left: Text, right: Text) -> Text` (one `/` between non-empty parts).

### 21.5 Processes (`Launcher`, `ChildProcess`, `std.process`)

Resource kind `Launcher`:

| property | type | default | meaning |
|---|---|---|---|
| `maximum_children` | `Int` | `16` | the most children started and not yet waited, at once; `1..=1024` (E104) |

`std.process` (new file `std/process.ignite`, base layer, a substrate module) declares the
records that cross the ABI, as `Endpoint` does (section 16.13):

```ignite
type Invocation {
  program: Text
  arguments: List<Text> = []
  directory: Text = ""
  environment: List<Text> = []
  inherit_environment: Bool = true
  timeout: Duration = 30 seconds
  output_limit: ByteSize = 1 MiB
  input: Bytes = b""
}

type ChildExit {
  ending: Int
  code: Int
  stdout: Bytes
  stderr: Bytes
  dropped: Int
}

enum Ending { ExitCode(Int), BySignal(Int), TimedOut, Killed }

fn outcome(exit: ChildExit) -> Ending
fn succeeded(exit: ChildExit) -> Bool
```

- `program`: a bare name, looked up in the `PATH` of the child's environment, or an absolute
  path. A relative path containing `/` is the error `program <p>: a relative path with /`; a
  name not found is `program not found: <p>`.
- `arguments`: passed as they are; no shell.
- `directory`: `""` keeps the program's own working directory; otherwise an absolute path
  (`directory <d>: not absolute`). `Directory.absolute_path()` gives one. It is checked before
  the child starts: one that does not exist, is no directory or cannot be entered is the error
  `directory <d>: <reason>` (the OS reason, such as `No such file or directory`), never
  `program not found`. `program not found: <p>` means only that the program's path does not
  exist; any other failure to start is `program <p>: <reason>`.
- `environment`: entries `NAME=value`, applied over the inherited environment, or over an empty
  one when `inherit_environment` is false. An entry without `=` or with an empty name is an
  error. The default `true` hands the server's whole environment, secrets included, to every
  child; a caller that runs untrusted programs sets it to false and passes `PATH` and what else
  the child needs (design §9.3 asks that effects name what a build reaches).
- `timeout`: positive (`timeout must be positive`). On expiry the child's process group gets
  SIGKILL and the result is `TimedOut`.
- `output_limit`: per stream, at most 16 MiB (`output_limit must be at most 16 MiB`). Output
  past it is read and discarded and counted in `dropped` (both streams together), so a child
  that writes a lot never blocks on a full pipe.
- `input` (add-only, story `lsp-server`, 2026-11-05): the bytes written to the child's stdin,
  which is then closed. Empty, the default, keeps stdin the null device. A thread named
  `child-input` writes them while the watcher reads the output, so neither side blocks on a
  full pipe; a child that exits without reading ends the write with `EPIPE`, which is ignored
  (both targets ignore SIGPIPE). The language server passes an editor buffer to `ignite check
  --stdin-for` this way.
- `ChildExit.ending`: 0 exited (`code` is the exit status), 1 signalled (`code` is the signal
  number), 2 timed out, 3 killed (`code` -1 for both). `outcome(exit)` maps it to `Ending`; a
  fn named `ending` would shadow the field in a UFCS call.

Methods:

| method | result | behaviour |
|---|---|---|
| `launcher.execute(invocation)` | `Result<ChildExit, IoError>` | starts the child and waits for it on the calling task |
| `launcher.start(invocation)` | `Result<ChildProcess, IoError>` | starts the child and returns at once |
| `child.id()` | `Int` | the child's process id, which is also its process group id; pure |
| `child.wait()` | `Result<ChildExit, IoError>` | waits for the end; a second `wait` is the error `child already waited` |
| `child.kill()` | `Result<Unit, IoError>` | sends SIGKILL to the child's group while the child is unreaped; `Ok` and nothing sent once it was reaped; the result of a later `wait` is `Killed` unless it had already ended |

The substrate (`ignite_io::child`):

- Every child is started by its own watcher thread (named `child`), also for `execute`, so that
  `PR_SET_PDEATHSIG` (which fires when the starting thread ends) lives as long as the watcher.
- The child starts with `Command` (Rust std), stdin the null device (a pipe when `input` is
  not empty), stdout and stderr pipes,
  its own process group (`process_group(0)`) and an empty signal mask. Rust's `Command`
  resets SIGPIPE but keeps the signal mask of the thread that forks, which blocks SIGTERM (16.13;
  row 13 failed without the hook), so a `pre_exec` hook empties the mask; other dispositions the
  server inherited as ignored stay ignored. The same hook sets `PR_SET_PDEATHSIG` to SIGKILL
  (and exits at once if the parent already died), and marks every descriptor from 3 up
  close-on-exec with `close_range` (row 14), so a descriptor the server inherited without
  close-on-exec does not reach the child either.
- SIGCHLD is reset to its default action at program start on both targets (`drain::install`,
  which `ignite_io::run` and the runtime's `init` call) and before the first spawn (the test
  runner). A SIGCHLD inherited as ignored (a parent's `trap '' CHLD` survives exec) makes the
  kernel reap every child itself, so no status could be read. Children therefore start with
  SIGCHLD at its default too. Should a child still be reaped by someone else (`waitid` fails
  with ECHILD), it counts as ended with `ending 1 code -1`, and nothing signals its pid or
  group again, since either may be reused.
- The watcher reads both pipes with `poll`, enforces the deadline, waits for the child's end
  with `waitid(WEXITED | WNOWAIT)` (the child stays a zombie, so its pid and group id cannot be
  reused), sends SIGKILL to the group, reaps the child, then reads the pipes until end of file
  or 100 ms, whichever comes first. A grandchild that left the group and holds a pipe open
  therefore delays the result by at most 100 ms. `kill` sends to the group only while the child
  is unreaped, under the same lock as the reaping, so it never reaches a reused group id.
- A child counts against `maximum_children` from `start` until its `wait` returns (`execute`
  waits itself). An unwaited child keeps counting until the process exits, on both targets, so
  a program that starts and never waits runs into `too many children (<n>)` instead of growing.
  Nothing waits for a free slot.
- When the program ends in order (`main` returns, `Process.exit`, a fatal `?` report, the end of
  a SIGTERM drain, or a second SIGTERM during the drain), every live child's group gets SIGKILL
  before the process exits, through the exit hooks of `ignite_io` (21.12). When the program is
  killed (SIGKILL, a crash), `PR_SET_PDEATHSIG` kills each direct child; its grandchildren are
  not reached (21.15). Row 17.
- LLVM: `start` allocates a table slot and `wait` frees it, so a waited child leaks no slot;
  `execute` allocates none. The `ignite-rt` adapter answers a released `Child` handle (its
  generation tells released from never-allocated) as Rust answers a waited child: `wait` is
  `Err` `child already waited`, `kill` is `Ok`, `id` returns the id kept in the released entry
  (row 11). The entry keeps the id by the handle's generation, so a waited handle answers its
  id after a later handle reused the slot, as the Rust target does; that is 16 bytes per waited
  child until the process exits. An unwaited child keeps its slot and its output, up to twice `output_limit`, until
  the process exits; `maximum_children` bounds how many (21.15).

### 21.6 Arguments (`Setting`)

Resource kind `Setting`:

| property | type | default | meaning |
|---|---|---|---|
| `flag` | `Text` | required (E104) | the flag, `--` followed by `[a-z0-9-]+` |
| `default` | `Text` | absent | the value when the flag is not given; absent makes the flag required at launch |

- The generated `main` reads each setting's flag, and each `Directory`'s `flag`, from the
  process arguments before it binds a listener, by the rule of `--port` (`args::port_from_args`):
  the first `--flag V` or `--flag=V` wins; arguments it does not know are ignored, so `--port`
  and `--ref` keep their meaning and the same argument list serves all. A value after `--flag`
  that itself starts with `--` is no value: `--mode --tool /x` is `--mode needs a value`.
  `--flag=V` takes `V` as it is, also when it is empty or starts with `--`.
- Arguments are read as the bytes the OS gave, on both targets (`args_os` on Rust, `argv` kept
  as bytes by `ignite-rt`'s `init` on LLVM); nothing is decoded lossily. A value that is not
  UTF-8 has no `Text`, so the read that asks for it refuses it: `error: <flag>: value <bytes>
  is not UTF-8`, exit status 2, `<bytes>` as Rust's `escape_ascii` writes them (`/bin/\xff`),
  as 21.4 names an entry. This holds for `--port`, `--ref` (each binding is read) and every
  setting and directory flag. An argument that is not UTF-8 and that no read asks for is
  ignored, like any other argument a read does not know. (As landed, U4:
  `ignite_io::args::{flag,port,endpoint}_from_os_args`; before U4 the Rust target panicked in
  `std::env::args` on any such argument and the LLVM target replaced its bytes with U+FFFD.)
- A required flag that is absent ends the program with exit status 2 and `error: <flag>:
  required argument missing` on stderr; a flag without a value, exit status 2 and `error:
  <flag> needs a value`, as `--port` does (the `error: ` prefix is `ignite_io::run`'s).
- `setting.value()` returns the value as `Text` and needs `<root>.argument` (21.3).
- Flags are unique per system and differ from `--port` and `--ref` (E141).
- A `default` is part of the declaration and so of the model `ignite plan` prints; a secret
  belongs in a flag given at launch, not in a `default`.

`ignite run` and `ignite apply` pass no setting values in this slice. They start an executable
with its own arguments (`--port`, `--ref`), so a setting takes its `default` and a `Directory`
its `path`. A relative `path` resolves against the executable's working directory: under
`ignite run` the run's (F7), under `ignite apply` the `ignite host`'s, which a saved plan does
not record (21.16 Q2). A system that `ignite run` or `ignite plan --for-apply` would start with a
`Setting` without `default` is E142 before anything starts and before the host is contacted
(row 26): the alternative, a member that exits 2 at every restart until its policy gives up,
reports the same mistake late.

The model (section 15.5) is unchanged in format: each resource appears as `{"id", "kind",
"properties"}` with its declared and defaulted properties (`default` only when given), as
row 25 fixes.

### 21.7 Random bytes (`Entropy`)

Resource kind `Entropy`, no properties (E047 for any). At launch the generated `main` opens
`/dev/urandom` with `O_CLOEXEC` once; `entropy.random_bytes(count)` (op `draw`) reads `count`
bytes from it, `0..=65536`, else the error `random_bytes: count must be in 0..=65536`.
`/dev/urandom` gives cryptographically secure bytes once the kernel's pool is initialised,
which on Linux happens during boot; no new dependency (`getrandom`) is needed for that.

### 21.8 Typed API (add-only)

Builtin types (in `ignite-check`'s builtin type list and `ignite-types`' `Ty`): `Directory`,
`Launcher`, `ChildProcess`, `Entropy`, `Setting`. Declaring a type of one of these names is E004.

Builtin statics, all test-only (E051 outside tests and test-only items):

| static | type |
|---|---|
| `Directory.temporary()` | `Result<Directory, IoError>`: a new empty directory inside the per-test scratch directory that the test runner creates before it starts the test child and removes with its contents after it reaped the child, so a test killed at its timeout leaves nothing; the scratch directory lies under `std::env::temp_dir()`, and its path reaches the child in the environment variable `IGNITE_TEST_SCRATCH` |
| `Launcher.new()` | `Launcher` with `maximum_children = 16` |
| `Entropy.new()` | `Entropy` |
| `Setting.of(flag: Text, value: Text)` | `Setting` |

Builtin methods: those of 21.3, with the signatures of 21.4 to 21.7. They dispatch on the
receiver's type and are not in `BUILTIN_METHODS` (D3): `ignite-check` finds them by the
receiver's declared type in `CAPABILITY_METHODS` (type, method, effect operation), and
`ignite-types` types them by the receiver's type; none is added to `EFFECTFUL_METHODS`. The
std modules their signatures name are loaded with any file that names the type (16.22).

Resource kinds: `Directory`, `Launcher`, `Entropy`, `Setting`, beside `TcpListener` and
`Store`. The generated `main`'s order becomes: `run`; stores; settings and directory flags read
from the arguments; directories; launchers; entropy; bind listeners (with `--port`); build the
values; call `K.main`.

`ignite-io` (new public modules): `fs` (`Directory`), `child` (`Launcher`, `ChildProcess`,
`Invocation`, `ChildExit`), `entropy` (`Entropy`), and in `args` the function
`flag_from_args(args, flag) -> Result<Option<String>, IoError>`; in `lib.rs` an exit-hook
registry, `at_exit(hook: fn())`, whose hooks `exit`, the end of `run`, the end of a drain and
the second-SIGTERM path call once before the process ends. `ignite-rt`: the files `fs.rs`,
`child.rs`, `entropy.rs`, the `Handle` variants `Directory`, `Launcher`, `Child`, `Entropy`,
`Setting`, `repr(C)` records `IgniteInvocation` and `IgniteChildExit` beside `IgniteEndpoint`,
and `ignite_rt_args_flag` beside `ignite_rt_args_port` in `process.rs`. Public APIs of the
compiler crates only gain items.

### 21.9 Diagnostics

New codes (section 8 gets a "Codes of slice 17" paragraph):

| code | stage | meaning | fixture |
|---|---|---|---|
| E140 | type checker | a `Text` literal path argument of a `Directory` method that is absolute, has a `..` component or a NUL; the run-time check stays (row 2) | `e140_path_leaves_root` (ignite-types) |
| E141 | type checker | a flag of a `Setting` or `Directory` that another resource of the system also uses, or that is `--port` or `--ref` | `e141_flag_collision` (ignite-types) |
| E142 | `ignite run`, `ignite plan --for-apply` (before the host is contacted) | a `Setting` without `default` in a member the run or the plan would start | `e142_setting_without_default` (crates/ignite) |

Changed codes:

- E104 gains three texts: `` `path` of `Directory` must be a non-empty text without NUL``,
  `` `maximum_children` of `Launcher` must be in 1..=1024, got <n>``, `` `flag` must be `--`
  followed by lowercase letters, digits or `-`, got <text>`` (for `Setting` and `Directory`);
  and the required-field text for `Directory.path` and `Setting.flag`. A property that is not a
  literal where a literal is needed (`flag`, `path`, `default`, `maximum_children`) is E104 with
  `` `<p>` of `<Kind>` must be a literal``.
- E047 for an unknown property of the new kinds, listing the kind's properties.
- E020's resource text becomes ``unsupported resource kind `X`; expected `TcpListener`,
  `Store`, `Directory`, `Launcher`, `Entropy` or `Setting` `` in the type checker and in
  `lower_resources`.
- E051 covers the four test-only statics of 21.8.
- E016 and E042 cover the new methods as section 5.1 and 11.7 define them, with the existing
  text ``missing effect `<root>.<op>` for `<receiver>.<method>`; add it to the `requires` of
  `<fn>` `` (rows 8, 15, 21).
- E056 also names `clock` (row 35).
- `every_checker_code_has_a_fixture` and `every_type_code_has_a_fixture` list the new and
  extended codes; the `crates/ignite` fixture list gains E142.

### 21.10 Stale points

| # | where | stale text | resolution, by whom |
|---|---|---|---|
| S1 | `docs/vision.md` §7 T4 | the ranges end at "E130 to E139: slice 13 (F4)" | add "E140 to E149: slice 17 (playground substrate)"; U6 |
| S2 | `docs/layering.md` and `layering::` | no rows for `std.fs`, `std.process` | add both to the base layer with `std.net`; U1 |
| S3 | section 7 (substrate) and section 6.1 (system lowering) | list only TCP, pipes, stdin, stdout, process exit and arguments, and only `TcpListener` and `Store` | one forward line each to 21; U1 |
| S4 | `AGENTS.md` layout table | no `std/fs.ignite`, `std/process.ignite`; the `crates/ignite-io` row omits files, children and entropy | rows; U6 |
| S5 | `std/net.ignite` doc comment listing the substrate builtins | lists only the network builtins | unchanged; the new builtins are documented in `std/fs.ignite` and `std/process.ignite` (`Entropy`, `Setting` in `std/process.ignite`); U1 |
| S6 | the E020 text in fixtures and tests (row 31) | "expected `TcpListener` or `Store`" | the new text; U1 |
| S7 | `docs/design.md` §3.5 | effect interfaces for files and processes as ambient interfaces | unchanged (design is direction, T2); this section is the contract, and the difference is recorded in `docs/vision.md` §6 as a new X-row by U6 |
| S8 | `docs/vision.md` §3 invariant 3 | enumerates the handle types (`TcpStream`, `PipeStream`, `TcpListener`, `Store<T>`, …) | add `Directory`, `Launcher`, `ChildProcess`, `Entropy`, `Setting`; U6 |
| S9 | section 5.1, the roots rule | lists `TcpStream`, `TcpListener`, `Store`, `Stdin`, `Stdout` as handle types | add the five; U1 |
| S10 | `feat/slice16-language-gaps` 20.3.1, the move analysis' handle predicate | "a handle type (invariant 3)" without the five, which would be moved as aggregates | whichever of slice 16 and slice 17 merges second adds the five to that predicate and a move test for one of them; that unit's commit names S10 |
| S11 | `docs/vision.md` §3, the L0 substrate row | no files, children or entropy | add them; U6 |

### 21.11 Decisions

| # | decision | alternative | why |
|---|---|---|---|
| D1 | capabilities are resource handles passed in values | ambient statics with effects such as `fs.write` | P3: a handler may act only on its state; vision invariant 4 |
| D2 | four kinds, one per authority, each with its own op names | one `Host` capability with all four | a handler that only reads files should not be able to run programs; the state record composes them (probe p4), and distinct ops keep `requires [b.read]` from granting more than reads (21.3) |
| D3 | effects checked by the receiver's declared type, with `static_type` arms for the constructors and `start` | by method name in `EFFECTFUL_METHODS` | a by-name table would capture user fns named `start`, `value`, `wait`, `kill` or `execute` (F5); the arms spare tests the annotations `Store` needs |
| D4 | confinement by an `openat` walk with `O_NOFOLLOW`, every symlink refused | `openat2` with `RESOLVE_BENEATH` (follows inside links); `canonicalize` and a prefix check | the walk needs no kernel 5.6 feature; the prefix check races (row 4); inside links are rare in a workspace. Since story `fs-resolve-beneath` the walk is the fallback: `openat2` with `RESOLVE_BENEATH \| RESOLVE_NO_SYMLINKS` refuses inside links as the walk does and resolves in one call (21.4) |
| D5 | no `Directory.subdirectory` (attenuation) in this slice | `open(path) -> Directory` sharing the receiver's roots | on LLVM every call would hold a table slot until exit (F10), unbounded in a handler; relative paths cover the playground |
| D6 | `execute` beside `start` | only `start` and `wait` | `execute` allocates no LLVM slot and is what a run button does |
| D7 | one watcher thread per child, which also starts it | one shared reaper thread with `poll` over every pipe | the test runner's loop already works per child (F12); `PR_SET_PDEATHSIG` needs the starting thread to live as long as the child; `maximum_children` bounds the threads |
| D8 | environment as `List<Text>` of `NAME=value` | `Map<Text, Text>` | `List<Text>` already crosses the ABI (`texts` in `ignite-rt`); a map does not |
| D9 | `ChildExit.ending: Int` and the enum `Ending` beside it | the enum in the record | an enum with payload does not cross the ABI today; `Endpoint`-style records do |
| D10 | `Setting` as the name, a resource kind and not a parser | `Flag`, `Argument`; extending `std.cli` | `Argument` is a `std.cli` type; `Flag` is design §3.6's future CLI vocabulary; the story excludes a parsing library, and one flag per resource is none (21.16 Q6) |
| D11 | settings and the directory flag are read once at launch | a `process.args` static readable anywhere | P3, and a misconfigured server fails before it binds a port (row 18) |
| D12 | `ignite run` passes no setting values; E142 for a setting without default | `--setting member.flag=value` on `ignite run` | the playground starts its server directly; the flag syntax for `run` and the saved-plan identity of the values deserve their own decision (21.16 Q1) |
| D13 | `/dev/urandom`, no new dependency | the `getrandom` crate | one file read; AGENTS.md asks a line for every new dependency, and none is needed |
| D14 | E104 for malformed property values, new E141 for collisions | a new code for every property rule | E104 already carries property texts (the loopback rule of 14.11); a collision is a system rule, like E105 |
| D15 | a frontend unit U1 before the four behaviour units | each story adds its own frontend | the four touch the same tables in `ignite-check`, `ignite-types`, `ignite-eval` and both backends; one owner avoids four-way conflicts, as 16.18 to 16.20's skeleton did |
| D16 | a `pre_exec` hook with `close_range` and `PR_SET_PDEATHSIG`, the second `unsafe` block of `ignite-io` (the first is `net.rs`'s `shutdown`) | close-on-exec only; no death signal | close-on-exec cannot cover descriptors the server inherited (R10); without the death signal a killed server leaves children unbounded (R11) |
| D17 | a child counts against `maximum_children` until waited, on both targets | until reaped | an unwaited child keeps an LLVM slot; counting it bounds the slots (invariant 5) and keeps both targets alike |
| D18 | `clock` becomes an ambient head (E056) in this slice | defer the finding of probe p1 | it is the P3 hole this slice exists to respect, a one-line change, and no parameter named `clock` exists |

### 21.12 Work split

| unit | story | owns | depends on | first failing test | rows |
|---|---|---|---|---|---|
| U0 | `story:slice-17-contract` | this section, the section-8 rows, `docs/slice17-review.md` | — | — | — |
| U1 | `story:slice-17-frontend` | in `crates/ignite-check/src/lib.rs` the builtin types, statics, methods and effects, `can_hold_handle`, the `static_type` arms of 21.3, `clock` in `AMBIENT_HEADS`; in `crates/ignite-types/src/` the `Ty` variants (`lib.rs`), `builtins.rs`, `typer.rs` (`effect_site`, E140), `typer/effects.rs`, `typer/system.rs` (kinds, E104, E141); `crates/ignite-eval/src/{eval,value}.rs` (the model arm and builtin values); `std/fs.ignite` and `std/process.ignite` with records, helpers, builtin doc comments and the line `// ---- entropy tests ----`; `SUBSTRATE_MODULES`, `docs/layering.md`, `layering::`; resource, flag and method lowering in `crates/ignite-rustgen` and in `crates/ignite-llvm` (`lower.rs`, `region.rs`, `layout.rs`, `glue.rs` as needed), calling the substrate; skeleton modules `ignite_io::{fs,child,entropy}`, `args::flag_from_args`, the `at_exit` registry in `crates/ignite-io/src/lib.rs` called by `exit`, `run` and `drain.rs`, the per-test scratch directory in `crates/ignite-io/src/test.rs` (created before, removed after each test child); `crates/ignite-rt/src/{fs,child,entropy}.rs`, the `Handle` variants in `net.rs`, `ignite_rt_args_flag` in `process.rs`; the `nix` features `dir` and `poll`; every operation a placeholder arm (21.13); the fixtures of rows 8, 15, 19, 21 to 23, 27, 35; `studio.ignite` and the golden of row 25; the goldens of row 36; `CODEGEN_REVISION`; the forward lines and lists of S3 and S9 | U0 | `fixtures::fixtures_match_expected` (ignite-check) on `e016_directory_effect` | 8, 15, 19, 21 to 23, 25, 27 to 29, 31, 32, 35, 36 |
| U2 | `story:file-system` | `crates/ignite-io/src/fs.rs` and its unit tests (`Directory.temporary()` inside the scratch directory), `crates/ignite-rt/src/fs.rs`, the tests in `std/fs.ignite` | U1 | `fs::symlinks_out_are_refused` | 1 to 7 |
| U3 | `story:process-spawn` | `crates/ignite-io/src/child.rs` (the watcher, its `pre_exec` hook, its `at_exit` hook killing live groups); in `crates/ignite-io/src/test.rs` only `run_child` and `drain_output`, moved onto the watcher with the runner's process-group behaviour kept; `crates/ignite-rt/src/child.rs`; the process tests in `std/process.ignite` above the entropy line; fixtures `signal_mask.ignite`, `orphans.ignite` and their tests in `crates/ignite/tests/slice17.rs` (the file and its `mod` line) | U1 | the test "a child's output and status are captured" in `std/process.ignite` | 9 to 14, 16, 17 |
| U4 | `story:program-arguments` | `args::flag_from_args` in `crates/ignite-io/src/lib.rs` (the `args` module only), `ignite_rt_args_flag`'s body, the setting and directory-flag reads in both backends' generated `main` (U1 wrote the lowering, U4 completes what row 18 needs) and `CODEGEN_REVISION` if it changes generation; fixture `arguments.ignite` and its tests appended to `slice17.rs`; E142 in `crates/ignite/src/run/` and `crates/ignite/src/apply/plan.rs` with fixtures `e142_setting_without_default.ignite`, `studio_defaults.ignite` and the `run::` and `apply::` tests of row 26 | U1, U3 (for `slice17.rs`) | `slice17::rust::settings_read_their_flags` | 18, 26, 32 |
| U5 | `story:random-bytes` | `crates/ignite-io/src/entropy.rs`, `crates/ignite-rt/src/entropy.rs`, the entropy test in `std/process.ignite` below the entropy line | U1 | the test "two draws differ" | 20 |
| U6 | `story:slice-17-records` | row 24's test appended to `slice17.rs`; `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/vision.md` §3 (S8, S11), §4.1, §6 (S7) and §7 T4 (S1); `docs/performance/17/` | U2 to U5 | `slice17::rust::studio_serves_through_its_state` | 24, 34 |

Rows 30 and 33 belong to every unit.

Order: W1 U0; W2 U1; W3 U2, U3 and U5; W4 U4; W5 U6.

Shared files:

- W3: U2, U3 and U5 each own their own files in `crates/ignite-io/src/` and
  `crates/ignite-rt/src/`, which U1 created. `lib.rs` of both crates, `drain.rs`, `net.rs`,
  `process.rs` (rt) and `Cargo.toml` are U1's, and no W3 unit edits them: the exit kill goes
  through `at_exit`, the test scratch through U1's runner change. U3 alone edits `test.rs` in
  W3, and only `run_child` and `drain_output`. U2 adds tests to `std/fs.ignite`; U3 and U5 add
  tests to `std/process.ignite`, U3 above the line `// ---- entropy tests ----`, U5 below it.
- W4: U4 edits the `args` module of `crates/ignite-io/src/lib.rs`,
  `crates/ignite-rt/src/process.rs`, `crates/ignite/src/run/` and `apply/plan.rs` alone, and
  appends to `slice17.rs` after U3 merged.
- W5: U6 appends to `slice17.rs` after U4.

Rules: one integration-test binary per crate (`AGENTS.md`), so `slice17.rs` is a `mod` line in
`crates/ignite/tests/main.rs`. Each unit's commit names the rows of 21.0 it makes pass. The story
ids above are the contract's names; `story:slice-17-frontend`, `story:slice-17-contract` and
`story:slice-17-records` are new; the steering session creates or updates the stories, U0 does
not edit the store. Stale point S10 belongs to whichever of slices 16 and 17 merges second.

### 21.13 Placeholder arms

After U1 every new operation exists end to end and fails at run time with one of:

- `unimplemented!("slice 17: Directory.<method>")` and `unimplemented!("slice 17:
  Directory.temporary")` in `ignite_io::fs`;
- `unimplemented!("slice 17: Launcher.<method>")`, `unimplemented!("slice 17:
  ChildProcess.<method>")` in `ignite_io::child`;
- `unimplemented!("slice 17: Entropy.random_bytes")` in `ignite_io::entropy`;
- `unimplemented!("slice 17: args::flag_from_args")` in `ignite_io::args`.

The `ignite-rt` adapters of U1 call these and contain no placeholder of their own. No existing
program declares the new resources, so the workspace stays green after U1; rows 22, 25 and the
fixtures check without running anything. `studio.ignite` builds on both targets after U1
(checked by U1 with `ignite build`, not run).

### 21.14 Gate time

Method of 18.18 (section 15.1), through `tools/ignite-perf` (modes `cold`, `warm`, `programs`,
`cli`). "Before" is a `git archive` copy of the slice base, "after" the closed tree, each with its
own `CARGO_TARGET_DIR`. One cold gate per tree, then three interleaved warm passes. A pair whose
two 1-minute loads differ by more than 5 is dropped. Report the medians of the pair ratios next
to the absolute times.

Units: those of 18.18, plus `ignite test std/fs.ignite` and `ignite test std/process.ignite`
per target, cold and warm (after only), and the build of `studio.ignite`, both targets, dev and
release (after only). A unit that rises over 10 % gets a measured cause or the record labels the
cause inferred. No baseline moves. The apply suites stay the slice-13 exception.

Records: a "Slice 17" paragraph in `AGENTS.md`'s gate-time section; raw logs and scripts under
`docs/performance/17/`.


### 21.15 Deferred

- Symlinks that stay inside the root; `Directory.subdirectory` (attenuation, D5); renaming;
  permissions.
- Writing to a child's stdin, streaming output, pipes between children.
- Killing grandchildren when the program dies without an orderly exit: `PR_SET_PDEATHSIG`
  reaches only direct children (21.5); a cgroup or a child subreaper would reach the rest.
- Resetting signal dispositions that the server inherited as ignored, beyond what Rust's
  `Command` resets (21.5).
- Releasing an unwaited `ChildProcess` slot on LLVM before exit (F10); `maximum_children`
  bounds it meanwhile (D17).
- Setting values through `ignite run` and `ignite apply` (D12).

### 21.16 Open questions

Each has a default the units follow unless the steering session says otherwise.

| # | question | default |
|---|---|---|
| Q1 | How does `ignite run` pass a setting's value? | not in this slice; E142 when a started member has a setting without default |
| Q2 | What is a relative `Directory.path` relative to? | the executable's working directory: `ignite run`'s under `run` (F7), `ignite host`'s under `apply`, which the saved plan does not record; a declaration meant for `apply` uses an absolute `path` |
| Q3 | Is 16 MiB the right read bound, and fixed? | fixed 16 MiB, as the other bounds of 21.4 and 21.5; a property later if a consumer needs more |
| Q4 | Should `Setting` values be redacted? | the journal records no argument values; the model prints a `default`, so a secret is passed at launch and never written as a `default` (21.6) |
| Q5 | Does a `Directory` root need `create = true` by default? | no: a missing root is a launch error unless the declaration asks for creation |
| Q6 | Is `Setting` inside `story:program-arguments`, whose out-of-scope line excludes "an argument-parsing library beyond what `std.cli` declares"? | yes: one flag per resource read by the generated `main` is no parsing library (D10); the steering session confirms when it updates the story |
| Q7 | Should `inherit_environment` default to false? | no: the playground's children (`ignite`, `cargo`) need `PATH` and `HOME`; the risk is documented in 21.5 |

### 21.17 Resolution of the review

`docs/slice17-review.md` (37 findings at `212d2eb`) is resolved in this text as follows.

| finding | resolution |
|---|---|
| R1 (blocker) | rows 8, 15, 21 quote the existing E016 text; 21.9 states it |
| R2 (blocker) | `child.id()` added (21.5, pure); row 17 prints it |
| R3 (blocker) | the `ignite-rt` adapter answers a released `Child` handle as Rust answers a waited child (21.5); row 11 adds `kill` after `wait` |
| R4 (blocker) | `static_type` arms for the four constructors and `start` (21.3, D3, U1) |
| R5 | `absolute_path` needs `read` (21.3, 21.4) |
| R6 | U1 owns `lib.rs`, `drain.rs`, the `at_exit` registry, the runner's scratch directory and `ignite_rt_args_flag`'s declaration; U3 edits only `run_child` and `drain_output` in `test.rs`; U4's `args` edit is stated (21.12) |
| R7 | F11 and row 29 corrected: `fs` and `process` are on already, U1 adds `dir` and `poll`, observed with `cargo tree -e features` |
| R8 | row 23 runs both crates' fixtures; `e017_ambient_in_handler` declares its `HttpApp`-shaped record |
| R9 | stale points S8, S9, S10 |
| R10 | `close_range` from 3 in `pre_exec` (21.5, D16); row 14 leaks a descriptor 5 on purpose |
| R11 | the second-SIGTERM path runs the exit hooks; `PR_SET_PDEATHSIG` from the watcher thread (21.5, D7, D16); row 17 gains both cases; grandchildren deferred (21.15) |
| R12 | a child counts until waited, on both targets (21.5, D17); row 16 checks the slot comes back after `wait` |
| R13 | `read_file` refuses non-regular files via `O_NONBLOCK` and `fstat` (21.4); row 3 adds a FIFO |
| R14 | `waitid(WNOWAIT)`, then `killpg`, then reap; `kill` sends only while unreaped (21.5) |
| R15 | row 4 uses `renameat2(RENAME_EXCHANGE)` and allows any `Err`; 21.4 maps `ELOOP` and `ENOTDIR` |
| R16 | row 36 (goldens on both backends); row 19 checks the code list; 21.9 names the lists |
| R17 | row 26 uses `e142_setting_without_default` and adds `plan --for-apply` |
| R18 | F5, D3 and row 30 name the real collisions (`start`, `value`, `wait`, `kill`, `execute`) |
| R19 | 21.3 cites vision §6 L4 |
| R20 | rows 18 and 21.4, 21.6 give the `error: ` prefix |
| R21 | `<p>` is the path as given; row 6 expects both forms |
| R22 | the fixture fns of rows 8, 15, 21 declare `requires` |
| R23 | stale point S11 |
| R24 | 21.6 and Q2 name `ignite host`'s working directory under `apply` |
| R25 | a value starting with `--` is no value (21.6, row 18) |
| R26 | bounds: `output_limit` at most 16 MiB, 65,536 directory entries, 256 levels for `remove_tree` (21.4, 21.5; rows 7, 12) |
| R27 | row 26 runs in a temporary working directory |
| R28 | the runner keeps its process-group behaviour (F12, U3) |
| R29 | `Directory.temporary()` lives in a runner-owned per-test scratch directory (21.8) |
| R30 | 21.5 says what `Command` resets; the rest is deferred (21.15) |
| R31 | rows 10 and 17 count a zombie as gone |
| R32 | U1's list completed; U4 bumps `CODEGEN_REVISION` if it changes generation; row 32 runs `apply_admission::` |
| R33 | row 31 reads `git diff --stat` |
| R34 | 21.5 documents the inherited environment, 21.6 the printed `default`; Q4, Q7 |
| R35 | row 6 makes an outside link with `Launcher` and checks on both targets that a read through it is refused and that removal leaves the target |
| R36 | distinct ops `draw` and `argument` (21.3, D2) |
| R37 | Q6 |

### 21.18 The close (U6)

Row 24 is `slice17::{rust,llvm}::studio_serves_through_its_state` in `crates/ignite/tests/slice17.rs`.
It passed on its first run: U2 to U5 had landed what it needs. A mutation (`--tool /bin/true`)
fails it on both targets at `POST /run` with `(200, "")`.

Stale points of this section, found during the slice and fixed at the close:

- the opening said sections 19 and 20 were not on `main`; both are;
- the base was `c539392`; the slice started at `651be04c`, and the merge base at the close is
  `99647132` (planning-store commits only after `651be04c`); rows 31 and 32 diff against
  `651be04c`, and row 32 names the revisions (`0.1.0-16` at the base, `-17` after U1, `-18`
  after U4);
- LLVM became the default target before U1 (PR 32): a paragraph after the base says what a
  command without `--target` builds, and row 26 names the target of `run::studio_defaults_runs`;
- row 18 passed `--ref peer=127.0.0.1:9`; a `Ref` resolves by its path `<System>.<member>`
  (16.9), so the test passes `--ref Arguments.peer=127.0.0.1:9`, and the row now says so.

The facts of 21.1 at the close:

| fact | at the close |
|---|---|
| F1 | holds: `--port` and `--ref` keep their meaning beside the settings (row 18) |
| F2, F3 | resolved by U1: the four capabilities are builtin types with `CAPABILITY_METHODS`; E020 names the six kinds (row 31) |
| F4 | holds; the handler of row 24 reaches the capabilities only through `state` |
| F5 | resolved by U1 as D3: `CAPABILITY_METHODS` by receiver type, nothing added to `EFFECTFUL_METHODS` (row 30) |
| F6 | resolved by U1: `clock` is in `AMBIENT_HEADS` (row 35) |
| F7 | confirmed by U4: `run::studio_defaults_runs` finds the relative root `workspace` in the run's working directory |
| F8 | resolved by U3 (row 13), as stated in 21.1 |
| F9 | resolved by U3: the child closes every descriptor from 3 up (row 14) |
| F10 | resolved by U3: `wait` releases the child's slot, and a released child handle answers as a waited child on both targets (row 11) |
| F11 | resolved by U1: `ignite-io` enables `signal`, `dir` and `poll` (row 29) |
| F12 | resolved by U3: `ignite_io::child::watch` is the watcher of `Launcher` and of the test runner's `run_child` |
| F13 | holds: the new kinds appear in the model unchanged in format (row 25) |
| F14 | holds: the names of F14 stay free (row 30) |
| F15 | resolved: E140 to E142 are taken; `docs/vision.md` §7 T4 reserves E140 to E149 for slice 17 (S1) |

One gap found during the slice and closed by U6: the standard `HttpServer<S>` had no `machine`
hook (16.16), so a system that used it directly under the standard `Deployment` with
`nativeProcess` was E129, and the studio fixtures of row 26 and of the hosting rule (15.6) wrapped
it in a kind of their own. `HttpServer` now has `recovery: Restart` and `rollout: DrainRollout`
(both defaulted), `HttpServer.machine` as `TelnetServer.machine` and `HttpServer.validate` (E120:
`limits.head` at least 1 byte, `limits.idle_timeout` and `limits.drain_timeout` positive).
`studio_deployment.ignite` uses `HttpServer` directly, and
`run::studio_under_the_standard_deployment_runs` runs it under `ignite run` with its supervisor
`Running`. The model of every `HttpServer` member gains the two defaulted properties
(`todo.model.json`, `studio.model.json`). `studio_defaults.ignite` keeps its own kind: its
placement is not `std.deploy`'s.

### 21.19 The command line (`CommandLine`), added after the slice

Added 2026-11-05 by `story:program-argument-list`: a command-line tool with subcommands and
positional arguments (`todo add "buy milk"`, `todo done 3`) needs the whole argument list, which
`Setting` (one `--flag V` each, 21.6) does not give. The form follows 21.3 (P3) and D11 as
`Setting` does.

Resource kind `CommandLine`, no properties (E047 for any):

```ignite
type Todo {
  command_line: CommandLine
  executable: Text
}

fn Todo.main(self: Todo) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  ...
}

system TodoCli {
  resource command_line: CommandLine
  component cli: Todo { command_line = command_line, executable = "todo" }
}
```

- The generated `main` reads the process arguments once at launch, with the settings (21.8's
  order: settings and command lines, directories, launchers, entropy): every argument after the
  executable's name, in order, flags included. `todo add "buy milk" --port 0` gives `["add",
  "buy milk", "--port", "0"]`; `--port`, `--ref` and the setting flags keep their meaning, as
  every read sees the same list (21.6). No arguments is the empty list.
- Every argument is read, so each needs a `Text`: an argument that is not UTF-8 ends the
  program before `K.main` with exit status 2 and `error: argument <n>: value <bytes> is not
  UTF-8`, `<n>` its position from 1 after the executable's name and `<bytes>` as 21.6 writes
  them.
- `command_line.arguments() -> List<Text>` needs `<root>.argument`, the op of `setting.value()`:
  both read the launch arguments, and a command line can read every flag a setting reads, so a
  separate op would grant nothing narrower (D2 separates authorities, not two views of one).
- `CommandLine.of(arguments: List<Text>) -> CommandLine` is the test-only constructor (21.8);
  anywhere else it is E051, as `Setting.of(..)`.
- D11 holds: there is no `process.args` static and no new ambient head. A handler reaches the
  command line only if its state holds the handle.
- Parsing (subcommand tables, help text) stays out of `std` (the story's out-of-scope line); a
  program or `std.cli` builds it on the list.
- `ignite run` and `ignite apply` start an executable with `--port` and `--ref` only (21.6, D12),
  so under them a `CommandLine` sees those.
- `ignite-io`: `args::CommandLine` (`of`, `launch`, `from_os_args`, `arguments`), re-exported
  as `ignite_io::CommandLine`. `ignite-rt`: the `Handle` variant `CommandLine` and the entries
  `ignite_rt_command_line_launch`, `ignite_rt_command_line_of`,
  `ignite_rt_command_line_arguments`. `ignite-types`: `Capability::CommandLine`, the builtins
  `CommandLineOf` and `CommandLineArguments`. The model (15.5) is unchanged in format.
- E020's resource text (21.9) ends ``…, `Entropy`, `Setting` or `CommandLine` ``.
  `apply::plan::CODEGEN_REVISION` becomes `ignite-codegen/0.1.0-21`.

Tests: `slice17::{rust,llvm}::the_command_line_reaches_the_program` and
`an_argument_that_is_not_utf8_ends_the_launch` (fixture `slice17/command_line.ignite`);
`slice17::the_command_line_parse_runs_inline_on_both_targets` runs that file's inline tests,
which parse `CommandLine.of(..)`; the inline test "a command line keeps its arguments in order"
in `std/process.ignite`; `e051_command_line_of` (ignite-check).

### 21.20 Environment, sleep, wall clock and flush, added after the slice

Added 2026-11-07 by the process builtins story. The case study
(its gaps G2, G3 (sleep only), G8, G13; `docs/friction.md` rows 61, 62,
67, 72) needed environment variables, a sleep, a wall clock and an explicit flush.

| builtin | type | effect | behaviour |
|---|---|---|---|
| `environment.get(name: Text)` | `Option<Text>` | `<root>.variable` | the value of the variable `name` when the program was launched; `None` when it was not set |
| `Environment.of(entries: List<Text>)` | `Environment` | test-only (E051) | entries `NAME=value`, split at the first `=`; the last entry of a name wins; an entry without `=` or with an empty name is skipped |
| `Clock.sleep(duration: Duration)` | `Unit` | `clock.sleep` | sends what the calling task wrote, then blocks the task for `duration`; zero or less does not block |
| `Clock.now()` | `Duration` | `clock.read` | the wall clock since 1970-01-01T00:00:00Z in nanoseconds, the instant of `std.time`; `Clock.now().to_ms()` is epoch milliseconds |
| `Process.flush()` | `Unit` | none | sends what the calling task wrote (section 7), as a wait or the end of the task would |

Resource kind `Environment`, no properties (E047 for any):

```ignite
type Relay {
  environment: Environment
  executable: Text
}

fn Relay.main(self: Relay) -> Result<Unit, IoError> {
  let store = self.environment.get("SESSION_STORE").unwrap_or("memory")
  ...
}

system RelayService {
  resource environment: Environment
  component relay: Relay { environment = environment, executable = "relay" }
}
```

- The generated `main` reads the environment once at launch, in the first group of 21.8's
  order (settings, command lines and environments; directories; launchers; entropy). Later
  changes to the process environment are not seen.
- A variable whose name or value is not UTF-8 is skipped: it cannot be a `Text`, and refusing
  it would end a program over a variable it never reads. The launch never fails.
- `ignite run` and `ignite apply` start an executable with the environment of the driver or of
  the host process, unchanged.

Decisions:

- `Environment` is a capability, not an ambient static (G2 guessed `Env.get`): 21.3 (P3) and D11
  hold, so there is no new ambient head, and a handler reaches the environment only through its
  state. Its op is `variable`, not `argument`: the environment and the command line are
  separate authorities (D2), and `requires [b.argument]` grants no variable.
- `get` returns `Option<Text>`, so an unset variable differs from an empty one.
- `Environment.of` takes the `NAME=value` form of `Invocation.environment` (21.5).
- `Clock.sleep` and `Clock.now` are statics of `Clock`, beside `Clock.monotonic`, with ambient
  effects under the existing head `clock` (E056 already names it). `Clock.now` needs
  `clock.read`, as `Clock.monotonic` does: both read a clock. `Clock.sleep` needs
  `clock.sleep`. As with `Clock.monotonic`, an `HttpApp` handler cannot call them: its fn type
  grants `state.*` only (E017, probe p2 of 21.0). A program sleeps in `K.main` or in a spawned
  task.
- `Clock.now` returns a `Duration`, not a new instant type: `std.time` already reads an instant
  as a `Duration` since 1970-01-01T00:00:00Z, so `month_start(zone, Clock.now())` works. It is
  negative before 1970 and saturates at the bounds of `Int`.
- `Clock.sleep` flushes first. Section 7 sends a task's writes before every operation that may
  wait, and a sleep is one. G3's waiter that writes and then sleeps sends its bytes.
- The flush is `Process.flush()`, not a free `flush()`. A builtin free function name is E004 for
  every user fn of that name (`BUILTIN_FUNCTIONS`), and programs declare `fn flush`: the
  examples repository's ember does. `Process` is already a static owner, so the new name breaks
  nothing.
- `Process.flush()` is pure, as `log` is: it grants no authority and only sends bytes that the
  task was already allowed to write. So a handler may call it, and the evaluator returns `Unit`.
- The manual clock. `ignite run --clock manual` (16.4) times the reducers; no program reads it.
  - Under either `--clock`, a program's `Clock.sleep` waits in real time, and `Clock.now`
    reads the real wall clock. `Clock.monotonic` already behaved this way.
  - The journal holds only what the driver observes, at the manual instant at which it
    observes it. A member that sleeps changes when its exit is seen in real time, not the
    exit's `at`.
  - Replay re-steps the journal and reads no program clock, so `ignite run --clock manual`
    and `ignite replay` stay deterministic.
  - A probe attempt that sleeps longer than the settle wait stays in flight (16.4).

Layers:

- `ignite-io`: `clock::sleep`, `clock::now`, and `environment::Environment` (`of`, `launch`,
  `from_os_vars`, `get`), re-exported as `ignite_io::Environment`.
- `ignite-rt`: the `Handle` variant `Environment` and the entries
  `ignite_rt_environment_launch`, `ignite_rt_environment_of`, `ignite_rt_environment_get`,
  `ignite_rt_clock_sleep`, `ignite_rt_clock_now` and `ignite_rt_flush`.
- `ignite-types`: `Capability::Environment` and the builtins `EnvironmentOf`,
  `EnvironmentGet`, `ClockSleep`, `ClockNow` and `ProcessFlush`.
- `ignite-check`: the rows of `BUILTIN_TYPES`, `BUILTIN_STATICS`, `EFFECTFUL_STATICS`,
  `static_effect`, `CAPABILITY_METHODS` and `CAPABILITY_TYPES`.
- E020's resource text ends ``…, `Setting`, `CommandLine` or `Environment` ``.
- `apply::plan::CODEGEN_REVISION` becomes `ignite-codegen/0.1.0-40`.
- The model (15.5) is unchanged in format.

Out of scope: timers, condition variables and channels (the rest of G3, `docs/actors.md`), and
`setrlimit` (G10).

Tests:

- The inline tests "an environment gives a variable's value, …", "a sleep waits at least its
  duration, …", "the wall clock is an instant of std.time …", "a flush sends what the task
  wrote …" and "a sleep sends what the task wrote first" in `std/process.ignite`, on both
  targets.
- `process_builtins::{rust,llvm}::g2_reads_an_environment_variable`, `g3_sleeps` and
  `g8_reads_the_wall_clock`, and `process_builtins::g13_flush_is_callable_on_both_targets`.
  These run the reproducers adapted to these names (`fixtures/process_builtins/case_gaps.ignite`).
- `run::a_member_sleeps_and_reads_the_wall_clock_in_real_time_under_the_manual_clock`
  (fixture `run_clock.ignite`).
- The ignite-check fixtures `e016_process_builtins` and `e051_environment_of`.
- The ignite-io and ignite-rt unit tests of the new functions.

## 22 Slice 18: one service operated end to end (F5)

**Status: contract written 2026-11-05. The independent review (`docs/slice18-review.md`) found 4
blockers, 6 majors and 18 minors; all are resolved in this text, and 22.17 maps each finding to
its resolution. Implemented 2026-11-05 on `feat/slice18-one-service`, units U1 to U8; the
standing of each row of 22.0 is in `STATUS.md`, the rows reworded as landed are 18 and 27, and the
gaps found during the slice are 22.16 Q6 to Q10.** The rows of 22.0 are required checks, not results.

This section is numbered 22 and follows section 21 (slice 17, merged into `main` as PR 34,
`421742b3`); no line of section 21 changes here.

Sources:

- the planning store at `c4e24bc`: `epic:slice-18-one-service-operated`,
  `story:slice-18-contract`, `story:apply-recovery-way-out`;
- `docs/research/review-2026-11-04.md`: findings R2, R10, R11, R13, priorities 1, 2 and 4, and
  Timo's correction in its section 2 (the LLVM target is the fast-build path and the target the
  service is operated on);
- this file: 15.4 and 15.5 (the model and its encoding), 16.5 to 16.10 (journal, reducers,
  probes, signals, contract evaluation), 16.16 and 16.17 (hooks and their checks), 17 (`ignite apply`), 21.3 to 21.6
  (`Directory`, capability resources, settings); `docs/vision.md` §3 invariants 5, 6, 7 and 9,
  §7 T4.

Base: `c19ab173`, the head of `feat/slice17-processes-files` when this contract was written; rebased onto `f3abeb20` (slice 17's U6 close, 21.18), whose `HttpServer` changes are cited at that commit (F8, 22.5, row 9).
Every code citation below was read at that commit, nothing was built or run (the contract is
documents only). Once slice 17 merges, the base becomes the merge of slice 17 into `main`; every
`git diff` of 22.0 and the before tree of 22.15 are taken from it.

The slice makes one real HTTP service, the todo example, an operated system: its data lives in
a `Directory` (21.4) and survives a SIGKILL and an `ignite apply` rollout; an HTTP health probe
kind and its outcome type are declared in `examples/todo/`, and an `Availability` contract over
them is evaluated by the existing reducers through a typed predicate; `ignite apply` gets fault
rows on that service and a way out of a recovery no corrective resolves; the build and the
edit-build-restart cycle are timed on both targets.

### 22.0 Acceptance

`both` means the test runs on the Rust and the LLVM target. Fixture paths without a directory
are under `crates/ignite/tests/fixtures/`. "Temporary working directory" means a directory under
the test's own scratch directory that the test passes as `current_dir` to every process it
starts, so that the relative `Directory` path of 22.4 never resolves inside the repository.

| # | check | command | exact required observation | unit |
|---|---|---|---|---|
| 1 | the Telnet outcomes are predicates | `ignite test std/sli.ignite --target all` | the test "the prompt outcomes are predicates" passes on both targets: for `fast = ProbeAttempt { received = true, latency = 50 ms }`, `slow = { received = true, latency = 900 ms }`, `missed = { received = false, latency = 2 seconds }`: `prompt_received_before_timeout` is true, true, false; `prompt_received_within_threshold(_, 200 ms)` is true, false, false; `prompt_received_within_threshold(slow, 900 ms)` is true. `std/sli.ignite` declares no fn `satisfies` and no fn `known_outcome` | U1 |
| 2 | the design file keeps both outcomes | `ignite check examples/terminal/terminal.design.ignite`; `cargo test -p ignite --test main plan::` | silent, exit 0, with `good = prompt_received_before_timeout` (contracts `reachable`, `customerAccess`) and `good = prompt_received_within_threshold` (`promptFast`); `plan::design_model_is_byte_identical_across_processes_and_directories` passes against `crates/ignite/tests/golden/terminal.design.model.json`, whose three contracts carry `"good": {"fn": "std.sli.prompt_received_before_timeout", "type": "Fn"}` (resp. `…within_threshold`), and the explain golden `terminal.design.explain.Terminal.reachable.txt` names the same fn | U1 |
| 3 | the classify hook is required | `cargo test -p ignite --test main plan::fixtures_match_expected` | `e150_classify_missing.ignite` (a probe kind and a contract kind declared in the fixture; the contract's `machine` returns `Evaluate` and its kind declares no `classify`) gives exactly one E150: ``contract `S.c` of kind `fixture_e150.Watch` subscribes to `S.p.beats` of `fixture_e150.Beat` but declares no `fn Watch.classify(self: Watch, sample: fixture_e150.Beat) -> Bool` ``; the filter of the test includes `e15` | U1 |
| 4 | the element type must be an outcome | same | `e151_undecodable_element.ignite` (a probe kind with `beats: Signal<fn() -> Int>`, run by the assembly, and a contract whose machine subscribes to it) gives exactly one E151: ``signal `S.p.beats` carries `fn() -> Int`; a probe outcome holds no fn, handle, reference or signal`` | U1 |
| 5 | the run of the terminal is unchanged | `cargo test -p ignite --test main run::` | every `run::` test passes; `run::terminal_runs_under_ignite_run{,_on_the_llvm_target}` publish `Compliant` standings for `Terminal.reachable` as before; the journal header's `formatVersion` is `ignite-journal/0.2`; every `signal` record's `value` is a `std.sli.Sample` and every `Emitted` event a `std.sli.Verdict` | U1 |
| 6 | an old journal is refused | `cargo test -p ignite --test main run::e126_journal_mismatch` | the fixture's journal carries `formatVersion` `ignite-journal/0.1` (it does already, `crates/ignite/tests/run.rs:1010` to `:1029`); after the bump `ignite run` exits 2 and, newly in the test, `ignite replay` exits 1 (16.3: replay maps `Error::Diagnostics` to exit 1 like `plan`), each with exactly one E126 of the existing version text (``the journal has formatVersion `ignite-journal/0.1`, this driver writes `ignite-journal/0.2` ``, `crates/ignite/src/lib.rs:1459` to `:1475`); the fixture's `.expected` changes from the model text to that text | U1 |
| 7 | reducers agree | `cargo test -p ignite --test main control_trace::` | `control_trace::reducers_agree_across_the_evaluator_and_both_targets` passes with the fixture's emissions written as `Sample` and its evaluator events as `Verdict` | U1 |
| 8 | no general layer names the probe | `git grep -n -e HttpHealth -e HealthCheck -e 'examples\.todo\.health' -- 'crates/*/src' std 'tools/*/src'` | prints nothing, exit 1. Tests and goldens under `crates/*/tests` may name the example (row 11's golden does); the epic's "no file under `crates/`" means the general layers, and the steering session rewords the epic line to match | every unit |
| 9 | the todo checks | `ignite check examples/todo/todo.ignite` | silent, exit 0, with its server bound by `nativeProcess` (the `HttpServer.machine` hook landed in slice 17, 21.18; `std/protocol/http.ignite:144`, `:145`, `:160` to `:169` at `f3abeb20`) | U4 |
| 10 | a store is hosted by its creator | `cargo test -p ignite --test main -- plan::fixtures_match_expected plan::store_is_hosted_by_its_server` | `store_is_hosted_by_its_server`: `ignite plan store_hosted.ignite` (a `TcpListener`, a `Store<Int>` and an `HttpServer` over it; bindings for the listener and the server only) exits 0 and its `edges` hold `{"from": "S.server", "kind": "hosts", "to": "S.counts"}`; `e110_unbound_member` keeps its expected output | U2 |
| 11 | the todo model | `cargo test -p ignite --test main plan::todo_example_model_matches_golden` | passes against `crates/ignite/tests/golden/todo.model.json`, whose `machines` are exactly `TodoService.health.scheduler`, `TodoService.reachable.evaluator`, `TodoService.server.rollout`, `TodoService.server.supervisor`; whose `observations` hold `{"element": "examples.todo.health.HealthCheck", "interval": {"ns": 10000000000, "type": "Duration"}, "producer": "TodoService.health", "ref": "TodoService.health.checks"}`; and whose `edges` hold `hosts` edges from `TodoService.server` to `TodoService.data`, `TodoService.loaded` and `TodoService.todos` | U4 |
| 12 | data survives SIGKILL | `cargo test -p ignite --test main todo::` | `todo::{rust,llvm}::todos_survive_sigkill`: `todod --port 0` in a temporary working directory; `POST` titles `a`, `b`, `c` (ids 1, 2, 3), `DELETE /todos/1`, `PATCH /todos/2 {"done":true}`; SIGKILL; a new `todod` in the same directory answers `GET /todos` with exactly `[{"id":2,"title":"b","done":true},{"id":3,"title":"c","done":false}]`, `GET /todos/1` with 404, and the next `POST` with id 4; afterwards `todo-data/todos/` lists exactly `1/`, `2/`, `3/`, `4/`, and `todo-data/todos/2/` lists exactly `2.json` | U4 |
| 13 | concurrency kept | same | `todo::{rust,llvm}::{concurrent_posts,concurrent_patches}` pass unchanged in a temporary working directory each; after `concurrent_posts`, a SIGKILL and a restart in the same directory, `GET /todos` lists 1,600 todos | U4 |
| 14 | inline tests of the todo | `ignite test examples/todo/todo.ignite --target all` | the existing tests "to_json", "handle creates, lists, reads, updates and deletes" and "handle answers 400, 404 and 405" pass on both targets with the same assertions (their helpers `answer` and `status` take the state record, and their `Store.new()` stores become the state record of 22.4 built with `Directory.temporary()`); the new test "a todo written is loaded by a fresh state" passes: a todo created through one state record is listed through a second record over the same directory with fresh stores | U4 |
| 15 | the probe kind | `ignite test examples/todo/health.ignite --target all` | the test "a check line is the model encoding" passes on both targets: `encode_check(HealthCheck { answered = true, status = 200, latency = 12 ms })` equals `{"fields":{"answered":true,"latency":{"ns":12000000,"type":"Duration"},"status":200},"type":"examples.todo.health.HealthCheck"}` plus `\n`; `healthy` holds for it and not for `status = 503` or `answered = false` | U4 |
| 16 | the contract reaches each status | `cargo test -p ignite --test main slice18::` | `slice18::{rust,llvm}::todo_contract_reaches_every_status`: `ignite run <copy>/todo.ignite --clock manual --target <t> --port TodoService.ingress=<free>` in a temporary working directory; the test advances the clock by `10 seconds` per slot and waits for each attempt's `signal` record (or its `AttemptFailed` event) before the next advance. Phases: 3 good slots; 4 slots with the run's copied probe executable (`<state>/bin/<generation>/health`) replaced by a script that exits 1, so the attempts fail and emit nothing while `expected` grows; the executable restored and 31 good slots, so the gap leaves the 5-minute window; then SIGSTOP of `todod` and 3 slots past their timeouts; then SIGCONT before `stop`. The statuses of the `standing` records of `TodoService.reachable`, consecutive repeats collapsed, are exactly `Compliant`, `InsufficientEvidence`, `Compliant`, `Violated` (an evaluator publishes only at an emission, and the first emission has `expected` 1, `std/control.ignite:631` to `:645`, `:793` to `:844`) | U6 |
| 17 | durable across SIGKILL and a rollout, LLVM | same | `slice18::llvm::todo_survives_sigkill_and_a_rollout`: `ignite host` in a temporary working directory; plan and apply the todo copy (`--target llvm`), exit 0; `POST /todos {"title":"milk"}` answers 201 id 1; SIGKILL of the `TodoService.server` pid that `ignite inspect --json` reports; within 10 s inspect reports another pid ready; `GET /todos` is `[{"id":1,"title":"milk","done":false}]`; the copy's health body `ok\n` is changed to `ok 2\n`, replanned and applied, exit 0, with the action `replace` for `TodoService.server`; `GET /health` answers `ok 2\n` and `GET /todos` is the same list | U6 |
| 18 | R11: host killed after an effect, before its receipt | same | `slice18::llvm::a_host_killed_after_dispatch_never_reports_an_unobserved_start`: the rollout of row 17 in three host runs: a host under `IGNITE_DEBUG_HOST_FAULT=after_intent` accepts the rollout and dies after journaling its acceptance; a host under `after_dispatch` resumes it and dies after dispatching its spawn; a clean host starts last. (As landed, U6: the row first named one host under `after_dispatch`, which cannot be built: such a host dies at the restart of the committed revision 1 that every host start dispatches first (17.5), before any rollout is submitted.) The clean host's inspect reports the operation either `completed` with a `Ready` record in the host journal for the pid it names, or `recovery_required` with reason `outcome_unknown`; in neither case is more than one process running `revisions/*/…/todod`, and within one run of an operation the host journal holds no two `Started` records of `TodoService.server` with one generation (each host start begins a fresh journal segment whose generations count from 1 again, 17.5, so the rule holds per run, not across runs); `GET /todos` lists `milk` once the operation is `completed` (after a retry of the same plan if it was resumed) | U6 |
| 19 | R11: a plan saved before a source change | same | `slice18::llvm::a_plan_saved_before_a_source_change_is_refused`: plan saved, then a response text of the copy changed; `ignite apply` exits 2 with a reason naming the changed source file; the `TodoService.server` pid in inspect is unchanged, no `transition` record follows the refusal, `GET /todos` lists `milk` | U6 |
| 20 | R11: a torn last record | same | `slice18::llvm::a_torn_last_journal_record_is_dropped_and_recorded`: host stopped with `stop`; the bytes `{"kind":"sta` appended to `host-journal.jsonl`; the next `ignite host` prints `ignite host: dropped a torn final journal record of 12 bytes` on stderr, its journal's first new record is `journalRepaired` with `{"droppedBytes": 12, "droppedDigest": "sha256:…"}` (the digest of the 12 bytes), `ignite inspect --json` holds `journalRepairs` with that entry, and the committed revision serves `GET /todos` with `milk` | U3 (unit test), U6 (row) |
| 21 | R11: an altered last record | same | `slice18::llvm::an_altered_last_journal_record_refuses_the_host`: host stopped; one byte inside the last record's `value` changed (the line stays valid JSON); `ignite host` exits 3 with stderr ``error: host journal record <n>: digest mismatch``, starts no process, and leaves every file of the state byte-identical; `ignite inspect --json` exits 0 with `"online": false`, `"journal": {"intact": false, "record": <n>, "reason": "digest mismatch"}` and `"nextAction": "restore host-journal.jsonl or move the state aside; the host does not start on a damaged journal"` | U3 (unit test), U6 (row) |
| 22 | R11: the last records cut at a line boundary | same | `slice18::llvm::a_journal_cut_before_its_last_state_record_refuses_the_host`: host stopped; the journal truncated to the end of the line before its last `state` record; `ignite host` exits 3 with ``error: host journal ends before record <n> that status.json anchors``; inspect reports `"journal": {"intact": false, "record": <n>, "reason": "shorter than status.json"}` and the `nextAction` of row 21 | U3 (unit test), U6 (row) |
| 23 | a revision journal that differs from the host's copy | `cargo test -p ignite --test main apply_integrity::` | `apply_integrity::an_altered_revision_journal_line_refuses_the_host`: after a committed apply of the terminal design (Rust), one byte of the last record of `revisions/<r>/journal.jsonl` that has a `revisionRecord` copy in the host journal is changed; `ignite host` exits 3 with ``error: revisions/<r>/journal.jsonl record <seq>: differs from the host journal``; inspect reports `"journal": {"intact": false, "record": <seq>, "reason": "revision journal differs", "file": "revisions/<r>/journal.jsonl"}` | U3 |
| 24 | journal integrity, unit level | `cargo test -p ignite --lib apply::host::journal::` | `a_torn_final_line_is_dropped_and_repaired` (row 20's bytes give `journalRepaired`), `an_altered_record_names_its_index` (`History::load` fails with `host journal record 3: digest mismatch` for an altered third record), `a_journal_shorter_than_its_anchor_is_refused`, `a_missing_anchor_is_accepted` (a state with no `status.json`, or one without `journalAnchor`, loads as today) pass | U3 |
| 25 | a way out of a non-resolvable recovery | `cargo test -p ignite --test main apply_way_out::` | `apply_way_out::{rust,llvm}_operator_recover_returns_the_host_to_accepting_plans`: a host driven into the state of 22.9 (a committed revision, a failed operation `gave_up`, and a later reason no corrective resolves); `ignite apply <corrective>` exits 2 with `host has an active or unresolved operation` (`crates/ignite/src/apply/host/admission.rs:279` to `:281`) and a retry of the failed plan exits 1 with `operation requires recovery; inspect its durable status` (`:244` to `:249`); `ignite recover --state S --operation <id>` exits 0 and prints `recovered <id>: stopped <n> instances; committed revision <r> restarted`; the host journal holds one `operatorRecovered` record naming the operation, the stopped pids and `"by": "ignite recover"`, and between the `recoverStarted` record and the fresh start no `Started` record of any member; inspect reports `recovery` null, the operation's receipt `state` `abandoned` with its reasons kept, and the committed revision ready; a retry of the abandoned plan exits 2 with `operation <id> was abandoned by ignite recover; prepare a new plan`; a new plan applies with exit 0 and `completed` | U5 |
| 26 | recover refuses what it cannot prove | same | `apply_way_out::recover_refuses_an_unaccounted_process`: as row 25 with a process running an executable under `<state>/revisions/` that no ownership record names; `ignite recover` exits 1 with `recovery refused: unaccounted process <pid> runs <path>`, signals nothing (the process is alive afterwards), and journals nothing; `ignite recover` of an operation that is not `recovery_required` exits 2 with `operation <id> requires no recovery` | U5 |
| 27 | the build is a fraction of rustc's | 22.15, `ignite-perf programs` | the records of 22.15 exist; in the after tree the median dev build of the todo program (`ignite build examples/todo/todo.ignite`, both executables `todod` and `health`; harness unit `todod-<target>-dev`) on LLVM is at most 0.15 of the median on Rust in the same passes (the 12b rows give 0.309 / 3.998 = 0.077). As landed (U8): the row first asked for `todod` alone, timed apart from `health`; `ignite build` has no option to build one executable of a program, and the slice adds no compiler change only to time one, so the unit times the program's build, which holds both | U8 |
| 28 | the edit-build-restart cycle | 22.15, `ignite-perf apply-cycle` | records exist for the todo (after tree, both targets) and the terminal design (both trees, both targets); in the after tree the todo cycle's median on LLVM is below its median on Rust | U7 (mode), U8 (records) |
| 29 | the code lists | `cargo test -p ignite --test main plan::fixtures_match_expected`; section 8 | section 8 has a "Codes of slice 18" paragraph naming E150 and E151 with their fixtures; both fixtures exist and match | U0 (paragraph), U1 (fixtures) |
| 30 | reducer and evaluator revisions | `git diff <base> -- crates/ignite/src/apply/plan.rs`; `cargo test -p ignite --test main apply_recovery::a_changed_reducer_revision_starts_the_host_without_driving_and_asks_to_replan` | `REDUCER_EVALUATOR` differs from the base (U1); the test passes; `CODEGEN_REVISION` is unchanged unless a unit changes generation, which that unit's commit then says | U1 |
| 31 | gate | `task gate` | exit 0 on the slice branch at the close | every unit |
| 32 | gate time | `ls docs/performance/18/`; `grep -n "^Slice 18" AGENTS.md` | the directory holds the raw records, logs and scripts of every unit of 22.15; `AGENTS.md`'s gate-time section has one paragraph starting `Slice 18` that names the gate's head, exit, cargo passed/failed/ignored counts and inline counts per target | U8 |
| 33 | the host keeps its working directory | `cargo test -p ignite --test main apply_integrity::a_host_started_elsewhere_is_refused` | a host started once in directory A writes `<state>/workdir` (A, canonical); stopped and started from directory B it exits 2 with ``error: state <S> is hosted from <A>; start ignite host there``, starts no process and changes no file; started from A again it runs. A state without `workdir` (written before the slice) gets one at its next start | U3 |
| 34 | `classify` is a checked hook | `cargo test -p ignite-types --test main fixtures::` | fixture `e109_classify_signature` (a contract kind whose `classify` takes no `sample`, and one whose `classify` declares `requires [stdout.write]`) gives exactly two E109 | U1 |

### 22.1 Current state and verdicts (at `c19ab173`)

Read, not run: every fact below was read in the cited code; a statement marked "inferred" was
not.

| fact | where | verdict |
|---|---|---|
| F1 a contract's `good` is a `Text` matched against two Telnet outcomes; any other text holds for no attempt and `validate` refuses it (E120) | `std/sli.ignite:22` (`satisfies`), `:30` (`known_outcome`); `std/contract.ignite:33`, `:132` | R2 holds. `good` becomes `fn(T) -> Bool` (22.3, D1) |
| F2 the reducers carry `ProbeAttempt` in `AttemptDone`, `Emitted` and `Emit`, and the scheduler builds the timeout sample itself | `std/control.ignite:63`, `:66`, `:88`, `:519` | a probe whose outcome is another type cannot pass through them; the payloads become `std.sli.Sample` and `Verdict` (22.3) |
| F3 the evaluator classifies with `satisfies` and derives the slot from the sample's latency | `std/control.ignite:639`, `:626` | the classification moves to the driver's routing; the slot comes from the scheduler (22.3, D14) |
| F4 the driver decodes a probe's line by the payload type of `EventKind.AttemptDone`, not by the signal's element type | `crates/ignite/src/run/live.rs:303` to `:311`, `:883` | the doc comment of `crates/ignite/src/run/probe.rs:4` ("decodes it by the signal's element type") and the review's R5 verdict are true only because the one element type is `ProbeAttempt`; U1 makes them true (22.3) |
| F5 the model already records each signal's element type | `ModelObservation::element`, `crates/ignite-eval/src/model.rs:233` to `:241` | the driver reads it (22.3) |
| F6 the driver routes an emission to every alive subscriber unchanged | `Core`, the `Emit` arm, `crates/ignite/src/run/engine.rs:565` to `:581` | the classification point (22.3) |
| F7 the machine set is closed | `std/control.ignite:24` to `:33` (O39) | kept (out of scope, R4); the probe kind returns `Schedule`, the contract `Evaluate` |
| F8 `HttpServer<S>` has `recovery`, `rollout`, `machine` and `validate` since slice 17's close (21.18), so a `nativeProcess` binding of it is no longer E129 | `std/protocol/http.ignite:144` to `:169` at `f3abeb20`; `std/deploy.ignite:97` to `:100` | nothing left for this slice; at `c19ab173` the hook was missing (`:137` to `:149`) |
| F9 the todo example keeps its data in a `Store<Todo>` in memory and declares no assembly | `examples/todo/todo.ignite:80` to `:89` | R10 holds; 22.4 |
| F10 only a capability resource is hosted implicitly; `admits` hosts a resource only when it is a `TcpListener` | `implicit_host`, `crates/ignite-eval/src/resolve.rs:341` to `:370`; `admits`, `std/deploy.ignite:52` to `:77` | inferred: an assembly of the todo with its `Store` resource is E110 today; U2 hosts a `Store` as a capability (22.5, D6) |
| F11 replacing a member whose kind starts with `Store<` is an unsupported migration; an implementation-only change gives `replace` only to members with an artifact | `stateful_kind`, `crates/ignite/src/apply/plan.rs:930`; `annotate`, `:551` to `:556`, `:620` to `:630` | inferred from the code: a changed todo build replaces `TodoService.server` and leaves `TodoService.todos` `unchanged`; row 17 checks it |
| F12 `write_file` replaces atomically by rename, without `fsync` | 21.4 | a SIGKILL of `todod` keeps every file whose write returned (the page cache outlives the process; inferred from POSIX, row 12 checks); a power loss is not covered (22.2) |
| F13 the host journal is a SHA-256 digest chain per record; a final line without `\n` is dropped silently; a broken chain or digest stops the host with exit 3 | `chained`, `crates/ignite/src/apply/host/journal.rs:129` to `:141`; `History::load`, `:199` to `:233`; `host::run`, `crates/ignite/src/apply/host/mod.rs:86` | a checksum exists; what is missing is a record of the drop, an index in the error, an anchor against a cut at a line boundary, and an offline report (22.7) |
| F14 offline `ignite inspect` reads `status.json` only | `inspect`, `crates/ignite/src/apply/mod.rs:391` to `:424` | it reports a damaged journal as a healthy snapshot; 22.7 |
| F15 every run record the host acknowledges is copied into the host journal as `revisionRecord`; a retained revision replays its own `journal.jsonl` at start | `Host::record`, `crates/ignite/src/apply/host/controller.rs:1316` to `:1328`; `crates/ignite/src/apply/revision.rs:118` to `:130` | the copy is the checksum of the revision journal (22.7); inferred: every record of a revision journal but an unacknowledged tail has a copy, row 23 checks |
| F16 a corrective is admitted only for `gave_up` and `listener_unavailable`; under any other later reason every apply is refused: a corrective with exit 2 `host has an active or unresolved operation`, a retry of the failed plan with exit 1 `operation requires recovery` | `resolvable`, `crates/ignite/src/apply/host/controller.rs:142` to `:147`, `correctable`, `:410` to `:426`; `crates/ignite/src/apply/host/admission.rs:244` to `:249`, `:279` to `:281`; 17.5 "Precedence" | `story:apply-recovery-way-out` holds; U5 (22.9) |
| F17 crash points exist after intent, before and after dispatch and mid-transition | `IGNITE_DEBUG_HOST_FAULT`, `crates/ignite/src/run/process.rs:95` to `:125`, `:278`; `crates/ignite/src/apply/host/journal.rs:176` to `:182` | row 18 reuses `after_dispatch`; no new crash point |
| F18 the apply tests start the host with the repository as working directory, and `todo.rs` starts `todod` without one | `crates/ignite/tests/apply_recovery.rs:73`, `apply_host.rs:99`; `crates/ignite/tests/todo.rs:88` to `:95` | a relative `Directory` would land in the repository; every slice-18 test passes a temporary working directory (22.0) |
| F19 E150 to E159 are unused; T4 ends at slice 13 on `main` and slice 17 takes E140 to E149 | section 8, 21.9; `docs/vision.md:250` | this slice takes E150 to E159 (S1) |

### 22.2 Scope and non-goals

In scope: typed probe outcomes and contract predicates (R2, R13's measurement), supervision of
`HttpServer`, the durable todo with its HTTP probe kind and contract, the fault rows of R11 on
that service, journal integrity reporting, the operator way out of a non-resolvable recovery,
and the timing of builds and of the apply cycle.

Not in scope:

- more than one host, fencing, distributed authority (R8);
- user-defined machines (R4) and a new protocol (R13's other parts); the probe writes its HTTP
  request by hand (D13);
- binding executables written in other languages (R12);
- durability against power loss or a kernel crash: `Directory.write_file` does not `fsync`
  (21.4); a durable store in the substrate (D4 C) is a later slice;
- a schema migration of the todo files; the format of 22.4 is `version 1`;
- consistency between two processes of the todo: during an occupied-listener rollout the
  draining instance may still answer a request on an open keep-alive connection after the new
  instance has loaded (16.12). Each process allocates ids and versions from its own memory
  store, so the two can write the same `<shard>/<id>/<version>.json`, and `write_file` replaces
  (21.4): an acknowledged write in the drain window can be lost or hidden from the new instance.
  This is a known limit, stated here, in the todo's header comment and in `AGENTS.md` (S9, S10);
  rows 17 and 18 create their todo before the rollout and send no request on the old instance
  after it; a store shared across processes is D4 C's (22.16 Q3);
- the todo's apply cycle before the slice: the todo cannot be applied at the base (F9, F10; at the slice-17 base before 21.18 also F8),
  so its cycle is timed after only and the terminal design's on both trees (D12). The epic asks
  for a todo before; the steering session records this deviation in the epic;
- changing the default fuel, the closed machine set or the `ignite run` command surface.

### 22.3 Typed outcomes: `good` is a predicate (D1)

A probe kind declares its outcome type `O` as the element type of a `Signal<O>` output, prints
one line per attempt, the 15.4 encoding of an `O`, and returns `Machine.Schedule` from its
`machine` hook, as `TelnetPrompt` does. A contract names a predicate over `O`:

```ignite
type Availability<T> {
  signal: Signal<T>
  good: fn(T) -> Bool
  target: Ratio
  window: Window
  missing_data: Text = "insufficient_evidence"
  minimum_completeness: Ratio = 90%
}

fn Availability.classify<T>(self: Availability<T>, sample: T) -> Bool = self.good(sample)
```

- `std.contract`'s three kinds become generic in `T`: `Availability<T>` and `SLA<T>` with
  `good: fn(T) -> Bool`, `ThresholdCompliance<T>` with `good: fn(T, Duration) -> Bool` and
  `classify` = `self.good(sample, self.threshold)`. `T` is inferred from `signal` and `good`, as
  `S` of `HttpServer<S>` is from its application (the todo declares `HttpServer` without type
  arguments, `examples/todo/todo.ignite:84`); a predicate over another type is E040.
- A predicate is a named top-level fn: the model encodes it as `{"fn": "<qualified name>",
  "type": "Fn"}` (15.4); a closure there is E118 as today.
- `std.sli` keeps `ProbeAttempt` and declares the two Telnet outcomes as predicates with their
  old names: `fn prompt_received_before_timeout(attempt: ProbeAttempt) -> Bool =
  attempt.received` and `fn prompt_received_within_threshold(attempt: ProbeAttempt, threshold:
  Duration) -> Bool = attempt.received && attempt.latency <= threshold`. `satisfies` and
  `known_outcome` are removed; `reasons` in `std/contract.ignite` drops its `good` check, because
  an unknown name is now a name-resolution error (E003) at check time.
- Declarations change from text to name: `good = "prompt_received_before_timeout"` becomes
  `good = prompt_received_before_timeout` (D2).

`std.sli` gains the two records the reducers carry instead of `ProbeAttempt`:

```ignite
// One attempt as a scheduler emits it: the slot it was dispatched for and the
// probe's printed line, `None` when the attempt timed out.
type Sample {
  slot: Duration
  line: Option<Text>
}

// One emission as an evaluator receives it: the slot and whether its contract's
// `classify` held for the decoded line (`false` for `line = None`).
type Verdict {
  slot: Duration
  good: Bool
}
```

`std.control` changes (the reducer interface, 16.6):

- `EventKind.AttemptDone(Int, Text)`: the probe's line. The driver still decodes it at once with
  the element type of the scheduler's `signal` (the model observation whose `ref` is that field,
  so a probe kind with several `Signal` outputs is decoded by the one it schedules) and turns an undecodable line into `AttemptFailed` with
  `undecodable probe line: <reason>` (`live.rs:883` to `:899`), so the line a reducer sees
  always decodes.
- `ProbeScheduler` gains `in_flight_slot: Option<Duration>`; `AttemptDone` emits
  `Emit(signal, Sample { slot = <in_flight_slot>, line = Some(line) })`, the deadline emits
  `Sample { slot, line = None }` (today `ProbeAttempt { received = false, latency = timeout }`,
  `:519`). `AttemptFailed` still emits nothing (missing evidence).
- `Effect.Emit(Text, Sample)`, `EventKind.Emitted(Text, Verdict)`.
- `Evaluator` loses `good` and `threshold`; `emission` counts `verdict.good` in the bucket of
  `verdict.slot` (D14: the dispatched slot, not `at - latency`).
- `std.control` imports `Sample` and `Verdict` from `std.sli` and no longer `ProbeAttempt` or
  `satisfies`.

The driver (`crates/ignite/src/run/`, and its callers in `crates/ignite/src/lib.rs` and
`crates/ignite/src/apply/revision.rs`):

- Today the driver gets one `TypedProgram`, that of `std.control`: `ignite run` takes
  `modules.get(CONTROL_MODULE).types` (`crates/ignite/src/lib.rs:1579` to `:1591`) and passes
  it to `Live::new` and `replay::replay` (`:1613`, `:1659` to `:1668`), as `ignite replay`
  (`:1726` to `:1733`) and the apply revision (`crates/ignite/src/apply/revision.rs:111` to
  `:115`, `:129`, `:186`) do. `Value::decode` finds only records of that module or of a module
  it was typed against (`TypedProgram::record_fields`, `crates/ignite-types/src/lib.rs:1440` to
  `:1448`), so a `HealthCheck` line would not decode. The Telnet case works only because
  `std.sli` is an import of `std.control`.
- After U1, `Live::new`, `Core` and `replay::replay` also receive the loaded modules
  (`Loaded::modules()`), and decoding a line looks up the typed program of the element type's
  module by the qualified name in the observation's `element` (F5), not `AttemptDone`'s payload
  (F4). The element text is parsed into a `Ty` by the type printer's grammar (15.4); the outcome
  rule of E151 keeps it to the types that grammar names.
- At the `Emit` arm of `Core` (F6), for each alive subscriber, the driver builds the `Verdict`:
  for `line = Some(l)` it decodes `l` with the signal's element type and calls the subscriber
  member's hook `K.classify(contract, sample)` through the evaluator, with the step's fuel; for
  `None`, `good = false` without a call. The `signal` record keeps the `Sample`; each subscriber's
  `Emitted` event carries its own `Verdict`. A `classify` that fails (fuel, a run-time error) is
  E128 of that machine, as a failing reducer step is.
- Replay re-derives each `Verdict` from the journaled `Sample` and the model (16.15); a
  journaled `Emitted` that differs is E127, as today.
- `JOURNAL_VERSION` (`crates/ignite/src/run/engine.rs:26`) becomes `ignite-journal/0.2`: a
  journal of 0.1 is E126 through `check_header`'s existing version text (row 6, D3). `REDUCER_EVALUATOR`
  (`crates/ignite/src/apply/plan.rs:21`) changes, so a host whose committed revision was planned
  before the slice reports `reducer_revision_changed` and asks to replan (17.5 D2).
- The hook names the driver knows become `machine`, `validate` and `classify` (vision invariant
  6 lists hooks by name; U1 updates its line).

Check-time rules (in `ignite-eval`, beside E129):

- E150: a member whose machine subscribes to a signal, and whose kind declares no `classify`
  hook with `self` and one parameter of the signal's element type returning `Bool` (row 3).
- E151: a signal that the assembly runs (its producer's binding has entries, `std.deploy.entries`,
  as E121 and E129 decide; the compiler names no binding variant, 16.17) and that a member with
  a machine subscribes to (`ModelMachine::subscribes`: every signal ref in the member's
  properties at any depth, a list element included, collected by `signal_refs` in
  `crates/ignite-eval/src/model.rs`, which E121 and E150 read too), whose element type is not an outcome type. An outcome type is `Bool`, `Int`,
  `Text`, `Byte`, a quantity (`Duration`, `ByteSize`, `Ratio`), or a `List`, `Option`, tuple,
  record or enum of outcome types. This is a policy, not a claim about decoding:
  `Value::decode` also reads fn, `Ref` and `Signal` values (`crates/ignite-eval/src/value.rs:520`
  to `:551`), but a probe outcome holds none of them (row 4).
- E109 also checks `classify`: `fn K.classify(self: K, sample: T) -> Bool`, pure, through
  `hook_signature` (`crates/ignite-types/src/typer/system.rs:1662` to `:1673`); row 34.

### 22.4 The durable todo (D4, D7)

`examples/todo/todo.ignite` declares:

```ignite
type Todo {
  id: Int
  title: Text
  done: Bool = false
  version: Int = 1
  deleted: Bool = false
  deleted_by: Int = 0
  stored: Int = 0
  superseded: Int = 0
}

type Todos {
  todos: Store<Todo>
  loaded: Store<Bool>
  data: Directory
}

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>
  resource loaded: Store<Bool>
  resource data: Directory { path = "todo-data", create = true }

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = Todos { todos = todos, loaded = loaded, data = data }, handle }
    executable = "todod"
  }

  component health: HttpHealth {
    target = server
    path = "/health"
    interval = 10 seconds
    timeout = 2 seconds
  }

  contract reachable: Availability {
    signal = health.checks
    good = healthy
    target = 99%
    window = rolling(5 minutes)
    missing_data = "insufficient_evidence"
  }
}

assembly local: Deployment {
  system = TodoService
  bindings = {
    TodoService.ingress = localSocket()
    TodoService.server = nativeProcess(includes = [])
    TodoService.health = scheduledProbeHost()
  }
}
```

`HttpHealth`, `HealthCheck` and `healthy` are imported from the sibling module
`examples.todo.health` (22.6). The window is five minutes so that row 16 reaches `Compliant`
within a test (31 slots of 10 s); the values are the example's, not a recommendation.

Files, under the root of `data` (the working directory of `todod`, of `ignite run` or of
`ignite host`, 21.6 and 21.16 Q2; D7). Under `apply` that working directory is recorded: the
host writes `<state>/workdir` at its first start and refuses to start from another directory
(exit 2, row 33, U3), so a host restarted elsewhere cannot silently open an empty `todo-data`.
This settles 21.16 Q2's open end for `apply` without changing 21.4's resolution rule.

- `<shard>/<id>/<version>.json`: the `to_record` JSON of the todo at that version, an object with
  `id`, `title`, `done`, `version` and `deleted`, written with `write_file` (atomic replace,
  21.4). The HTTP bodies keep the three fields of `to_json` (`examples/todo/todo.ignite:20`), so
  every existing response and test is unchanged.
- The shard of an id (`shard_of`) is `todos` for ids 1 to 65,536 and `todos-<k>` for ids
  `k * 65536 + 1` to `(k + 1) * 65536`. `read_directory` lists at most 65,536 entries (21.4) and
  every id keeps its directory forever, so no shard outgrows a listing; the root of `data`
  lists one shard per 65,536 ids, which bounds the ids at about 4.29 billion (65,536 shards).
  Before W3's fix every id was under `todos/`, and the 65,537th made every later load fail
  (`todo::todo_w3_adversary::*::the_65537th_todo_survives_a_restart`); a `todos/` of up to
  65,536 ids written before is the first shard as it is.
- A deleted todo is a file whose `deleted` is true at the next version (a tombstone); its id is
  never reused.

Behaviour:

- The memory `Store<Todo>` is the authority inside a process; every change goes through one
  atomic `update` or `upsert` that also increments `version` (`f` stays pure, 11.3), and the
  handler then creates `<shard>/<id>` (`create_directory`; `write_file` needs its parent, 21.4)
  and writes the returned value's file before it answers. A failed write answers 500 with the
  body `not stored\n`; 500 means "not durable", not "not applied": the memory value stays, and a
  later successful write of that id persists it. After the write, the save removes one file, decided
  in a second atomic `update` over the memory-only fields `stored` (the newest version whose file
  this process wrote or loaded) and `superseded` (the one that file replaced): a save newer than
  `stored` takes its place and removes the file of `superseded`; a save older than `stored` (its
  rename came after a newer save's) removes its own file. Concurrent saves of one todo therefore
  remove each superseded file once and list no directory (story `todo-save-without-races`); a
  failed removal is ignored (the next load takes the highest version), and a file a crash leaves
  between rename and removal stays on disk, where loads skip it as older.
- Load: each request first checks `loaded.get(0)`; while it is not `Some(true)`, the request
  loads: `create_directory("todos")`, then for each shard directory at the root of `data`
  (`todos/` and every `todos-<digits>/`; other entries are skipped) and each `<id>/` in it the
  highest `<version>.json` that parses, merged with `todos.upsert(id, |held| keep the higher version)`, then
  `loaded.put(0, true)`. Loads are idempotent and monotone, so concurrent first requests may each
  load and no load overwrites a newer change. `GET /health` loads too, then answers 200
  `ok\n`; a failed load answers 503 `loading failed: <reason>\n`.
- `POST /todos` takes `todos.next_id()` until `todos.get(id)` is `None` (after the load every id
  on disk, tombstones included, is in memory), so ids continue after a restart (row 12).
- `GET` and the listing skip tombstones; `PATCH` and `DELETE` of a tombstone answer 404.
- `DELETE` decides 204 or 404 inside its one atomic `update`, not from a `get` before it: the
  request takes a token, `loaded.next_id()`, and the tombstone it makes carries it in
  `deleted_by` (memory only; `to_record` leaves it out and a loaded todo has 0). The request
  answers 204 when the value `update` returns carries its token, else 404, so of two concurrent
  DELETEs of one todo exactly one answers 204, as two in sequence do (W3 adversary,
  `todo::todo_w3_adversary::*::concurrent_deletes_answer_one_204`).
- The tests of `todo.ignite` build the state record with `Store.new()` twice and
  `Directory.temporary()?` (21.8).

### 22.5 `HttpServer` under `ignite run` and `ignite apply` (D5, D6)

- `std/protocol/http.ignite`: nothing to do. Slice 17 (21.18) gave `HttpServer<S>` `recovery`,
  `rollout`, `machine` and `validate` (`std/protocol/http.ignite:144` to `:169` at `f3abeb20`),
  and updated `todo.model.json` and `studio.model.json` itself.
- `crates/ignite-eval/src/resolve.rs`: `implicit_host` also hosts a resource of kind
  `Store<T>` (F10): by the process of the first declared executable component that creates it.
  A `Store` still provisions nothing (16.14). `admits` is unchanged.

### 22.6 The HTTP probe kind (`examples/todo/health.ignite`)

The module `examples.todo.health` declares, and nothing under `crates/`, `std/` or `tools/`
names (row 8):

```ignite
type HealthCheck {
  answered: Bool
  status: Int
  latency: Duration
}

fn healthy(check: HealthCheck) -> Bool = check.answered && check.status == 200

type HttpHealth {
  target: Ref
  path: Text = "/health"
  interval: Duration = 10 seconds
  timeout: Duration = 2 seconds
  checks: Signal<HealthCheck>
  executable: Text
}
```

- `HttpHealth.main`: `Endpoint.of(self.target)?`, connect, set the idle timeout to `timeout`,
  write `GET <path> HTTP/1.1\r\nHost: <address>\r\nConnection: close\r\n\r\n`, read until the
  first `\r\n`, take the status from `HTTP/1.1 <code> …`, print `encode_check(check)`. A refused
  connection, a read past the timeout or a status line that does not parse is `answered = false,
  status = 0`; exit 0. No `--ref` binding exits 1, as `TelnetPrompt.main` (16.9).
- `HttpHealth.machine`: `Machine.Schedule(ProbeScheduler { id = "{id}.scheduler", member = id,
  target = self.target.to_text(), signal = self.checks.to_text(), interval, timeout })`.
- `HttpHealth.validate`: `path` starts with `/` and has no space.
- `encode_check` writes the 15.4 encoding, keys in byte order (row 15).
- `target` is the kind-erased `Ref` (`Ty::Ref(None)`, 14.10), not `Ref<HttpServer<Todos>>`:
  `examples.todo` imports the probe, so `health.ignite` must not import `examples.todo` back
  for `Todos` (E032). E122 still checks that the ref resolves to exactly one listener (16.9).
  `health.ignite` imports general std modules only (`std.net`, `std.control`, `std.sli`,
  `std.text`, the `IoError` result type), never `examples.todo`. `Endpoint.of` takes the
  kind-erased `Ref` (`("Endpoint", "of") => sig(EndpointOf, vec![Ty::Ref(None)], …)`,
  `crates/ignite-types/src/builtins.rs:297`), and E122 matches any `Ty::Ref(_)`
  (`crates/ignite-eval/src/validate.rs:509`).

### 22.7 Journal integrity (D8 to D10)

The host journal's digest chain (F13) is the checksum: a record whose bytes changed fails its
digest, a removed or reordered record breaks the chain. No length frame is added (D8). This
slice adds:

- A torn final line (bytes after the last `\n`) is still dropped, because every record is synced
  before the host acts on it (17.5), so a torn line is a write the host never acted on. Today
  `History::load` truncates and syncs at once (`journal.rs:208` to `:211`). After U3 the load
  is split: a read-only verifier (chain, digests, anchor, revision journals; used by offline
  `inspect`) and the repair, which runs after the lock is held and appends `journalRepaired
  {"droppedBytes": n, "droppedDigest": "sha256:<hex of the bytes>", "previousDigest": <last good>}`
  in place of the torn bytes in one atomic step (`Writer::repair`: the good records and the
  repair record written to `host-journal.<pid>.writing` in the same directory and synced, renamed
  over the journal, the directory synced), so a crash after the rename leaves the record and a
  crash before it leaves the old journal, torn line included, to repair again; the good records
  never stand without the repair record (unit test
  `a_crash_during_the_repair_leaves_the_torn_journal`). The host prints `ignite host: dropped a
  torn final journal record of <n> bytes`, and `inspect` lists every such record in `journalRepairs` (row 20). Offline `inspect`
  of a journal that is only torn reports `"intact": true, "pendingRepair": {"droppedBytes": n}`.
- An altered record: `History::load` reports its 1-based line number, `host journal record <n>:
  digest mismatch` (or `: does not parse`, `: chain diverged`), exit 3, before anything is
  written (row 21). The host does not truncate or rewrite the journal (D10).
- A cut at a line boundary: `persist` writes `journalAnchor: {"record": n, "digest": "<digest>"}`
  of the `state` record it just appended into `status.json`. At start, a `status.json` whose
  anchor names a record the journal does not hold with that digest is ``host journal ends before
  record <n> that status.json anchors``, exit 3 (row 22). A cut after the anchored record is not
  detectable and is reconciled as a crash after that record (17.5). A `status.json` without
  `journalAnchor` (written before the slice) is accepted.
- A revision journal (F15): at start, before a retained revision is replayed, every record of
  `revisions/<r>/journal.jsonl` that has a `revisionRecord` copy must equal it: the host
  journal keeps the parsed record (`("record", record.clone())`, `controller.rs:1316` to
  `:1328`), so the check is that the copy's compact encoding equals the line. Records after the
  last copy (written, never acknowledged) are kept as today. A difference is
  ``revisions/<r>/journal.jsonl record <seq>: differs from the host journal``, exit 3 (row 23).
  The check runs in `host/mod.rs` before `verify_history` (`recovery.rs:218` to `:283`), so U3
  does not touch `recovery.rs`.
- Offline `inspect` (F14) verifies the chain, the anchor and the revision journals read-only and
  adds `journal: {"intact": bool, "record"?, "reason"?, "file"?}`; when `intact` is false its
  `nextAction` is the text of row 21. Online `inspect` reports `{"intact": true}` and
  `journalRepairs`.

### 22.8 Fault matrix (R11)

| R11 case | 17.7 row covering it on the terminal | slice 18 row on the todo | outcome required |
|---|---|---|---|
| interrupted deployment (crash after intent, before dispatch, mid-transition) | 11 | none new | covered; unchanged |
| lost acknowledgement: host killed after an effect, before its receipt | 11 (`after_dispatch` in `crash_after_intent`, `apply_recovery.rs:472`) | 18 | `completed` only over an observed `Ready`, else `outcome_unknown`; one instance |
| client disconnect | 10 | none new | covered |
| stale plan: saved before a source change | 5 (changed source of `apply::{rust,llvm}_saved_apply_…`) | 19 | refused, exit 2, nothing changes |
| stale base, wrong state | 5 | none new | covered |
| corrupted journal: torn last record | none end to end (unit test `a_torn_final_line_is_dropped_and_the_chain_continues`) | 20 | dropped and recorded |
| corrupted journal: altered record | first record only (`apply::tampered_host_history_is_rejected_before_startup`, `apply.rs:756`) | 21, 23 | refused, exit 3, reported offline |
| corrupted journal: cut at a line boundary | none | 22 | refused when the anchor shows it |
| uncertain external effect | 12 (unaccounted generation process) | none new | covered |
| a recovery no corrective resolves | none (`story:apply-recovery-way-out`) | 25, 26 | `ignite recover` |

Rows 17 to 22 run on LLVM only: the target is the slice's service target (review section 2),
and the 17.7 twins already run the host on both.

### 22.9 Operator recovery (`ignite recover`, D11)

`story:apply-recovery-way-out` is folded in as U5: R11 asks that the controller never repeat an
unsafe operation, and a host that refuses every apply forever leaves the operator only the
unsafe way of deleting the state.

```
ignite recover --state DIR --operation ID [--json]
```

- Sent over the control socket as operation `recover` (`ignite-host-control/0.1`, 17.2). The
  host accepts it only for the active operation when it is `recovery_required` and admission
  refuses a corrective for it (F16); otherwise exit 2 `operation <id> requires no recovery`. A
  `recover` while one is in progress (its run driver still stopping) exits 2 `operation <id> is
  being recovered`.
- Until then the instances of that state keep serving: the run of the committed revision, which
  supervises them, has every further effect refused (17.5) and is not ended by the refusal, so
  `recover` finds them alive and stops them below.
- The host scans as at start (17.5): an unaccounted process under `<state>/revisions/` refuses
  with exit 1 and its pid and path, nothing signalled, nothing journaled (row 26).
- Otherwise, in this order: journal `recoverStarted {"operationId"}`; stop the run driver of
  every revision, as the host's `stop` does (`crates/ignite/src/apply/host/mod.rs:174`), so no
  supervisor sees the exits and restarts anything; drain and stop every owned and retiring
  instance of the failed and the committed revision through their pidfds (start time
  re-checked) and wait until each ended; journal `operatorRecovered {"operationId", "by":
  "ignite recover", "stopped": [pids], "laterReasons": [...]}`; set the operation's receipt
  `state` to `abandoned` (its `reason` and `laterReasons` kept); clear `recovery`; start the
  committed revision afresh as a host start does after reconciliation, which recopies its
  artifacts (`copy_into`, `crates/ignite/src/run/live.rs:1587` to `:1599`). Exit 0 once that
  revision is ready; exit 1 if it is not ready within its readiness timeout, with the reason.
  Between `recoverStarted` and the fresh start the host journal holds no `Started` record
  (row 25).
- `abandoned` is a new receipt state beside those of 17: a retry of an abandoned plan exits 2
  with `operation <id> was abandoned by ignite recover; prepare a new plan`, and `apply`
  prints `operation <id> is abandoned` (`crates/ignite/src/apply/mod.rs:369` to `:384`).
- `recover` is repeatable. A host killed between `recoverStarted` and `operatorRecovered`
  restarts with the operation still `recovery_required` and its instances ended, reconciles as
  after any crash (17.5), and a second `ignite recover` completes it.
- The state row 25 drives (on the terminal design): commit revision 1; apply revision 2 whose
  server exits at once, so the supervisor gives up (`gave_up`); then replace the run's copy of
  revision 1's executable, `revisions/<r>/bin/<generation>/terminald` (not the plan artifact,
  which the fresh start of `recover` copies over it again), with other bytes and SIGKILL its
  process, so the supervisor's restart finds "ready process artifact differs from the admitted
  desired artifact" (`crates/ignite/src/apply/host/controller.rs:1506` to `:1541`), a later
  reason no corrective resolves. Inferred from the code: the unit's first test confirms the
  path; if the restart refuses before `Ready`, the reason is the failed start of 17.5, which is
  also non-resolvable.
- 17.5 gains one line after "Precedence of recovery reasons": "Since slice 18 `ignite recover`
  (22.9) is the operator's way back to accepting plans."

### 22.10 Diagnostics

Section 8 gets a "Codes of slice 18" paragraph:

| code | stage | meaning | fixture |
|---|---|---|---|
| E150 | check (`ignite-eval`) | a member whose machine subscribes to a signal and whose kind declares no `classify(self, sample: <element type>) -> Bool` | `e150_classify_missing` |
| E151 | check (`ignite-eval`) | a signal the assembly runs and a machine subscribes to, whose element type is not an outcome type (22.3) | `e151_undecodable_element` |

Changed: E120 loses the text "unknown good outcome" (an unknown predicate is E003); E109 also
checks `classify` (fixture `e109_classify_signature`, row 34); E126 is unchanged, and the
version bump makes `e126_journal_mismatch` hit its existing version branch (row 6).
`fixtures_match_expected` (`crates/ignite/tests/plan.rs:431`) includes `e15`. Exit codes of `ignite host`, `inspect` and
`recover` are those of 17.1; no new code for them.

### 22.11 Stale points

| # | where | stale text | resolution, by whom |
|---|---|---|---|
| S1 | `docs/vision.md` §7 T4 | ends at slice 13 (slice 17 adds E140 to E149) | add "E150 to E159: slice 18 (F5)"; U8 |
| S2 | 16.6, 16.8, 16.9, 16.10 | `ProbeAttempt` as the reducers' payload, `satisfies`, the latency-derived slot | one forward line each to 22.3; U1 |
| S3 | 16.17 table, E120 row | "an unknown `good` outcome" | forward line to 22.10; U1 |
| S4 | `crates/ignite/src/run/probe.rs:4`, `live.rs` doc comments | decode "by the signal's element type" (F4) | true after U1; the comments name `observations`; U1 |
| S5 | `site/book/src/{design/contracts.md,design/index.md,design/intent.md,guide/operate.md,reference/std/contract.md,reference/std/sli.md,reference/std/control.md,reference/std/index.md,reference/std/telnet.md}`, `site/index.html`, the doc comment at `crates/ignite/src/lib.rs:755` | `good = "…"`, `satisfies`, the `ProbeAttempt` payloads of `AttemptDone`, `Emitted` and `Emit` | the predicate form; `site::` passes; U1 |
| S6 | `docs/design.md` §11.3 | `good = "prompt_received_before_timeout"` | unchanged (direction, T2); an X-row in `docs/vision.md` §6; U8 |
| S7 | `docs/layering.md` | outcome names in the `std.sli` row | updated; U1 (`std.sli`) |
| S8 | 17.5 | no path beyond `inspect` | the line of 22.9; U5 |
| S9 | `examples/todo/todo.ignite:7` | "kept in memory" | the comment of 22.4, including the drain-window limit of 22.2; U4 |
| S10 | `AGENTS.md` layout and commands | no `ignite recover`, `examples/todo/health.ignite`, durable todo, `Sample`/`Verdict` | rows; U8 |
| S11 | README and `site/` wording (review R6) | any sentence that says a check or a build proves a contract holds at run time | deferred: no sentence of the epic asks for it (scope critic, 2026-11-05); a later slice rewords it to "states and measures" |
| S12 | `docs/vision.md` §3 invariant 6 | the hook names `machine` and `validate` | add `classify`; U8 |

### 22.12 Decisions

Each has a default the units follow unless the steering session says otherwise.

| # | decision | options | default and why |
|---|---|---|---|
| D1 | how a probe kind outside `std/` names its outcome type, and how `good` becomes typed | A: generic contract kinds with `good: fn(T) -> Bool`, a `classify` hook, the driver decodes by the signal's element type and classifies at routing; B: the probe prints the names of the predicates that hold, `good` stays `Text`; C: a builtin `Signal<T>.decode(Text) -> Option<T>` and a closure in `Evaluator`; D: a closed outcome per probe kind in `std` | A: the predicate is checked against the probe's type at compile time, the reducers stay non-generic (O39) and need no decode in Ignite; B keeps `good` untyped; C needs a decode on both backends for code only the evaluator runs; D breaks row 8 |
| D2 | the Telnet outcomes | A: two named predicates with the old names, declarations lose their quotes; B: keep `Text` beside `fn` | A: a field has one type; the names and meanings stay |
| D3 | old run journals | A: `ignite-journal/0.2`, a 0.1 journal is E126; B: keep 0.1 | A: the payloads changed; reading 0.1 as 0.2 would be E127 at an arbitrary record instead of a clear refusal |
| D4 | where the todo's data lives | A: memory store as authority, one file per todo version, lazy monotone load; B: a todo-owned server kind that loads in `main`; C: a durable `Store` in the substrate | A: no new substrate code, no std field of fn type with a default, correct under concurrent requests (row 13); C needs a per-type encoding on both backends, later |
| D5 | supervising an HTTP service | settled by slice 17 (21.18): `std`'s `HttpServer` has `recovery`, `rollout` and `machine` | — |
| D6 | hosting a `Store` resource | A: implicitly, as a capability (21.3); B: a new binding `processState()` | A: a store lives in its creator's process; B adds vocabulary that says nothing new |
| D7 | the data directory's path | A: relative `todo-data`, `create = true`, against the working directory of the process that starts `todod`; B: absolute; C: a `flag` | A, with the host recording its working directory (`<state>/workdir`, row 33) so that a host started elsewhere is refused instead of opening an empty directory: B is not portable and C is not passed by `run` or `apply` (21.16 Q1) |
| D8 | detecting a corrupt record | A: the existing SHA-256 chain, plus a `status.json` anchor; B: a length frame per record (format 0.2); C: both | A: the chain already detects any changed byte; a length frame adds nothing the newline does not, and neither detects a cut at a boundary, which the anchor does for `state` records |
| D9 | a torn last record | A: drop it and journal `journalRepaired`; B: refuse to start | A: a torn record was never synced, so the host never acted on it; the record makes the drop visible |
| D10 | an altered record or a detected cut | A: refuse to start, exit 3, report offline; B: truncate to the last good record and continue | A: B could drop a synced acceptance and start the operation a second time |
| D11 | the way out of a non-resolvable recovery | A: `ignite recover` stops all owned instances and restarts the committed revision; B: move the state aside; C: admit a corrective with `--force` | A: it keeps history and ownership and proves every instance ended before anything starts; C would act on the premise 17.5 says no longer holds |
| D12 | the "before" of the apply cycle | A: the terminal design on both trees, the todo after only; B: no before | A: the todo cannot be applied at the base (F8, F9, F10) |
| D13 | the probe's HTTP request | A: written by hand in the example; B: a client role in `std.protocol.http` | A: a client role is a protocol change outside the slice (R13) |
| D14 | the slot of an emission | A: carried in `Sample` from the scheduler; B: `at - latency` | A: the outcome type need not have a latency |

### 22.13 Work split

| unit | story | owns | depends on | first failing test | rows |
|---|---|---|---|---|---|
| U0 | `story:slice-18-contract` | this section, the section-8 paragraph, `docs/slice18-review.md` | — | — | — |
| U1 | `story:typed-probe-outcomes` | `std/sli.ignite`, `std/control.ignite`, `std/contract.ignite`, the doc comments of `std/protocol/telnet.ignite`; `crates/ignite/src/run/{engine,live,probe,replay}.rs`; the driver call sites in `crates/ignite/src/lib.rs` (`run`, `replay`, the doc comment at `:755`) and `crates/ignite/src/apply/revision.rs` (the loaded modules, 22.3); `hook_signature` in `crates/ignite-types/src/typer/system.rs` and fixture `e109_classify_signature` (row 34); the fixture `e126_journal_mismatch` and its test in `run.rs` (row 6); `crates/ignite-eval/src/{model,validate}.rs` (E150, E151, `classify` discovery); `REDUCER_EVALUATOR` in `crates/ignite/src/apply/plan.rs`; `examples/terminal/terminal.design.ignite`; the goldens `terminal.design.model.json` and `terminal.design.explain.Terminal.reachable.txt`; the contract declarations in `crates/ignite/tests/apply_transition.rs` and `fixtures/control_trace.ignite`; the readers of a signal's `received` in `crates/ignite/tests/run.rs` and `tools/ignite-perf/src/readiness.rs`; fixtures `e150_*`, `e151_*`, the `e15` filter in `plan.rs`; S2 to S5, S7 (`std.sli`) | U0 | the test "the prompt outcomes are predicates" in `std/sli.ignite` | 1 to 7, 29, 30, 34 |
| U2 | `story:store-hosted` | `implicit_host` in `crates/ignite-eval/src/resolve.rs`; fixture `store_hosted.ignite` and `plan::store_is_hosted_by_its_server` appended to `crates/ignite/tests/plan.rs` | U0 | `plan::store_is_hosted_by_its_server` | 10 |
| U3 | `story:host-journal-integrity` | `crates/ignite/src/apply/host/journal.rs` (repair record, anchor, line numbers, revision-journal check, unit tests), `crates/ignite/src/apply/host/mod.rs` (start: the repair, the revision-journal check before `verify_history`, the `workdir` record of row 33), `inspect` and `describe` in `crates/ignite/src/apply/mod.rs`; `crates/ignite/tests/apply_integrity.rs` and its `mod` line | U0 | `apply::host::journal::tests::an_altered_record_names_its_index` | 23, 24, 33; unit level of 20 to 22 |
| U4 | `story:durable-todo-service` | `examples/todo/todo.ignite`, `examples/todo/health.ignite`; `crates/ignite/tests/todo.rs`; the golden `todo.model.json`; S9 | U1, U2, slice 17 | `todo::rust::todos_survive_sigkill` | 9 (check), 11 to 15 |
| U5 | `story:apply-recovery-way-out` | `ignite recover` in `crates/ignite/src/main.rs`; the client `recover` in `crates/ignite/src/apply/mod.rs`; the `recover` operation in `crates/ignite/src/apply/host/{transport,admission,controller,recovery}.rs`; `crates/ignite/tests/apply_way_out.rs` and its `mod` line; the 17.5 line (S8) | U3 | `apply_way_out::rust_operator_recover_returns_the_host_to_accepting_plans` | 25, 26 |
| U6 | `story:slice-18-fault-matrix` | `crates/ignite/tests/slice18.rs` and its `mod` line; helpers it needs in `crates/ignite/tests/common/apply.rs` (added, none changed) | U4, U5 | `slice18::llvm::todo_survives_sigkill_and_a_rollout` | 16 to 22 |
| U7 | `story:apply-cycle-timing` | mode `apply-cycle` in `tools/ignite-perf/src/` (22.15) and its `self-check` case | U4 | `ignite-perf self-check` naming `apply-cycle` | 28 (mode) |
| U8 | `story:slice-18-records` | `docs/performance/18/`, `AGENTS.md`, `STATUS.md`, `PLAN.md`, `docs/vision.md` (S1, S6, S12), README and `site/` wording (S10) | U6, U7 | — (records) | 27, 28, 32 |

Rows 8 and 31 belong to every unit.

Order: W1 U0; W2 U1, U2, U3; W3 U4, U5; W4 U6, U7; W5 U8.

Shared files:

- W2: U1 and U2 both touch `ignite-eval`, in different files (`model.rs`, `validate.rs` against
  `resolve.rs`); U1 alone edits `crates/ignite/tests/plan.rs`'s filter, U2 appends one test to
  the end of that file (merge order U1 then U2, or a trivial append conflict). U3 edits
  `crates/ignite/src/apply/host/{journal,mod}.rs`, `apply/mod.rs` and
  `crates/ignite/tests/main.rs` (one `mod` line); U1 edits `apply/plan.rs` and
  `apply/revision.rs`, which U3 does not touch. U1 edits the contract declaration in
  `apply_transition.rs`, the readers and the E126 test in `run.rs`, `crates/ignite/src/lib.rs`
  and the design goldens; U2 and U3 touch no golden.
- W3: U4 owns `examples/todo/` and `todo.rs`, and rewrites `todo.model.json`;
  U5 owns `apply/host/{transport,admission,
  controller,recovery}.rs`, `apply/mod.rs` (after U3 merged), `main.rs` (CLI) and its test
  file. No file is shared.
- W4: U6 owns `slice18.rs` and appends to `common/apply.rs`; U7 owns `tools/ignite-perf/`.
  Only U6 adds a `mod` line to `crates/ignite/tests/main.rs` in W4; in W3 only U5 does, in W2
  only U3.

Rules: one integration-test binary per crate (`AGENTS.md`). Each unit's commit names the rows of
22.0 it makes pass. The story ids above are the contract's names; all but
`story:slice-18-contract` and `story:apply-recovery-way-out` are new, and the steering session
creates them; U0 does not edit the store.

### 22.14 Placeholder arms

None: U1 changes the reducers and the driver together, and every other unit adds behaviour
behind tests that fail first. Between U1 and U4 no system in the repository has a probe kind
outside `std/`; rows 1 to 7 hold with the Telnet probe alone.

### 22.15 Gate time and build time

Method of 21.14 (18.18, section 15.1) through `tools/ignite-perf`. "Before" is a `git archive`
copy of the slice base, "after" the closed tree, each with its own `CARGO_TARGET_DIR`. One cold
gate per tree, then three interleaved warm passes; a pair whose two 1-minute loads differ by more
than 5 is dropped; medians of pair ratios beside absolute medians.

Units: those of 18.18, plus:

- `todod` and `health` builds, per target and profile, fresh `--out` and `--target-dir`,
  `SCCACHE_RECACHE=1` (mode `programs`; `health` after only), and the ratio LLVM dev / Rust dev
  of `todod` in each tree (row 27). As landed (U8): one build of the todo program per run, which
  writes both executables in the after tree (`todod` alone in the before tree); row 27 says why.
- The apply cycle (new mode `apply-cycle`, U7): in a temporary working directory, `ignite host`
  running and revision 1 of the system applied; then per sample: change one response literal of
  the server (for the todo the `/health` body, for the terminal one command's reply), `ignite plan
  --for-apply --target <t>` (which builds the artifact), `ignite apply`, until the replaced server
  answers the new text. Recorded split: plan (build included), apply until `completed`, first
  answer. Per target, 5 samples per tree after one discarded warm-up; units `apply-cycle-
  terminal-<target>` (both trees) and `apply-cycle-todo-<target>` (after only, D12).

A unit that rises over 10 % gets a measured cause or the record labels the cause inferred. No
baseline moves. The apply suites stay the slice-13 exception. Records: a "Slice 18" paragraph
in `AGENTS.md`'s gate-time section; raw records, logs and scripts under
`docs/performance/18/`.

### 22.16 Open questions

| # | question | default |
|---|---|---|
| Q1 | Is `rolling(5 minutes)` the window the example should carry? | yes for the example, so that row 16 runs in a test; a real deployment declares its own |
| Q2 | Should old `ignite-journal/0.1` journals be converted? | no: `--fresh` (16.3) starts a new journal; history stays readable as a file |
| Q3 | Should the new instance of a rollout wait for the old one's last write? | no in this slice (22.2); a durable store with cross-process locking is D4 C's |
| Q4 | Does `ignite recover` need a confirmation flag? | no: it refuses whatever it cannot prove (row 26) and journals what it did |
| Q5 | Should `healthy` also bound latency? | no: `ThresholdCompliance` with a two-argument predicate is the form for that; the example keeps one contract |
| Q6 | Known gap: should the model pin the body of a `good` predicate? A `good` fn value is recorded by its name only (`crates/ignite-eval/src/model.rs:1677`), while `ignite replay` and a resumed `ignite run` call the current source's `classify` and through it the current body of `good` (`crates/ignite/src/run/engine.rs:345`). An edited body gives E127 at whichever record first classifies differently, not at the edit (W2 review finding, not reproduced). | not in this slice: recorded by the W2 review; a fix pins the body's digest in the model or journal |
| Q7 | Known gap: should a signal's element decode with the types of every module it names? `decoder` picks the module of the first record or enum inside the element type (`crates/ignite/src/run/engine.rs:772`, used at `:320`), so the whole element is decoded against that module's types only, also a record or enum of another module inside it (W2 review finding, not reproduced). | not in this slice: recorded by the W2 review; a fix decodes against the program's merged types |
| Q8 | Open: is a `Released` lost after `Kill`? 16.12 said the old process's exit during `Draining` is not a failure; the supervisor counts it as one (`ended` in `std/control.ignite`: a `Running` supervisor without its own kill request fails), and since `fix/rollout-drain-stuck` (PR 46) the rollout relies on that restart as its new process, so 16.12 is corrected (slice 18, U8). What stays open: when the old process exits on `SIGTERM` before the driver handles `AwaitRelease`, the adapter finds no live process and only notes `<member> has no live process to release` (`Dispatch::AwaitRelease` in `crates/ignite/src/run/live.rs`); no `Released` is sent, so the disruption counts from the exit, not from the listener's close. | not in this slice: the rollout completes on the restart (PR 46); whether the adapter should send `Released` for a process that already ended is a later decision |
| Q9 | Known gap: a ready process whose binary differs from the admitted artifact ends the run. The host's `Ready` check (`ready artifact identity mismatch` in `crates/ignite/src/apply/host/controller.rs`) rejects the operation and returns an error that ends the revision's run, which takes the other committed instances of that run down with it, though only one instance differs. | not in this slice: `ignite recover` (22.9) is the way back; a fix stops only the differing instance and keeps the run |
| Q10 | Known limit (U4, U5): `ignite recover` waits a fixed 60 s for the committed revision it starts to be ready (`RECOVER_READY` in `crates/ignite/src/apply/host/recovery.rs`), whatever the revision's own readiness deadline is. | not in this slice: a revision whose start takes longer makes `recover` exit 1; a fix reads the deadline from the revision's model |

### 22.17 Resolution of the review

`docs/slice18-review.md` (28 findings at the contract's first draft) is resolved in this text as
follows.

| finding | resolution |
|---|---|
| R1 (blocker) | row 8 greps `crates/*/src`, `std` and `tools/*/src` only; tests and goldens may name the example; the epic line is reworded by the steering session |
| R2 (blocker) | row 16 forces failed attempts between good ones and requires `Compliant`, `InsufficientEvidence`, `Compliant`, `Violated` |
| R3 (blocker) | row 25 expects exit 2 `host has an active or unresolved operation` for the corrective and exit 1 for a retry; F16 corrected |
| R4 (blocker) | 22.3 states the single `std.control` typed program and decodes with the loaded modules by the element's module; `crates/ignite/src/lib.rs` and `apply/revision.rs` are U1's |
| R5 | 22.9 stops every run driver before signalling; `recoverStarted` record; row 25 asserts no `Started` in between |
| R6 | 22.2 states the drain-window loss plainly (S9, S10); rows 17 and 18 avoid it; Q3 |
| R7 | the host records `<state>/workdir` and refuses another (row 33, U3, D7) |
| R8 | E151 is an outcome-type policy, the decoder's set cited (22.3, 22.10, section 8) |
| R9 | E151 uses `entries` and the subscriber rule, no binding variant name |
| R10 | void after the rebase onto `f3abeb20`: slice 17 added the `HttpServer` fields and updated both goldens itself (21.18); U2 now owns only Store hosting |
| R11 | `abandoned` declared with its retry answer (22.9); row 25 asserts it |
| R12 | status line names the counts and this table |
| R13 | sources cite 16.16 and 16.17 |
| R14 | row 10's command uses `--` |
| R15 | 22.6 cites `builtins.rs:297` and `validate.rs:509`; fallback dropped; imports are general std modules |
| R16 | E109 covers `classify` (row 34, U1) |
| R17 | 22.7 splits a read-only verifier from the repair; repair order and offline `pendingRepair` stated |
| R18 | the check compares the compact encoding; it runs in `host/mod.rs` before `verify_history` |
| R19 | E126 unchanged; row 6 names the existing text and adds `replay`; fixture and test are U1's |
| R20 | S5 lists the missed pages and the `lib.rs:755` comment |
| R21 | row 16 names `<copy>/todo.ignite` and SIGCONT before `stop` |
| R22 | row 14 says "with the same assertions" |
| R23 | row 27 times `todod` alone, `health` separately |
| R24 | 22.9 names `revisions/<r>/bin/<generation>/terminald` and cites `copy_into` |
| R25 | decoding uses the observation of the scheduler's `signal`; the element text parses by the type printer's grammar within E151's set |
| R26 | 22.4 creates `todos/<id>` before writing; 500 means "not durable" |
| R27 | 22.2 records the D12 deviation for the steering session to carry into the epic |
| R28 | 22.9 states that `recover` is repeatable and how a crash in it reconciles |

## 23 Static executables (`--static`)

**Status: implemented 2026-11-06 on `feat/static-linking` (`story:static-linking`). Sizes and
build times: `docs/performance/static/README.md`.**

Sources: the planning store's `story:static-linking` (decisions D1 to D4, its acceptance);
this file 6.2 (the Rust target's build) and `docs/llvm.md` section 1 (the LLVM link and the
runtime archive the driver carries).

### 23.1 The flag

`ignite build`, `ignite test` and `ignite conformance` take `--static` on both targets
(`BuildOptions::static_link`). Every executable the command writes is then linked against musl:
no `PT_INTERP` program header, no `DT_NEEDED` entry, so it runs on a host or in a container
without a libc and with an empty environment. Without `--static` nothing changes: the same link
commands, the same stamps, the same `CODEGEN_REVISION` (no generated IR or Rust changes; the
flag changes only how the objects are linked).

`ignite run` and `ignite plan --for-apply` do not take it: their builds are keyed by
`RunOptions` and the saved plan's artifact identities, which name no link mode; adding one is
a change to the plan format, out of this story's scope.

### 23.2 The triple (D1)

The musl triple is `--triple` when given, else `<arch>-unknown-linux-musl` for the driver's
architecture (`std::env::consts::ARCH`). A `--triple` that names no musl target (no
component starting with `musl`) with `--static` is exit 2: `--static links against musl, and
--triple T is no musl target; …`. Static glibc is not offered (D1: NSS and `dlopen` break
under it). On the LLVM target the musl triple is the one clang compiles every module for, so
the IR is pinned to it (`docs/llvm.md` section 1); on the Rust target it is cargo's
`--target` (23.5).

### 23.3 The LLVM link and the musl kit (D2)

A static LLVM build links each executable with

```text
clang -o DIR/bin/<exe> --target=<musl triple> -static -nostdlib
  KIT/crt1.o KIT/crti.o KIT/crtbegin.o main_<exe>.o program.o std.o <runtime>
  -Wl,--gc-sections [-Wl,--strip-debug] KIT/libunwind.a KIT/libc.a KIT/crtend.o KIT/crtn.o
```

and none of `RUNTIME_LIBS`: the runtime archive built for musl needs `-lunwind -lc`
(`cargo rustc -p ignite-rt --target x86_64-unknown-linux-musl --print native-static-libs`), and
the kit supplies both. `-nostdlib` keeps clang from adding the host's libraries and start
files; nothing of the host's libc reaches the executable.

The kit is the runtime archive for the musl triple plus the start files, `libc.a` and
`libunwind.a` of the Rust target's `self-contained` directory
(`$(rustc --print target-libdir --target T)/self-contained`), the files rustc itself links a
musl program with. It is found in this order:

1. the kit the driver carries: `crates/ignite/build.rs` builds `ignite-rt --features runner`
   for the driver's target and, for a `<arch>-unknown-linux-gnu` driver whose toolchain has the
   Rust target `<arch>-unknown-linux-musl` installed, for that target too, in the same nested
   `cargo build` (two `--target` flags); it embeds that archive and the seven kit files
   (`OUT_DIR/musl_kit.rs`). The first static build writes them, a temporary file and a rename
   each, to `$XDG_CACHE_HOME/ignite/runtime/<triple>-<hash>/`, as the dynamic runtime is
   written (`docs/llvm.md` section 1). `--runtime` replaces the carried archive, not the
   kit's other files;
2. else the installed Rust target's `self-contained` directory, with the runtime archive from
   `--runtime`; without `--runtime` exit 2 naming both ways out (pass an archive built with
   `cargo build -p ignite-rt --features runner --target T`, or rebuild `ignite` where the
   target is installed).

D2 left open whether the driver carries the musl archive. It does, because a static build
then needs clang alone, like a dynamic one; the cost is the driver's size and the nested
build's time, measured in `docs/performance/static/README.md`. A driver built where the musl
target is not installed carries no kit and builds without error; its `--static` takes path 2.

The link stamp (`docs/llvm.md` section 1) holds the kit's paths, so a switch between a dynamic
and a static build relinks; the objects recompile because their `--target` differs. Both write
the same `DIR/bin/<exe>`.

### 23.4 A missing musl target (D4)

`Error::Toolchain` (exit 2) states what is missing and how to get it; no build falls back to a
dynamic link:

- the Rust target's library directory holds no `libstd-*` (the target is not installed):
  `--static links against musl and needs the Rust target T, which is not installed: install it
  with \`rustup target add T\``. Checked before cargo runs on the Rust target, and on the LLVM
  target when the driver carries no kit for T;
- the target is installed but a kit file is missing: the path and `rustup target add T`;
- no runtime archive for T (23.3 path 2).

`static_link::static_without_the_musl_target_exits_2_naming_it` triggers the first with
`--triple aarch64-unknown-linux-musl` on both targets (the test asserts that target is not
installed on the host).

### 23.5 The Rust target (D3)

`ignite build --target rust --static` runs `cargo build --target <musl triple> --config
'target.<musl triple>.rustflags=["-C", "relocation-model=static"]'` on the generated project;
the executables are in `<target-dir>/<triple>/<profile>/`. Rust's musl targets link statically
by default (`crt-static`) with the same `self-contained` files, but the x86_64 target writes a
static PIE, which carries a `PT_DYNAMIC` segment for its own relocations and which `ldd`
reports as `statically linked` instead of `not a dynamic executable`. The relocation model
makes it a position-dependent static executable like the LLVM target's, so both targets meet
the same acceptance (`file`: `statically linked`; `ldd`: `not a dynamic executable`). A
`RUSTFLAGS` in the environment replaces the configured `rustflags`, as cargo defines; the build
is then a static PIE, still without `PT_INTERP` and `DT_NEEDED`.
`ignite test --target rust --static` builds the test project the same way.

### 23.6 Tests

- `static_link::` (`crates/ignite/tests/static_link.rs`): `ignite build
  examples/todo/todo.ignite --static` on each target; every executable has no `PT_INTERP` and
  no `DT_NEEDED` (read from the ELF program headers), `file` says `statically linked`, `ldd`
  says `not a dynamic executable`; `todod`, copied alone into an empty directory and started
  with an empty environment, answers a POST with 201 and a GET with the list. `ignite test
  std/json.ignite std/process.ignite --target all --static` passes and both test executables
  are static. The two exit-2 cases of 23.2 and 23.4.
- The end-to-end suites that run a compiled program, `todo::` and `terminal::`, run every case
  on four builds: `rust`, `llvm`, `rust_static`, `llvm_static` (`common::Target`).
  `terminal::idle_terminald_runs_one_thread` counts threads on all four.

### 23.7 Decisions taken here

- The driver carries the musl kit (D2's open question), 23.3.
- Both targets write position-dependent static executables, not static PIEs (23.5): the
  acceptance asks for `ldd`'s `not a dynamic executable`, and one link mode on both targets
  keeps the end-to-end matrix one behaviour. A static PIE keeps address-space randomisation;
  offering it is a later flag, not this story.
- `ignite run` and `ignite plan --for-apply` take no `--static` (23.1).

## 24 The typed program (`ignite emit --typed-json`)

**Status: implemented 2026-11-06 on `feat/typed-program-json` (`story:typed-program-json`).**

```text
ignite emit --typed-json FILE [--std DIR]
```

prints the parsed and typed program of FILE as one JSON document, so that a tool written in
Ignite reads Ignite programs through `std.json` without a parser of its own (the first reader is
`codespec-read-ignite` of `ignite-lang/ignite-codespec`, which runs the command through
`Launcher`). The serialiser is `crates/ignite/src/typed_json.rs` (`ignite::typed_json::emit`,
`document`, `ty`, `FORMAT_VERSION`).

### 24.1 Pipeline and output

- FILE is loaded as `ignite plan` loads it (`ignite::load`): parsed with its sibling modules,
  checked and typed against the reachable standard library. The evaluator does not run. A
  diagnostic is exit 1 with the text form of `ignite check`, a file that cannot be read exit 3;
  nothing is printed on stdout then.
- The document covers FILE alone. A sibling module or a standard-library module is a document
  of its own: run the command on its file.
- Compact JSON (no whitespace, object keys in byte order, the string escaping of 15.5),
  followed by a newline. The same inputs give the same bytes; no time, host or absolute path
  appears unless FILE was given as one (`files` holds FILE as written).
- `--typed-json` conflicts with `--substrate`; without it `ignite emit` is unchanged.

### 24.2 The document

Statements, expressions and patterns are not nested in their parents: each is one object in a
flat table, and a parent names a child by its index in that table. The nesting depth of the
document does not grow with the program's (`std.json.parse` refuses depth over 64). Tables are
in pre-order: a parent's index is below its children's, siblings in source order.

| key | content |
|---|---|
| `formatVersion` | `"ignite-typed/0.1"` |
| `module` | the module path (`main` for a file without `package`) |
| `package` | the dotted `package`, or `null` |
| `files` | the source files spans index; in 0.1 exactly one, FILE as given on the command line |
| `imports` | `{module, path, names, glob, span}` per `import`, in source order |
| `decls` | one object per top-level declaration, in source order |
| `stmts` | every statement of the file |
| `exprs` | every expression of the file |
| `patterns` | every pattern of the file |

**Span**, on every declaration, statement, expression and pattern and on parameters, fields,
variants, arms, properties and blocks: `{file, line, column, endLine, endColumn, start, end}`.
`file` indexes `files`; lines and columns are 1-based, columns in characters (as section 8);
`start` and `end` are byte offsets, `end` exclusive.

**Type**, the value of every `type`, `ret`, `bindingType`, `declaredType`, `ownerType` and of the
items of `args`, `params` and `typeArgs`: `{kind, text, ..}`. `text` is the type in Ignite
syntax with records and enums unqualified (`List<Text>`, `Bytes`, `Box`). `kind` is the
constructor:

| `kind` | further keys |
|---|---|
| `Int`, `Bool`, `Byte`, `Unit`, `Text`, `Never`, `TcpListener`, `TcpStream`, `PipeStream`, `IoError`, `Input`, `SystemRef`, `Protocol` | none |
| `List`, `Option`, `Result`, `Map`, `Store`, `Sender`, `Receiver` (30), `Tuple`, `Ref`, `Signal` | `args`: the type arguments (`Bytes` is `List` of `Byte`; `Ref` without a kind has none) |
| `Record`, `Enum` | `module`, `name` (the declaring module; `""` for a builtin), `args` (empty unless generic) |
| `Fn` | `params`, `ret`, `requires` (`null` for a pure fn type, else the effect names), `paramNames` with `requires` |
| `Param` | `name`: a type parameter |
| `Quantity` | `name`: `Duration`, `ByteSize` or `Ratio` |
| `Capability` | `name`: `Directory`, `Launcher`, `ChildProcess`, `Entropy`, `Setting` or `CommandLine` |

Written type expressions (parameters, fields, payloads, `let x: T`) are given as the type they
denote, aliases resolved. A `type` is `null` only where the typer kept none; no file under
`std/` or `examples/` has one (`typed_json::every_file_of_std_and_examples_gives_a_complete_document`).

**Declarations**: `kind`, `name`, `span` and

| `kind` | keys |
|---|---|
| `fn` | `qualifiedName` (`Owner.name` for an associated fn), `owner`, `typeParams`, `bounds` (`{param, interfaces}`), `params` (`{name, type, span}`), `ret` (`Unit` when not written), `requires` (`null` when not written, else the written effects), `effectful` (written or inferred), `satisfies`, `testOnly`, `body` |
| `record` | `typeParams`, `fields` (`{name, type, default, satisfies, span}`, `default` an expression index or `null`), `satisfies`, `testOnly` |
| `enum` | `typeParams`, `variants` (`{name, payload, span}`, `payload` a list of types), `testOnly` |
| `const` | `type`, `value` (expression index), `testOnly` |
| `alias` | `typeParams`, `type` |
| `test` | `body`, `verifies` (the dotted requirement paths); `name` is the test's text |
| `system` | `resources` (`{name, kind, args, properties, span}`), `components`, `contracts`, `policies` (`{name, kind, properties, span}`); `kind` of a member is the kind's name as written |
| `protocol` | `roles` (each a `fn` object with `kind` `role`), `conforms`, `scope` (`{parties, sections, span}` or `null`), `statuses` (`{kind, requirement, reason, span}`) |
| `assembly` | `kindName`, `properties` |

A **block** is `{stmts, span}`, `stmts` the statement indexes. **Properties** and record
literal fields are `{name, value, span}`, `value` an expression index.

**Statements**: `kind`, `span` and

| `kind` | keys |
|---|---|
| `let` | `name`, `mutable` (`var`), `declaredType` (or `null`), `bindingType`, `value` |
| `letPattern` | `mutable`, `pattern` (pattern index), `value` |
| `assign` | `target` (`{base, fields, ownerType, span}`, `ownerType` the type of the record whose field is written, `null` without fields), `value` |
| `return` | `value` (or `null`) |
| `while` | `cond`, `body` |
| `loop` | `body` |
| `for` | `name`, `bindingType`, `iter`, `body` |
| `forPattern` | `pattern`, `iter`, `body` |
| `break`, `continue` | none |
| `expr` | `expr`: an expression in statement position; the last one of a block is its value |
| `assert` | `cond`, `message` (or `null`) |

**Expressions**: `kind`, `span`, `type` and, for a generic use, `typeArgs`; for a reference,
signal, system or protocol value (14.4), `reference` (its text). By `kind`:

| `kind` | keys |
|---|---|
| `int`, `str`, `bool` | `value` |
| `name` | `name` |
| `record` | `name`, `fields` |
| `recordUpdate` | `name`, `fields`, `base` |
| `list`, `tuple` | `items` (`()` is a `tuple` without items) |
| `call` | `function` (the callee expression), `args`, `callee`, `ufcs` (`false`), `namedArgs` |
| `method` | `receiver`, `name`, `args`, `callee`, `ufcs`, `namedArgs` |
| `field` | `receiver`, `name` |
| `tupleField` | `receiver`, `index` |
| `index` | `receiver`, `index` |
| `try`, `spawn`, `await` | `operand` |
| `unary` | `op` (`!`, `-` or `~`), `operand` |
| `binary` | `op` (the operator's symbol), `left`, `right` |
| `match` | `scrutinee`, `arms` (`{pattern, body, span}`) |
| `if` | `cond`, `then` (a block), `else` (an expression index or `null`) |
| `block` | `block` |
| `closure` | `params` (`{name, declaredType, type, span}`), `ret`, `captures` (`{name, type}`), `body` |
| `return` | `value` (or `null`) |
| `break`, `continue` | none |
| `quantity` | `literal`, `unit`, `quantity` (`{kind, value}`, the value in base units, 14.1) |
| `namedArg` | `name`, `value` |
| `brace` | `entries` (`{key, value, span}`, `key` dotted) |

A string with interpolation is the expression the parser makes of it: `+` of `str` parts and
`to_text` method calls.

`callee` is what a call or method call resolves to (`docs/llvm.md` section 2), or `null`:

| `kind` | keys |
|---|---|
| `fn` | `module`, `name` (the fn's qualified name, `Owner.name` for an associated fn) |
| `const` | `module`, `name`: a constant of fn type called |
| `role` | `module`, `protocol`, `role` |
| `variant` | `module`, `enum`, `index`, `variant` (its name): a variant constructor |
| `builtin` | `name`: the row of the builtin table (sections 6 and 7) by its name in `ignite_types::Builtin` |
| `local` | none: a local, parameter or binding of fn type |
| `fieldFn` | `module`, `record`, `field`: a record field of fn type called |

`ufcs` is `true` for a method call `x.f(args)` that calls the free fn `f(x, args)` (section 13);
a builtin method is `ufcs: false` with a `builtin` callee. `namedArgs` is `null`, or for a call
with named arguments (14.2) the argument index of each parameter in parameter order.

**Patterns**: `kind`, `span` and

| `kind` | keys |
|---|---|
| `wildcard` | none |
| `int`, `str`, `bool` | `value` |
| `binding` | `name`, `bindingType` |
| `rest` | `name` (or `null`), and `bindingType` when named |
| `or`, `tuple`, `list` | `items` |
| `variant` | `path` (the names as written), `args`, `constant` (`{module, name}` when the path names a constant, else `null`) |
| `range` | `start`, `end` (pattern indexes), `bounds` (`[lo, hi]`) |

### 24.3 Versioning

`formatVersion` is `ignite-typed/0.1`. As `ignite-model/0.1` (15.5): a change that removes or
reinterprets a key bumps it; an added key, an added `kind` value or an added table does not. A
reader ignores keys it does not know. Node indexes are positions in one document, not stable
across edits of the source (`story:stable-node-ids`).

### 24.4 Tests

- `typed_json::` (`crates/ignite/tests/typed_json.rs`): every file under `std/` and `examples/`
  exits 0; each document has the format version; every declaration, role, statement,
  expression and pattern has a span; every expression a type, and every `type`, `ret` and
  `bindingType` anywhere is a type object; the counts of declarations, statements, expressions
  and patterns are those of the parsed file, counted by a walk of the AST in the test; the todo
  example gives the same bytes twice; `examples/typed/sample.ignite` gives `describe`'s
  parameters `Box`, `Int`, `Text` and a UFCS `split` resolved to `std.text.split`.
- `examples/typed/reader.ignite` reads the document of `examples/typed/sample.ignite` with
  `std.json` and finds `describe`'s parameter names and types (an inline test, both targets).
  An inline test cannot run the compiler, so the document is checked in as a raw string;
  `typed_json::the_reader_fixture_is_the_sample_document` fails when it differs from the
  compiler's output, and `IGNITE_BLESS=1` rewrites it.
- `apply::plan::CODEGEN_REVISION` is unchanged: no generated code changes.

## 25 Language gaps found by the codespec adapters

**Status: implemented on `story:codespec-friction`, in three pull requests.**

Sources: the planning store's `story:codespec-friction` (its fix list, items 1 to 5);
`docs/friction.md` rows 47 to 59, which record each gap with the place in
ignite-lang/ignite-codespec where it bit.

### 25.1 Ordering on `Text` (item 3, friction row 49)

`<`, `>`, `<=` and `>=` take two `Text` operands and give `Bool`. The order is the byte order of
the UTF-8 encodings: the first differing byte decides, unsigned, and a proper prefix sorts
first. On valid UTF-8 that is the order of the code points, and it is Go's string order (Go
compares strings bytewise). It is the order in which a `Map` keeps `Text` keys (14.3), so the
keys of a map come back in the order `<` sorts them.

| layer | lowering |
|---|---|
| typer | `operator` admits the four orderings on `Text` beside `Int` and `Byte`; any other type stays E045 |
| Rust target | `a < b` on `String`, Rust's `Ord` for `str`, bytewise |
| LLVM target | `glue::compare` of the two `Text` values (the `ignite.text_cmp` helper a map's search uses: `memcmp` of the common length, then the shorter first), then `icmp` of the result against 0 |
| evaluator | `Value::Text` ordered by `as_bytes()` |

No locale and no normalisation: `"Z" < "_" < "a"`, `"z" < "é"`, `"é" < "€"`.

### 25.2 Wrapping arithmetic on `Int` (item 5, friction row 51)

Three builtin methods of `Int`, `a.wrapping_add(b)`, `a.wrapping_sub(b)` and
`a.wrapping_mul(b)`, each `Int -> Int`: the exact result reduced modulo 2^64 into the range of
`Int` (signed 64-bit, two's complement), which is Go's `int` arithmetic on a 64-bit platform.
They never fail; `+`, `-` and `*` keep failing on overflow (`docs/vision.md` T6). Builtins
`IntWrappingAdd`, `IntWrappingSub`, `IntWrappingMul`; their names are in
`ignite_check::BUILTIN_METHODS`, so on an `Int` receiver the builtin wins over a user fn of
the same name (13, row 5), and a call `wrapping_add(a, b)` of such a fn is unchanged.

| layer | lowering |
|---|---|
| Rust target | `i64::wrapping_add(a, b)` (and `_sub`, `_mul`) |
| LLVM target | `add`, `sub`, `mul` on `i64` without the overflow check |
| evaluator | `i64::wrapping_*` |

No wrapping form for `Byte`, `/` or `%`, and no unary negation: `(0).wrapping_sub(x)` negates.

Checked forms (`story:checked-arithmetic`, row E7 of the examples repository's
`examples/friction.md`): `a.checked_add(b)`, `a.checked_sub(b)` and `a.checked_mul(b)`, each
`Int -> Option<Int>`, are `Some` of the exact result when it lies in the range of `Int` and
`None` when it does not, so a program tests for overflow before it would end its task
(`(9223372036854775807).checked_add(1)` and `(-9223372036854775807 - 1).checked_mul(-1)` are
`None`, `(3).checked_mul(4)` is `Some(12)`). They never fail; `+`,
`-` and `*` keep failing. Builtins `IntCheckedAdd`, `IntCheckedSub`, `IntCheckedMul`; their
names are in `ignite_check::BUILTIN_METHODS`, so on an `Int` receiver the builtin wins over a
user fn of the same name, as for the wrapping forms, and a call `checked_add(a, b)` of such a
fn is unchanged.

| layer | lowering |
|---|---|
| Rust target | `i64::checked_add(a, b)` (and `_sub`, `_mul`), Rust's `Option<i64>` |
| LLVM target | `llvm.sadd.with.overflow.i64` (and `ssub`, `smul`), the overflow bit selects the `None` tag of the `Option<Int>` and the sum is the `Some` payload |
| evaluator | `i64::checked_*` |

No checked form for `Byte`, `/`, `%` or unary negation: `(0).checked_sub(x)` negates.

### 25.3 `Stderr` (item 2, friction row 48)

`Stderr.write_all(bytes) -> Result<Unit, IoError>` writes to standard error and flushes, as
`Stdout.write_all` does for standard output. It needs the ambient effect `stderr.write`;
`Stderr` is a builtin type name with no values (E040 as a type, like `Stdout`), and `stderr` is
an ambient head, so a parameter cannot take the name (E056). `log(line)` keeps writing a line to
the same stream. Builtin `StderrWriteAll`; `ignite_io::Stderr::write_all` on the Rust target,
`ignite_rt_stderr_write_all` (declared in every LLVM module, as the other runtime entries are)
on the LLVM target. In `ignite test` the runner captures it per test and shows it under
`---- <test> stderr ----` when the test fails.

### 25.4 List element assignment (item 4, friction row 50)

`l[i] = v` is a statement: the parser turns it into the builtin method call `l.set(i, v)`
(`ListSet`), whose span is that of `l[i]`. `set` replaces the element at `i` in place. It
applies to `Bytes` as to any list, since `Bytes` is `List<Byte>` in the typer, the evaluator and
both backends (`b[i] = v`, `b.set(i, v)`; `std.text`'s `to_lower` and `to_upper` change a copy
of `text.to_bytes()` with it):

- the receiver is a place a mutating builtin may change, as for `push` (E006 through a `let`,
  a `for` element or a pattern binding; in a closure, E006 for a captured list), so
  `r.items[i] = v` and `grid[i][j] = v` work;
- an index outside `0..len` is the `index` failure of a read of `l[i]`, at the same position:
  `index failed at <file>:<line>:<column>: index <i> out of range for length <n>`;
- the effect roots of `v` join those of the list, as for `push` (the effects audit probe
  `a22` now reports E016 for a stream assigned into a list).

`l[i].f = v` stays E001 (friction row 41); the E001 text names the three forms a left side may
take. `set` is a new builtin method name: on a `List` receiver it wins over a user fn `set`
(13, row 5).

| layer | lowering |
|---|---|
| Rust target | `*ignite_io::checked::at_mut(&mut <place>, i, <site>) = v` after binding `i` and `v` |
| LLVM target | bounds check (`ignite_rt_fail_index`), then a store of the owned value into the element |
| evaluator | the element replaced in the local's value; out of range is the `index` failure |

### 25.5 `List + List` (item 4, friction row 50)

`a + b` on two `List<T>` (and `Bytes`) is a new list: the elements of `a`, then those of `b`;
neither operand changes. `[] + l` takes its element type from `l` (the typer types an empty list
literal on the left after the right operand). Rust: `[&a[..], &b[..]].concat()`; LLVM: an owned
copy of `a` with `b`'s elements appended as `append` copies them; evaluator: the two lists
joined.

### 25.6 Sibling modules (item 1, friction row 47)

14.4's siblings of a program file of package `p` stay every file `<name>.ignite` beside it that
declares `package p.<name>`, loaded whether or not anything imports it. Two more kinds of
module are loaded when imported, transitively (`parse_siblings` in `crates/ignite/src/lib.rs`):

- a sibling module given as the file: when the file `a.ignite` declares `package p.a`, an
  import of `p.<name>` loads `<name>.ignite` beside it if that file declares `package p.<name>`;
  before, the import was E031;
- another program file of the package: an import of `p.<name>` (from the program of package
  `p`, or from a sibling given as the file) loads `<name>.ignite` beside it if that file declares
  `package p`, as the module `p.<name>`. Its declarations, tests and systems are those of the
  module `p.<name>`; `ignite build` of the importing file builds the importing file's systems
  only.

Only imported files are loaded this way, so a directory of unrelated programs of one package
(the fixtures of `ignite_test::`) loads nothing new.

`ignite test FILE` runs the tests of FILE's modules in this order: the files named on the
command line, then their sibling modules not named themselves, each module once per test
executable. Before, a sibling module was built into the test executable with its tests dropped.

### 25.9 Tests

- `ignite_test::codespec_friction_on_both_targets` runs `fixtures/tests/codespec.ignite` on the
  Rust and LLVM targets, dev and release: the orderings on fresh and literal `Text` (empty,
  prefix, ASCII case, two-, three- and four-byte UTF-8), an insertion sort with `<` that equals
  the keys of a `Map<Text, Int>` filled with the same words, and the three wrapping methods at
  the bounds of `Int`.
- `eval::evaluator_agrees_with_the_rust_target` and `_llvm_target`: `int_wrapping` and
  `text_compare` of `fixtures/eval_equivalence.ignite` print the same in the evaluator and on
  both targets.
- `ignite_test::codespec_stderr_and_lists_on_both_targets` runs
  `fixtures/tests/codespec_lists.ignite` on both targets, dev and release: element assignment
  through a `var`, a field and nested lists, the source list unchanged after a copy is assigned
  into, an out-of-range assignment that fails with the `index` report at its position, `List +
  List` (empty operands, chains, `Bytes`, nested lists, the operands unchanged), and a test that
  writes `to out` to `Stdout` and `to err` to `Stderr` and fails, whose report shows each
  under its own stream.
- `list_set_concat` of `fixtures/eval_equivalence.ignite` prints the same in the evaluator and on
  both targets; `e006_element_assign` (`ignite-check`) is E006 for an element assignment
  through a `let` and a `for` element.
- `siblings::test_runs_siblings_and_imported_program_files_on_both_targets`
  (`fixtures/codespec_siblings/`, both targets): `ignite test app.ignite` reports the tests of
  `cs`, `cs.util`, `cs.fmt` and `cs.reader` (a program file of package `cs`), 4 passed;
  `ignite test util.ignite` reports those of `cs.util` and of the sibling it imports, `cs.fmt`.
- `ignite_test::checked_arithmetic_on_both_targets` runs `fixtures/tests/checked.ignite` on both
  targets, dev and release: `checked_add`, `checked_sub` and `checked_mul` at both bounds of
  `Int`, on literal operands and on operands read from text, `None` past a bound and `Some`
  within it; `int_checked` of `fixtures/eval_equivalence.ignite` prints the same in the
  evaluator and on both targets.

## 26 Peer address, timed-out reads and connect timeouts

**Status: implemented in the network builtins story.**

Sources: the case study's G1, G4 and G5 (the service written in Ignite);
`docs/friction.md` rows 60, 63 and 64. Three builtins of `TcpStream`, all additive: `read`,
`Input` and `connect` keep their meaning, and no `match` on `Input` gains a variant.

| Ignite | type | effect | builtin |
|---|---|---|---|
| `s.peer()` | `Endpoint` (`std.net`) | none | `TcpStreamPeer` |
| `s.timed_out()` | `Bool` | `r.read` for each root `r` of `s` | `TcpStreamTimedOut` |
| `TcpStream.connect_within(address, port, timeout)` | `(Text, Int, Duration) -> Result<TcpStream, IoError>` | `net.connect` | `TcpStreamConnectWithin` |

The names are `ignite_check::BUILTIN_METHODS` (`peer`, `timed_out`) and `BUILTIN_STATICS`
(`connect_within`, arity 3), so on a `TcpStream` receiver the builtin wins over a user fn of the
same name (13, row 5). They are not `ByteStream` methods: a `PipeStream` and a
`ByteStream`-bounded receiver have none of them. Being builtin method names, `peer` and
`timed_out` cannot be called as fn-typed record fields: `hooks.peer(1)` is E041, as
`hooks.local_port(1)` already is.

### 26.1 `peer`

`s.peer()` is the other end of the connection: `Endpoint { address, port }`, the address as
text the way its family writes it (`127.0.0.1`, `::1`, `fe80::1`; no brackets, no zone). An
IPv4 peer of an IPv6 socket (`::ffff:a.b.c.d`) is written as IPv4. The substrate reads it once,
when `accept` or `connect` made the stream (`getpeername`), so `peer` never fails, never waits,
and stays the same after `close` and after the peer went away. A stream whose address the
operating system could not report then has `Endpoint { address = "", port = 0 }`.

A module that calls `peer` depends on `std.net` for `Endpoint`'s fields without importing it:
`ignite_types::builtin_method_std_modules` names the modules of the builtin methods of
`TcpStream`, `TcpListener` and `PipeStream`, and the loader's walk of a module's expressions
(`module_dependencies` in `crates/ignite/src/lib.rs`) adds them for every method call of that
name, one inside an interpolation included. The token scan of 16.22 does not see a call inside
an interpolation, and it would load `std.net` for any binding named `peer`.

### 26.2 `timed_out`

A read that waited longer than the idle timeout (`set_idle_timeout`, 11.5) still returns `Eof`.
`s.timed_out()` holds when the last read of the stream returned because the idle timeout
passed, and not when the peer closed or data arrived. Every read sets it anew, on any handle of
the stream (a clone shares it); it is false before the first read. With two tasks reading one
stream, "last" is the read that ended last: the flag is set under the stream's read lock from
that read's own outcome. It never fails and never waits; on the LLVM target
a handle that is no stream gives false.

```ignite
match stream.read(4096)? {
  Data(bytes) => handle(bytes)
  Eof => if stream.timed_out() { ping(stream)? } else { return Ok(()) }
}
```

### 26.3 `connect_within`

`TcpStream.connect_within(address, port, timeout)` is `connect` that gives up after `timeout`.
Each address the name resolves to is tried in turn with the time that is left; resolving the
name itself is not bounded. Past the deadline the error is `connect <address>:<port>: <the
operating system's message>` (`connection timed out`, exit code 1); a `timeout` that is not
positive is refused before any attempt with `connect <address>:<port>: the timeout must be
positive`. `connect(address, port)` keeps waiting for the kernel's retries.

### 26.4 Lowering

| layer | `peer` | `timed_out` | `connect_within` |
|---|---|---|---|
| `ignite-io` | `TcpStream::peer(&self) -> (String, i64)` | `TcpStream::timed_out(&self) -> bool` | `TcpStream::connect_within(String, i64, i64 ns)` |
| Rust target | `{ let (address, port) = s.peer(); Endpoint { address, port } }` | `s.timed_out()` | `::ignite_io::TcpStream::connect_within(a, p, t)` |
| LLVM runtime | `void ignite_rt_tcp_peer(Endpoint* out, i64 stream)`, the address in the task's arena | `i8 ignite_rt_tcp_timed_out(i64 stream)` | `i32 ignite_rt_tcp_connect_within(Text* address, i64 port, i64 timeout_ns, i64* out, IoError* err)` |
| LLVM target | `rt_out` into an `Endpoint` temporary | the `i8` compared with 0 | `rt_result` as `tcp_connect`; the binding owns the stream as one of `connect` (`own.rs`) |
| evaluator | no value at compile time, as every stream builtin | same | same |

`CODEGEN_REVISION` is `ignite-codegen/0.1.0-40`.

### 26.5 Tests

- `std/net.ignite`: three inline tests over loopback on both targets (`task test-ignite`): the
  peer of both ends in IPv4 and IPv6 and after `close`; a timed-out `Eof` against a closed one;
  `connect_within` to `10.255.255.1:80` failing with `connection timed out` after 180 to 300 ms
  of a 200 ms timeout, and to a loopback listener succeeding.
- `net_builtins::{rust,llvm}::*` (`crates/ignite/tests/net_builtins.rs`,
  `fixtures/net_builtins/`): G1, G4 and G5 with these names, built and run as processes. The
  server logs the address and port the test's client connected from; the two reads of G4 log
  `Eof timed_out=true` and `Eof timed_out=false`; G5 logs `connection timed out` after 480 to
  600 ms of a 500 ms timeout.
- `net_builtins::{rust,llvm}::adversary_edges_pass` runs the eight inline tests of
  `fixtures/net_builtins/adversary.ignite` (the review's edges) on each target.
- `ignite-io` `tcp::peer_is_the_other_end_in_both_families`,
  `tcp::timed_out_tells_an_idle_timeout_from_a_close`, `tcp::connect_within_bounds_the_wait`;
  `ignite-rt` `abi::peer_timed_out_and_connect_within`; `ignite-types`
  `builtins::tests::substrate_methods_name_net_for_peer`.

## 27 The data value model (`std.data`) and the JSON bridge

**Status: implemented in `story:data-value-model`.**

Source: `docs/serde.md` sections 4, 5.1, 5.2 and 6, step 1 of its section 8. Std only: no
compiler, backend, runtime or evaluator change, so `CODEGEN_REVISION` stays. `std.data` is the
tree every data format reads into and writes from; the codecs, YAML, TOML and the derived
`codec<T>()` of the design are later steps.

### 27.1 `std.data`

| item | definition |
|---|---|
| `enum Value` | `Null`, `Bool(Bool)`, `Int(Int)`, `Decimal(Text)` (any number `Int` cannot hold, its exact text), `Text(Text)`, `Bytes(Bytes)`, `Moment(Text)` (RFC 3339 text), `List(List<Value>)`, `Map(List<(Text, Value)>)` (members in document order) |
| `enum Case` | `Same`, `Camel`, `Pascal`, `Kebab`, `ScreamingSnake` |
| `enum Tagging` | `External`, `Internal(Text)` |
| `enum Unknown` | `Ignore`, `Deny` |
| `type DataLimits` | `depth: Int = 64`, `size: ByteSize = 16 MiB`, `entries: Int = 1000000` (items or members of one list or map) |
| `type Shape` | `keys: Case = Same`, `variants: Case = Same`, `tagging: Tagging = External`, `unknown: Unknown = Ignore`, `renames: List<(Text, Text)> = []`, `limits: DataLimits = DataLimits {}`; `Shape.default()` is `Shape {}` |
| `type DecodeError` | `path`, `expected`, `found`, all `Text`; `describe(error)` (UFCS `error.describe()`) is `<path>: expected <expected>, found <found>` |
| `type FormatError` | `message: Text`, `line`, `column`, `offset: Int`: the first bad byte of a read, `line` and `column` 1-based, `column` in characters, `offset` in bytes (the positions of 1.1); a write gives 0 for all three |
| `format_error(text, message, offset)` | the `FormatError` at byte `offset` of `text`, line and column counted |
| `base64(bytes)` | moved to `std.encoding` (section 34); `std.json` imports it from there for the text of `Value.Bytes` |

`describe` is a fn, not a `DecodeError.describe` hook: a hook of 16.16 is not callable as a
method. `Value`'s variants share names with `Json`'s (`Null`, `Bool`, `Decimal`), so a module
that imports both writes `Value.Null` or `Json.Null` where the expected type does not decide
(E005, section 4).

### 27.2 The bridge in `std.json`

| fn | meaning |
|---|---|
| `to_data(json) -> Value` | `Number` is `Int`, `Str` is `Text`, `Array` is `List`, `Object` is `Map`; `Null`, `Bool`, `Decimal` as they are |
| `of_data(value) -> Json` | the inverse; `Bytes` becomes `Str` of its `base64`, `Moment` `Str` of its text. `of_data(to_data(j)) == j` for every `j` |
| `read(text) -> Result<Value, FormatError>` | `read_with(text, DataLimits {})` |
| `read_with(text, limits)` | the one value of `text` as `parse` reads it, then `to_data`; refused: more than `limits.size` bytes (`document larger than <n> bytes` at offset `n`), nesting deeper than `limits.depth` (`nested deeper than <n>` at the bracket), more than `limits.entries` items or members in one array or object (`more than <n> entries` at the first one too many), a member name twice in one object (``duplicate member `<name>` `` at its opening quote) |
| `write(value) -> Result<Text, FormatError>` | `encode(of_data(value))`; refused: a `Decimal` whose text is no JSON number token (``decimal `<text>` is not a JSON number``, checked by `decimal`) |
| `write_pretty(value)` | as `write`, one item or member per line, two spaces per level, `"name": value`, `[]` and `{}` for empty ones |

`parse` and `encode` keep their behaviour: `parse` keeps a repeated member name and bounds only
the depth (64), as before. Both go through one parser, `parse_bounded(text, Bounds)`.
A text read back from a written `Bytes` or `Moment` is `Text`: JSON has neither, and the codec
of a later step decodes the text into the field's type.

### 27.3 Layering

`std.data` is in the base layer and imports nothing; `std.json` imports it and `std.encoding`
(`docs/layering.md`, `layering::` in `crates/ignite-check/tests/layering.rs`).

### 27.4 Not covered

`Codec<T>` and its combinators, `Shape`'s effect on names and tags, `Unknown.Deny` and
`renames` (`story:data-codec-combinators`); YAML and TOML (`story:data-yaml`,
`story:data-toml`); `codec<T>()` (`story:data-derived-codec`). `DataLimits.entries` and the
refused duplicate member follow `docs/serde.md` goal 5 and section 4.

### 27.5 Tests

- `std/data.ignite`: the default `Shape`, `describe` (the `base64` vectors are in `std/encoding.ignite`, 34),
  `format_error`'s line and column (`task test-ignite`, both targets).
- `std/json.ignite`: `of_data(to_data(j)) == j` for every `Json` variant, nested; `Bytes` as
  base64 and `Moment` as text; `read` of every kind; line, column and offset of a format error;
  the duplicate member; one test each for `depth`, `size` and `entries`; `write`,
  `write_pretty` and their round trip through `read`; the refused decimal.
- `data_json::read_and_write_agree_across_the_evaluator_and_both_targets`
  (`crates/ignite/tests/data_json.rs`, `fixtures/data_json.ignite`): ten documents, eight that
  read and two that give a format error, read and written in the evaluator and by the program
  built for each target; the debug texts of the values, the compact and indented texts and the
  round trips agree.

## 28 The open-files limit

**Status: implemented in the open-files-limit story.**

Source: the case study's G10 (`docs/research/case-study-gaps.md`). A server accepts as many
connections as it has descriptors. A container's default soft limit for `RLIMIT_NOFILE` (often
1,024) caps the connections far under its hard limit, and the reference implementation raises
the soft limit at start. An Ignite program had no way to.

### 28.1 The raise at start

Every compiled program raises its open-files soft limit to the hard limit before `K.main`
runs, on both targets, without code:

- Rust target: `ignite_io::run`, first, before it arms the drain (16.13).
- LLVM target: `ignite_rt_init`, the first call of every generated `main`, after storing the
  arguments and before arming the drain. The test runner's `main` calls it too.
- Both call `ignite_io::raise_open_files_limit`, which runs once per process.
- When the hard limit is unlimited, the target is the kernel's ceiling
  `/proc/sys/fs/nr_open`, because `setrlimit` refuses an unlimited `RLIMIT_NOFILE`.
- The hard limit is never changed. A soft limit already at the target is left alone.
- A failure (of `getrlimit` or of `setrlimit`) is ignored. It is reported once on stderr, as
  `warning: open-files limit stays at <soft>: <the operating system's message>`, only when
  the soft limit stays under 4,096 (`ignite_io::OPEN_FILES_REPORT_BELOW`). A failure above that
  prints nothing, so a working server's stderr stays as it was.

Decisions:

- A raise at start, not a static G10's reproducer guessed (`Process.set_open_files_limit(n)`).
  The hard limit is the operator's bound; a program that wanted less than it would not run out
  of descriptors, so no program needs to choose. No new authority is granted either: any
  process may raise its soft limit up to its hard limit.
- Before `main` and before any task, so the limit holds for every descriptor the program opens
  and for the children it starts. A child inherits the raised soft limit (21.5); a program it
  launches can lower its own.
- `ignite run` and `ignite apply` start members unchanged; the raise is the member's own.

### 28.2 `Process.open_files_limit()`

| Ignite | type | effect | builtin |
|---|---|---|---|
| `Process.open_files_limit()` | `() -> Int` | `process.limit` | `ProcessOpenFilesLimit` |

The current soft limit; an unlimited one, or one past `Int`, is the largest `Int`; one the
operating system cannot report is 0. It never fails.

- The effect is ambient under the existing head `process` (`AMBIENT_HEADS`), as `process.exit`
  and `process.args` are: it reads the process's state, and a value read at compile time would
  be the compiler's. The evaluator gives it no value, as every effectful builtin.
- An `HttpApp` handler cannot call it (E017), as with `Clock.now`.

Layers:

| layer | |
|---|---|
| `ignite-io` | `limits`: `raise_open_files_limit`, `open_files_limit`, `OPEN_FILES_REPORT_BELOW`; `nix` feature `resource` (no new dependency) |
| `ignite-rt` | `i64 ignite_rt_open_files_limit()`; `ignite_rt_init` raises |
| `ignite-check` | rows of `BUILTIN_STATICS`, `BUILTIN_STATIC_ARITIES`, `EFFECTFUL_STATICS`, `static_effect` |
| `ignite-types` | `Builtin::ProcessOpenFilesLimit`, effect `process.limit` |
| Rust target | `::ignite_io::open_files_limit()` |
| LLVM target | `call i64 @ignite_rt_open_files_limit()` |

`CODEGEN_REVISION` is `ignite-codegen/0.1.0-43` (wave 7, one bump for its four generation changes).

### 28.3 Tests

- `open_files_limit::{rust,llvm}::the_soft_limit_is_raised_to_the_hard_limit_at_start`
  (`crates/ignite/tests/open_files_limit.rs`, `fixtures/open_files_limit/limits.ignite`):
  the built program is started under `prlimit --nofile=256:65536`; its `/proc/<pid>/limits`
  shows soft and hard 65536, it logs `open files 65536` from `Process.open_files_limit()`, and
  its stderr holds nothing else.
- `open_files_limit::open_files_limit_is_callable_in_tests_on_both_targets`: the fixture's
  inline test on both targets.
- The inline test "the open-files soft limit is a positive count" in `std/process.ignite`.
- `ignite-io` `limits::tests::a_failure_is_reported_only_under_4096` and
  `the_raise_reaches_the_hard_limit`; `ignite-rt`
  `process::tests::open_files_limit_reads_the_soft_limit`.
- `ignite-check` fixture `e016_open_files_limit`.

## 29 Bitwise operators on `Int`

**Status: implemented on `story:bitwise-operators`.**

Sources: the case study's G7 (SHA-1, base64 and WebSocket masking written bit by bit with `/`
and `%`); `docs/friction.md` row 66, removed by this section.

### 29.1 Syntax and precedence

Five binary operators and one prefix operator, all on `Int`:

| operator | meaning | AST |
|---|---|---|
| `a & b` | and | `BinaryOp::BitAnd` |
| `a \| b` | or | `BinaryOp::BitOr` |
| `a ^ b` | exclusive or | `BinaryOp::BitXor` |
| `a << n` | shift left | `BinaryOp::Shl` |
| `a >> n` | arithmetic shift right | `BinaryOp::Shr` |
| `~a` | not (every bit flipped, `-a - 1`) | `UnaryOp::BitNot` |

The lexer has three new tokens, `&` (`Amp`), `^` (`Caret`) and `~` (`Tilde`); `|` stays `Pipe`
and `&&`, `||` are unchanged. `<<` and `>>` are no tokens: the parser reads two `<` or two `>`
whose spans touch (`Parser::shift_op`), so `List<List<Int>>` still closes two type argument
lists and `a > > b` is E001. The levels of section 3, from tight to loose: prefix (`!`, `-`,
`~`), `* / %`, `+ -`, `<< >>`, `&`, `^`, `|`, the comparisons, `==` `!=`, `&&`, `||`. This is
Rust's order: `a & b == c` is `(a & b) == c`, `1 << n + 1` is `1 << (n + 1)`. The new levels are
left-associative. A `|` in operator position is bitwise or; a `|` that starts an operand still
starts a closure, and between patterns it still separates alternatives:
`match x { 1 | 2 => a | b }`. Line breaks after the new operators are skipped as after any
binary operator. The book's grammar page (`site/book/src/reference/grammar.md`) has the table
and an `ignite` block that runs these rules.

### 29.2 Types and the shift rule

Both operands and the result are `Int` (or `U32`, 32.2); any other operand type, `Byte`
included, is E040 (`expected Int`), from `Typer::binary` and `Typer::binary_rest` through
`Typer::word`, and `~` checks its operand the same way. None of them fails: no failure site is recorded
and no backend emits a check. `&`, `|`, `^` and `~` act on the 64-bit two's complement.

A shift amount outside `0..64` is defined, compared as unsigned so a negative amount counts as
too large:

| | amount `n` in `0..64` | any other `n` (negative, 64 or more) |
|---|---|---|
| `a << n` | `a` shifted left, bits shifted out dropped (no overflow failure) | 0 |
| `a >> n` | `a` shifted right, copies of the sign bit shifted in | 0 for `a >= 0`, -1 for `a < 0` (the shift by 63) |

There is no logical shift right; `(a >> n) & mask` gives one for a known width.

### 29.3 Lowering

| layer | lowering |
|---|---|
| evaluator | `eval::bitwise`: `&`, `\|`, `^` on `i64`; `<<` by `n` when `n` is in `0..64`, else 0; `>>` by `n` when in `0..64`, else by 63; `~` is `!n` |
| Rust target | `a & b`, `a \| b`, `a ^ b`, `!a`; `i64::checked_shl(a, u64::min((n) as u64, 64) as u32).unwrap_or(0)` and `i64::wrapping_shr(a, u64::min((n) as u64, 63) as u32)` |
| LLVM target | `and`, `or`, `xor` on `i64`, `xor i64 a, -1` for `~`; `<<` as `shl` and a `select` of 0 when `icmp ult n, 64` fails (the unselected poison of an oversized `shl` does not propagate); `>>` as `ashr` by `select(icmp ult n, 64; n; 63)` |
| typed JSON (24) | `binary` with `op` the symbol (`&`, `\|`, `^`, `<<`, `>>`); `unary` with `op` `~` |
| apply identity | the canonical form tags `~` as `bitnot`; binary operators by their symbol, as before |

`apply::plan::CODEGEN_REVISION` is bumped: both backends generate code for the new operators.

### 29.4 Agreement across the evaluator and both targets

`bitwise::operators_agree_across_the_evaluator_and_both_targets`
(`crates/ignite/tests/bitwise.rs`) writes a program from random operands (xorshift, a fixed
seed, `IGNITE_BITWISE_SEED` to change it): per binary operator 36 edge pairs (0, 1, -1,
`i64::MIN`, `i64::MAX`, `0x5555…`; for the shifts the amounts 0, 1, 31, 32, 63, 64, 65, 100, -1,
-64, `i64::MIN`, `i64::MAX` against the same six left operands, 72 pairs) and 48 random pairs
whose shift amounts are mostly in `0..64` with some negative and some of 64 or more; `~` over the
edges and 48 random values; one fn that mixes the levels without parentheses. The operands are
list literals read in a loop, so no compiler folds them. The evaluator, a Rust-target build and
an LLVM-target build must each print the values Rust's `i64` gives under 29.2, fn by fn.

### 29.5 SHA-1 and base64, timed against hand-written Rust

`crates/ignite/tests/fixtures/bitwise/codecs.ignite` holds SHA-1 (RFC 3174) and base64 with
padding (RFC 4648 section 4) written with the operators, `wrapping_add` (25.2; `U32` since 32.5) for the
32-bit sums and `Bytes` read by index and built by `push`. Its four inline tests are RFC 3174's
tests 1 to 4 (test 3 is a million `a`; plus the empty message) and the seven vectors of RFC 4648
section 10; `bitwise::sha1_and_base64_vectors_on_both_targets` runs them with `ignite test` on
both targets. Reading a byte by index and `push` lower to an inline load or store behind a
bounds check on both targets (no call), so the story's "byte get/set cheaply" needs no new
builtin. Since section 34 the code is `std.digest.sha1` and `std.encoding.base64`, which the
fixture imports.

The file's executable `codecs` builds 1 MiB (`byte i = (31 i + 7) mod 256`) and runs 16 rounds
of SHA-1 and base64 over it, changing the first byte each round; it prints the last digest and
the total base64 length. `fixtures/bitwise/codecs.rs` is the same program in hand-written Rust
(`u32`, `rotate_left`, `chunks_exact`), built with `rustc -O`; the two print the same lines.
`bitwise::codecs_within_3x_of_hand_written_rust` (`#[ignore]`: it builds a release runtime and
times programs) builds `codecs` with `--release` on both targets, the LLVM one against
`ignite-rt` built `--release`, runs the three executables 9 times interleaved and requires each
target's median within 3 times the hand-written Rust's.

Recorded 2026-11-08, 20 cores shared with other sessions, base `419c69861` plus this story:

| executable | median of 9 | ratio | 1-minute load |
|---|---|---|---|
| hand-written Rust, `rustc -O` | 164.9 ms | 1 | 56 |
| `codecs`, Rust target, `--release` | 258.3 ms | 1.57 | 56 |
| `codecs`, LLVM target, `--release`, release runtime | 484.2 ms | 2.94 | 56 |

- The LLVM target holds with little margin. Its profile (`perf record`, same binary): `sha1`
  64 %, `base64` 20 %, kernel 10 % (page faults of the 1 MiB copies). The hot loops reload a
  list's data pointer from the stack at each `w[t]` and keep spilled locals in memory where the
  Rust target's build keeps them in registers; that is the LLVM backend's code quality, not
  the operators, and is not changed here. An earlier run with the test binary's debug runtime
  and `+` instead of `wrapping_add` in the SHA-1 sums gave 4.66 at load 69: the checked `+`
  costs an overflow branch per term, and the debug runtime's `ub_checks` showed in the profile.
- The Ignite code uses `wrapping_add` where the hand-written Rust uses `wrapping_add`; with `+`
  each sum is checked, as 25.2 keeps it.

### 29.6 Tests

- `ignite-syntax` `grammar::precedence_bitwise_ladder`, `bitwise_operators_are_left_associative`,
  `bitwise_not_is_a_prefix_operator`, `shifts_need_touching_angles`,
  `pipe_stays_a_closure_and_a_pattern_separator`, `newline_after_a_bitwise_operator_continues`.
- `ignite-types` fixture `e040_bitwise_operand.ignite`: a `Bool`, a `Byte` and a `Text` operand
  and a `Bool` under `~` are E040; an `Int` result where `Byte` is expected is E040.
- `crates/ignite/tests/bitwise.rs`: 29.4 and 29.5 above.
- `site/book/src/reference/grammar.md`'s `precedence` block (checked by `site::`).

## 30 Channels: a waitable signal between tasks

**Status: implemented in the channels and timers story.**

Source: the case study's G3. A long-poll, a heartbeat, a ping interval and a drain window each
wait for "a message or a deadline". Before this section the only waitable thing with a
deadline was a socket read (`set_idle_timeout`), so the service held a loopback socket pair as
a bell for every waiter: two descriptors per waiter and a write to wake it. A bounded channel
is that signal without a descriptor.

| Ignite | type | effect | builtin |
|---|---|---|---|
| `Channel.bounded(capacity)` | `(Int) -> (Sender<T>, Receiver<T>)` | none; E010 in a constant or a field default | `ChannelBounded` |
| `tx.send(value)` | `(T) -> Bool` | `tx.send` | `SenderSend` |
| `tx.try_send(value)` | `(T) -> Bool` | `tx.send` | `SenderTrySend` |
| `tx.close()` | `Unit` | `tx.send` | `SenderClose` |
| `rx.receive()` | `Option<T>` | `rx.receive` | `ReceiverReceive` |
| `rx.receive_within(timeout)` | `(Duration) -> Option<T>` | `rx.receive` | `ReceiverReceiveWithin` |
| `rx.closed()` | `Bool` | `rx.receive` | `ReceiverClosed` |
| `rx.close()` | `Unit` | `rx.receive` | `ReceiverClose` |

`Sender` and `Receiver` are builtin types of one argument (`ignite_check::BUILTIN_TYPES`),
so a program can no longer declare a type of either name (E004). The method names are
`ignite_check::CHANNEL_METHODS`, not `BUILTIN_METHODS`: on any other receiver `send`,
`close` and the others stay a user fn's names. The module `std.sync` (base layer,
`docs/layering.md`) declares nothing; it holds the API's comment and its inline tests.

### 30.1 Behaviour

- `Channel.bounded(n)` holds at most `n` values. A capacity below 1 fails the task with
  `Channel.bounded: the capacity must be at least 1, found <n>`.
- `send` waits while the channel is full and returns true once the value is queued; it
  returns false, and drops the value, when the channel is closed or closes while it waits.
  `try_send` never waits: false when full or closed.
- `receive` waits for a value. `receive_within(timeout)` waits at most `timeout`, then returns
  `None`; a `timeout` of zero or less only looks. A value already queued is returned even when
  the deadline has passed.
- `close` on either end closes the channel, and so does the last handle of all senders or of
  all receivers going away (the end of its scope, the end of the task it was handed to). A
  closed channel ends every waiter: each waiting `send` returns false, each waiting `receive`
  returns the values sent before the close, in order, then `None`. `closed()` holds once the
  channel is closed, queued values or not.
- A task flushes what it wrote (section 7) before it waits on a channel, as before
  `Clock.sleep` (21.20), and not when the operation does not wait.
- Waiters block on a condition variable inside the process: no descriptor, no thread and no
  socket per channel or per waiter.

### 30.2 Element type

`T` comes from the context, as for `Store.new()` (12.3) and `Map.empty()`: an expected type
(`let ends: (Sender<Int>, Receiver<Int>) = Channel.bounded(4)`, a parameter, a return type)
or, for a `let (tx, rx) = Channel.bounded(n)` without one, the first later use that fixes it
(`tx.send(1)`, `rx.receive() == Some("x")`, an argument of type `Sender<Text>`). An element
nothing fixes is E042 (`cannot infer the element type of `Channel.bounded(..)``); a
`Channel.bounded` where a type other than a pair of ends is expected is E040. `Sender<T>` and
`Receiver<T>` have no `==` (E045, as every handle), and `debug()` writes `<Sender>`
and `<Receiver>`.

### 30.3 Ownership and moves

- Both ends are handles (invariant 3): a copy of a binding shares the end. As a `spawn`
  argument at its last use the end moves into the task (20.3.1); before its last use the task
  gets a share and the binding keeps its own. The pattern bindings of
  `let (tx, rx) = Channel.bounded(n)` are fresh values the block owns, so they move at their
  last use like the ends of `Pipe.pair()`.
- The value argument of `send` and `try_send` is a move position, like the element of
  `Store.put`: a value dead after the call goes into the channel without a copy; a value read
  later is copied. The receiver gets its own value either way; nothing is shared between tasks.
- LLVM target: an end moved into a second binding, an annotated tuple, a record field or a
  `Store` is released with its last owner, as on the Rust target (case study gap G30). The
  rules (`own.rs`, `docs/llvm.md` section 4), which cover every handle kind (listeners,
  streams, pipe ends, channel ends, capabilities):
  - A block owns the handles of a binding whose `let` takes a value no one else owns: a
    builtin that creates handles (`Channel.bounded`, `Pipe.pair`, `TcpListener.bind`,
    `accept`, `connect`, `Launcher.new`, `Entropy.new`, `Directory.temporary`, …), with or
    without `?`; a copy out of a store or a channel (`get`, `remove`, `values`, `update`,
    `upsert`, `receive`, `receive_within`); or a call of a fn of the same module whose result
    holds only handles it created (its final expression such a value, a binding of one that
    escapes only there, or a tuple or record literal of those; no `return` of a value with a
    handle). A `match` statement, or a `match` that is the value of a `let`
    (`let present = match senders.get(id) {..}`), owns such a scrutinee when every arm keeps
    what it binds; a `for` statement owns such an iterated value (`for s in senders.values()`)
    when its body keeps every name the loop binds. The block registers every handle in the
    value (`own_handles.<T>`) and releases them when it is left.
  - The handles stay owned while every later mention keeps them in the block: a receiver of a
    builtin method that does not hand it on, a value `put` in a store or sent on a channel, a
    `spawn` argument, a plain argument of a call whose result holds no handle or only fresh
    ones; a field or element of the binding counts as the binding (`ends.0.close()`,
    `w.wake.receive()`), one of a type without handles keeps it anywhere (`w.id`). A `let` of
    the binding, of a field of it, or of a tuple or record literal around it makes an alias,
    which keeps it while the alias is kept in turn (`let r: Receiver<Int> = rx`,
    `let w = Waiter { id, wake = rx }`). A binding with an alias is not handed over to a
    `spawn` that is its last mention: the alias still reads the handle, so the block keeps
    its owner until it is left. Any other mention leaves the binding unowned, as before:
    never released, never closed early.
  - `crates/ignite/tests/fixtures/leak/attack.ignite` (six inline tests,
    `leak::reachable_handles_stay_usable_and_stored_reads_are_released`, both targets): an
    alias or a record field of an end handed to a task stays usable, a stored end read back
    or sent through a channel stays the store's or the receiver's, and a broadcast over
    stored senders and a look at one through a `let` of a `match` keep the census flat.
  - A heap copy in a store or a channel is an owner of its own: `copy_to_heap` adds an owner to
    every handle in it and `free_heap` drops it (`ignite_rt_handle_release`); `copy_from_heap`
    adds one for the copy it makes, so `remove` and `receive` hand the stored owner to their
    caller. A store's `update` and `upsert` call their fn on a copy that shares the stored
    handles.
  - A value no one else owns that no binding holds is a temporary, owned by the expression
    that consumes it, as the Rust target drops it at the end of its statement
    (`story:llvm-release-handle-temporaries`): an argument of a call whose result holds no
    handle or only fresh ones (`senders.get(id).is_some()`, the receiver of a fn called by
    UFCS included), the value of a `put`, `send` or `try_send`, an argument of `spawn` the
    task gets an owner of its own for, the scrutinee of a `match` that no statement claims
    and whose arms keep what they bind (an arm's value without braces), and a bare
    expression statement (`senders.remove(id)`). That expression takes an owner mark, the
    temporary registers its handles when it is made, and the mark is released when the
    expression is done, or by the `return`, `?`, `break` or `continue` that leaves it, or by
    the failure that ends the task. A temporary that is a fn's result or a `let`'s value is
    not one: its caller or its block owns it. A tuple or record literal around a binding
    passed to `put`, `send` or `try_send` keeps the binding, as the binding itself does: the
    heap copy is an owner of its own. `leak::stored_ends_read_in_every_form_are_released`
    (`fixtures/leak/attack2.ignite`, 16 tests) and
    `leak::stored_ends_read_into_temporaries_are_released_once`
    (`crates/ignite/tests/soak/temporaries.ignite`, 5 tests: a fn's result, `spawn`, a loop
    condition, `?`, a failing task; each fails on a copy never released and on one released
    twice) pass on both targets.
  - Known gap (pre-existing, `story:llvm-temporaries-without-owner-rule`): on LLVM a
    copied-out value that no owning `let`, statement `match`, `for` or expression above
    holds keeps that owner forever: the receiver of a builtin method
    (`senders.values().len()`), the rest of a fresh value read for one field
    (`open(s).id`), the thrown-away value of a `match` arm, a store read inside a record
    argument of a call, and a `match` binding put in a store inside `Some`.
    `leak::five_temporary_forms_are_not_released_on_llvm_yet`
    (`crates/ignite/tests/soak/temporaries_unreleased.ignite`) holds these five as the
    failing tests on LLVM; the Rust target passes all 5.
  - Known gap (pre-existing, `story:llvm-task-move-aliases`): on LLVM a `spawn` of an
    element a callee reads from a list with `xs.at(0)` moves the list's owner to the task,
    whose release closes the end the list still holds.
    `leak::an_element_read_from_a_list_and_moved_to_a_task_is_closed_early_on_llvm_yet`
    (`crates/ignite/tests/soak/moves_aliases.ignite`) holds it as the failing test on LLVM;
    the Rust target passes.
  - `spawn` adds an owner in the parent for every handle inside a tuple, record or enum
    argument (`retain_handles.<T>`); the task registers them and releases them when its fn
    returns, as it does a stream argument.

### 30.4 Lowering

| layer | form |
|---|---|
| `ignite-io` | `channel.rs`: `Channel::bounded<T>(i64) -> (Sender<T>, Receiver<T>)`, one `Mutex<State>` and two `Condvar`s (`not_empty`, `not_full`) per channel; each end counts its handles, and the drop of an end's last handle closes the channel |
| Rust target | `::ignite_io::Channel::bounded(n)`, `tx.send(v)`, `rx.receive_within(t)`, …; types `::ignite_io::Sender<T>`, `::ignite_io::Receiver<T>` |
| LLVM runtime | `channel.rs`: `void ignite_rt_channel_bounded(i64 n, ptr to_heap, ptr from_heap, ptr free, i64 size, i64 align, i64* sender, i64* receiver)`, `i8 ignite_rt_channel_send(i64, ptr)`, `i8 ignite_rt_channel_try_send(i64, ptr)`, `void ignite_rt_channel_close(i64)` (either end), `i8 ignite_rt_channel_receive(i64, ptr out)`, `i8 ignite_rt_channel_receive_within(i64, i64 ns, ptr out)`, `i8 ignite_rt_channel_closed(i64)`; each end is a handle-table entry (`Handle::Sender`, `Handle::Receiver`), released by `ignite_rt_tcp_release` |
| LLVM target | an end is an `i64` handle; a value crosses as a heap copy made, read and freed by the element's `Store` glue (`to_heap`, `from_heap`, `free_heap`); `receive` fills an `Option<T>` through `option_from` |
| evaluator | no value at compile time, as every substrate builtin |

`CODEGEN_REVISION` is `ignite-codegen/0.1.0-43` (wave 7, one bump for its four generation changes).

### 30.5 Tests

- `std/sync.ignite`: twelve inline tests on both targets (`task test-ignite`): order and
  `try_send` on a full channel, `receive_within` at its deadline, a send from another task
  waking a receiver, `send` waiting while full, close ending every waiter (a blocked sender
  refused, blocked receivers ended), queued values after a close, the last sender's task ending
  closing the channel, a sent value independent of the sender's later changes, and the four
  G30 soaks of 30.3 (33.4).
- `channels::{rust,llvm}::g3_long_poll_heartbeat_without_bells`
  (`crates/ignite/tests/channels.rs`, `fixtures/channels/g3.ignite`): a long-poll held to its
  200 ms deadline, one woken by a message after 50 ms, a heartbeat that ticks until its stop
  channel is closed; no socket.
- `channels::{rust,llvm}::a_thousand_waiters_hold_no_extra_descriptor`: 1,000 tasks wait on one
  channel and are woken by one sender; the process's `/proc/<pid>/fd` count while they wait
  equals the count before they were spawned, and the sum of the received values is 500,500.
- `ignite-io` `channel::*` (nine unit tests, among them 1,000 waiting threads woken by one
  sender); `ignite-rt` `channel::tests::values_are_copied_and_freed_and_the_last_release_closes`;
  `ignite-check` fixture `e010_channel_in_const` (E010, E016); `ignite-types` fixture
  `e042_channel_element` (E042, E040), `moves::channel_ends_move_into_tasks_and_send_moves_its_value`,
  `builtin_tables`.

## 31 The capacity hint

**Status: implemented in `story:list-capacity`.**

Source: `docs/fixed-width-ints.md` section 5 (F1) and acceptance row 2. A list or `Bytes` built
by `push` from empty doubles its buffer; a program that knows the final size had no way to say
so, where Rust has `Vec::with_capacity`.

### 31.1 The statics

| Ignite | type | effect | builtin |
|---|---|---|---|
| `List.with_capacity(n)` | `(Int) -> List<T>`, `T` from the expected type as for `List.empty()` | none | `ListWithCapacity` |
| `Bytes.with_capacity(n)` | `(Int) -> Bytes` | none | `BytesWithCapacity` |

An empty list with room for `n` elements. Only speed is observable: `len()` is 0, the value
equals `[]` and `Bytes.empty()`, `push` beyond `n` still grows, and `n < 0` is taken as 0. The
evaluator ignores the hint and gives the empty list.

- `List.with_capacity(n)` takes its element type as `List.empty()` does (section 13, row 3):
  from the expected type, else from later uses of the `let` it is bound by; E042 when neither
  decides (the message names `List.with_capacity(..)`), E040 when the expected type is no
  list. `n` is an `Int` (E040 otherwise).

Layers:

| layer | |
|---|---|
| `ignite-check` | rows of `BUILTIN_STATICS`, `BUILTIN_STATIC_ARITIES` |
| `ignite-types` | `Builtin::ListWithCapacity` (typed beside `List.empty()`), `Builtin::BytesWithCapacity` (`static_fn`); `ownership::change` none; `moves` treats `List.with_capacity` as a static receiver |
| `ignite-eval` | the empty list, the argument evaluated and dropped |
| Rust target | `Vec::with_capacity((n).max(0) as usize)`, `Vec::<u8>::with_capacity(..)`; a `let` bound to `List.with_capacity(n)` carries the typer's type, as one bound to `List.empty()` |
| LLVM target | the empty list, then `@ignite.reserve(list, n, size, align)`: one arena allocation of `max(n, 4)` elements for `n > 0`, none for `n <= 0` |
| typed document (24) | a `call` with a `builtin` callee, no new kind |

`CODEGEN_REVISION` is bumped once for this story.

### 31.2 Where std and the fixtures use it

- `fixtures/bitwise/codecs.ignite` (29.5): the 1 MiB input, base64's output
  (`(n + 2) / 3 * 4`), the SHA-1 schedule `w` (80), `hex`'s output and `repeated`. SHA-1's
  padded copy stays `var data = message` followed by `push`, as `codecs.rs` does
  (`message.to_vec()` then `push`): an exact-size `Bytes.with_capacity(length + 72)` followed by
  `append` measured more page faults on both targets (31.3).
- `std.json` and `std.protocol.http`: unchanged here. On this base `render_head` builds a `Text`
  (no hint applies), and `quote` and `encode` are rewritten by `perf/http-parse-fewer-copies`
  into `write_quoted` and `write_value`, whose buffer start is then a one-line change each.

### 31.3 Measured

Recorded 2026-11-08, base `0a36ee6c2` (before: its compiler, std and codecs fixture) against
this story; `codecs` built `--release` on both targets, the LLVM one against `ignite-rt` built
`--release`; `codecs.rs` built with `rustc -O`. 9 interleaved runs pinned to one CPU, `perf
stat` task-clock, page faults and user instructions, medians; 1-minute load 33 to 37.

| executable | task-clock | / Rust | page faults | / Rust | instructions / Rust |
|---|---|---|---|---|---|
| hand-written Rust | 63.7 ms | 1 | 997 | 1 | 1 |
| LLVM target, before | 168.9 ms | 2.65 | 30,576 | 30.67 | 2.18 |
| LLVM target, after | 129.8 ms | 2.04 | 3,836 | 3.85 | 2.18 |
| Rust target, before | 90.8 ms | 1.43 | 1,350 | 1.35 | 1.56 |
| Rust target, after | 92.5 ms | 1.45 | 1,341 | 1.35 | 1.56 |
| LLVM target, before, runtime of `perf/rt-hot-path` | 133.8 ms | 2.10 | 5,759 | 5.78 | 2.18 |
| LLVM target, after, runtime of `perf/rt-hot-path` | 126.9 ms | 1.99 | 2,216 | 2.22 | 2.18 |

- `docs/fixed-width-ints.md` acceptance row 2 (page faults at most 1.5 times the reference) is
  not met by the hint alone: 3.85 on this runtime, 2.22 with the large-block reuse of
  `perf/rt-hot-path` (the runtime the epic's 5.8 was measured with). The remaining faults are
  not growth: on this runtime every large block (over 16 KiB) is a fresh `calloc` and goes back
  at each release, so each round faults its 1 MiB and 1.3 MiB buffers in again; with the reuse,
  63 % of the faults are in libc's copy and fill functions (`perf record -e page-faults`), the
  first round's copies (SHA-1's padded copy, `to_text` of base64's output) among them.
- SHA-1's padded copy as `Bytes.with_capacity(length + 72)` and `append`: 19,348 faults on
  this runtime, 1,958 with the reuse, and 11,125 on the Rust target (from 1,350, glibc's
  dynamic trim threshold follows the largest freed block, 1.3 MiB instead of 2 MiB). Not used.
- Instructions do not move: the hint removes copies and faults, not work.

### 31.4 Tests

- `ignite_test::capacity_hint_on_both_targets` (`fixtures/tests/capacity.ignite`): four inline
  tests on both targets in dev and release: an empty list and `Bytes` with capacity, `push`
  past the capacity (100 pushes into 2), capacity 0 and negative, the element type from the
  return type of the enclosing fn.
- `eval::evaluator_agrees_with_the_{rust,llvm}_target`: `list_with_capacity` in
  `fixtures/eval_equivalence.ignite` gives the same debug text in the evaluator and on both
  targets.
- `ignite-types` fixtures `valid_local_inference` (the element type from a later `push` and
  from an argument) and `e042_with_capacity` (E042, E040).
- `bitwise::sha1_and_base64_vectors_on_both_targets` over the rewritten codecs fixture.

## 32 `U32`: modular 32-bit words

**Status: implemented in `story:u32-words`.**

Source: `docs/fixed-width-ints.md` sections 3 and 6 (F2), acceptance rows 4 and 5. SHA-1 and
its kin compute on 32-bit words modulo 2^32; with `Int` a program masks after every sum
(`.wrapping_add(x) & MASK`) and writes a rotate as two shifts, an `or` and a mask. `U32` is
the word itself.

### 32.1 Values and operators

`U32` holds 0 to 2^32 - 1. It is a word, not an integer: `+`, `-` and `*` are modulo 2^32 and
never fail; `Int` keeps failing on overflow (25.2, vision T6).

| operation | on `U32` | note |
|---|---|---|
| `a + b`, `a - b`, `a * b` | modulo 2^32 | never fails, no failure site |
| `-a` | `0 - a` modulo 2^32 | never fails |
| `a / b`, `a % b` | unsigned | `b == 0` fails with the `Int` report (`1 / 0`, 14.8) |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | unsigned | |
| `a & b`, `a \| b`, `a ^ b`, `~a` | on 32 bits | 29's operators |
| `a << n`, `a >> n` | `n: Int`; `>>` is logical | `n` outside `0..32` (negative included) gives 0 |
| `a.rotate_left(n)`, `a.rotate_right(n)` | `n: Int`, taken modulo 32 (`n & 31`) | never fails |
| `a.to_text()`, `a.debug()`, interpolation | decimal | |

`U32` is a builtin type name (`ignite_check::BUILTIN_TYPES`, E004 for a declaration named
`U32`), a scalar with `==` (13.5) and a `Copy` value: last-use moves (20.3) do not track it.
It is no `Map` key (E106) and no `std.data` shape; neither is needed yet.

### 32.2 Literals and typing

- An integer literal is typed by its expected type, as for `Byte`: `let k: U32 = 0x5A827999`.
  Outside 0 to 2^32 - 1 it is E044 (`integer literal 4294967296 is out of range for `U32` (0 to
  4294967295)`), also for `-n` with `n` not 0, list items, patterns and comparisons. A literal
  with no expected type is `Int`.
- An arithmetic or comparison operator's operands have one type; a literal takes the other
  side's (`x + 1`, `1 + x`, `x < 7`). Mixing widths is E040 with the existing text (`expected
  `U32`, found `Int``); there is no implicit widening, in an argument, a `let` with a type or
  an operand.
- `& | ^`: both operands `Int` or both `U32`, the literal side taking the other's, the result
  their type. `<<` and `>>`: the left operand `Int` or `U32`, the amount always `Int` (E040 for
  a `U32` amount), the result the left's type. `~`: an `Int` or `U32` operand, the result its
  type. Any other operand type is E040 `expected `Int``, as in 29.2; `Byte` stays E040.
- Patterns: an integer literal or range pattern on a `U32` scrutinee is typed `U32` (E044
  outside the range); a `U32` constant can be a pattern and a range bound; E046 splits a `U32`
  column into intervals of 0 to 2^32 - 1, as it does `Int` and `Byte`.
- The E040 text of a range pattern on another type now reads `ranges match `Int`, `Byte` and
  `U32``.

### 32.3 Conversions

Explicit only. Widening is total; narrowing is checked or wrapping, by name. Each is a builtin
whose name is in `ignite_check::BUILTIN_METHODS`, so on its receiver type the builtin wins over
a user fn of the same name (13, row 5).

| from \ to | `Int` | `U32` | `Byte` |
|---|---|---|---|
| `Int` | | `n.to_u32() -> Option<U32>` (`IntToU32`), `n.wrap_u32() -> U32`, the low 32 bits (`IntWrapU32`) | `n.to_byte()` (exists), `n.wrap_byte() -> Byte`, the low 8 bits (`IntWrapByte`) |
| `U32` | `x.to_int()` (`U32ToInt`) | | `x.to_byte() -> Option<Byte>` (`U32ToByte`), `x.wrap_byte()` (`U32WrapByte`) |
| `Byte` | `b.to_int()` (exists) | `b.to_u32()` (`ByteToU32`) | |

`U32ToText`, `U32RotateLeft` and `U32RotateRight` are the other builtins. None changes its
receiver (`ownership::change` none).

### 32.4 Lowering

| layer | `U32` |
|---|---|
| `ignite-types` | `Ty::U32`; `Typer::int_literal` and E044; `Typer::word` types a bitwise operand; `Typer::operator` records a failure site for `/` and `%` only |
| evaluator | `Value::U32(u32)`: `u32::wrapping_*`, `checked_div` and `checked_rem` (the failure on `None`), `rotate_*` by `n & 31`, `eval::bitwise_u32` for the bitwise operators; a JSON number in the system model (15.5) and in decoded values |
| Rust target | `u32`; a literal `123u32`; `u32::wrapping_add` (and `_sub`, `_mul`, `_neg`); `/` and `%` as `ignite_io::checked::div` and `rem` of the operands widened to `i64`, cast back (same report, no new substrate fn); `u32::checked_shl` and `checked_shr` by `min(n as u64, 32)` with `unwrap_or(0)`, a literal amount in `0..32` as Rust's shift; `u32::rotate_left(x, (n & 31) as u32)`; conversions by `as`, `u32::try_from`, `u8::try_from` |
| LLVM target | `i32` (size 4, align 4 in records, lists and enum payloads); `add`, `sub`, `mul` without overflow intrinsics; `udiv`, `urem` behind the zero check of `Byte`; `icmp` unsigned; `and`, `or`, `xor`, `xor -1`; `shl` and `lshr` behind `select(icmp ult n, 32; …; 0)`, a literal amount in `0..32` without the select; `llvm.fshl.i32(x, x, trunc n)` and `fshr` for the rotates (the intrinsic takes the amount modulo 32); `zext`, `trunc` and an `icmp ult` for the checked narrowing |
| typed document (24) | type `kind` `U32`, `text` `U32`, no further keys; the format stays `ignite-typed/0.1` (24.3) |
| system model (15.5) | a JSON number |
| apply identity | `apply::plan::CODEGEN_REVISION` bumped once for this story |

### 32.5 Agreement and the codecs fixture

`bitwise::u32_operators_agree_across_the_evaluator_and_both_targets`
(`crates/ignite/tests/bitwise.rs`, 29.4's method and seed) writes a program over operand lists
read at run time: every binary operator on the 25 pairs of the edges 0, 1, 2^31, 2^32 - 1 and
`0x55555555` and 48 random pairs (`/` and `%` without a zero divisor), the six comparisons, the
shifts and rotates of 13 words by -1, 0, 31, 32, 33, 1, 7, 64, `i64::MIN` and `i64::MAX`,
literal shift amounts, `-` and `~`, every conversion of 32.3 (from `Int` at -1, 0, 255, 256,
2^32 - 1, 2^32, `i64::MIN`, `i64::MAX` and two random values), a `match` with literal and range
patterns, and one SHA-1-shaped fn mixing the levels without parentheses. The evaluator, a
Rust-target build and an LLVM-target build must each print what Rust's `u32` gives.
`ignite_test::overflow_fails_the_same_everywhere` adds `U32` division and remainder by zero
(one report on both targets, dev and release) and wrapping sums that pass.

`fixtures/bitwise/codecs.ignite` (29.5) computes SHA-1 on `U32`: the state, the schedule `w`
(a `List<U32>` written by `w[t] = ..`, 25.4), `rotate_left`, `+` without masks, the big-endian
word as `data[at].to_u32() << 24 | ..`; `byte` is `n.wrap_byte()`. base64 stays on `Int`.

Measured 2026-11-08 with `perf stat` (`task-clock`, `page-faults`, `instructions:u`), 9
interleaved runs per binary pinned to one core, medians, release builds, start load 53.8 and end
load 50.8 (the host shared with other builds; instructions are the load-independent measure).
"Before" is the 29.5 fixture (SHA-1 on `Int` with masks) built by the compiler without `U32`,
"after" this fixture built by this compiler; all five binaries print the same two lines.

| binary | task-clock | page faults | instructions | against plain Rust |
|---|---|---|---|---|
| plain Rust (`codecs.rs`) | 90.8 ms | 997 | 1,023,223,402 | 1 |
| LLVM target, before | 131.0 ms | 2,215 | 1,631,412,158 | time 1.44, faults 2.22, instructions 1.59 |
| LLVM target, after | 113.1 ms | 2,216 | 1,433,219,124 | time 1.25, faults 2.22, instructions 1.40 |
| Rust target, before | 113.1 ms | 1,003 | 1,445,580,996 | time 1.25, faults 1.01, instructions 1.41 |
| Rust target, after | 97.6 ms | 1,003 | 1,181,061,076 | time 1.07, faults 1.01, instructions 1.15 |

Instructions fall 12 % on LLVM and 18 % on the Rust target. The optimised LLVM `sha1` holds
`llvm.fshl.i32` for both rotates and no mask; what remains of its gap is the bounds check of
every `w[t]` and `data[at]` (`ignite_rt_fail_index`, F3 of `docs/fixed-width-ints.md`). Page
faults do not change, so the LLVM target's 2.2 times (row 2 of `docs/fixed-width-ints.md` 8) is
not the arithmetic; this story does not attribute it.

### 32.6 Tests

- `ignite-types` fixtures `valid_u32` (every operator, rotate, conversion, literal, constant
  and range pattern, `List<U32>` element assignment), `e044_u32_range` (a constant, `-1`, a
  list item, a pattern, a comparison), `e040_u32_mixed` (`U32` with `Int` in `+`, `&`, `<`,
  either order; a `U32` shift amount and rotate amount; `U32` where `Int`, `Byte` is expected and
  back; `~` of an `Int` where `U32` is expected; `Bool ^ U32`); `e043_range` with the new text;
  `builtin_tables::the_checker_table_matches_the_type_checker` with a `U32` receiver.
- `typed_json::a_u32_is_named_in_the_document`: a parameter, a method call and a literal of
  type `U32` carry `{"kind": "U32", "text": "U32"}`.
- `crates/ignite/tests/bitwise.rs` and `ignite_test.rs`: 32.5 above;
  `bitwise::sha1_and_base64_vectors_on_both_targets` over the rewritten codecs fixture.

## 33 The census and the soak (`std.runtime`)

Story `leak-census` of `epic:leak-guard` (`docs/leak-guard.md` 5.1, 5.2, 7). A test counts the
live runtime objects of its process per kind and the heap bytes it holds, on both targets, and
soaks a body: `assert soak(2000, body).growth == []` fails, naming the kind that grew, when the
body keeps a counted object or bytes.

### 33.1 The census

`crates/ignite-io/src/census.rs`: one counter per kind, raised where the object is made and
lowered where it is dropped. A `Counted` token lives in the shared part of each object (the
`Arc` its clones share), so a copy of a handle is not a second object.

| field | counts | Rust | LLVM | soak |
|---|---|---|---|---|
| `tasks` | spawned and not returned (`ignite_io::spawn`) | yes | yes | exact |
| `workers` | worker threads, idle or running | yes | yes | reported |
| `channels` | a channel's shared state | yes | yes | exact |
| `listeners`, `streams` | a `TcpListener`'s socket, a `TcpStream`'s | yes | yes | exact |
| `pipes` | a `Pipe.pair()`: one per pair, shared by its ends | yes | yes | exact |
| `children` | a started `ChildProcess` | yes | yes | exact |
| `directories` | a `Directory` root | yes | yes | exact |
| `capabilities` | `Launcher`, `Entropy`, `Setting`, `CommandLine`, `Environment` | yes | yes | exact |
| `handles` | filled slots of the handle table | 0 | yes | exact |
| `handle_slots` | slots ever issued, the table's high-water mark | 0 | yes | reported |
| `heap_copies` | live `HeapValue`s (stores, channels) | 0 | yes | exact |
| `stores` | a `Store`; resident on LLVM | yes | yes | reported |
| `heap_bytes` | bytes allocated through the counting allocator and not freed; on LLVM less the large arena blocks kept for reuse | yes | yes | 64 KiB slack |
| `arena_bytes` | arena blocks of every task but the caller | 0 | yes | 64 KiB slack |
| `resident_bytes` | `VmRSS` | yes | yes | reported |

Heap bytes: `ignite_io::census::Counting` is a global allocator over the system allocator
with 64 cache-line counter blocks, a thread adding to its own. The generated Rust executables
(each `src/bin/*.rs`, the test executable included) and `ignite-rt` declare it. The LLVM
target's generated `malloc` and `free` (heap copies, `glue.rs`) stay uncounted: they are
counted as `heap_copies`. `CODEGEN_REVISION` moves once.

### 33.2 `std.runtime`

A base-layer module (`docs/layering.md`): the records `Census` (the fields of 33.1 in that
order, all `Int`), `Growth { kind, before, after }`, `Soak { runs, warmup, growth }`, the fn type
`Body`, and `soak(runs, body)`.

| static | type | lowering |
|---|---|---|
| `Runtime.census()` | `Census` | Rust: `ignite_io::census::read()` into the record; LLVM: `ignite_rt_runtime_census(ptr out)`, sixteen `i64` |
| `Runtime.settle(tasks: Int, within: Duration)` | `Bool` | waits until at most `tasks` tasks are live (`ignite_io::live_tasks`), false at the deadline; LLVM `i8 ignite_rt_runtime_settle(i64, i64)` |

Both are pure (as `Process.flush`) and have no value in the evaluator. `soak` runs `body(i)`
for `i` in `0..runs`, after each run `Runtime.settle` to the task count before the first run
within 5 s; the census after the first `max(10, runs / 10)` runs is compared with the census
after the last: every exact kind must not grow, `heap_bytes` and `arena_bytes` not by more than
64 KiB. `Body` is `fn(run: Int) -> Unit requires [run.*, channel.*, net.*, clock.*, process.*,
directory.*, launcher.*, entropy.*, setting.*, command_line.*, environment.*]`, so an effectful
body is a value (E017 otherwise); `docs/leak-guard.md` 5.2 wrote `fn(Int) -> Unit`.

### 33.3 Seeded leaks

A debug build of `ignite-io` and `ignite-rt` reads `IGNITE_DEBUG_LEAK=<kind>` once and skips
one release per object of that kind: `task`, `channel`, `listener`, `stream`, `pipe`, `child`,
`directory`, `capability` (both targets), `handle`, `heap_copy` and `arena` (a task's last
arena block kept at its end; LLVM); `bytes` keeps 370 bytes per dropped channel, which no
counted object holds (both targets). Release builds ignore the variable.

### 33.4 Tests

- `std/runtime.ignite`: the soak of an idle body and of a channel per run finds no growth; a
  channel, a task, a pipe pair and a capability count while they live and leave the census
  once released; a store counts; a held 1.1 MB value raises `heap_bytes` by at least 1 MiB and
  its drop returns it within 64 KiB.
- `leak::every_soak_passes_without_a_seeded_leak` and
  `leak::each_seeded_leak_fails_its_soak_naming_the_kind` over
  `crates/ignite/tests/fixtures/leak/seeded.ignite`, both targets: every soak passes without a
  seed and each seed fails its soak on both targets. Until `story:llvm-release-all-handles` five
  soaks were known failures on the LLVM target (`soak: listener`, `soak: stream`, `soak:
  child` by its launcher, `soak: directory`, `soak: capability`): the lowering released only
  the channel ends, pipe ends and streams a block owned; since 30.3 it releases every handle
  kind. The census test of a capability checks its release on both targets.
- G30 (`docs/leak-guard.md` 6.1): the soaks "a channel end bound a second time is released
  (G30)", "a channel end kept in a record field is released (G30)", "an annotated pair of
  channel ends is released (G30)" and "a channel end put in a store and taken out is released
  (G30)" are inline tests of `std/sync.ignite` (2,000 runs each), green on both targets; on the
  LLVM target before 30.3's rules each grew `handles` and `channels` by one or two per run.
  `leak::g30_reproducer_stays_flat_in_every_mode` builds the case study's reproducer
  (`fixtures/leak/repro.ignite`) on both targets and runs every mode at `N=20000`: each mode's
  peak resident size (`VmHWM`, the lowest of up to three runs) is within 2 MB of `none`'s.

### 33.5 The evaluator's model of the substrate under a test

Story `eval-substrate-model`. `Evaluator::run_test` runs a test's body as task 0 of an
in-memory, deterministic model of the substrate (`crates/ignite-eval/src/substrate.rs`). The
model exists only while a test runs: `ignite check`, `ignite plan` and every other evaluation
at compile time still report `evaluation failed: builtin `X` has no value at compile time` for
the same builtins.

| has a value under a test | as on the targets |
|---|---|
| `Store.new()` and every `Store<T>` method | ids from 1, values in id order; a copy shares the store |
| `Setting.of`, `CommandLine.of`, `Environment.of` and their methods | what the capability was made of; the last entry of a variable wins |
| `Pipe.pair()`, every `PipeStream` method and the `ByteStream` methods on a pipe end | 18.4: a task's writes wait in the send buffer until a flush point (a read, `Process.flush()`, an operation that waits, the task's end, 64 KiB, `close`, the end's drop) and arrive as one batch; the 16 MiB queue, `pipe peer is gone`, `write to a closed stream`, the urgent byte and the idle timeout |
| `spawn` of a fn, a role, a fn value, a constant or a field fn, and calls of a role | the callee and arguments are evaluated by the spawning task; a fn value's call only when every argument is a name, as the Rust target evaluates that form inside the task |
| `Channel.bounded` (capacity of at least 1), every `Sender` and `Receiver` method | 30: a waiting `send` or `receive` first delivers the task's writes |
| `Clock.monotonic()`, `Clock.sleep(d)` | virtual time: nanoseconds since the test began, advanced only by the scheduler |
| `Process.flush()` | delivers the task's writes |

Tasks are cooperative. The first task in spawn order that can run holds the turn until it waits
(a read, a `send` into a full channel, a `receive` from an empty one, a sleep) or ends; then
the turn passes to the first task, in spawn order, that is ready or whose deadline has come.
When none is, the virtual time moves to the earliest deadline; with no deadline the test ends
as `evaluation failed: every task of the model waits and none has a deadline (a deadlock)`.
Every task has an OS thread of its own and only the turn holder runs, so the order is the same
on every run. The tasks share the test's fuel: a task that never waits ends the test on fuel. A
failure (14.8) ends only its task, as on the targets; the test is over when its body ends. A
last use moves its value (20.3.1), so a handle given to a task is dropped with the task. No
wall clock is read: a timeout or a sleep costs no time.

The values the model holds (stored values, queued values, pipe bytes by 64) count against the
node limit (15.3). A handle is not encodable (E118) and its debug text is the type's name
(`<PipeStream>`). Still without a value under a test: TCP and TLS, `Launcher`, `Directory`,
`Entropy`, `Stdout` and `Stderr`, `Process.exit`, `Process.open_files_limit`, `Clock.now()`,
`Runtime.census()` and `Runtime.settle` (33.2), and `main`. A unit that reaches one of them
stays one the evaluator cannot run (`quality/baseline.json` `eval_cannot_run`).

Tests: `crates/ignite/tests/eval_model.rs` (the order of tasks, the deadlock report, a task
that ends on fuel, a failing task that ends only itself, pipe flush points and batches, virtual
time, and the refusal outside a test) and the differential corpus run, where every unit the
model runs agrees with the Rust target.

## 34 SHA-1, SHA-256 and base64 in std (`std.digest`, `std.encoding`)

**Status: implemented in `story:std-digest`.**

Case-study gap G22 (`docs/research/case-study-gaps.md`): a WebSocket handshake, a token check
or a content hash wrote its digest and its base64 by hand with bit arithmetic. Both are std
modules, written in Ignite on 29's operators and 32's `U32`; no builtin and no substrate code is
added, so the compiler and both backends are unchanged.

### 34.1 `std.encoding`

RFC 4648. Encoding is total; decoding returns `Result<Bytes, EncodingError>`.

| item | meaning |
|---|---|
| `type EncodingError` | `offset: Int`, `message: Text`: the byte offset of the first character refused, the text's length when only the length is wrong |
| `base64(data) -> Text` | section 4, `=` padding to a multiple of four characters |
| `base64url(data) -> Text` | section 5 (`-` and `_` for `+` and `/`), padded |
| `base64url_unpadded(data) -> Text` | section 5 without padding, which section 3.2 allows when the referring specification says so (JWS, RFC 7515) |
| `base64_decode(text)`, `base64url_decode(text)` | the bytes of a padded text in that alphabet |
| `base64url_unpadded_decode(text)` | the bytes of an unpadded base64url text |
| `hex(data) -> Text` | section 8 in lowercase, two digits per byte, as a digest is printed |

Decoding refuses, in this order: a padded text whose length is no multiple of 4 (`the length
of padded base64 is a multiple of 4`, at the length); an unpadded one whose length is 1 more
than a multiple of 4 (`no base64 text has this length`); a `=` that is not one of the last two
characters of a padded text (`` `=` only as the last one or two characters ``); any character
outside the alphabet, the other alphabet's two and a `=` in an unpadded text included (section
3.3, `not a base64 character`); non-zero bits after the last byte (section 3.5's canonical
encoding, `the bits after the last byte are not zero`, at the last character). Line breaks and
white space are characters outside the alphabet.

`std.data.base64` (27.1) is gone: the function moved here unchanged in output, and `std.json`
imports it from `std.encoding` for the text of `Value.Bytes`.

### 34.2 `std.digest`

| item | meaning |
|---|---|
| `sha1(data) -> Bytes` | the 20-byte SHA-1 digest (RFC 3174) |
| `sha256(data) -> Bytes` | the 32-byte SHA-256 digest (FIPS 180-4 section 6.2) |
| `enum Algorithm` | `Sha1`, `Sha256` |
| `type Digest` | `algorithm`, `state: List<U32>` (the words after the last whole block), `pending: Bytes` (fewer than 64), `length: Int` (bytes taken) |
| `Digest.sha1()`, `Digest.sha256()` | the state before any input |
| `update(self: Digest, data) -> Digest` | the state after `data`; by UFCS, `d.update(data)` |
| `finish(self: Digest) -> Bytes` | the digest of everything taken |

`sha1(data)` is `Digest.sha1().update(data).finish()`. `update` fills the pending block first,
then compresses every whole block of `data` where it lies (no copy of the input), and keeps
the rest; the one-shot form therefore copies at most 63 bytes of its input. `update` and
`finish` are free fns because an associated fn `Digest.update` is not callable as a method
(E041 on `d.update(..)`; `describe` in 27.1 is a fn for the same reason): a caller imports `std.digest.{Digest, update, finish}`. Input
lengths are counted in `Int`; `length * 8` fails past 2^60 bytes as every `Int` overflow does.

### 34.3 Layering

Both are in the base layer (`docs/layering.md`, `LAYERS` in
`crates/ignite-check/tests/layering.rs`). `std.encoding` imports nothing; `std.digest` imports
`std.encoding.hex` for its tests only.

### 34.4 The codecs fixture

`crates/ignite/tests/fixtures/bitwise/codecs.ignite` (29.5, 32.5) no longer writes SHA-1 and
base64 itself: it imports `std.digest.sha1` and `std.encoding.{base64, hex}`, keeps its four
inline tests and its executable `codecs`, which prints the same two lines as
`fixtures/bitwise/codecs.rs`. `bitwise::sha1_and_base64_vectors_on_both_targets` runs it on
both targets; `bitwise::codecs_within_3x_of_hand_written_rust` (`#[ignore]`) is the
acceptance's 1 MiB timing and now times std's code. Not recorded in this story: timing runs
happen on the dedicated measurement host, not on the shared workstation; the numbers of 32.5
are the last record and were taken over the same SHA-1 rounds.

### 34.5 Tests

- `std/encoding.ignite` (both targets): RFC 4648 section 10's vectors in base64; base64url
  padded and unpadded; every decoder reverses its encoder on those vectors; each refusal of
  34.1 with its offset; `hex`.
- `std/digest.ignite` (both targets): RFC 3174's tests 1 to 4 (test 3 is a million `a`) and the
  empty message; FIPS 180-4's SHA-256 examples (`abc`, the 448-bit and 896-bit messages), the
  empty message and a million `a`; `Digest` fed in pieces of 1, 3, 55, 56, 63, 64, 65, 128 and
  300 bytes gives the one-shot digest of a 300-byte message for both algorithms.
- `layering::` with the two new rows; `bitwise::` over the rewritten fixture.

## 35 std.protocol.resp: RESP3 client with pub/sub and Sentinel

**Status: implemented in `story:std-resp-client`.**

Case-study gap G9: no Redis or Sentinel client in std. `std.protocol.resp` is a client of the
Redis serialization protocol, RESP2 and RESP3, generic over `T: ByteStream`, written in Ignite on
the existing stream, channel and spawn builtins; no builtin and no substrate code is added, so the
compiler and both backends are unchanged and no generated code changes.

### 35.1 Values and limits

| item | meaning |
|---|---|
| `enum RespValue` | `Simple(Text)`, `Error(Text)`, `Integer(Int)`, `Blob(Bytes)`, `Null`, `Double(Text)`, `Boolean(Bool)`, `BlobError(Bytes)`, `Verbatim(Text, Bytes)`, `BigNumber(Text)`, `Array`, `Map` (pairs), `Set`, `Push`, `Attributed(pairs, value)`: every RESP3 type; `Double` and `BigNumber` keep the exact text, as `std.json`'s `Decimal` |
| `type RespLimits` | `line` 64 KiB (one line's text), `bulk` 16 MiB (one bulk string, blob error or verbatim string; a streamed string in total), `elements` 1,048,576 (items of one aggregate, a map's pairs), `depth` 32 (aggregates inside aggregates; the value an attribute annotates is one level deeper), `reply` 64 MiB (the encoded bytes of one value) |
| `enum RespError` | `UnknownType(Byte)`, `Malformed(Text)`, `LineTooLong`, `BulkTooLarge`, `TooManyElements`, `TooDeep`, `ReplyTooLarge`, `Truncated`; `describe` |
| `encode(value) -> Bytes` | the RESP3 form; a line type's text must hold no CR or LF |
| `encode_command(args)`, `words(parts)` | a command as an array of bulk strings |
| `decode(data, limits) -> Split` | `Complete(value, rest)`, `Partial`, `Invalid(error)`, without reading |
| `read_value(stream, limits, buffer) -> IoResult<Read>` | `Got(value, rest)`, `Closed`, `Failed(error)` |

The decoder reads RESP2's `$-1` and `*-1` as `Null`, a leading `+` on an integer, and RESP3's
streamed strings (`$?`, `;` parts, `;0`) and streamed aggregates (`*?`, `~?`, `%?`, ended by `.`)
as the value they spell. It refuses a line holding a CR or LF of its own, a length or integer that
is no number, a negative length other than RESP2's nulls, and every limit one past it. A value
that is still incomplete is decoded again from its start when more bytes arrive; a bulk string's
length is checked before its bytes are, so only aggregates of many small items pay for the
repeated parse.

### 35.2 Commands

`type Client<T> { stream: T, limits: RespLimits = RespLimits {} }`; `connect(endpoint, timeout)`
is one over TCP whose reads also give up after `timeout`. `command(client, args)` returns the
reply as a value, the server's error included; `pipeline(client, commands)` writes every command
in one write and returns one reply per command in order. A push frame on a command connection is
skipped, before the replies and after them; anything else after the last reply is the `IoError`
`resp: a reply no command asked for`. So under RESP3 a `SUBSCRIBE` sent with `command` or
`pipeline` waits until the stream's idle timeout, its confirmation being a push; `subscribe`
(35.3) is the way to subscribe. A client serves one task at a time.

The typed helpers, called as plain fns (`get(client, key)`; the method form is E042 on a
generic record receiver): `ping`, `hello`, `auth`; strings `get`, `set`, `set_for` (`PX`),
`set_if_absent` (`NX`), `mget`, `incr`, `incr_by`; hashes `hget`, `hset`, `hgetall`, `hdel`,
`hexists`; sets `sadd`, `srem`, `smembers`, `sismember`, `scard`; keys `del`, `exists`,
`rename`, `key_type`; expiry `expire` (`PEXPIRE`), `persist`, `ttl` (`PTTL` as `Expiry`:
`Missing`, `Persistent`, `Remaining(Duration)`); `publish`. Keys, fields and channels are
`Text`, values `Bytes`. Each reads RESP2 and RESP3 replies alike (a map or a flat array for
`HGETALL`, a set or an array for `SMEMBERS`, attributes seen through); the server's error is an
`IoError` whose message is the error line, a reply of another type the `IoError`
`resp: unexpected reply to <NAME>: <debug text>`.

### 35.3 Pub/sub

`subscribe(stream, topics: Topics, limits) -> IoResult<Subscription<T>>` sends `SUBSCRIBE` and
`PSUBSCRIBE` for `Topics { channels, patterns }` and reads the first confirmation, so a refusal
(`-NOAUTH`) is its error. Then a spawned task reads the connection and sends every `Message
{ channel, pattern, payload }` (`message`, `pmessage`, `smessage`, RESP2 arrays or RESP3 pushes)
on `Subscription.messages`, a bounded channel of 1,024, until an unsubscribe confirmation leaves
no subscription, the connection ends or sends bytes that are no value, or the receiver is
closed; then the channel closes and `receive()` gives `None`. `unsubscribe(subscription)` ends
every subscription. The connection is closed, not reused, after a subscription.

Reconnecting is the caller's: `retry_delay(attempt)` (100 ms doubled per attempt, at most 10 s)
and `resubscribe(stream, ended, limits)` (a new subscription to the ended one's topics); the
module's comment shows the loop. Messages published while no connection was subscribed are lost,
as pub/sub keeps none.

### 35.4 Sentinel

`ask_master(stream, name, limits)` is `SENTINEL get-master-addr-by-name`: `Some(Endpoint)`, or
`None` for a name the sentinel does not monitor. `master_from(sentinels: List<T>, ..)` and
`master_at(sentinels: List<Endpoint>, name, timeout, limits)` (connect, ask, close) ask a list in
order and return the first master named; a sentinel that fails, cannot be reached or knows no
such master is skipped, and the error names each. `watch_master(stream, limits)` subscribes to
`+switch-master`, `switched_master(message, name)` reads `<name> <old address> <old port> <new
address> <new port>`, and `next_master(subscription, name)` waits for the next switch of `name`.

### 35.5 Layering

A file under `std/protocol/` is a protocol name for `layering::` (`docs/layering.md` rules 3 and
5), and `resp` begins `response`, `respond` and `respects`, which the compiler crates and
`std.console` use. The rules now read a name as contained unless it lies inside an English word
listed for that protocol (`ORDINARY_WORDS` in `crates/ignite-check/tests/layering.rs`); a word is
a run of letters, so `RespValue` and `HttpResponse` still name it. Matching for the other
protocols is unchanged.

### 35.6 Tests

- `std/protocol/resp.ignite`, both targets, 23 tests over `Pipe.pair()` with a scripted server
  (`script`: reads each expected request byte-exact, writes its reply): every RESP3 type encodes
  as the specification writes it and decodes back with the following bytes kept; every proper
  prefix of each is `Partial`; RESP2 nulls and streamed values; each refusal and each limit;
  a value split over five deliveries; truncation; the typed helpers with RESP2 and RESP3
  replies; a pipeline in one delivery; server errors, pushes, extra and cut replies; a
  subscription over channels and a pattern ended by `unsubscribe`, a refused one, one whose
  connection ends and its `resubscribe`; `retry_delay`; `master_from` over four sentinels,
  `master_at` over TCP; `+switch-master` followed.
- `resp::a_real_server_and_sentinel_on_both_targets` in `crates/ignite/tests/resp.rs`,
  `#[ignore]` like `image::`, since neither server is installed where the gate runs: per target,
  a master, a replica and a sentinel on free loopback ports, then commands,
  a pipeline, `HELLO 3`, a subscription, `master_at` past a dead port, a forced `SENTINEL
  failover` and `next_master` to the replica. Run with
  `cargo test -p ignite --test main resp:: -- --ignored` where `redis-server` and
  `redis-sentinel` are on `PATH`.
- `layering::` with `ordinary_words_are_no_mention_and_every_other_occurrence_is`.

## 36 std.protocol.nats: NATS core client

**Status: implemented in `story:std-nats-client`.**

Case-study gap G9: no NATS client in std. `std.protocol.nats` is a client of NATS core (no
JetStream), generic over `T: ByteStream`, written in Ignite on the existing stream, channel,
clock and spawn builtins; no builtin and no substrate code is added, so the compiler and both
backends are unchanged and no generated code changes.

### 36.1 Wire and limits

| item | meaning |
|---|---|
| `type NatsLimits` | `line` 64 KiB (one control line from the server, INFO included), `payload` 1 MiB (one message, its header block included; publishing is also bounded by the server's `max_payload`), `headers` 64 KiB (one header block), `pending` 4,096 and `pending_bytes` 8 MiB (the messages held while no connection is up, each counted with its header block), `pending_age` 1 s (a message older than this is dropped, not sent), `subscription` 1,024 (one subscription's channel), `backlog` 8 MiB (the bytes a connection may leave unwritten; past it the connection is lost), `lingering` 4 (the lost connections whose tasks still run before `connect_to` waits to dial) |
| `enum NatsAuth`, `type NatsOptions` | `Anonymous`, `UserPassword(user, password)`, `Token(token)`; `name`, `verbose`, `pedantic`, `echo`, `ping_interval` 120 s, `max_pings_out` 2, `inbox_prefix` (`_INBOX.<token>` when empty) |
| `type ServerInfo` | the INFO fields a client reads: `server_id`, `server_name`, `version`, `proto`, `max_payload`, `headers`, `auth_required`, `client_id`; a missing member keeps its default |
| `type NatsMessage` | `subject`, `reply`, `headers` (`(name, value)` pairs in order), `status` (the code of `NATS/1.0 503`), `payload` |
| `encode_connect(options, info)` | `CONNECT {json}`: `verbose`, `pedantic`, `tls_required`, `lang`, `version`, `protocol` 1, `echo`, `name`, `headers` and `no_responders` when the server has headers, `user` and `pass` or `auth_token` |
| `encode_publish(message)`, `encode_subscribe(subject, queue, sid)`, `encode_unsubscribe(sid, max)` | PUB, or HPUB with the header block `NATS/1.0`, the headers and an empty line; SUB with an optional queue group; UNSUB at once or after `max` messages |
| `enum ServerOp`, `decode(data, limits) -> Split` | `Info`, `Delivery(sid, message)` (MSG or HMSG), `Ping`, `Pong`, `Accepted` (+OK), `Refused(reason)` (-ERR, quotes removed); `Complete(op, rest)`, `Partial`, `Invalid(NatsError)` |
| `enum NatsError` | `UnknownOperation(name)`, `Malformed(what)`, `LineTooLong`, `PayloadTooLarge`, `HeadersTooLarge`, `Truncated`; `describe` |
| `valid_subject`, `valid_filter`, `subject_matches(filter, subject)` | the server's rules: tokens separated by `.`, none empty or holding whitespace; a subject to publish to has no wildcard token; in a filter `*` stands for one token and `>`, the last token only, for one or more; a `*` inside a longer token is literal; case-sensitive |

Operation names are read in any case and fields are split on runs of spaces and tabs, as the
server writes them. A line holding a CR or LF of its own, a sid or size that is no decimal
number, a header size larger than the total, a payload not ended by CR LF, a header block that
does not start with `NATS/1.0` or does not end with an empty line, a header line without a
colon, a status that is not three digits, an INFO that is no JSON object and a PING, PONG or
+OK with arguments are refused, and each limit one past it.

### 36.2 The client

`connect(stream, options, limits)` reads INFO, writes CONNECT and PING, and returns when the
PONG arrives, answering the server's PING and reading `+OK` and INFO updates meanwhile; a
`-ERR` (`Authorization Violation`) is its error `nats: <reason>`. It is bounded by the stream's
idle timeout, which the caller sets. Then per connection a reader decodes the server's
operations and a writer writes the stream, and a client task owns all state, fed by one
bounded channel of commands (1,024). The client task never writes the stream: it hands bytes
to the writer through a channel of one chunk and holds the rest itself, up to
`limits.backlog`, past which the connection is lost. A server that stops reading blocks only
the writer, so the keepalive still finds it. The fns are plain fns over `Nats<T>`, as the
method form on a generic record receiver is E042:

| fn | what it does |
|---|---|
| `publish(client, subject, payload)`, `publish_message(client, message)` | checks the subject, the reply subject, header names and values; the client task then checks headers against the newest connection's INFO `headers`, and the size, header block included, against the smaller of `limits.payload` and its `max_payload`, and answers; then the message is queued with the time it was queued |
| `subscribe(client, filter)`, `queue_subscribe(client, filter, queue)` | a `Subscription { sid, subject, queue, messages }` whose bounded channel gets each delivery in order; a full channel drops and counts it (`slow`) |
| `unsubscribe`, `unsubscribe_after(client, subscription, max)` | UNSUB at once or after `max` messages in all; the channel closes after the last |
| `request(client, subject, payload, timeout)`, `request_message` | SUB to `<inbox prefix>.<sid>`, UNSUB after one, PUB with it as the reply subject, then `receive_within(timeout)` on the reply channel (`docs/slice.md` 30); at the deadline UNSUB and the error `nats: no reply to <subject> within <ms> ms`; a reply with status 503 is `nats: no responders for <subject>`. The one reply closes the channel, so a channel found closed at the deadline is read once more before it counts as shut down; `flush` does the same |
| `respond(client, message, payload)` | PUB to the message's reply subject |
| `flush(client, timeout)` | PING after everything queued before it, and waits for its PONG |
| `stats(client) -> NatsStats` | `connected`, `connections`, `losses`, `sent`, `received`, `pending`, `stale`, `overflow`, `slow`, `refused`, `dropped`, `lingering`, `errors`, `last_error` |
| `shutdown(client)` | after the commands queued before it, hands the writer every byte queued, closes the connection and every subscription's channel; every later call, and every one queued behind it, is `nats: the client is shut down` |

The client task sends PING every `ping_interval` and loses a connection that left
`max_pings_out` of them unanswered; it answers the server's PING with PONG. A `-ERR` that does
not end the connection (a permissions violation) is counted in `errors` and `last_error`.

### 36.3 Reconnect and the outgoing buffer

A lost connection (the connection ended, a write failed, bytes that are no operation, the
backlog, or the keepalive) sends the number of losses so far on `Nats.losses`. Delivery is at
most once: the client task takes back the chunk the writer has not taken and drops it with the
unsent bytes, and their messages move from `sent` to `dropped`; what the writer took is not
sent again. Every acknowledged message is in exactly one of `sent`, `pending`, `stale`,
`overflow`, `refused` and `dropped` (the doc of `NatsStats`). While no
connection is up the client task holds each published message, up to `pending` messages and
`pending_bytes` bytes, header blocks included; one past either is dropped and counted
(`overflow`). A message is stale when more than
`pending_age` (1 s, the case study's rule) passed since it was queued: a stale one is dropped and
counted (`stale`) instead of written, whether it waited in the buffer or in the command channel,
so a reconnect never replays a stale message. `reconnect(client, stream)` runs the handshake on
a new stream in the caller's task and hands it over; the client task closes the old connection
if one is up, subscribes again to every live subscription (UNSUB with the messages left for one
with a limit), writes the held messages that are still fresh and that the new server's INFO
takes (the others are counted in `refused`; a held request among them is answered with the
refusal at once, and its inbox is not subscribed again), and starts a reader and a writer for
the new connection; the reader and writer of an old one are ignored. From then on publishing
is checked against the new INFO, and against each INFO update the server sends.
`connect_to(endpoint, timeout, options, limits)` is a client over TCP that reconnects itself:
a task reads `losses` and dials again, waiting `retry_delay(attempt)` (100 ms doubled per
attempt, at most 10 s) before each attempt; connect and handshake give up after `timeout`.

`shutdown` hands the writer the chunk it has not taken and the unsent bytes as one last chunk,
closes its channel and returns; it waits for no write. The writer writes everything the client
acknowledged, then closes the stream. Commands queued behind `Shutdown` are answered as shut
down (a publish, request, subscribe, flush or stats call, and a second `shutdown`), so no caller
waits without a bound.

The substrate's `close` ends a stream's write half only (a TCP `shutdown(Write)`), and a read
or write blocked on a silent peer is not woken by it. So `dial` sets the stream's idle timeout,
after the handshake, to `quiet_limit(options)`, `ping_interval` times `max_pings_out + 1`,
saturating at about 100 years: a live server answers a PING within it, and the reader of a dead
connection ends. A `ping_interval` of 0 or less is refused by the handshake. A writer blocked on
a server that stopped reading stays blocked until the server or the kernel ends the connection:
the substrate has no operation that ends a blocked write, a full `shutdown(Both)` of a TCP
stream (or an abort that closes the socket) callable from another task without the stream's
write lock. Per lost connection to such a server, the growth is two tasks (the writer until
then, the reader until `quiet_limit`), one socket, and the one chunk of the blocked write, at
most `backlog` bytes plus one message; the client task holds none of it. The client counts these
connections (`NatsStats.lingering`, from the `Ended` each task sends) and `connect_to` does not
dial while `NatsLimits.lingering` (4) of them run, so the growth is at most `lingering` times
that. A caller that reconnects itself reads `lingering` from `stats`. After `shutdown` the
connection that was up may linger the same way.

### 36.4 Tests

- `std/protocol/nats.ignite`, both targets, 37 tests, all but eight over `Pipe.pair()`, with a
  scripted server (`script`: reads each expected request byte-exact, writes its reply, and with
  `hold` keeps the connection open until the client closes it): every client operation encodes
  as the protocol writes it; every server operation decodes with the following bytes kept, and
  every proper prefix of each is `Partial`; each refusal and each limit; the defaults; subject
  matching and validity; the handshake with user and password, with a token, and with
  `verbose`, a server PING and an INFO update inside it; a refused, ended and garbled
  handshake; subscriptions with a wildcard and a queue group, HMSG headers and reply,
  publishing with headers and `respond`; the checks before sending; the server's PING and a
  counted `-ERR`; request and reply, its deadline and no responders; `unsubscribe_after` and
  `unsubscribe`; a lost connection that drops a message stale when published and one gone
  stale while held, and sends the fresh one and the subscription on `reconnect`; the bounded
  outgoing buffer; `connect_to` over TCP, which reconnects by itself and subscribes again; a
  keepalive that loses a silent connection; a shut-down client; `retry_delay`. The adversary's
  cases, moved in: a TCP server that stops reading during a burst of forty 1 MB publishes is
  lost by the keepalive within 3 s, and without the burst too; one is lost by a 1 MiB backlog
  with the keepalive at 120 s; the buffer's byte bound counts the header block; after a
  reconnect publishing and requests are checked against the new server's `max_payload`, and a
  held message it refuses is counted, not sent; `unsubscribe_after` sends the messages left on
  the new connection; a reply after a request's deadline reaches nobody; MSG, HMSG and PING
  written one byte per read are read whole. From the second pass: forty 1 MB publishes
  acknowledged while the server does not read are all written after `shutdown`; after a loss
  by the backlog `sent` times the message size is what the server reads, and the counters add
  up to the 40 acknowledged; eight publishers racing `shutdown` all return; a `connect_to` loop
  against a server that stops reading stops at `lingering` (2) lost connections, and after
  `shutdown` only their two tasks each remain until the server ends; an INFO update on the live
  connection bounds publishing; a held request the new server refuses gets the refusal and no
  SUB of its inbox; a quiet TCP connection whose server answers each PING is kept;
  `quiet_limit` saturates, and a `ping_interval` of 0 is refused; on the client task's state,
  a loss leaves no chunk in the writer's channel and counts its messages as dropped.
- `nats::a_real_server_on_both_targets` in `crates/ignite/tests/nats.rs`, `#[ignore]` like
  `image::`, since `nats-server` is not installed where the gate runs: per target, a server with
  a user and a password, then `connect_to` with them, wildcards, a queue group, headers, a
  request answered by a task, no responders, a wrong password, and a reconnect through a proxy
  in the test that cuts the connection: a message held 1.5 s is dropped, a fresh one arrives.
  Run with `cargo test -p ignite --test main nats:: -- --ignored` where `nats-server` is on
  `PATH`.
- `layering::` reads `nats` as a protocol name from the file name; no word of the tree contains
  it, so `ORDINARY_WORDS` gets no entry.

## 37 HttpApp sees its connection, holds, takes over and answers oversized bodies

**Status: implemented in `story:http-app-connection`.**

Case study gap G28 (`docs/research/case-study-gaps.md`): a WebSocket or long-poll service kept
a hand-written HTTP server, because the `HttpApp` handler never saw its connection (no peer
address for an allow list, no takeover for an upgrade), could not wait (waiting on a channel
not in the state is E017 in a handler), and std answered a body over the limit with its own 413
before the application ran. Waiting uses section 30's channels.

### 37.1 The surface

```ignite,fragment
// std.net
type Connection {
  peer: Endpoint
  local: Endpoint
  tls: Bool = false
}
fn Connection.unknown() -> Connection
fn Connection.of(stream: TcpStream) -> Connection

// std.protocol.http
type Request { .., connection: Connection = Connection { peer = Endpoint { address = "", port = 0 }, local = Endpoint { address = "", port = 0 } } }

type HttpApp<S, T> {
  state: S
  handle: fn(state: S, request: Request) -> Response requires [state.*] = not_found
  piece: fn(state: S, request: Request) -> IoResult<Option<Bytes>> requires [state.*] = no_piece
  reply: Option<fn(state: S, request: Request) -> Reply<S, T> requires [state.*]> = None
  on_oversized: Option<fn(state: S, head: Request, limit: ByteSize) -> Response requires [state.*]> = None
}

enum Reply<S, T> {
  Respond(Response)
  Hold(Receiver<Response>, Duration, Response)
  Upgrade(Response, fn(state: S, request: Request, stream: T, early: Bytes) -> IoResult<Unit> requires [state.*, stream.read, stream.write])
}

type Holds { events: Option<Sender<HoldEvent>> }
enum HoldEvent { Held(Receiver<Response>), Drain }
fn Holds.none() -> Holds
fn Holds.start() -> Holds
fn drain_holds(holds: Holds) -> Unit
fn not_found<S>(state: S, request: Request) -> Response
fn serve_connection<T: ByteStream, S>(stream: T, connection: Connection, limits: HttpLimits, application: HttpApp<S, T>, holds: Holds) -> IoResult<Unit>
fn serve_listener<S>(listener: TcpListener, limits: HttpLimits, application: HttpApp<S, TcpStream>) -> IoResult<Unit>

type HttpServer<S> { listener: TcpListener, application: HttpApp<S, TcpStream>, .. }
```

- `HttpApp` gains the type parameter `T`, the stream type an `Upgrade`'s takeover gets. A fn
  type names effects only on its own parameters (13.3), and the takeover reads and writes the
  stream, so the stream must be one of its parameters, and a fn value has one type: `T` is that
  type. `HttpServer<S>` keeps one parameter and names `HttpApp<S, TcpStream>`; a pipe test's
  `HttpApp { .. }` infers `PipeStream` from `Http.serve`'s stream: a record literal passed to a
  generic fn is now typed against the parameter as far as the arguments before it bind it
  (`crates/ignite-types` typer, `generics::a_record_literal_argument_takes_its_type_parameters_from_the_call`).
  A closure is deferred to the last pass and sees every other argument. A record literal with a
  type parameter of its own that neither the arguments before it nor its given fields bind
  waits for the plain arguments after it and is typed before the closures, so a literal before
  the argument that binds its parameter infers it too
  (`adversary_http_app::a_record_literal_before_the_argument_that_binds_its_parameter`); one
  whose parameter no argument binds is still E042 at the literal.
  A literal bound by `let` with no annotation still has no `T` (E042), so such a binding writes
  `let app: HttpApp<S, PipeStream> = ..`. A declaration that wrote
  `HttpApp<X>` now writes `HttpApp<X, TcpStream>` (two fixtures of `crates/ignite/tests`
  did). Every literal `HttpApp { state, handle }` stays as it was.
- `reply` and `on_oversized` are `Option`s of fn types, not fns with defaults, so that their
  absence keeps the behaviour before this section: without `reply` the request goes to `handle`,
  without `on_oversized` a body over the limit is 413 and a lingering close. A default fn could
  not call `handle`, nor tell std's 413 from an application's.
- `handle` gains the default `not_found` (404), for an application that answers through
  `reply` alone.
- A generic `reply` or takeover fn is used as a value at the instance the field's type gives:
  `fn poll<T>(state: Waiters, request: Request) -> Reply<Waiters, T>` and
  `fn echo<T: ByteStream>(state: Unit, request: Request, stream: T, early: Bytes) -> ..`.

### 37.2 The connection

`Request.connection` is the connection the request came on, set by the session for every
request it asks of the application (`handle`, `reply`, `piece`, `on_oversized`); a request
`next_request` parses has `Connection.unknown()`. `serve_listener`, which `HttpServer.main` is,
gives each accepted stream `Connection.of(stream)`: `peer()` (26.1) and the new builtin
`local()`. `Http.serve` knows no address of a `T: ByteStream` and gives `Connection.unknown()`;
`serve_connection` takes the view as an argument, which is how the tests serve a pipe as a
connection from a given peer. `tls` is false: no TLS layer exists yet (`Tls<T>`, AGENTS.md).

`s.local()` on a `TcpStream` is this end of the connection, an `Endpoint` written as `peer`
writes it: the address and port an accepted connection arrived at, the ones a connected stream
left from. Pure, never fails, never waits; the substrate asks the operating system at each call
(`getsockname`), so a stream that never asks pays nothing, and it stays known after `close`. A
handle that is no stream gives `Endpoint { address = "", port = 0 }` on the LLVM target.

| layer | `local` |
|---|---|
| `ignite-check` | `BUILTIN_METHODS`, `builtin_applies` on `TcpStream` |
| `ignite-types` | `Builtin::TcpStreamLocal`, `(TcpStream, "local") -> Endpoint`, no effect; `builtin_method_std_modules` names `std.net` for it as for `peer` |
| `ignite-io` | `TcpStream::local(&self) -> (String, i64)` |
| Rust target | `{ let (address, port) = s.local(); Endpoint { address, port } }` |
| LLVM runtime | `void ignite_rt_tcp_local(Endpoint* out, i64 stream)`, the address in the task's arena |
| LLVM target | `rt_out` into an `Endpoint` temporary; the receiver is kept (`own.rs`) |
| evaluator | no value at compile time |

The generated code of a program that does not call `local` is unchanged; one that calls it
needs the new builtin: `CODEGEN_REVISION` is `ignite-codegen/0.1.0-49` (wave 10, one bump with 38).

### 37.3 Holding a request

`Hold(receiver, deadline, on_timeout)` makes the connection's task wait (`wait_held`):
`receiver.receive_within(deadline)` gives the answer, `on_timeout` is the answer when the wait
ends empty: at the deadline, or at once when the channel is closed (every sender gone, or the
drain). The answer goes through `checked` and is sent as `Respond`'s would be, so the
connection goes on after it. The receiver is closed after the wait, so the application's `send`
to an answered request returns false. A held request holds up only its own connection: each
connection is a task of its own, and a channel wait blocks a task, not a worker (30.1); requests
pipelined behind it wait. The wait does not watch the stream: a held request whose client has
gone keeps its task, its receiver and its place in the register until a message, the deadline
or the drain ends the wait, so the hold's deadline bounds what a vanished client costs.
Effects: the wait is the session's, on a receiver the reply returned,
so the handler's type keeps `requires [state.*]` and needs no effect of its own; no change to
the effects of `crates/ignite-types` was needed for it.

A thousand held requests over TCP with the clients in the same process (the last test of 37.7)
first lost answers under the default IO mode `readiness`, and so did fifty in-process clients
of an in-process echo server with no HTTP at all: a short read hid an end of stream behind it in
the substrate. With that fixed (`fix(io): a short read no longer hides an end of stream behind
it`) every request is answered in both IO modes. The test starts its clients a hundred at a
time, each hundred once the ones before are held: `TcpStream.connect` is a blocking `connect`
on the task's worker and the listener's backlog is 128, so a thousand connects at once overflowed
the listener's queue, each dropped SYN blocked its worker until the retransmit, and once every
worker was blocked so, the accepting task's among them, nothing was accepted for minutes.

The drain (16.13). `serve_listener` keeps one `Holds` for all its connections,
`Holds.start()` when the application has a `reply` and `Holds.none()` otherwise (only a `reply`
holds a request; story `http-reply-cost-back`, section 40). `Holds.start()` is
a bounded channel of `HoldEvent`s to a register task, `keep_holds`. Each held
receiver is sent to it (`Held`) before the wait; the register keeps the receivers not yet
closed, dropping answered ones as the list grows. Once the drain begins, `accept` fails, and
`serve_listener` calls `drain_holds` before it returns the error: `Drain` closes every kept
receiver, so each held request is answered at once with what was sent to it or with its
`on_timeout`. The register then closes its channel, closes the receivers already queued behind
`Drain` and ends, so the drain does not wait for its task; a request held later finds the channel
closed (`send` is false) and `wait_held` closes its receiver itself, so it is answered at once
too. One channel orders both, so no hold is missed. A register, not a `Store`,
because `Store.new()` is test-only (E051) and a non-test fn cannot make one. `main` then
returns and the process ends when the open connections have. `Http.serve` waits with
`Holds.none()`, which no drain reaches.

### 37.4 Taking the connection over

`Upgrade(response, takeover)`: the session writes the head of `response`, `checked`, framed as
an inline response to the request (`inline_framing`; a 101 has no body) but without a
`Connection` field of Http's own: the application names `Connection: upgrade` and `Upgrade`
(RFC 9110 section 7.8). Then it calls `takeover(state, request, stream, early)` on the
connection's task, `early` being the bytes already read after the request (a client may send its
first frame with the upgrade request). The session ends when `takeover` returns, and
`serve_connection` closes the stream. A response `checked` refuses (`checked(r) != r`) is
answered with `reject(.., 500, ..)`, and nothing takes over.

### 37.5 A body over the limit

`next_request` returns `Oversized(head, rest, length)` for a body over `limits.body` where it
returned `Reject(413)`: the head, the bytes read after it, and the declared `Content-Length`,
-1 for a chunked body found over the limit while it was decoded. Without `on_oversized` the
session answers it as before, `reject(.., 413, ..)` (RejectHead for HEAD). With it
(`oversized`), the hook's answer to `head` (asked as `asked` asks) and `limits.body` is sent;
the connection goes on when the request keeps it, the answer does not list `close` and
`length` is known: `discard` reads and drops the rest of the body and the session reads the
next request from the bytes after it. Otherwise the answer is followed by `linger`, the
lingering close `reject` uses. A chunked body over the limit always ends the connection, since
its remaining chunks are not decoded; so does a body announced with `Expect: 100-continue`
(`expects_continue`, any case): such a client waits for 100 before it sends the body (RFC 9110
section 10.1.1), the server never sends 100 for a body it refuses, and reading `length` bytes
would take the client's next request for the body. The discard of a `Content-Length` body is
bounded by the length and by the idle timeout of each read, not by `limits.drain`.

The inline tests that compare `next_request` with `Reject(413)` read it through `next_with`,
which maps `Oversized(head, ..)` to `rejected(head, 413)`, the session's answer without a hook;
their assertions are unchanged.

### 37.6 Claims

No `satisfies` or `verifies` clause moved; `next_request`, `session` and `reject` keep theirs,
and `ignite conformance std/protocol/http.ignite` prints the claims it printed before on both
targets. What this section adds is outside the imported catalogue: `std/rfc/rfc9110.ignite`
imports sections 5, 6, 7.2 and 9.x of RFC 9110, not Upgrade (section 7.8), Expect (section
10.1.1) or 413 Content Too Large (section 15.5.14), so no requirement of those sections is
claimed, verified or reported as a gap; their behaviour here rests on the inline tests of 37.7.

### 37.7 Tests

- `std/protocol/http/connection_tests.ignite` (`std/protocol/http.ignite` until wave 11, which
  moved them to keep that file under 3,000 lines; both targets, over `Pipe.pair()` unless
  named): "the handler sees
  its connection and an allow list refuses a peer with 403" (a pipe from a listed and an
  unlisted peer, a TCP stream through `Connection.of`, and `Http.serve`'s unknown connection);
  "a held request is answered by its message and the connection goes on"; "a held request is
  answered on_timeout at its deadline"; "a held request does not block another connection";
  "the drain answers every held request at once with its on_timeout"; "an upgrade writes its
  response and hands takeover the stream and the bytes after the request"; "an upgrade whose
  response is refused is 500 and nothing takes over"; "on_oversized answers a body over the
  limit, which is discarded, and the connection goes on" (in one delivery and in three); "on_oversized
  closes the connection when its answer or the request asks to, or the body is chunked"; "a
  thousand requests held over TCP are each answered by their message" (1,000 clients over
  loopback, each held until one message, a further connection answered while they wait).
- `std/net.ignite`: "the local end of a stream is this end, in both families".
- `drain::the_drain_answers_a_held_http_request_at_once` (`crates/ignite/tests/drain.rs`,
  `fixtures/http_hold_drain.ignite`, both targets): an `HttpServer` holds a request for 60 s;
  after SIGTERM the client reads the hold's `on_timeout` within 30 s and the server exits.
- `ignite-types` `generics::a_record_literal_argument_takes_its_type_parameters_from_the_call`.
- `ignite-io` `net::tests::a_stream_knows_its_local_end_in_both_families`; `ignite-rt`
  `abi::peer_timed_out_and_connect_within` (now also `tcp_local`); `ignite-types`
  `builtins::tests::substrate_methods_name_net_for_peer` (now `local` and `peer`).
- Moved, not changed: `render_head` renders the status line and header fields through the new
  `render_fields` (an upgrade's head has no `Connection` field of Http's own), so
  `ignite-rustgen` `allocations::text_is_appended_in_place_and_a_final_conversion_takes_the_buffer`
  looks for the in-place header loop in `render_fields`. The plan goldens `todo.model.json` and
  `studio.model.json` show the two new `HttpApp` fields, `None`, and moved std spans.

## 38 TLS client sessions as a ByteStream

**Status: implemented in `story:tls-client`.**

Case-study gap G25: the service refuses https backend URLs at start. A program opens a TLS
client session over a `TcpStream`, with the server's certificate verified against the system
roots by default, and uses it wherever a `ByteStream` is taken, so a protocol role generic over
its stream runs over TLS unchanged. The TLS engine is rustls in `crates/ignite-io` (`AGENTS.md`,
layout rules: the substrate knows sockets and TLS sessions); the module `std.tls` holds the
options record, the error test and the inline tests.

### 38.1 Surface

| item | meaning |
|---|---|
| `Tls.client(stream: TcpStream, server_name: Text, options: TlsOptions) -> Result<TlsStream, IoError>` | builtin static, effect `net.connect`: the client handshake on `stream`; `server_name` is sent as SNI and is the name (DNS name or IP address) the certificate must be valid for |
| `TlsStream` | builtin handle type (`Ty::TlsStream`), visible in every module as `TcpStream` is; implements `ByteStream`, so E096 accepts it for a bounded parameter; a copy shares the session |
| `read`, `write_all`, `close`, `set_idle_timeout`, `write_urgent`, `urgent_pending` | the six stream methods, resolved to the `Stream*` builtins with the receiver's effects of 18.3 (`read`: `<root>.read`, the rest `<root>.write`, `urgent_pending` `<root>.read`); each backend dispatches by the receiver's type as for a bounded parameter |
| `session.alpn() -> Text` | builtin `TlsStreamAlpn`, pure: the protocol the server chose from `options.alpn`, empty for none |
| `type TlsOptions` (`std.tls`) | `alpn: List<Text> = []` (offered, most preferred first), `authorities: Text = ""` (PEM certificates trusted instead of the system roots), `verify: Bool = true`, `handshake_timeout: Duration = 10 seconds` |
| `certificate_refused(error: IoError) -> Bool` (`std.tls`) | whether the error is the refusal of the server's certificate |

Behaviour:

- The handshake reads with the time left before `handshake_timeout` as the stream's idle
  timeout and restores the stream's own idle timeout when it returns. A timeout that is not
  positive is refused before any IO.
- System roots: `SSL_CERT_FILE`, else the first system bundle that exists
  (`/etc/ssl/certs/ca-certificates.crt` and the other distributions' paths), else every file of
  `SSL_CERT_DIR` or of `/etc/ssl/certs` and `/etc/pki/tls/certs`; read once per process.
- `verify: false` takes any certificate for any name; the handshake signatures are still
  checked.
- `close` sends close_notify, then closes the write half of the TCP stream. A read returns
  `Eof` after the server's close_notify and when the stream's idle timeout passes; a connection
  closed without close_notify is an error, since the data may have been cut short. Dropping the
  last handle closes the socket without close_notify.
- `write_urgent` is refused and `urgent_pending` is false: TLS carries no urgent data.
- A reader task and a writer task share one session: the rustls connection is behind a lock
  never held across a wait; a read holds the session's read lock across its wait for
  ciphertext, a write holds its write lock until its records are in the stream's send buffer.
- Records nobody wrote (the handshake's, an alert, a key update) are sent before the call
  returns: the session flushes right after queuing them, holding its write lock. Left in the
  stream's send buffer, the client's Finished kept the stream on the worker's flush list, and a
  reader task's flush then waited for the write lock of its writer task parked on a full socket,
  a deadlock on the Rust target under `readiness` (adversary pass 1).

Errors, each an `IoError` whose message starts with `tls <server_name>: `:

| case | message |
|---|---|
| a certificate the client does not accept | `tls <name>: certificate refused: <reason>`, `<reason>` one of `expired`, `not yet valid`, `not valid for <name>`, `unknown issuer`, `revoked`, or rustls' name of the error |
| the deadline passed | `tls <name>: handshake timed out after <ms> ms` |
| the timeout is not positive | `tls <name>: handshake timeout <ms> ms is not positive` |
| no certificate in `authorities` | `tls <name>: no certificate in the authorities` |
| a server name that is no DNS name or IP address | ``tls: invalid server name `<name>` `` |
| an ALPN entry that is empty or over 255 bytes | ``tls <name>: ALPN protocol `<p>` is not 1 to 255 bytes`` |
| the server closed during the handshake | `tls <name>: the server closed the connection during the handshake` |
| anything else rustls refuses (a peer that speaks no TLS, an alert) | `tls <name>: <rustls' message>` |

### 38.2 Lowering

- Rust target: `::ignite_io::TlsStream::client(stream, name, ::ignite_io::TlsOptions { .. })`,
  the record converted field by field; the stream methods as `<::ignite_io::TlsStream as
  ::ignite_io::ByteStream>::read(..)`; `alpn` as the inherent method.
- LLVM target: a session is a handle of the runtime's table (`Handle::Tls`), owned and
  released as a stream is; `ignite_rt_tls_client(i64 stream, Text* name, TlsOptions* options,
  i64* out, IoError* err)` with `TlsOptions` as its record layout (`IgniteTlsOptions`:
  `alpn`, `authorities`, `verify` as `i8`, `handshake_timeout` in nanoseconds),
  `ignite_rt_tls_alpn(Text* out, i64)`, and `ignite_rt_tls_<op>` for the six stream methods
  with the signatures of the `pipe_<op>` entries. A `tls_*` entry given another handle fails
  with "`<h>` is not a TLS stream".
- Generation changes (two builtins, one type), so `CODEGEN_REVISION` is `ignite-codegen/0.1.0-49` (wave 10, one bump with 37).

### 38.3 Dependencies

`rustls` 0.23 with the `ring` provider, `std` and `tls12` (no default features, so no
`aws-lc-rs`), in `ignite-io`; its tree adds `ring`, `rustls-webpki`, `rustls-pki-types`,
`untrusted`, `subtle`, `zeroize`, `once_cell` and `getrandom` 0.2. It sits behind the
`ignite-io` feature `tls`: the runtime archive of the LLVM target turns it on, and the Rust
target turns it on in a generated crate's `Cargo.toml` (program or `ignite-std`) whose source
names `::ignite_io::Tls..`, so a program without TLS compiles neither rustls nor ring
(`crates/ignite-rustgen/tests/streams.rs`,
`only_a_program_with_tls_turns_on_the_substrate_feature`). On the LLVM target a program that
opens no session links none of the engine (`llvm_build::a_program_without_tls_links_no_tls_engine`):
the runtime's handle table holds a session behind `dyn Any`, so only `ignite_rt_tls_client`
reaches the engine's drop and clone, and the workspace's dev profile builds these eight crates
and `rustls` at `opt-level = 2`, so they export no shared generic instances that the runtime's
own code would link in their place (in the dev runtime archive they made the LLVM release
executables of terminald and todod 9 and 11 % larger than at wave 9, `quality::release_binaries_stay_within_10_percent_of_v0_15_0`).
Nothing is replaced: the workspace
had no TLS. `rcgen` 0.14 is a dev-dependency of `ignite-io`
and `ignite` only, for the authorities the tests generate. No crate reads the system roots for
us (`rustls-native-certs` was not taken): the substrate reads the PEM bundle itself.

### 38.4 Tests

- `crates/ignite-io/tests/tls.rs` (`tls::`): a TLS server in the test with an authority
  generated by `rcgen`, on the loopback interface: a verified session carries bytes both ways
  as a `ByteStream`; a wrong host name, an expired certificate and the test authority under the
  system roots are refused with their messages; `verify: false`; an IP address as the name;
  ALPN; the handshake deadline against a silent server; a timeout that is not positive; a peer
  that speaks no TLS; an invalid server name; authorities without a certificate; a handshake
  read waits only for the time left; `verify: false` still checks the handshake signature;
  urgent data.
- `crates/ignite-io/tests/tls_adversary.rs` (adversary pass 1): names, wildcards, chains,
  TLS 1.2, a trickling or hanging-up server, truncation, and a reader and a writer on one
  session over 10 MiB, as threads and as tasks under both IO modes.
- `tls::rust::sessions_run` and `tls::llvm::sessions_run` (`crates/ignite/tests/tls.rs`): the
  same servers started by the test, and `ignite test` of a program with the authority's PEM and
  the ports filled in, on each target: a session as a `ByteStream` through a generic fn, ALPN,
  a wrong host name (and `certificate_refused`), an expired certificate, verification against
  the system roots by default, `verify: false`.
- `tls_adversary::rust::adversary_cases_run` and `tls_adversary::llvm::adversary_cases_run`
  (`crates/ignite/tests/tls_adversary.rs`), each under both IO modes: among them a writer task
  and the reader task it spawns carrying 10 MiB on one session, more than the socket buffers
  hold.
- `std/tls.ignite`, both targets, without a TLS server: the defaults, the handshake deadline
  against a server that never answers, a timeout that is not positive, a peer that speaks no
  TLS, an invalid server name, authorities without a certificate.
- No network: every certificate is generated in the test.

## 39 A dev build, a check and a plan take no longer than at v0.15.0

Story `build-time-back`: rows 6.2 (`llvm`, `rust`), 6.1 (`all/check`, `all/plan`) and the dev
cells of 3.2 of `docs/quality.md` at v0.15.0 or better. The dedicated host has no cargo
configuration, so its dev profile has full debug information; every rule below holds for it
and for a configured `debug` alike.

- Runtime archive. The workspace's dev profile builds every crate from outside the workspace
  without debug information (`[profile.dev.package."*"] debug = false`) and `ignite-rt` and
  `ignite-io` with line tables (`debug = "line-tables-only"`); it replaces the five `debug =
  false` lines of the TLS engine's crates. `ring`'s build script compiles its C and assembly
  with `-g3` in every profile, so `.cargo/config.toml` sets `CFLAGS = "-g0"`, which `cc`
  passes last (a `CFLAGS` of the environment wins). A dev executable on the LLVM target holds no compile
  unit of a third-party crate (`dev_build::dev_todod_links_no_third_party_debug_information`,
  with the archive built as configured and with `CARGO_PROFILE_DEV_DEBUG=2`). The driver gives
  its own cargo runs (the build ahead and the program's build) `HOST_CFLAGS=-g0` and
  `TARGET_CFLAGS=-g0` wherever it stands, unless `CFLAGS`, `TARGET_CFLAGS`, `HOST_CFLAGS` or a
  `CFLAGS_<target>` is set (`c_flags_tests::the_driver_sets_cflags_only_where_the_user_set_none`),
  so a TLS program built outside the repository holds no third-party compile unit either
  (`dev_build_adversary::a_tls_program_built_outside_the_repository_holds_no_third_party_debug_information`).
  `cc` passes them after `CFLAGS`, and the driver leaves `CFLAGS` alone: a user's `[env]
  CFLAGS` of a cargo configuration, which cargo does not apply over a `CFLAGS` of its
  environment, reaches the C compiler
  (`dev_build_adversary::c_flags_of_the_users_cargo_configuration_reach_the_c_compiler`). A
  profile without debug information for the crates that compile C would not do: `ring` asks
  for `-g3` whatever cargo's `DEBUG`.
- Linker. A dev build on the LLVM target without `--triple` and `--static` links with
  `-fuse-ld=lld` when `ld.lld` lies beside the clang it runs, its symbolic links resolved, and
  with clang's default linker otherwise (`docs/llvm.md` section 1); both carry a build id
  (`dev_build::dev_link_uses_lld_beside_clang_else_the_default_linker`). The dedicated host
  installs `lld-<major>` of the LLVM it installs (`tools/ignite-perf/src/remote-user-data.sh`,
  `remote::the_host_installs_lld_beside_its_clang`).
- Rust target. The generated `Cargo.toml` builds the crates from outside the project without
  debug information and `ignite-io`, the program and `ignite-std` with line tables, split out
  into `.dwo` files (section 6.2), whatever the profile's own `debug`; a build or a test build
  into a directory without `Cargo.lock` starts from the substrate workspace's lock, so cargo
  makes no index request (`dev_build::fresh_rust_build_makes_no_index_request`).
- Dependencies ahead (Rust target). Before it parses (after, when `--out` is not given), the
  driver writes a crate that depends on `ignite-io` with the generated project's profiles into
  `<target-dir>/ignite-prebuild/`, with the project's lock or else the substrate workspace's,
  and starts `cargo build --quiet --offline -p corosensei -p nix -p termios` from it into the
  build's target directory, its output discarded: the crates `ignite-io` builds on, which the
  program's build then finds fresh whatever features of `ignite-io` it turns on (`process` and
  `tls` change none of them; `units::only_tls_changes_a_dependency_of_ignite_io`,
  `dev_build::dependencies_built_ahead_are_fresh_in_the_programs_build`). The program's build
  starts after that cargo has ended. A target directory that has built `nix` for the profile
  builds nothing ahead, and neither does a `--static` build. A load, type or generation error
  stops that cargo and every process under it before the driver reports the error
  (`dev_build::a_type_error_stops_the_build_ahead`), and so does a panic as it unwinds
  (`prebuild::tests::a_panic_stops_the_build_ahead_and_what_it_started`). While it runs, SIGTERM,
  SIGINT and SIGHUP sent to the driver alone are taken by a thread of the driver's, which stops
  it the same way and ends the driver by that signal, also one it takes while it is being
  stopped (`prebuild::tests::a_signal_the_watcher_takes_while_it_stops_ends_the_driver`); for
  that time every other thread blocks them, the main thread too, which waits for the compiling
  thread and blocks them when asked. A signal the driver ignores stays ignored, one it was
  started with blocked stays blocked
  (`dev_build_adversary::a_driver_ended_by_sigterm_leaves_no_build_ahead`, `_sigint_`,
  `_sighup_`, `a_driver_started_with_an_ending_signal_blocked_keeps_it_blocked`). The cargo
  starts (`posix_spawn`) with the signal mask the driver was started with, so it and what it
  starts block none of the three
  (`dev_build_adversary::the_build_ahead_blocks_no_signal_and_ends_on_sigterm`). A command that
  builds nothing ahead makes no signal call for it and reads no `/proc/self/status`
  (`dev_build_adversary::a_command_without_a_build_ahead_makes_no_system_call_for_its_signals`).
  SIGKILL cannot be taken: the build ahead then runs to its end, and the next build
  finds what it built fresh (or waits for it on cargo's lock of the target directory). Row 6.2 rust: the driver's own work then
  runs beside the longest chain of the dependency build (`libc`, `nix`) instead of before it.
- Substrate. `nix` is built without `term`: standard input's raw mode reads and sets the
  terminal attributes through the `termios` crate (`tcgetattr`, `tcsetattr`: one `ioctl` each,
  as through `nix`), and only the tests that open a pty turn `term` on
  (`units::ignite_io_builds_nix_without_term`); `term` cost 938 M of `nix`'s 6,038 M
  instructions before its metadata. `ignite-io`'s `child` and `pipe` modules are behind its
  feature `process`, which the generated manifest turns on when the program or its
  `ignite-std` names a launcher, a child process or a pipe, and `runner` and the runtime archive
  turn on (`streams::only_a_program_with_processes_or_pipes_turns_on_the_substrate_feature`);
  without it `ignite-io` takes 383 M fewer before its metadata. A program without it does not
  reset SIGCHLD at start: it starts no child.
- Loader. The dependencies of each module (`module_dependencies`) are computed once per load
  and kept with the parsed module (`dependency_scan_tests::check_and_plan_scan_each_module_once`
  in `crates/ignite/src/lib.rs`); `plan.json` of the terminal design is unchanged
  (`plan::design_model_is_byte_identical_across_processes_and_directories`).
- Backtraces. Under `gdb`, a dev executable that fails names file and line of the Ignite fn
  and of `ignite-rt`'s and `ignite-io`'s frames on the LLVM target, and of `ignite-io`'s and
  the generated program's frames on the Rust target, whose failure report names the Ignite
  line (`dev_build::dev_backtrace_names_ignite_and_substrate_lines_on_both_targets`).
- Not taken: `incremental = false` in the generated dev profile (`ignite-perf` builds with
  `CARGO_INCREMENTAL=0` already, so row 6.2 would not see it, and a user's rebuild would lose
  it); `nix` without its `socket` feature: `recv` with `MSG_DONTWAIT` into an initialised
  buffer has no safe call in `std` or `socket2` (their flagged receives take
  `[MaybeUninit<u8>]`), and a socket made non-blocking at creation cannot block: the read of a
  worker's only task in a request-reply rhythm (`sched::block_alone`), one blocking `recv` per
  request now, would wait in `poll` first (`todod` 2 to 3 calls per keep-alive request), and
  `connect` would return `EINPROGRESS` and wait in `poll` (one call more per outgoing
  connection); whether a read may block is decided per read, so no socket can be given the
  right mode at creation. Building `ignite-io` ahead as well: the program's build would wait for
  its code generation instead of `nix`'s (worse in the model); one cargo for the dependencies and
  the program: cargo fixes its units when it starts, before the driver has generated the
  program. `nix` without `dir` (48 M) or `resource` (71 M, and `std` has no `setrlimit`), and
  `entropy` and `environment` behind features (70 M together).

## 40 An HTTP reply allocates no more than at v0.15.0, and a server without held requests starts no task for them

Story `http-reply-cost-back` (rows 3.3 `rust/todo/get-1` and 3.8 `todo/get-1` of
`docs/quality.md`).

- Allocations per `GET /todos/1` reply of `todod`, counted by `valgrind --tool=memcheck
  --trace-malloc=yes` over 200 replies after 50 on one connection
  (`reply_cost::a_get_reply_allocates_no_more_than_at_v0_15_0`, the bounds measured by
  `reply_cost::count_a_given_todod` over the v0.15.0 binaries): Rust target 37 at v0.15.0, 53
  at wave 10, 37 at the unit's merge, 40 after `unsafe-sites-back`, 36 now; LLVM target 1, 1, 0.
- The 16 came with `Request.connection` (section 37): `session` copied the connection into the
  request it kept for the answer (its two addresses, 2 allocations), `asked` copied it again (2),
  the request asked was copied once more for `handle` or `reply` (8) and `Respond`'s response was
  copied out of the reply (4). `session` now sets the connection on the copy the application is
  asked only, moves that copy into `handle` or `reply` and the response into `respond`; the
  request kept for the answer gets the connection only for a `Streamed` response, whose `piece`
  sees it. An `Upgrade` copies the request again for its takeover. The connection is still copied
  once per request (2 allocations).
- Two allocations per reply that v0.15.0 made too pay for those 2: `read_head` takes the first
  read of a request as the request's bytes instead of copying it (`read_once`: a binding of
  `stream.read` matched in place is a borrow of the stream to the last-use analysis, so it was
  copied), and `ignite_io::flush` gives the emptied list of streams to send back to the task
  instead of dropping it, so the task's next write does not allocate it again (the one
  allocation per reply of the LLVM target).
- Story `unsafe-sites-back` removed the counting allocator's own `realloc`: every growth of a
  buffer became a new allocation, a copy and a free, and the three growths of rendering a reply
  made the Rust count 40. The allocator reallocates through the system allocator again
  (`census::tests::a_growing_buffer_is_reallocated_not_allocated_again` counts the calls that
  reach `alloc` and `realloc`), and an inline reply is rendered into one buffer made with room
  for its head (`HEAD_ROOM`, 128 bytes, and each header field) and its body (`render_fields`'s
  `after`, `text_with_room`): the head is written into it, the body is appended to it from the
  response instead of being copied first, and the buffer becomes the bytes sent, each step
  taking the buffer of the one before, not copying it (on the Rust target a conversion is a
  move only as a function's last expression, which `head_bytes` and `render_framed` keep;
  `allocations::text_is_appended_in_place_and_a_final_conversion_takes_the_buffer` of
  `crates/ignite-rustgen` reads the generated code). Rust target 36 now, LLVM target
  0.
- A task's stack overflow is reported on the worker's alternate signal stack. On the Rust target
  that stack is std's, which every thread std starts has; `ignite-io` makes none. The LLVM
  runtime, whose `main` is not Rust's, gives each worker its own
  (`ignite_io::signal::give_workers_an_alternate_stack`, set by `init`). On the Rust target
  `ignite-io` puts the ignore back over its handler for a signal found ignored at the start,
  and std gives its threads a stack only when it installed a handler, for a signal not
  ignored. An overflow faults with `SIGSEGV`, so by the start on the Rust target: nothing
  ignored, or only `SIGBUS`, the overflow is reported and the process aborts; `SIGSEGV`
  ignored, alone or with `SIGBUS`, the overflow ends the process by `SIGSEGV` with no report, as
  a v0.15.0 program started so did (also when only `SIGSEGV` is ignored and std gave the
  thread a stack), and other faults of an ignored signal reach the ignore too. The LLVM target
  reports the overflow in all four starts (`stack_overflow::` and
  `stack_overflow_adversary::` of `crates/ignite`; `scheduler::started_with_faults_ignored_*`
  and `reply_adversary::started_with_only_*` of `crates/ignite-io`).
- `serve_listener` starts the register of held requests (`Holds.start()`, the task
  `keep_holds`) only when the application has a `reply`, the only way to hold a request:
  `reply_cost::a_server_without_reply_starts_no_task_for_held_requests` builds
  `tests/fixtures/http_census.ignite` on both targets and reads `Runtime.census().tasks` while
  each of its servers answers its only connection: 1 without `reply` (2 before), 2 with one (as
  before).
- `valgrind` is a prerequisite of `reply_cost::`, as `strace` is of `scheduler::`.

## 41 Every binary is at or below its v0.15.0 size, and peak memory at one connection with it

Story `binary-size-back`: rows 3.9 and 3.2 of `docs/quality.md` towards v0.15.0's bytes
without a program running slower, and row 3.8 at one connection with them. Each rule names the test that holds it; the bytes
of each step are in the commits.

- Dev links. The line tables of `ignite-rt`, `ignite-io`, the generated program and the
  generated `ignite-std` are split out of their objects into `.dwo` files beside them
  (`split-debuginfo = "unpacked"` in the workspace's and the generated dev profile), so the
  linker neither copies nor relocates them. With lld the LLVM target's dev link compresses
  the two address tables only
  (`-Wl,--compress-sections=.debug_aranges=zlib`, the same for `.debug_addr`), with another
  linker every debug section; the Rust target compresses nothing. gdb, lldb,
  binutils and the standard library's symbolizer read both, so a backtrace keeps its files
  and lines while the target directory holds the `.dwo` files, on the Rust target in the
  standard library's frames too
  (`dev_build::dev_backtrace_names_ignite_and_substrate_lines_on_both_targets`,
  `binary_size_adversary::a_rust_dev_backtrace_names_file_and_line_of_a_standard_library_frame`). A dev link
  takes fewer instructions than without either (`docs/llvm.md` section 1).
- Packed relative relocations. Every dynamic executable of the LLVM target links with
  `-z pack-relative-relocs` and needs glibc 2.36 or newer (`GLIBC_ABI_DT_RELR`; before, 2.34);
  a `--static` build takes none. The Rust target links without it and needs 2.34 as before:
  there it made the start longer, and the `rustflags` table that carried it replaced the
  user's `build.rustflags`
  (`binary_size::llvm_executables_pack_their_relative_relocations_and_rust_ones_do_not`,
  `binary_size_adversary::a_rust_build_keeps_the_build_rustflags_of_the_cargo_configuration`).
- Run time first. A binary may stay above its v0.15.0 bytes; a step stays only if the
  instructions a program runs (callgrind: start, each request kind, a connection, a terminal
  session, both IO modes, both targets) and the syscalls it makes are equal or fewer with it.
  The commits that undid a step give the counts.
- Scheduler. A worker's timers are a binary heap with at most one entry per task, whose
  position each task slot records: the entry of a task that ends is taken out at once, so the
  worker never wakes for it, and one armed earlier moves up in place (`sched::tests` in
  `crates/ignite-io/src/sched.rs`,
  `timer_adversary::a_worker_does_not_wake_for_the_timer_of_a_session_that_ended`). The epoll
  reactor holds its registrations in a slab whose tokens carry a generation and whose free
  slots form a list in the slab (`reactor::*::a_reused_registration_has_a_token_of_its_own`).
  `IGNITE_IO=threads` stays.
- Opt-levels. The generated manifest builds `nix` and `corosensei` at `opt-level = "s"`: the
  start about 200 instructions shorter, the stripped executable about 570 bytes larger
  (`units::release_profile_builds_nix_and_corosensei_at_opt_level_s`); `ignite-rt`, `ignite-io`
  and `ignite-std` keep the default level.
- Release LLVM link. `-Wl,--discard-all` drops the symbols local to the linked objects;
  global names stay
  (`binary_size::an_llvm_release_executable_drops_the_local_symbols_of_its_objects`).
  A release executable's failure report is the dev one's on both targets
  (`binary_size::a_release_failure_report_is_the_dev_one_on_both_targets`).
- Release runtime. The LLVM target's release runtime archive is built on the standard
  library from source without its backtrace symbolizer, one recipe for the driver's build
  script, `ignite-perf runtime`, `task bench-throughput` and the dedicated host
  (`crates/ignite-rt/release_std.rs`, `docs/llvm.md` section 1); a toolchain without
  `rust-src` builds on the prebuilt one and says so
  (`runtime::the_release_runtime_leaves_out_the_symbolizer_unless_the_std_source_is_missing`).
  `AGENTS.md` § Releases names the requirement.
- TLS engine. A program that names no `Tls` holds no symbol and no compile unit of `rustls`,
  `webpki`, `ring` or `untrusted` on the LLVM target, dev and release, with the runner
  archive (`binary_size::an_llvm_program_without_tls_links_no_tls_engine_in_dev_and_release`).
  With the release-profile archive (`--static`, `ignite-perf runtime`) no section of those
  crates is kept either, but functions of `ignite-io` that LLVM merged with identical ones
  over `rustls` types keep those names as aliases; one crate of their own for the engine
  (story `tls-engine-own-crates`) takes them out.
- Libraries as needed. The LLVM target links its runtime libraries after `-Wl,--as-needed`,
  as rustc links the Rust target, so an executable loads no `libm` it calls nothing of
  (`binary_size::an_llvm_executable_loads_no_library_it_does_not_call`).
- Not taken: DWARF 5, split or not (larger archive and executable, a longer link); zstd
  debug sections (the standard library's symbolizer does not read them); compressing the
  debug strings or every debug section (the link's instructions above their uncompressed
  count); `ignite-io` at `opt-level = 1` in the generated dev profile (a dev `todod` 0.5 MB
  smaller, compiling `ignite-io` four times the instructions); one codegen unit per crate
  in it (a few tens of KB, compiled on one core); dropping `IGNITE_IO=threads` (decided to
  stay); `opt-level = "s"` for `ignite-rt`, `ignite-io` and `ignite-std` (5 to 7 % more
  instructions per request); a worker limit of its own from the affinity mask and the cgroup
  quota (about 16,000 more instructions at the first connection than
  `std::thread::available_parallelism`).
- Generation changes in wave 11 (the LLVM target closes a served connection's socket at the
  end of its session; the Rust target's release project carries the linker layout above), so
  `CODEGEN_REVISION` is `ignite-codegen/0.1.0-50` (one bump for both).

## 42 std.protocol.mysql: MySQL wire client with caching_sha2_password

**Status: implemented in `story:std-mysql-client`.**

Case-study gap G11: no MySQL client in std and no FFI. `std.protocol.mysql` is a client of the
MySQL wire protocol (protocol 41, handshake version 10), generic over `T: ByteStream`, written in
Ignite on the existing stream, channel, TLS and digest builtins (`std.digest.sha1` and `sha256`
are the two scrambles' hashes); no builtin, no substrate code and no dependency is added, so the
compiler and both backends are unchanged and no generated code changes. Its first consumer reads
another party's database: the client sends no statement of its own except the one `read_only`
asks for, and nothing it returns as a description of an error holds a statement or a credential.

### 42.1 Packets, limits and errors

| item | meaning |
|---|---|
| `frames(payload, sequence) -> Bytes` | one packet: frames of a 3-byte length and a sequence number modulo 256; a payload of 16 MiB - 1 bytes or more is split, and one that fills its last frame is followed by an empty frame |
| `read_packet(stream, maximum, buffer, at, sequence)` | the next packet with its frames joined, the bytes read so far, the offset after it and the next sequence number; a frame with another number is `OutOfOrder(expected, got)`, a payload above `maximum` is `PacketTooLarge` when its frame header is read, before its bytes are |
| `type MysqlLimits` | `packet` 64 MiB (one packet, frames joined), `row` 16 MiB (one row's packet), `rows` 1,000,000 (rows of one result set), `columns` 4,096 (its columns) |
| `enum MysqlError` | `Server(ServerError)`, `Io(Text)`, `Closed`, `OutOfOrder(Int, Int)`, `PacketTooLarge`, `RowTooLarge`, `TooManyRows`, `TooManyColumns`, `Malformed(Text)`, `Unsupported(Text)`, `UnsupportedPlugin(Text)`, `FullAuthenticationNeedsTls`, `TlsNotOffered`, `WrongParameterCount(Int, Int)`, `NoConnection` |
| `type ServerError` | the server's error packet: `code`, `sql_state` (empty when the server sent none), `message` |
| `describe(error) -> Text`, `io_error(error) -> IoError` | the error in words; of `Server` the code and the SQL state only (`server error 1064 (42000)`), since the server's message can quote the statement and name the account; of `UnsupportedPlugin` the printable ASCII among the first 64 bytes of the name, which the server chose |

Every call returns `Result<_, MysqlError>`. `Server` leaves the connection usable, as does
`WrongParameterCount`, which is found before anything is sent. Every other error of a call on a
connection closes its stream, so a connection that is out of step is not used again. `Closed` is
the end of the stream and also a read that passed the stream's idle timeout, which the substrate
reports alike. The module logs nothing.

### 42.2 Connecting and authentication

`type MysqlOptions { user, password = "", database = "", read_only = false, charset = 45
(utf8mb4), limits, connect_timeout = 10 seconds, read_timeout = 30 seconds, server_name = "",
tls: TlsOptions }`. `connect(endpoint, options)` is a `Client<TcpStream>`; `connect_tls` is a
`Client<TlsStream>`: it reads the server's handshake on the TCP stream, sends the SSLRequest
packet and opens the `std.tls` session with `options.tls` for `options.server_name` (the
endpoint's address when empty) before the account is sent, and fails with `TlsNotOffered` when
the server's handshake offers no TLS. Both dial with `TcpStream.connect_within(connect_timeout)`
and set the stream's idle timeout to `read_timeout`. `start(stream, options)` authenticates on
any stream already connected (the tests' `PipeStream`); `request_tls` and `authenticate` are the
two steps of `connect_tls` for a caller with a stream of its own. Whether a connection uses TLS
is the caller's choice of function, because the two give different stream types; there is no
"TLS when offered" mode. On the LLVM target a connection's descriptor is released when the
process exits: `quit` and every failure close the stream, but a client record that a function
returns is owned by no block (`docs/llvm.md`, section 4), so a program that opens connections
without end runs out of descriptors there. The Rust target releases the descriptor when the
client is dropped or quit.

| the server | the client |
|---|---|
| handshake names `mysql_native_password` | answers SHA1(password) xor SHA1(scramble, SHA1(SHA1(password))) |
| handshake names `caching_sha2_password` | answers SHA256(password) xor SHA256(SHA256(SHA256(password)), scramble) |
| authentication switch (0xFE) to either plugin | answers with that plugin's scramble over the new nonce |
| `caching_sha2_password` fast authentication (0x01 0x03) | reads the OK that follows |
| `caching_sha2_password` full authentication (0x01 0x04) on a TLS stream | sends the password and a NUL |
| the same on a plain stream | `FullAuthenticationNeedsTls`, which names TLS; nothing is sent, the stream is closed. The RSA key exchange is not spoken |
| any other plugin, in the handshake or in a switch | `UnsupportedPlugin(name)`; the pre-4.1 switch is `mysql_old_password` |
| an error packet | `Server`, as for a wrong password (1045, `28000`) |

An empty password answers with nothing under both plugins. The client asks for the capabilities
`LONG_PASSWORD`, `LONG_FLAG`, `PROTOCOL_41`, `TRANSACTIONS`, `SECURE_CONNECTION`, `PLUGIN_AUTH`,
`PLUGIN_AUTH_LENENC_CLIENT_DATA`, `CONNECT_WITH_DB` for a database and `SSL` after the
SSLRequest. It asks for no compression, no `LOCAL_FILES`, no `MULTI_STATEMENTS`, no
`MULTI_RESULTS` and not for `DEPRECATE_EOF`. A server without protocol 41 or without plugin
authentication, a handshake of another version, a user or database name that holds a NUL and an
authentication of more than 8 packets are refused. With `read_only` the client sends `SET SESSION
TRANSACTION READ ONLY` after authentication and returns only when the server took it: the server
then refuses every write of the session (1792, `25006`).

### 42.3 Statements and values

`query(client, sql)` is `COM_QUERY` over the text protocol. `prepare(client, sql)` is
`COM_STMT_PREPARE` and gives a `Statement { id, parameters, columns }`; `execute(client,
statement, parameters)` is `COM_STMT_EXECUTE` over the binary protocol with one
`Option<MysqlValue>` per `?` mark; `close(client, statement)` is `COM_STMT_CLOSE`;
`execute_once(client, sql, parameters)` does the three. `ping` is `COM_PING`, `quit` is
`COM_QUIT` and closes the stream. A command of 16 MiB - 1 bytes or more (a long statement, a
large parameter) travels as two frames or more, and its reply is read at the sequence number
after its last frame. `query`, `execute` and `execute_once` give a `ResultSet
{ columns, rows, affected, last_insert_id, warnings }`: the `Column`s (`schema`, `table`,
`name`, `charset`, `length`, `kind`, `flags`, `decimals`) and the rows, each a
`List<Option<MysqlValue>>` with `None` for NULL, or for a statement without a result set its
affected rows and inserted id.

A parameter is never put into the statement's text: `prepare` sends `sql` as written, and
`execute` sends each parameter as a typed value behind the NULL bitmap (`Int` as BIGINT, `Bool`
as TINYINT, `Decimal` as DECIMAL in text, `Text` as VAR_STRING, `Bytes` as BLOB, `None` as
NULL). `query` sends `sql` as it is; a value from outside belongs in a parameter. Another number
of parameters than the statement has is `WrongParameterCount(expected, got)` before anything is
sent.

| column type | `MysqlValue` |
|---|---|
| BIT(1) | `Bool` |
| TINYINT (TINYINT(1), BOOL and BOOLEAN among them), SMALLINT, MEDIUMINT, INT, BIGINT, YEAR, signed and unsigned | `Int`; a BIGINT UNSIGNED above the largest `Int` is `Decimal` with all its digits |
| DECIMAL | `Decimal`, the server's exact text |
| FLOAT, DOUBLE | `Decimal`: the language has no floating-point type (the story's `Float`), so the value is its decimal text, as `std.json`'s and `std.data`'s `Decimal` |
| CHAR, VARCHAR, TEXT, ENUM, SET, JSON | `Text`; bytes that are no UTF-8 become U+FFFD |
| DATE, TIME, DATETIME, TIMESTAMP | `Text` as the server prints them: `2024-02-29`, `-34:11:12`, `2024-02-29 23:59:58.123456` |
| BINARY, VARBINARY, BLOB, GEOMETRY, BIT wider than one | `Bytes` (a column whose character set is 63) |

A TINYINT(1) is an `Int`, not a `Bool`: the 1 is a display width, and the column holds -128 to
127 (0 to 255 unsigned), so a stored 2 reads as 2. BOOL and BOOLEAN are TINYINT(1) on the
server and read as `Int` 0 or 1; only a BIT(1) is a `Bool`. A `Bool` parameter is still sent as
TINYINT.

The two protocols give the same value for the same column. The binary protocol sends a FLOAT and
a DOUBLE as their four and eight bytes; `std.protocol.mysql.binary` (`float_text`,
`double_text`) prints them as the shortest decimal digits that read back as the same number, by
Burger and Dybvig's free-format algorithm over natural numbers in base 10^9, of two last digits
equally near the even one, and in the notation of the server's text protocol: plain digits from
1e-15 to below 1e15 (`0.1`, `123456.789`, `0.000000000000001`; `1000000000000000.2`, whose point
has a digit after it), one digit, a fraction and an exponent beyond (`1e15`, `1.5e-16`), `-0` for
a negative zero. The text protocol carries the server's own text, which for a FLOAT is rounded
to six significant digits, so the same FLOAT can print with more digits from `execute` than from
`query`; a DOUBLE prints alike (tested with values of no declared number of decimals). The
module also prints the binary dates and times as the text protocol does (`date_text`,
`time_text`, the fraction's digits from the column's decimals) and an unsigned 64-bit integer
(`unsigned_text`).

Refused, each as a typed error that closes the connection: the server's request for a local file
(`LOAD DATA LOCAL INFILE`, `Unsupported`; the client never reads a file and sends nothing in
answer), a second result (`Unsupported("more than one result")`), a row above `limits.row`
(`RowTooLarge`, refused at its frame header), more rows or columns than the limits, a binary
column of a type the client does not know (`Unsupported("column type N")`), and every reply that
is cut, out of sequence, out of shape or followed by bytes (`Closed`, `OutOfOrder`,
`Malformed`). A server error in place of a result set or between its rows is `Server` and leaves
the connection usable.

### 42.4 Several connections

A connection carries one statement at a time: two tasks that send on one connection read each
other's replies. A program whose tasks ask concurrently holds several connections in a
`Pool<T>`: `pool_of(clients, wait)` makes one of connections already open, which holds as many
as it starts with; `checkout(pool)` takes one out, waiting up to `wait` (`NoConnection` after
it); `checkin(pool, client)` puts one back, or puts a new connection in the place of one that
failed, and quits a connection the pool has no room for. `pool_query(pool, sql)` and
`pool_execute(pool, sql, parameters)` take a connection, run the statement (`query`,
`execute_once`) and put the connection back. A connection whose call failed with anything but
the server's error is closed and not put back: a failed connection is out of the pool, and the
pool is smaller until the caller hands it a new one. No statement is run a second time on
another connection. `quit_idle(pool)` quits the connections that are free.

### 42.5 Layering

`mysql` is a protocol name for `layering::` (`docs/layering.md` rules 3 and 5): no module
outside `std.protocol.*` and no file under `crates/*/src` names it, and none did. The module
imports `std.net`, `std.io`, `std.tls`, `std.digest`, `std.encoding`, `std.result` and
`std.text`, all of the base layer, and its own `std.protocol.mysql.binary`, which imports
nothing.

### 42.6 Tests

- `std/protocol/mysql.ignite`, both targets, 56 tests with a scripted server (`play`: reads each
  expected request byte for byte, writes its reply; the requests are written out by hand, the
  scrambles are known answers computed outside the module), over `Pipe.pair()` but for two over
  TCP: a command of two frames, which a pipe does not hold, and the close of a failed handshake,
  which only a `TcpStream` tells from its idle timeout:
  - packets: integers and lengths at every width; a packet over many small deliveries; a
    payload of 16 MiB - 1 with its empty frame, and one longer, joined when read; a frame out of
    sequence; a packet one byte above the limit, refused at its header; a cut packet;
  - the handshake: the server's packet field by field and each refusal; both scrambles for two
    passwords and two nonces; the response and the SSLRequest byte for byte; each plugin as the
    server's default; the fast path; full authentication on a plain stream (the typed error, and
    no byte sent after it); full authentication after the SSLRequest at sequence 2; a switch in
    either direction; `sha256_password`, `mysql_clear_password` and the pre-4.1 switch refused
    by name, and what `describe` keeps of a forged name; a refused account; a server without
    `SECURE_CONNECTION`; eight authentication packets followed and the ninth refused; bytes
    after the authentication; a NUL in the database name; the stream closed by a failed
    handshake; a read-only session, and that no other session sends a statement;
  - statements: a column definition from the protocol's documentation; a result set of 31
    columns, one per supported type, with a row of values and a row of NULLs, over the text and
    over the binary protocol with the same expected values (its TINYINT(1) holds 2); a
    TINYINT(1) holding 0, 1, 2, -1 and, unsigned, 255 as that `Int` in both protocols, a BIT(1)
    as a `Bool`; OK, an empty result set, a server
    error first and between rows; the local-file request; a second result; each limit at the
    bound and one past it; every cut, out-of-sequence and out-of-shape reply; `prepare` with
    and without parameters and columns, with 4,096 columns and refused with 4,097; the bytes
    of `execute` for one, six, nine and no parameters, a hostile text among them; a parameter
    of 16 MiB - 1 bytes, whose command is two frames and whose reply is read at sequence 2;
    the NULL bitmap's offset; `close`; a wrong parameter count;
  - the pool: one caller per connection, the wait, a full pool, a connection returned after a
    result and after a server error, a failed one taken out and replaced, three tasks on one
    connection.
- `std/protocol/mysql/binary.ignite`, both targets, 9 tests: 36 doubles and 13 floats against
  the digits Python's `repr` and NumPy's `float32` print, in the server's notation (the largest,
  the smallest and the lowest normal number, both notations at their bounds, a tie between two
  last digits, the zeros, what is no number), unsigned integers up to 2^64 - 1, dates and times
  at every length the protocol sends.
- `mysql::real_servers_with_both_plugins_on_both_targets` in `crates/ignite/tests/mysql.rs`:
  two containers of `mysql:8.0` on free loopback ports, one whose default plugin is
  `caching_sha2_password` and one whose default is `mysql_native_password`, both with TLS under
  an authority the test generates; then 10 Ignite tests on each target: the password over TLS
  only and then the fast path, each plugin directly and after a switch, a wrong password,
  `sha256_password`, an unknown authority, another host name, a closed port, a table with a
  column of every type read alike by `query` and by `execute`, a TINYINT(1) holding 0, 1, 2
  and -1 and a BOOLEAN holding TRUE read as `Int` by both, parameters (a hostile text
  selects nothing), a read-only session refusing an INSERT, a pool of two connections under
  three tasks with a killed connection taken out, and the read timeout. It is not `#[ignore]`:
  it runs where `docker version` succeeds and the image is present (`docker pull mysql:8.0`),
  and says on standard error why it did nothing anywhere else; it pulls nothing itself.
  `IGNITE_MYSQL_IMAGE` names another image, `IGNITE_CONTAINER_LABEL` a label for the containers.
- `mysql_adversary::` in `crates/ignite/tests/mysql_adversary.rs`, two tests. One runs as
  `mysql::` does, against one container: a parameter of 16 MiB - 1 bytes and a statement of
  16 MiB, each a command of two frames, and 17 DOUBLE values that print alike from `query` and
  from `execute`. The other needs no container: 40 connections quit, 40 refused, 40 refused
  TLS requests and 40 pools of three leave no descriptor on the Rust target and keep one each
  on the LLVM target, which the test pins exactly (42.2).
- `layering::`, with `mysql` in the protocol names it reads from std.
