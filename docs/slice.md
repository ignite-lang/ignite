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
compare    := additive (('<' | '>' | '<=' | '>=') additive)?
additive   := multiplicative (('+' | '-') multiplicative)*
multiplicative := unary (('*' | '/' | '%') unary)*
unary      := ('!' | '-') unary | 'await' unary | 'spawn' unary | postfix
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

A builtin method name (`len`, `to_text`, ...) is never resolved by 2 or 3: on a receiver type
without that builtin it is E041. A field of fn type and a fn with the same name both
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
| `TcpListener.bind(a, p)` | `net.listen` |
| `TcpStream.connect(a, p)` | `net.connect` |
| `f(a, ..)`, `a.f(..)`, `P.role(a, ..)`, called or spawned | each entry `q.op` of the callee's declared or inferred set whose `q` is a parameter of the callee becomes `r.op` for each root `r` of the argument passed for `q` (the receiver of `a.f(..)` is the first argument); every other entry (`stdin.read`) is needed unchanged |

`x.local_port()` is pure and needs nothing. `x.set_idle_timeout(ms)` needs `r.write`; the
`Store` methods, calls of values of effectful fn type and `x.*` are in section 11.7.

**Roots.** The roots of an expression are the parameters of the enclosing function its value
may come from:

- a parameter `p` has the root `p`;
- a local (`let`, `var`, `for`, pattern binding) has the roots of the expression it was bound
  from; an assignment `x = e` (or `x.f = e`) adds the roots of `e` to those of `x`, whatever
  the control flow, so a `var` assigned from two parameters needs effects on both; `l.push(e)`
  and `l.append(e)` add the roots of `e` to those of the local `l`;
- `e.f`, `e[i]`, `e.copy()`, `e.at(i)`, `e.slice(a, b)` and `e?` have the roots of `e`;
  `a.concat(b)` has those of `a` and `b`;
- a tuple, list literal or record literal has the roots of its elements or field values;
- `if`, `match` and a block have the roots of every value they may produce: the last
  expression of each branch, arm or block, with that branch's locals in scope;
- the result of any call has the roots of each argument whose type can hold a substrate
  handle (`TcpStream`, `TcpListener`, `Store`, `Stdin`, `Stdout`, directly or in a field,
  payload, element or type argument; a type parameter or an unknown type may). This covers a
  user fn called by name or by UFCS (the receiver is an argument), pure or effectful,
  `requires []` included; a fn value or fn-typed field; a variant constructor (`Some(s)`); a
  protocol role; and a `Store` method (receiver and arguments). A callee cannot launder a
  parameter by returning it;
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
| `List<T>` | `Vec<T>` |
| `Option<T>`, `Result<T, E>` | same |
| `fn(A, B) -> R` | `std::sync::Arc<dyn Fn(A, B) -> R + Send + Sync>` (13.3) |
| `TcpListener`, `TcpStream`, `Stdin`, `Stdout`, `Input`, `IoError` | `ignite_io::` same name |
| record `type R { a: T }` | `#[derive(Clone, Debug)] pub struct R { pub a: T }` |
| `enum E { A, B(T) }` | `#[derive(Clone, Debug)] pub enum E { A, B(T) }` |
| `fn f(p: T) -> U` | `pub fn f(p: &T) -> U`, or `pub fn f(p: T) -> U` when `f` consumes `p` (parameters, below) |
| `fn f(...) requires [...]` | `pub fn f(...)` |
| `type R<A> {..}`, `enum E<A> {..}`, `fn f<A>(..)`, a role `fn serve<A>(..)` | `pub struct R<A: Clone + Send + Sync + 'static>` and so on: every type parameter carries these bounds (11.1) |
| `Store<T>` | `ignite_io::Store<T>` |
| `fn(p: A) -> R requires [p.*]` | `std::sync::Arc<dyn Fn(A) -> R + Send + Sync>` |
| `protocol P { fn serve ... fn connect ... }` | `pub struct P;` and `impl P { pub fn serve ... pub fn connect ... }` |
| `P.serve(a, b)` | `P::serve(a, b)` |
| `import std.a.b.{X, Y}` in a program | `use ignite_std::a::b::{X, Y};` |
| `import std.a.b.{X, Y}` in a std module | `use crate::a::b::{X, Y};` |
| `import std.net...`, `import std.io...` | nothing |
| `let x = e` / `var x = e` | `let x = e;` / `let mut x = e;` |
| `"text"` | `String::from("text")` |
| `[a, b]` | `vec![a, b]` |
| `R { a = e }` | `R { a: e }` |
| `Unit` (value) | `()` |
| `E.V`, `V`, `E.V(x)` | `E::V`, `E::V(x)` |
| `Some(x)`, `None`, `Ok(x)`, `Err(e)` | same |
| `Data(b)`, `Eof` | `ignite_io::Input::Data(b)`, `ignite_io::Input::Eof` |
| `x.f` | `x.f` |
| `e?` | `(e)?` |
| `await e` | `(e)` |
| `spawn f(a, b)` | `{ let __spawn_0 = a; let __spawn_1 = b; ignite_io::spawn(move || { if let Err(error) = f(__spawn_0, __spawn_1) { ignite_io::log(&format!("task ended with error: {}", error)); } }); }` |
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

