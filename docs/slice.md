# Ignite slices 1 to 10: language subset, standard library, substrate and lowering

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
| `fn(A, B) -> R` | `fn(A, B) -> R` |
| `TcpListener`, `TcpStream`, `Stdin`, `Stdout`, `Input`, `IoError` | `ignite_io::` same name |
| record `type R { a: T }` | `#[derive(Clone, Debug)] pub struct R { pub a: T }` |
| `enum E { A, B(T) }` | `#[derive(Clone, Debug)] pub enum E { A, B(T) }` |
| `fn f(p: T) -> U` | `pub fn f(p: &T) -> U` (parameters, below) |
| `fn f(...) requires [...]` | `pub fn f(...)` |
| `type R<A> {..}`, `enum E<A> {..}`, `fn f<A>(..)`, a role `fn serve<A>(..)` | `pub struct R<A: Clone + Send + Sync + 'static>` and so on: every type parameter carries these bounds (11.1) |
| `Store<T>` | `ignite_io::Store<T>` |
| `fn(p: A) -> R requires [p.*]` | `fn(A) -> R` |
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
| `a + b + c` on `Text` | `format!("{}{}{}", a, b, c)` (one call per left-folded chain) |
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
followed by `.field`) used in a value position is emitted with `.clone()`. Value positions:
call and method arguments, `let`/`var` initializers, assignment right-hand sides, `match`
scrutinees, `return` operands, list elements, record field values, `for` iterables, and
operands of `await`, `?`, `!` and `-`. Non-value positions: the receiver of a method call or
field access, the left-hand side of an assignment, and operands of binary arithmetic,
comparison and logic operators. Every substrate handle implements `Clone` so this rule has
no exceptions.

**Parameters.** Every parameter of a fn (not of a protocol role, which a generated `main`
calls with owned values) is a reference `p: &T`, so a call copies nothing: an argument that
is a place is passed as `&place`, a parameter that is itself a reference as it is, any other
argument as `&(value)`. A parameter the body assigns or changes in place (`push`, `append`,
`clear` on it or a field of it), and one of a `Copy` type (`Int`, `Byte`, `Bool`, `Unit`, fn
types), is copied once on entry: `let mut p = p.clone();`. In the body a reference
parameter is dereferenced as an operand of an operator, a condition or an index (`(*p)`), and
copied where it is the result of a block, arm or branch. A fn of the module or an imported
one used as a value is a closure that borrows for it, cast to the fn pointer type:
`((|__a0: i64| f(&__a0)) as fn(i64) -> i64)`; `spawn f(a)` calls `f(&__spawn_0)`.

