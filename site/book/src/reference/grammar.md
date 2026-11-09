# Grammar

This page gives the grammar of Ignite as the parser accepts it, then the rules
for line breaks and the operator precedence. Tokens are described in
[Lexical structure](lexical.md). What the declarations mean is in [Types](types.md),
[Effects](effects.md), [Modules and imports](modules.md) and
[Tests and assertions](tests.md). Statements and patterns are explained in
[Statements and expressions](statements.md), system declarations in
[System declarations](systems.md).

## Notation

- `'fn'` is a keyword or punctuation token, written as in the source.
- `ident` is an identifier. `name` is an identifier or a keyword. It appears in paths, field
  names and after `.`.
- `INT`, `DECIMAL`, `STRING`, `BYTE` and `BYTES` are literals. `STRING` covers plain,
  interpolated and raw strings. `BYTE` is `b'c'`, and `BYTES` is `b"..."` or `br"..."`.
- `NL` is a line break (one token for any run of them).
- `x?` is optional, `x*` repeats zero or more times, `x+` one or more times, `a | b` is a
  choice, `( )` groups.
- Two helper rules describe lists in braces and blocks:
  - `braced(x) := '{' NL* (x (sep+ x)* sep*)? '}'` with `sep := ',' | NL`. A trailing comma
    is allowed.
  - `block := '{' NL* (stmt (ssep+ stmt)* ssep*)? '}'` with `ssep := ';' | NL`.

## The grammar