The `spawn` arguments are evaluated (and cloned, ownership rule) before the task starts, so
the spawning function keeps its own copies.

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
on a place `l` passes the element borrowed from the list, `ignite_io::checked::at(&l, i,
<site>)`, unless another argument of the call changes `l`. Such a parameter of type `Int`,
`Byte`, `Bool`, `Unit` or a fn type (an `Arc`, copied by a reference count) is copied once on entry: `let mut p = p.clone();`. An argument to a
consumed parameter or to a role parameter is a value: a place that moves is emitted bare,
any other place with `.clone()`, so `s = step(s, i)` passes `s` without a copy and `let t =
step(s, 1); t.count + s.count` passes `s.clone()`. In the body a reference parameter is
dereferenced as an operand of an operator, a condition or an index (`(*p)`), and copied
where it is the result of a block, arm or branch. A fn of the module or an imported one used
as a value is a closure in an `Arc` cast to the fn value type (13.3); it takes its arguments
by value and passes each as its parameter is declared:
`(std::sync::Arc::new(|__a0: i64| f(&__a0)) as std::sync::Arc<dyn Fn(i64) -> i64 + Send +
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
`__m` to the reference. Every other `match` copies its scrutinee.

**Builtin methods** are lowered by name; the receiver type is not known, so each name has one
meaning. The generator emits exactly the Rust on the right.

| Ignite | Rust | note |
|---|---|---|
| `t.len()` | `(t.len() as i64)` | Text, Bytes, List |
| `t.is_empty()` | `t.is_empty()` | |
| `t.trim()` | `t.trim().to_string()` | |
| `t.starts_with(p)` | `t.starts_with(&p)` | |
| `t.strip_prefix(p)` | `t.strip_prefix(&p).map(\|s\| s.to_string())` | |
| `t.concat(u)` | `format!("{}{}", t, u)` | |
| `t.to_bytes()` | `t.clone().into_bytes()`; `t.into_bytes()` when `t` is a fresh text nothing else owns (a literal, a `+` chain or interpolation, `to_text` of Int, Byte or Bool, `concat`, `trim`) | |
| `b.to_text()` | `String::from_utf8_lossy(&b).into_owned()` | |
| `b.slice(i, j).to_text()` | `String::from_utf8_lossy(ignite_io::checked::slice_ref(&b, i, j, <site>)).into_owned()`: the slice is borrowed, not copied, with the failure of `checked::slice` | Bytes |
| `b.at(i)` | `b[(i) as usize]`, then `.clone()` when the element is not `Int`, `Byte` or `Bool` (also `l[i]`) | Bytes, List |
| `l.map(f)` | `l.iter().cloned().map(f).collect::<Vec<_>>()` | List |
| `b.to_int()` | `(b as i64)` | Byte |
| `i.to_byte()` | `u8::try_from(i).ok()` | Int |
| `Int.parse(t)` | `ignite_io::parse_int(&t)`: decimal digits with an optional leading `-`, `None` for a leading `+` and for anything `str::parse::<i64>` rejects; the LLVM runtime's `int_parse` calls the same function | static |
| `Int.parse(b.to_text())`, `Int.parse(b.slice(i, j).to_text())` | `ignite_io::parse_int(&b)`, `ignite_io::parse_int(ignite_io::checked::slice_ref(&b, i, j, <site>))`: no `Text` is built; bytes that are not UTF-8 give `None`, as their lossy text does. The LLVM target passes `b` (or the slice) to `int_parse` without `text_from_bytes_lossy` | static |
| `s.next_id()`, `s.get(id)`, `s.put(id, v)`, `s.remove(id)`, `s.values()`, `s.update(id, f)`, `s.upsert(id, f)` | the same `ignite_io::Store` method | Store |
| `s.set_idle_timeout(ms)` | `s.set_idle_timeout(ms)` | TcpStream |
| `b.slice(i, j)` | `b[(i) as usize..(j) as usize].to_vec()` | |
| `b.push(x)` | `b.push(x)` | Bytes, List |
| `b.append(c)` | `b.extend_from_slice(&c)`, `c` borrowed; `&c.clone()` when `c` is rooted at the variable of `b` | Bytes, List |
| `b.clear()` | `b.clear()` | |
| `l.join(sep)` | `l.join(&sep)` | List<Text> |
| `x.copy()` | `x.clone()` | |
| `Bytes.empty()` | `Vec::<u8>::new()` | static |
| `Bytes.of(list)` | `(list).iter().map(\|x\| *x as u8).collect::<Vec<u8>>()` | static |
| `Bytes.of([])`, `b""` | `Vec::<u8>::new()` | static; an empty list has no element type to infer |
| `List.empty()` | `Vec::new()` | static |
| `Int.to_text(i)` | `i.to_string()` | static |
| `log(t)` | `ignite_io::log(&t)` | free function, writes a line to stderr |

`rest_after_word` and `first_word` are not builtins: `std.console` defines both in Ignite, with
the same whitespace rule (ASCII space, tab, CR, LF, FF, VT).

| Ignite | Rust |
|---|---|
| `const N: T = e` | `#[allow(non_snake_case)] pub fn N() -> T { e }` |
| `N` (a constant, own or imported) in any expression | `N()` |
| `IoError.new(m)` | `ignite_io::IoError::new(m)` |
| `Enum.Variant(p)` pattern | `Enum::Variant(p)` |
| `x.to_text()` on `Int`, `Byte`, `Bool` | `x.to_string()` |
| `t.to_text()` on `Text` | `t.clone()` |