**Block values.** A block in *value position* emits its last statement, when it is an
expression, without a trailing `;` so it becomes the block's value. Value positions: a
`match` arm body, both branches of an `if` that is itself in value position, and the body of
a function whose declared return type is present and not `Unit`. Every other block (`while`,
`loop`, `for` bodies, an `if` without `else`, an `if` in statement position, a function body
with no return type or `Unit`) ends every statement with `;`. An `if` is in value position
when it is a `let`/`var` initializer, a `return` operand, an argument, an arm body, or the
last statement of a value-position block.

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
| `t.to_bytes()` | `t.clone().into_bytes()` | |
| `b.to_text()` | `String::from_utf8_lossy(&b).into_owned()` | |
| `b.at(i)` | `b[(i) as usize]`, then `.clone()` when the element is not `Int`, `Byte` or `Bool` (also `l[i]`) | Bytes, List |
| `l.map(f)` | `l.iter().cloned().map(f).collect::<Vec<_>>()` | List |
| `b.to_int()` | `(b as i64)` | Byte |
| `i.to_byte()` | `u8::try_from(i).ok()` | Int |
| `Int.parse(t)` | `Some(t).filter(\|t: &String\| !t.starts_with('+')).and_then(\|t\| t.parse::<i64>().ok())` | static |
| `s.next_id()`, `s.get(id)`, `s.put(id, v)`, `s.remove(id)`, `s.values()`, `s.update(id, f)` | the same `ignite_io::Store` method | Store |
| `s.set_idle_timeout(ms)` | `s.set_idle_timeout(ms)` | TcpStream |
| `b.slice(i, j)` | `b[(i) as usize..(j) as usize].to_vec()` | |
| `b.push(x)` | `b.push(x)` | Bytes, List |
| `b.append(c)` | `b.extend_from_slice(&c)` | Bytes |
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
| E030 | protocol role missing or malformed, a role without `requires`, a call to a role the protocol lacks (the component rules of slices 1 to 9, `Client` without `connect` and `Service` without `serve`, retired with those kinds in slice 10: a kind's `main` calls the role, and a missing role is this E030 at the call) |
| E031 | unknown module or imported name (including a name imported from `std.net` or `std.io` that is neither a builtin nor declared in that module's file), or an import from a module that is neither `std.*` nor a sibling of the program's package (section 14.4) |
| E032 | import cycle |
| E049 | a binding inside an or-pattern |
| E050 | two tests of one module with the same name (section 12.2) |
| E051 | a test-only item used outside tests and test-only items, or imported (section 12.2), or `Store.new()` there (section 12.3); a UFCS use is reported by the type checker |
| E053 | a test name that is empty or interpolated; reported by the parser (section 12.1) |
| E041 | (slice 9) a name two glob imports provide, used without an explicit import (section 13.1 row 6) |
| E054 | nesting deeper than 256 levels of brackets, type arguments, prefix operators or `return` (section 3); string literals nested deeper than 32 interpolations (section 13.1 row 1) |
| E055 | a record that contains itself through fields and tuples alone, so a value would be infinite; recursion through an enum payload, `Option`, `Result` or `List` is allowed and boxed (section 13.4) |
| E056 | a parameter of a fn, role, fn type or closure named like an ambient capability: `stdin`, `stdout`, `process`, `net` (section 13.11) |
| E057 | a type parameter of a generic record, enum or alias that no field, payload or aliased type uses (section 13.11) |

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
alias that refers to itself, E052 `assert` condition not `Bool` (section 12.3).
`ignite-check` reports E047 and E048 (an unknown record in a record literal is E048, not
E003); `ignite-types` reports them too for a module it is given unchecked, and alone reports
alias cycles.

Codes of slice 10 (section 14.11), all from the type checker: E100 brace literal without an
expected type, E101 quantity operands that do not fit an operator, E102 `module("p")` not
found or a field it cannot fill, E103 named argument misuse, E104 declaration property
(three texts, section 14.11), E105 component cycle, E106 `Map` key type, E107 quantity
literal out of range, E108 assembly binding key, E109 `K.main` signature. Inside a `system`
or `assembly` the type checker also reports E003 for an unknown name, E041 for `System.x`
when `S` has no member `x`, E047 for a repeated map key or a `TcpListener` property it does
not know, and E045 for `%` on a quantity.

Fixtures: E001 (including a keyword as a variable, fn and type name, beside
`valid_keyword_fields`), E002, E053 and E054 under `crates/ignite-syntax/tests/fixtures`;
E003, E004, E006, E010, E013, E015, E016, E030, E031, E041 (glob conflict), E047, E048, E049,
E050, E051, E055, E056 and E057 under `crates/ignite-check/tests/fixtures`; E003 (variant out
of scope, payload variant used bare), E005, E017, E040 to E046, E048 (alias cycle), E051
(UFCS), E052 and E100 to E109 (E104 three fixtures: `e104_required_field`,
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
  methods and `to_text` on a type parameter are E045/E041.
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

- Value semantics: `get`/`values` return copies; `put` stores a copy.
- One implementation in `ignite-io`: `Store<T: Clone + Send>` over
  `Arc<Mutex<BTreeMap<i64, T>>>` plus an `AtomicI64` counter. The Rust target uses it
  directly. `ignite-rt` exposes `ignite_rt_store_*` over a `Store<HeapValue>`, where
  `HeapValue` is a pointer to a malloc'd deep copy made and freed by generator glue
  (`copy_to_heap.<T>`, `copy_from_heap.<T>`, `free_heap.<T>`), passed to `store_new`.
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
| d | `push` and `append` on a field, index or tuple-field receiver extend the roots of the receiver's place root (`h.items.push(s)` roots `h` in `s`) | `effects/a07`, `b10` |
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
| `std/probe.ignite` | `TelnetPrompt { target: Ref<TelnetServer>, interval, timeout, expected_prompt, close_after_probe, attempts: Signal<ProbeAttempt> }`, `ProbeAttempt` |
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
| E109 | executable kind's `main` has the wrong signature |

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