```text
program      := NL* (decl (NL+ decl)*)? NL*

decl         := package | import | record | alias | enum | fn | const
              | protocol | system | assembly | test
              | 'test' (fn | record | enum | const)          test-only item

package      := 'package' path
import       := 'import' path ( '.' '{' ident (',' ident)* '}'
                              | '.' '*' )?
path         := name ('.' name)*

record       := 'type' ident tparams? satisfies? braced(field)
field        := name ':' type ('=' expr)? satisfies?          field with a default
alias        := 'type' ident tparams? '=' type
tparams      := '<' ident (',' ident)* '>'                    no bounds here
enum         := 'enum' ident tparams? braced(variant)
variant      := ident ('(' type (',' type)* ')')?

fn           := 'fn' (ident '.')? ident fn_tparams?           `fn Owner.name` is associated
                '(' (param (',' param)*)? ')'
                ('->' type)? requires? satisfies? (block | '=' expr)
fn_tparams   := '<' fn_tparam (',' fn_tparam)* '>'
fn_tparam    := ident (':' ident ('+' ident)*)?               bound: `S: ByteStream`
param        := ident ':' type
requires     := 'requires' '[' (effect (',' effect)*)? ']'
effect       := name ('.' name)* ('.' '*')?                   `stream.write`, `app.*`

const        := 'const' ident ':' type '=' expr
protocol     := 'protocol' ident conforms? '{' NL* ((fn | status) NL*)* '}'
                                                              its fns are the roles
conforms     := 'conforms' '[' ident (',' ident)* ','? ']' scope?
scope        := 'scope' braced(scope_item)
scope_item   := 'parties' '=' '[' (ident (',' ident)* ','?)? ']'
              | 'sections' '=' '[' (STRING (',' STRING)* ','?)? ']'
status       := ('unsupported' | 'not_applicable') reqref STRING
satisfies    := 'satisfies' '[' reqref (',' reqref)* ','? ']'
verifies     := 'verifies' '[' reqref (',' reqref)* ','? ']'
reqref       := ident '.' ident                               `Spec.Requirement`
test         := 'test' STRING verifies? block

type         := ident ('<' type (',' type)* '>')?
              | 'fn' '(' (fparam (',' fparam)*)? ')' ('->' type)? requires?
              | '(' ')'                                       Unit
              | '(' type ')'                                  grouping
              | '(' type ',' type (',' type)* ')'             tuple
fparam       := (ident ':')? type                             all named or none

system       := 'system' ident '{' NL* (member (NL+ member)*)? NL* '}'
member       := 'resource' ident ':' ident ('<' type (',' type)* '>')? braced(init)?
              | ('component' | 'contract' | 'policy') ident ':' ident braced(init)
assembly     := 'assembly' ident ':' ident braced(init)

stmt         := ('let' | 'var') ident (':' type)? '=' expr
              | ('let' | 'var') pattern '=' expr              `_` or a tuple of bindings
              | '_' '=' expr                                  discard
              | place '=' expr                                assignment
              | postfix '[' expr ']' '=' expr                 element: `l.set(i, v)`
              | 'return' expr?
              | 'while' expr block
              | 'loop' block
              | 'for' ident 'in' expr block
              | 'for' pattern 'in' expr block                 `for (k, v) in map`
              | 'break' | 'continue'
              | 'assert' expr (',' expr)?                     condition, message
              | expr
place        := ident ('.' name)*

expr         := or
or           := and ('||' and)*
and          := equality ('&&' equality)*
equality     := compare (('==' | '!=') compare)?
compare      := bitor (('<' | '>' | '<=' | '>=') bitor)?
bitor        := bitxor ('|' bitxor)*
bitxor       := bitand ('^' bitand)*
bitand       := shift ('&' shift)*
shift        := additive (('<<' | '>>') additive)*
additive     := multiplicative (('+' | '-') multiplicative)*
multiplicative := unary (('*' | '/' | '%') unary)*
unary        := ('!' | '-' | '~' | 'spawn') unary | postfix
postfix      := primary ( '(' args ')'                        call
                        | '.' name ('(' args ')')?            field or method call
                        | '.' INT                             tuple field
                        | '[' expr ']'                        index
                        | '?' )*                              propagate
args         := (arg (',' arg)*)?
arg          := (ident '=')? expr                             named argument

primary      := INT unit? | DECIMAL unit                      quantity literal
              | STRING | BYTE | BYTES | 'true' | 'false'
              | ident
              | ident record_body                             ident starts uppercase
              | braced(entry)                                 brace literal
              | '[' (expr (',' expr)*)? ']'                   list
              | 'match' expr braced(arm)
              | if
              | 'return' expr?
              | 'break' | 'continue'
              | closure
              | '(' ')'                                       the Unit value
              | '(' expr ')'
              | '(' expr ',' expr (',' expr)* ')'             tuple
unit         := ident                                         a unit name, or a `%` touching the number
record_body  := '{' NL* ((init | '..' expr) (sep+ (init | '..' expr))* sep*)? '}'
                                                              `..base` comes last
init         := name ('=' expr)?                              `x` alone means `x = x`
entry        := name ('.' name)* '=' expr
if           := 'if' expr block ('else' (if | block))?
closure      := ('|' (cparam (',' cparam)*)? '|' | '||') (block | expr)
cparam       := ident (':' type)?

arm          := pattern '=>' (block | expr)
pattern      := alternative ('|' alternative)*                or-pattern
alternative  := atom ('..=' atom)?                            range
atom         := '_'
              | '-'? INT | BYTE | STRING | 'true' | 'false'
              | '(' ')' | '(' pattern ')'
              | '(' pattern ',' pattern (',' pattern)* ')'    tuple
              | '[' (elem (',' elem)*)? ']'                   list pattern
elem         := pattern | '..' ident?                       `..` or `..rest`, at most one
              | ident ('.' ident)* ('(' pattern (',' pattern)* ')')?
```

## Rules the grammar does not show

Declarations:

- A file has at most one `package` line. Declarations, including `import`, may come in any
  order.