**Guarded arms.** Rust cannot match a `String` against a text pattern or a `Vec` against a
list pattern, and a constant is not a Rust pattern. An arm whose pattern contains a text,
list or constant pattern anywhere becomes `__m if <test> => { let a = (<access>).clone(); ...
body }`: `<test>` evaluates the whole pattern against `&__m` (`*s == N()` for a constant,
`*s == "x"` for text, `s.len() == 2 && ...` for a list, a `match` with `_ => false` for a
variant payload), and each binding is read out of `&__m` by its path (`&s.0`, `&s[1]`,
`match s { E::V(_, __x) => __x, _ => unreachable!() }`) and cloned. Every other arm is a
Rust pattern as written.

The generator reads types (`generate_typed`, `generate_std_typed`) for UFCS, fn-field calls,
`to_text`, `+` on `Text`, constant patterns and variants resolved by expected type; without
types it falls back to the name-based rules above.

Any other method name is passed through unchanged (`x.m(a)` → `x.m(a)`), so substrate
methods need no table entry. Substrate methods are written in Ignite with the same names as in
Rust: `bind`, `accept`, `connect`, `read`, `write_all`, `close`, `local_port`. Substrate
statics: `TcpListener.bind`, `TcpStream.connect`, `Stdin.read`, `Stdout.write_all` lower to
`ignite_io::<Owner>::<name>(args)`.

`Process.exit(c)` lowers to `ignite_io::exit((c) as i32)`, which returns `!`, so it has type
Never in Rust as in Ignite and needs no `?`. `e?` where `e` has type Never lowers to `e`
alone (without types: where `e` is a `Process.exit` call).

A bare variant whose enum the module neither declares nor imports (resolved by expected type,
section 4) lowers to the enum's path in `ignite-std`: `ignite_std::console::Action::Close` in a
program, `crate::console::Action::Close` inside `ignite-std`, in expressions and patterns.

### 6.1 System lowering

A `system` produces one executable per executable component (section 14.5): a component
whose kind `K` is a record type with `fn K.main(self: K) -> Result<Unit, IoError>`. The
compiler-known kinds `Service` and `Client` of slices 1 to 9 are retired in slice 10: their
accept loop, connect and exit codes are Ignite in `std.protocol.telnet.TelnetServer`,
`std.protocol.telnet.TelnetClient` and `std.protocol.http.HttpServer` (section 14.6), and
the backends know no component kind by name.

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
use terminal::*;
use ignite_std::protocol::telnet::TelnetServer;      // the program's imports are repeated here
fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let ingress = ignite_io::TcpListener::bind(String::from("127.0.0.1"), port.unwrap_or(2323))?;
        ignite_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        let console = CommandConsole { prompt: String::from("terminal> "), commands: ConsoleCommands { respond: ... }, .. };
        let server = TelnetServer { listener: ingress, application: console.clone(), executable: String::from("terminald"), .. };
        TelnetServer__main(&server)
    }))
}

// src/bin/tn.rs
fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let ingress_endpoint = Endpoint { address: String::from("127.0.0.1"), port: port.unwrap_or(2323) };
        let client = TelnetClient { target: ingress_endpoint.clone(), executable: String::from("tn") };
        TelnetClient__main(&client)
    }))
}
```

In order: `ignite_io::run`; every `Store` resource the component (or an embedded component)
uses, created first (`let s: ignite_io::Store<T> = ignite_io::Store::new();`); every
`TcpListener` resource it uses as a listener, bound and logged as `listening on
<address>:<port>` (the first one takes the `--port` override; `--port 0` binds a free port
and the line reports it); the `Endpoint` value of every `TcpListener` resource it names
where an `Endpoint` is expected (section 14.5; `--port` applies to it too); the embedded
components' values in dependency order; the component's own value with the kind's defaults
for omitted fields, `executable` defaulting to the component name and a `Signal` output
set to its reference text; then `K.main(value)`. A resource whose name is one of `main`'s
own (`application`, `port`, `stream`, `error`) gets a trailing `_`.

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

For `ignite build terminal.ignite --out DIR`:

```
DIR/Cargo.toml          [package] name = <package last segment>, edition 2024, [workspace] empty,
                        [dependencies] ignite-io = { path = <substrate> }, ignite-std = { path = "std" } when the program imports a std module
                        one [[bin]] per component: name = executable, path = src/bin/<executable>.rs
