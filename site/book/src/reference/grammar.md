# Grammar

This page gives the whole grammar of Ignite as the v0.3.0 parser accepts it, then the rules
for line breaks and the operator precedence. Tokens are described in
[Lexical structure](lexical.md); what the declarations mean is in [Types](types.md),
[Effects](effects.md), [Modules and imports](modules.md) and
[Tests and assertions](tests.md).

## Notation

- `'fn'` is a keyword or punctuation token, written as in the source.
- `ident` is an identifier. `name` is an identifier or a keyword; it appears in paths, field
  names and after `.`.
- `INT`, `DECIMAL`, `STRING`, `BYTE` and `BYTES` are literals. `STRING` covers plain,
  interpolated and raw strings; `BYTE` is `b'c'`; `BYTES` is `b"..."` or `br"..."`.
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

record       := 'type' ident tparams? braced(field)
field        := name ':' type ('=' expr)?                     field with a default
alias        := 'type' ident tparams? '=' type
tparams      := '<' ident (',' ident)* '>'                    no bounds here
enum         := 'enum' ident tparams? braced(variant)
variant      := ident ('(' type (',' type)* ')')?

fn           := 'fn' (ident '.')? ident fn_tparams?           `fn Owner.name` is associated
                '(' (param (',' param)*)? ')'
                ('->' type)? requires? (block | '=' expr)
fn_tparams   := '<' fn_tparam (',' fn_tparam)* '>'
fn_tparam    := ident (':' ident ('+' ident)*)?               bound: `S: ByteStream`
param        := ident ':' type
requires     := 'requires' '[' (effect (',' effect)*)? ']'
effect       := name ('.' name)* ('.' '*')?                   `stream.write`, `app.*`

const        := 'const' ident ':' type '=' expr
protocol     := 'protocol' ident '{' NL* (fn NL*)* '}'       its fns are the roles
test         := 'test' STRING block

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
              | '[' (pattern (',' pattern)*)? ']'             list of exactly that length
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
- `type N = T` is an alias; `type N { .. }` is a record. `test type N = T` is E001: an alias
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
- The pattern of `let`, `var` and `for` must be irrefutable: a name, `_` or a tuple of those
  (E001 otherwise; use `match`).
- A one-element tuple `(a,)` is E001, as a value, type or pattern. Write `(a)` to group.
- Comparisons do not chain: `a < b < c` is E001. Write `a < b && b < c`.
- `return` has no value when a line break, `;`, `,`, `}`, `)` or `]` follows it.
- In a type, a closing `>` may touch `=`: `x: Option<Int>= None` reads as `>` then `=`.
- Brackets, type arguments, prefix operators and `return` nest at most 256 levels (E054).
  Long chains of binary operators are not nesting and have no limit.

## Line breaks and separators

A line break ends a statement, a field, a variant, an arm, a system member or a declaration.
Within an expression, Ignite decides by position whether a line break is a separator or is
skipped.

Line breaks are **skipped**:

- inside `( )` and `[ ]`: argument lists, parameter lists, tuples, list literals, patterns,
  `requires [..]` lists, variant payloads;
- after a binary operator, so a long expression breaks after `+`, `&&` and the like;
- after `=`, `=>`, `->` and `,`;
- after `{` and before `}`;
- after the `,` between imported names in `import a.b.{X, Y}`.

Line breaks **separate**:

- inside `{ }` of a block: between statements, together with `;`;
- inside `{ }` of a record, enum, record literal, brace literal or `match`: between items,
  together with `,`;
- at the top level: between declarations.

Some tokens must stay on the line of what comes before them, even inside `( )` and `[ ]`:

- a postfix `(`, `[`, `.` or `?` must start on the same line as its operand. On a new line
  it starts something new: `[a` NL `(b)]` is E001, not a call of `a`, and
  a method chain cannot continue on the next line with a leading `.`;
- `else` must follow the `}` of its `if` on the same line;
- the `{` of a block starts on the line of its header (`fn ..`, `if cond`, `while cond`,
  `for x in e`, `loop`, `match e`);
- `requires` stays on the line of the signature, and its `[` on the line of `requires`;
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

Trailing commas: allowed in braces (records, enums, literals, `match`), in tuples and in
tuple and list patterns;
not allowed in argument lists, parameter lists, list literals or import lists.

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
- The bitwise operators `&`, `|`, `^`, `<<`, `>>` and `~` take `Int` only (E040 for any other
  type) and never fail. They bind as in Rust: tighter than the comparisons, so
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