- Declarations are top level only. `package`, `import`, `type`, `enum`, `fn`, `const`,
  `protocol`, `system`, `assembly` or `test` inside a block is E001 ("declarations are
  only allowed at the top level").
- A bound (`<S: ByteStream>`) is allowed only on the type parameters of a fn, an associated
  fn or a protocol role. On a record, enum or alias it is E001. `ByteStream` is the only
  interface (E099 otherwise).
- `type N = T` is an alias. `type N { .. }` is a record. `test type N = T` is E001: an alias
  cannot be test-only.
- Type arguments are never written at a use: `swap(p)`, not `swap<Int, Text>(p)`.
- A fn whose return type is a fn type puts that type in parentheses before its own
  `requires`: `fn make() -> (fn(Int) -> Int) requires [..]`. A `requires` right after a fn
  type belongs to that fn type.
- The members of a `system` are separated by line breaks, not commas. A `resource` may leave
  out its braces (`resource todos: Store<Todo>`); the other members need them.

Expressions and statements:

- `Name { .. }` is a record literal only when `Name` starts with an uppercase letter.
- In the condition of `if` and `while`, the scrutinee of `match` and the iterable of `for`, a
  record literal is not parsed, so `if x == Point { x = 1 } { .. }` fails. Put the literal in
  parentheses.
- `match` and `if` are expressions. The last expression of a block is its value, and the
  value of a fn body.
- `if` used as a value needs an `else`.
- `fn f(..) -> T = e` is a fn whose body is the single expression `e`.
- The pattern of `let`, `var` and `for` must be irrefutable: a name, `_` or a tuple of those.
  Anything else is E001. Use `match` for it.
- A one-element tuple `(a,)` is E001, as a value, type or pattern. Write `(a)` to group.
- Comparisons do not chain: `a < b < c` is E001. Write `a < b && b < c`.
- `return` has no value when a line break, `;`, `,`, `}`, `)` or `]` follows it.
- In a type, a closing `>` may touch `=`: `x: Option<Int>= None` reads as `>` then `=`.
- Brackets, type arguments, prefix operators and `return` nest at most 256 levels (E054).
  Long chains of binary operators are not nesting and have no limit.
- A list pattern holds at most one `..`, and `..name` must be its last element (E076).
  `..` outside the brackets of a list pattern is E001.

## Protocol clauses

`conforms`, `scope`, `satisfies`, `verifies`, `unsupported`, `not_applicable`, `parties` and
`sections` are identifiers. They are clause words only in the positions the grammar shows.
What a claim means, and how `ignite conformance` judges it, is in
[`ignite conformance`](cli.md#ignite-conformance) and [Diagnostics](diagnostics.md#conformance-claims).

- `conforms [..]` lists `Spec` constants by name, local or imported. A qualified path is
  E001. A spec named twice is E004.
- A `reqref` is `Spec.Id`. The first part names a `Spec` constant in scope, the second a
  `Requirement` constant of that spec's module, which need not be imported. A bare id is not
  accepted.
- `scope { parties = [..], sections = [..] }` narrows the claim. A requirement is in scope
  when its party is listed and its section equals an entry or starts with an entry followed
  by `.`. A missing item, or a missing `scope`, admits everything. Each item comes at most
  once. `scope` without `conforms` is E001.
- `satisfies` goes on a fn or role after its `requires`, on a record after its type
  parameters, or on a field after its default. It counts for every protocol of the module
  that conforms to the requirement's spec.
- `verifies` goes between a test's name and its block. It names the requirements the test is
  evidence for.
- `unsupported REF "reason"` and `not_applicable REF "reason"` are members of a protocol
  body. The reason is a non-empty plain string literal.

```ignite
import std.rfc.{Level, Requirement, Spec}

const RFC7001: Spec = Spec {
  rfc = 7001
  title = "A line echo"
  url = "https://example.invalid/rfc7001"
  digest = "sha256:7001"
  requirements = [R2_1, R2_2]
}

const R2_1: Requirement = Requirement {
  rfc = 7001
  section = "2"
  ordinal = 1
  level = Must
  text = "A server MUST echo every line."
}

const R2_2: Requirement = Requirement {
  rfc = 7001
  section = "2"
  ordinal = 2
  level = Must
  text = "A server MUST close on QUIT."
}

fn echo(line: Text) -> Text satisfies [RFC7001.R2_1] = line

protocol Echo conforms [RFC7001] scope { sections = ["2"] } {
  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {
    return stream.write_all(b"ok")
  }

  unsupported RFC7001.R2_2 "the session never ends"
}

test "a line comes back" verifies [RFC7001.R2_1] {
  assert echo("hi") == "hi"
}
```

## Line breaks and separators

A line break ends a statement, a field, a variant, an arm, a system member or a declaration.
Within an expression, Ignite decides by position whether a line break is a separator or is
skipped.

Line breaks are **skipped**:

- inside `( )` and `[ ]`: argument lists, parameter lists, tuples, list literals, patterns,
  `requires [..]` lists, variant payloads
- after a binary operator, so a long expression breaks after `+`, `&&` and the like
- after `=`, `=>`, `->` and `,`
- after `{` and before `}`
- after the `,` between imported names in `import a.b.{X, Y}`.

Line breaks **separate**:

- inside `{ }` of a block: between statements, together with `;`
- inside `{ }` of a record, enum, record literal, brace literal or `match`: between items,
  together with `,`
- at the top level: between declarations.

Some tokens must stay on the line of what comes before them, even inside `( )` and `[ ]`:

- a postfix `(`, `[`, `.` or `?` must start on the same line as its operand. On a new line
  it starts something new: `[a` NL `(b)]` is E001, not a call of `a`. A method chain
  cannot continue on the next line with a leading `.`
- `else` must follow the `}` of its `if` on the same line
- the `{` of a block starts on the line of its header (`fn ..`, `if cond`, `while cond`,
  `for x in e`, `loop`, `match e`)
- `requires` stays on the line of the signature, and its `[` on the line of `requires`
- a unit name stays on the line of its number (`200 ms`).

So a long expression breaks after an operator, never before it:

```ignite
fn total(a: Int, b: Int, c: Int) -> Int {
  let sum = a +
    b +
    c
  let scaled = scale(
    sum,
    2
  )
  scaled
}

fn scale(value: Int, factor: Int) -> Int = value * factor

fn send(out: TcpStream, line: Text) -> Result<Unit, IoError> requires [
  out.write
] {
  out.write_all(line.to_bytes())
}

test "line breaks" {
  let x = 1; let y = 2
  assert total(x, y, 3) == 12
}
```

A leading operator on the next line starts a new statement and fails:

```ignite,fails=E001
fn both(a: Bool, b: Bool) -> Bool = a
  && b
```

Trailing commas are allowed in braces (records, enums, literals, `match`), in tuples and in
tuple and list patterns. They are not allowed in argument lists, parameter lists, list literals or import lists.

## Operator precedence

From the tightest binding to the loosest:

| level | operators | associativity |
|---|---|---|
| 1 | quantity literal `200 ms`, `99.9%` | (one token pair) |
| 2 | postfix: call `f(x)`, field `x.f`, method `x.m()`, tuple field `t.0`, index `l[i]`, `?` | left |
| 3 | prefix: `!`, `-`, `~`, `spawn` | right |
| 4 | `*` `/` `%` | left |
| 5 | `+` `-` | left |
| 6 | `<<` `>>` | left |
| 7 | `&` | left |
| 8 | `^` | left |
| 9 | `\|` | left |
| 10 | `<` `>` `<=` `>=` | none: `a < b < c` is E001 |
| 11 | `==` `!=` | none: `a == b == c` is E001 |
| 12 | `&&` | left |
| 13 | `\|\|` | left |

- Postfix binds tighter than prefix: `-x.len()` is `-(x.len())`, and `!s.is_empty()` is
  `!(s.is_empty())`.
- The bitwise operators `&`, `|`, `^`, `<<`, `>>` and `~` take `Int` only and never fail. Any
  other type is E040. They bind as in Rust: tighter than the comparisons, so
  `flags & MASK == 0` is `(flags & MASK) == 0`, and looser than `+`, so `1 << n + 1` is
  `1 << (n + 1)`.
- `>>` is arithmetic: it copies the sign bit. A shift by an amount outside 0 to 63 is
  defined: `a << n` is 0 and `a >> n` is 0 for a non-negative `a` and -1 for a negative one.
- `<<` and `>>` are two `<` or two `>` that touch; `a > > b` is E001.
- A closure body, a `return` value and an `assert` condition extend as far to the right as
  an expression can.
- Assignment `=` is a statement, not an operator.

```ignite
test "precedence" {
  assert 1 + 2 * 3 == 7
  assert 10 - 4 - 3 == 3
  assert 7 / 2 * 2 == 6
  assert -"abc".len() == -3
  assert 1 < 2 == true
  assert true || false && false
  assert 2 seconds * 3 == 6 seconds
  assert 6 & 3 == 2
  assert 1 | 2 ^ 3 & 1 == 3
  assert 1 << 2 + 1 == 8
  assert -16 >> 2 == -4
  assert ~0 == -1
  assert 1 << 64 == 0
  assert -1 >> 100 == -1
}
```