DIR/src/lib.rs          the program's imports as `use`, every type, enum, protocol and fn
DIR/src/bin/<exe>.rs    `use <package>::*;`, the program's imports as `use`, plus the main above
DIR/std/Cargo.toml      [package] name = "ignite-std", version "0.0.0-<content hash>" so programs sharing a target dir do not share a stale artifact, edition 2024, [dependencies] ignite-io
DIR/std/src/lib.rs      one nested `pub mod` per std module (`pub mod protocol { pub mod telnet { ... } }`)
```

Every `.ignite` file under the standard-library root is part of `ignite-std`, checked in
import order. `ignite build` then runs `cargo build --manifest-path DIR/Cargo.toml
--target-dir TARGET` with `TARGET` from `--target-dir` (default `DIR/target`) and prints one
line per built executable: `built TARGET/debug/<executable>`. Exit status: 0 built, 1
diagnostics (in any file, rendered with that file's path), 2 cargo failed (its output passed
through), 3 a file could not be read or written.

## 7 Substrate `ignite-io`

The only hand-written Rust a generated program runs, and the one implementation of IO for
every target: the Rust target's generated code calls it directly, the LLVM target through the
C ABI adapter `ignite-rt` (`docs/llvm.md` section 5).

Slice 14 adds `Pipe`, `PipeStream`, `ByteStream` and the `ignite_rt_pipe_*` entries: 18.8 and
18.9 supersede this section where they differ.

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

**Tasks.** A task runs on one OS thread for as long as it runs. A thread that finishes a
task waits up to `LINGER` for the next one; `spawn` hands a task to the thread that went
idle most recently and starts a new thread only when none is idle. A task that panics
ends its thread. Thread-local state survives from one task to the next on the same thread.
The substrate's pending writes are always empty between tasks, because each task ends
with a flush. When the operating system cannot start a thread, the task is dropped without
running (a task that owns a stream closes it), the first such failure after a successful
start is logged as `cannot start a task: <reason>`, and the process continues.

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
| E006 | assignment to an immutable binding or a constant; since slice 9 also `push`, `append` or `clear` through a `let`, a `for` element or a pattern binding, on `x.to_bytes()`, or on a binding a closure captured (section 13.11) |
| E010 | effectful call or `spawn` in a pure context: a constant's value or a field default (section 5) |
| E011 | retired in slice 6 (was: effectful call not awaited) |
| E012 | retired in slice 6 (was: `await` on a pure call) |
| E013 | `spawn` of something that is not a call of an effectful user function or role, including a substrate operation |
| E014 | retired in slice 6 (was: constant used as a pattern; constant patterns are allowed, section 4.3) |
| E015 | constant cycle |
| E016 | effect missing from `requires` (section 5.1); a closure's effects not on its own parameters are charged where it is created (section 13.3) |
| E017 | effectful function used as a value where no effectful fn type covering its effects is expected (section 4.5); reported by the type checker since slice 7 |
| E020 | unsupported or malformed resource, component, contract, policy or assembly (also: a kind that is no record type, two components with one executable name, a `system` in a std module); reported by the type checker since slice 10. The Rust backend keeps E020 for a malformed resource and, when it runs without the typed program, for every component (a component is executable only through the type checker) |
| E021 | reserved (unused since slice 1) |
| E030 | protocol role missing or malformed, a role without `requires`, a call to a role the protocol lacks (the component rules of slices 1 to 9, `Client` without `connect` and `Service` without `serve`, retired with those kinds in slice 10: a kind's `main` calls the role, and a missing role is this E030 at the call); since slice 14 a `serve` role takes 2 parameters `(stream, application)` or 3 `(stream, limits, application)` (section 18.6) |
| E031 | unknown module or imported name (including a name imported from `std.net` or `std.io` that is neither a builtin nor declared in that module's file), or an import from a module that is neither `std.*` nor a sibling of the program's package (section 14.4) |
| E032 | import cycle |
| E049 | a binding inside an or-pattern |
| E050 | two tests of one module with the same name (section 12.2) |
| E051 | a test-only item used outside tests and test-only items, or imported (section 12.2), or `Store.new()` there (section 12.3), or `Pipe.pair()` there (section 18.4); a UFCS use is reported by the type checker |
| E053 | a test name that is empty or interpolated; reported by the parser (section 12.1) |
| E041 | (slice 9) a name two glob imports provide, used without an explicit import (section 13.1 row 6) |
| E054 | nesting deeper than 256 levels of brackets, type arguments, prefix operators or `return` (section 3); string literals nested deeper than 32 interpolations (section 13.1 row 1) |
| E055 | a record that contains itself through fields and tuples alone, so a value would be infinite; recursion through an enum payload, `Option`, `Result` or `List` is allowed and boxed (section 13.4) |
| E056 | a parameter of a fn, role, fn type or closure named like an ambient capability: `stdin`, `stdout`, `process`, `net` (section 13.11) |
| E057 | a type parameter of a generic record, enum or alias that no field, payload or aliased type uses (section 13.11) |
| E099 | (slice 14) a bound naming an interface other than `ByteStream` or more than one interface, or `ByteStream` in a type position (section 18.7) |
| E091 | (slice 15) a catalogue constant, of type `std.rfc.Requirement` or `std.rfc.Spec`, whose value is not literal data (section 19.4); the consistency trigger is the evaluator's (below) |
| E076 | (slice 16) a second `..` in a list pattern, or a `..name` that is not the last element (section 20.5); reported by the parser |

Keywords are accepted as names in package and import paths, property names and
record-literal field names.

Codes of the type stage (`docs/llvm.md` section 7), reported by `ignite check` and both build
targets: E003 bare variant of an enum out of scope that the expected type does not decide,
or a payload variant used bare (section 4), E005 ambiguous variant, E040 type mismatch,
E041 unknown method or field for the receiver type (also an ambiguous or inapplicable method call, section 4.8; a tuple index past the tuple; a fn named like a builtin method of its first parameter's type, section 13.1 row 5), E042 type cannot
be inferred (also a type parameter neither the expected type nor later uses in the body decide, a closure parameter without type, and an effectful operation
whose effects `ignite-check` could not check, section 11.7), E043 wrong number of arguments (also a range pattern whose start is above its end), E044 integer literal out of range for `Byte`,
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
gives no type (a call's result); E046 splits a list
column by length and names the missing lengths as list patterns. Fixtures: `e076_rest` and
`e001_rest_outside_list` under `crates/ignite-syntax/tests/fixtures`; `e040_not_callable`,
`e043_expression_call_arity`, `e042_effectful_expression_callee`, `e040_rest_binding` and
`e046_rest` under `crates/ignite-types/tests/fixtures`.

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
- One implementation in `ignite-io`: `Store<T: Clone + Send>` over
  `Arc<Mutex<BTreeMap<i64, T>>>` plus an `AtomicI64` counter. The Rust target uses it
  directly. `ignite-rt` exposes `ignite_rt_store_*` over a `Store<HeapValue>`, where
  `HeapValue` is a pointer to a malloc'd deep copy made and freed by generator glue
  (`copy_to_heap.<T>`, `copy_from_heap.<T>`, `free_heap.<T>`), passed to `store_new`.
  `get`, `values` and `update`'s result use the store's borrowing reads (`read`,
  `read_values`, `replace_with`): `copy_from_heap` copies the stored value straight into the
  calling task's arena under the lock, so a read makes one copy and frees nothing. `update`'s
  thunk `store_update.<T>` runs under the lock behind a callback guard: a failure inside `f`
  (14.8) leaves the old value, the lock is released, then the task ends with the failure's
  report, as on the Rust target, where the failure unwinds through `ignite_io::Store`.
- `upsert` is the atomic insert-or-update: two tasks that upsert one absent id lose neither
  write, where `update` (which answers `None` for it) followed by `put` can. `ignite_io`'s
  `upsert` is `upsert_with` over a clone of the stored value; `ignite_rt_store_upsert` calls
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
  aliases expanded), `x.f`, `x.copy()`, `e?`, `l[i]`, Store results, `for` over a list. Where
  it cannot, the type checker reports E042 at the call (``cannot check the effects of `get`:
  the type of its receiver is not declared; annotate the binding``).

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
  `Text`, `Unit`, `Input`, and `List`, `Option`, `Result`, tuples, records and enums (generic
  ones too) built only of such types. A `T` holding a fn, `TcpStream`, `TcpListener`,
  `IoError`, `Store` or a type parameter is E045. Outside an assert `==` is unchanged (E045 on
  a record). `assert (a == b) && c` is an ordinary `Bool` condition.
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
| 5 | none | `x.f(..)` is the builtin `f` when it applies to `x`'s type, else a UFCS call of the user fn `f` (`Callee::Fn`). A user fn whose first parameter type has a builtin method of the fn's name is E041 at the declaration | none: `callee()` says which |
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
LLVM: a mangled symbol); a free fn `name` of the same module is a different fn.

### 13.9 Diagnostics changed in slice 9

| code | change |
|---|---|
| E006 | also: assignment to, or `push`/`append`/`clear` on, a captured binding inside a closure; `push`/`append`/`clear` through a `let`, a `for` element or a pattern binding, or on `x.to_bytes()` (13.11) |
| E016 | also: a closure's effects on captures and ambient effects, charged where it is created; an effect reached through `map`, `push` or `append` on a field or index, or through a call of a local fn value (13.11) |
| E046 | `Int` and `Byte` columns with a range pattern split into intervals (13.7) |
| E056 | new: a parameter named like an ambient capability (13.11) |
| E057 | new: an unused type parameter of a generic record, enum or alias (13.11) |
| E017 | also: an effectful closure where a pure fn type, or an effectful fn type not covering its effects, is expected |
| E041 | also: a name two glob imports provide; a fn whose first parameter type has a builtin method of the fn's name; a tuple index past the tuple |
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
    arguments stay the parent's (`docs/llvm.md` section 4, `docs/friction.md` row 30).
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
| every resource and component of the system is a `bindings` key or in exactly one binding's `includes` | E110 when neither |
| no member is both a key and included, or included by two bindings | E111 |
| `admits(binding, member)` returns `Some(reason)` for a key | E112 with that reason |
| a binding's `includes` equals the components its member embeds (`ExecutableInfo.embedded`, 14.14 item 12) | E113, naming the missing or extra member |
| contracts and policies are never keys | E112 (`admits` is not consulted) |

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
- The old process's exit during `Draining` is not a failure. A failure of the new process
  follows 16.7.
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
- After `K.main` returns, `run` waits for the live tasks to end, then exits 0.
- A second SIGTERM exits at once.
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
| `edges.provisioning` | resource → the component that binds it (`Terminal.ingress` → `Terminal.server`) |
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
| E120 | a declaration its kind's `validate` refuses, with the reason text from std: an unknown `good` outcome, `missing_data`, `on_deadline` or time zone; `maximum_attempts` < 1; `target` not in (0 %, 100 %]; non-empty `exclusions`; a backoff initial greater than its maximum | check | `e120_validate_refused` |
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
- E121: a property whose value is a signal ref `S.p.f`: `f` must be an unset output of `S.p`
  (else "is no output"), and `S.p` must run.
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
Target defaults to Rust; profile defaults to dev. LLVM runtime selection follows `build`.
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
  the committed revision. Keeping the first reason never weakens what the later one blocks. While
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
| E096 | A use instantiates a `ByteStream`-bounded type parameter with a type that does not implement `ByteStream` (18.3). The message is "`T` does not implement `ByteStream`, which `S` of `f` requires; pass a `TcpStream`, a `PipeStream` or a type parameter bounded by `ByteStream`" | types (`ignite-types`) | `crates/ignite-types/tests/fixtures/e096_bound_not_met`, with one case each: `Int`, a record, an unbounded type parameter passed on, a fn value of the generic fn, and a `spawn` |
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
| 2 | P2 test 2: the HTTP matrix lists the §7.1 and §3.2 gaps and claims nothing | `cargo test -p ignite --test main conformance::http_matrix_lists_the_chunked_and_host_gaps` | The test runs `ignite conformance std/protocol/http.ignite --target rust`, then the same with `--target llvm`. The `rfc9112` block holds the row of `R7_1_2` (§7.1, "A recipient MUST be able to parse and decode the chunked transfer coding.") and the row of `R3_2_6` (§3.2, "A server MUST respond with a 400 (Bad Request) status code to any HTTP/1.1 request message that lacks a Host header field and to any request message that contains more than one Host header field line or a Host header field with an invalid field value."). Each row has level `MUST`, status `unsupported` and verdict `gap`. The rows of `R2_2_1` (§2.2, `MUST`), `R6_1_2` (§6.1, `MUST NOT`) and `R6_1_4` (§6.1, `MUST`) are `unsupported` and `gap` too (19.14 item 17). The block ends with `claim conformance of Http to rfc9112: no (15 gaps, 0 failures, 0 inconclusive)`, and stdout holds `claim conformance of Http to rfc9110: no (9 gaps, 0 failures, 0 inconclusive)` (19.10). Exit 1. Both targets print byte-identical stdout | U6 |
| 3 | P2 test 3: the Telnet matrix claims RFC 854 for an NVT that refuses every option | `cargo test -p ignite --test main conformance::telnet_matrix_claims_rfc854_for_an_nvt_that_refuses_every_option` | P2 test 3 is not met in this slice (19.14 item 16); the test keeps its name. `ignite conformance std/protocol/telnet.ignite`, once per target. Stdout holds `claim conformance of Telnet to rfc854: no (4 gaps, 0 failures, 0 inconclusive)` and `claim conformance of Telnet to rfc855: yes (0 gaps, 0 failures, 0 inconclusive)`. In the `rfc854` block the rows `RP5_3`, `RP5_4`, `RP10_2` and `RP12_4` each print status `unsupported`, their reason from `telnet.ignite` in double quotes as evidence, and verdict `gap`; they are the only rows whose verdict is `gap`. The RFC 855 option-negotiation rows `RFC855.RP2_3` (a receiver may find the end of a subnegotiation by IAC SE) and `RFC855.RP2_4` (either party may refuse by sending WON'T or DON'T) each print verdict `pass`. The evidence of each of the two rows names `decode_within`, the fn that answers `DO` with `WONT` and `WILL` with `DONT`, and names both tests, "a subnegotiation is skipped up to IAC SE" and "DO is answered WONT and WILL is answered DONT", as passed. Exit 1, since the RFC 854 claim does not hold. Both targets print byte-identical stdout | U6 |
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
3. If any row names a test, build FILE's test executable on the target (default `rust`)
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
| 16 | `protocols.md` §7 row P2, test 3; 19.0 row 3 | "The Telnet matrix claims RFC 854 for an NVT that refuses every option" | not met in this slice (steering, 2026-11-03): the RFC 854 claim prints `no (4 gaps, …)`, the RFC 855 claim holds. The four MUST-class rows are `unsupported` in `telnet.ignite`, checked against the plain-text RFC 854: `RP5_4` (p5, "the process must transmit the TELNET Go Ahead (GA) command") needs IAC GA after the prompt, or SUPPRESS-GO-AHEAD, which an NVT refusing every option cannot negotiate; `RP10_2` (p10, "must transmit the TELNET SYNCH sequence") and `RP12_4` (p12, "The pair DM-Urgent is to have required meaning") need the Synch, a DM with TCP urgent send and receive, in `ignite-io` and `ignite-rt` on both targets (`tn` sends IAC IP on Ctrl+C without it, and IAC DM is read as a no-op); `RP5_3` (p5, "implementers are cautioned to provide methods of locally signaling that all buffered data should be transmitted immediately") needs a key in `tn`'s line editor that sends a partial line, and its `Must` comes from the word-table limit of item 15. Cost, measured on the U6 tree: GA changes every server transcript (about 20 inline assertions in `telnet.ignite`, the transcript test in `examples/terminal/commands.ignite`, the raw reads in `crates/ignite/tests/terminal.rs`); the Synch adds a substrate operation on both targets, a pipe counterpart and a `CODEGEN_REVISION` step, and changes `tn`'s Ctrl+C transcript in `terminal.rs`; the partial-line key changes the `apply_key` test. A later slice |
| 17 | U6's claims of `http.ignite` and `telnet.ignite`; 19.0 rows 2 and 4 | statuses the U6 adversary found wrong (`crates/ignite/tests/conformance.rs`, `adversary_*`) | resolved (steering, 2026-11-03). Fixed in code: `RFC9112.R6_3_4` and `R6_3_5` (a field value is trimmed of SP and HTAB only, RFC 9110 OWS, so `Content-Length: 3` beside U+00A0, U+0085 or U+3000 is 400); `R9_6_6` and `R9_6_7` (a response whose application headers list the `close` option ends the connection); the client role prints CR NUL as a carriage return alone, CR LF and a lone CR as LF (`RFC854.RP11_3`, `RP13_3`; no transcript changed, and of the goldens only `terminal.model.json` and `todo.model.json` move, by the std source positions the claims of `1bc293f` shifted). Restated as `unsupported`, so the `rfc9112` claim counts 15 gaps instead of 12: `R6_1_2` and `R6_1_4` (were `not_applicable`: the application's `Transfer-Encoding` is sent as given on a kept connection), `R2_2_1` (was satisfied by `parse_head` and `read_head`: the head is decoded as lossy UTF-8 before it is parsed). Their adversary tests stay ignored with the gap id; story `http-rfc9112-9110-gaps` |

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
| D8 | `ignite conformance FILE [--target rust\|llvm]`, default `rust`. It builds FILE's test executable on that target through `ignite::build_tests`, runs it once, and reads the result lines between `running N tests` and the first blank line. Output and verdicts as 19.10, with the claim line `claim conformance of <P> to rfc<n>: yes\|no (<g> gaps, <f> failures, <i> inconclusive)`. Exit 0 when every claim holds, 1 when one does not, 2 on a diagnostic, usage error or no conforming protocol, 3 on a test build or run failure or IO | run on all targets at once; JSON output | one target per run matches `ignite test --target`; rows 2, 3 and 36 run both and compare. JSON can be added later without changing the text |
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
| 3 | positions and places | `moves::every_move_position_moves` and `moves::places_move_through_fields_only` | each move position of 20.3.1 with a dead place is marked; a receiver of a builtin method, a `match` scrutinee, a `for` iterable, an operator operand, a `spawn` argument, a closure capture and the initializer of a pattern `let` are not. `b.f` with `b.f` dead after is marked; `xs[0]`, a boxed field (13.4) and a place rooted at a pattern binding are not. Field paths are disjoint: in `a.state = a.handle(a.state, i)` the argument `a.state` is marked; in `Step { state = st.state, out = publish(st.out) }` with `st` dead after, `st.state` is marked; in `answer(state, reply, state.table.values())` `state` is not. No expression whose type is `Int`, `Byte`, `Bool`, `Unit`, a quantity, a handle type or a fn type is marked | U1 |
| 4 | consumed parameters are a fixpoint | `moves::consumed_is_a_fixpoint` | in a module with `a(x)` calling `b(x)` at its last use, `b(y)` changing `y` in place and calling `a`, and `c(z)` only reading `z`: `a` and `b` consume their parameter, `c` does not. `d(m)` whose only change is `m.put(k, v)` consumes `m`; `e(p)` that passes `p.items` to a consumed parameter on every path consumes `p`. `pick(a, b, c) = if c { a } else { b }` consumes neither `a` nor `b` (each is moved on one path only, 20.3.3), and neither does `unwrap_or` in `std/option.ignite` consume `fallback`. A program fn whose last use of a local passes it to a std fn that consumes its parameter moves it; `ModuleTypes::consumed` of the std module lists that fn | U1 |
| 5 | closure bodies are not entered | `moves::closure_bodies_never_move` | no expression inside a closure body is marked, and creating a closure that captures `s` counts as a read of `s` | U1 |
| 6 | the checker accepts calls of fn-typed expressions | `cargo test -p ignite-check --test main fixtures::` | fixture `crates/ignite-check/tests/fixtures/valid_expression_calls` (`adder(1)(2)`, `fs[1](10)`, `(|x: Int| x * 3)(2)`, `(h.run)(2)`, `pick(true)(3)`): exit silent. `nope(1)(2)` in fixture `e003_unknown_variable` gives one E003 `unknown name \`nope\`` and no `not callable` | U2 |
| 7 | the typer types them | `cargo test -p ignite-types --test main fixtures::` | fixture `valid_expression_calls` (the same source with a checked use of each result) is silent. `e040_not_callable`: `(1)(2)`, `xs[0](1)` with `xs: List<Int>` and `"a"(1)` give one E040 each, `type mismatch: this expression has type \`<T>\` and cannot be called`. `e043_expression_call_arity`: `adder(1)(2, 3)` gives one E043. `valid_expression_calls` also holds `hs[0](conn)` with `hs: List<fn(c: PipeStream) -> Int requires [c.write]>` and `(h.send)(conn)` with an effectful fn field, inside a fn that `requires [conn.write]`: silent, as `h.send(conn)` is today. `e042_effectful_expression_callee`: `pick_sender(true)(conn)`, with `pick_sender` returning that effectful fn type, gives one E042, the message of 20.7; the same fn without `conn.write` in `requires` gives the E042 and no E016. `e042_closure.expected` and `e017_closure.expected` are unchanged | U2 |
| 8 | closures already hold, both targets | `cargo test -p ignite --test main slice16::closure_shapes_pass_on_both_targets` | `ignite test slice16/closures.ignite --target all` exits 0, every test passed on both targets, 0 failed. The fixture holds the closure tests of 20.1's probes p02, p08 and p04b: captures by copy, a var captured and then changed (the closure keeps the old value), a generic fn returning a closure that captures a `T`, a closure returning a closure, a closure in a record kept in a `Store`, a closure passed to a spawned task, a closure that writes through a captured `PipeStream` called through its `let` binding, and a record holding `handle: fn(S, M) -> S` stepped in a loop (the actors §5.1 shape) | U2 |
| 9 | Rust: R1 is linear | `cargo test -p ignite-rustgen --test main slice16::state_threading_is_linear` | the test builds `slice16/moves_scale.ignite` once on the Rust target (dev profile) with the helpers `crates/ignite-rustgen/tests/slice9.rs` uses (`common::lower_tests` with the repository's `std/` and cargo), then runs the test executable twice, with the exact filters "50000 steps through a call" and "200000 steps through a call". Both pass. The time of each is the `finished in` figure of the runner's `test result:` line; the 200,000-step time is under 8 times the 50,000-step time (linear is about 4; p07's quadratic growth is about 20 per factor 4) | U3 |
| 10 | LLVM: R1 is linear | `cargo test -p ignite --test main slice16::llvm_state_threading_is_linear` | the same as row 9 on the LLVM target (dev, `-O0`), through `ignite test slice16/moves_scale.ignite --target llvm <filter>`, each filter run twice and the second run's `finished in` figure taken | U4 |
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
  `Some`, `Ok` or `Err` payload, a tuple element.

Not move positions: a receiver of a builtin method, a `match` scrutinee, a `for` iterable, an
operator operand (also `==` and `+` on `Text`), a `spawn` argument, an `assert` operand, a
closure capture.

A place in a move position moves when it is a tracked variable `v` or a field chain `v.f.g`
rooted at one, and that place is dead after the position (20.3.2). A chain through an index
(`v[i]`), through a boxed field or payload (section 13.4), or rooted at an untracked binding never
moves.

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
  `push`, `append`, `clear` (`List`, `Bytes`), `put` or `remove` (`Map`) through a place rooted
  at it. This set is the union of what `region::mutates` (LLVM) and `mutates_receiver` with
  `mutated_names` (Rust) count today, which the analysis replaces; `consumed` must be a superset
  of both, so that no parameter the backends own today becomes borrowed;
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
  scalars and fn types included (`x: &i64`, cloned on entry when Copy), and a role's parameters
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
  owned parameter; they need no change.
- `docs/llvm.md` §3's borrowed/owned sentence and §4's soundness sentence ("Callee arguments are
  fresh copies or the caller's places") get the consumed rule and the moved place.

#### 20.3.6 The evaluator

Unchanged. `ignite-eval` copies values by construction and reads neither `moves` nor
`consumed`.

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
| 1 | an effectful closure (one that uses a captured capability) passed as a fn value; E017 stays | no fn type can name an effect on a capture (20.4.3); wave C P1 to P3 pass no closures | actors P4, or the slice that adds effect polymorphism |
| 2 | inferring an unannotated closure parameter from the closure body (p05) | an annotation works (p05b); needs inference variables the typer does not have | actors P4 |
| 3 | capture by move (a closure taking a dead variable without a copy) | a capture copies once per closure creation, not per loop step; not on R1's path | later, with measurements |
| 4 | moves of pattern bindings, `for` elements, closure parameters, fn-typed variables, inside closure bodies, at `spawn` arguments and operator operands (`a + b` on `Text`), and beside another argument that only reads an overlapping place (`answer(state, reply, state.table.values())`; binding the argument first moves) | D2; each needs its own aliasing argument on LLVM | additive, when a measurement asks for one |
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
