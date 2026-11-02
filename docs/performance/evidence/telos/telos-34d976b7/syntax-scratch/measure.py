"""Measure verbosity patterns in the Telos corpus. Run: python3 -I measure.py ROOT"""
import re
import sys
from collections import defaultdict

ROOT = sys.argv[1]
FILES = [
    "std/text.telos",
    "std/json.telos",
    "std/console.telos",
    "std/protocol/telnet.telos",
    "std/protocol/http.telos",
    "examples/terminal/terminal.telos",
    "examples/todo/todo.telos",
    "examples/terminal/terminal.design.telos",
]

TOKEN = re.compile(
    r'br?"(?:\\.|[^"\\])*"|r?"(?:\\.|[^"\\])*"|b\'(?:\\.|[^\'\\])+\'|0x[0-9A-Fa-f]+|\d[\d_]*'
    r"|[A-Za-z_][A-Za-z0-9_]*|\.\.=|=>|->|==|!=|<=|>=|&&|\|\||\.\.|[^\sA-Za-z0-9_]"
)


def strip_comment(line):
    out, i, in_str = [], 0, False
    while i < len(line):
        c = line[i]
        if in_str:
            out.append(c)
            if c == "\\" and i + 1 < len(line):
                out.append(line[i + 1])
                i += 2
                continue
            if c == '"':
                in_str = False
        else:
            if c == '"':
                in_str = True
            elif line.startswith("//", i):
                break
            out.append(c)
        i += 1
    return "".join(out).rstrip()


def tokens(code):
    return TOKEN.findall(code)


hits = defaultdict(list)  # pattern -> [(file, line, saved_lines, saved_tokens, text)]


def hit(name, f, n, lines_saved, tokens_saved, text):
    hits[name].append((f, n, lines_saved, tokens_saved, text.strip()))


per_file = {}
for f in FILES:
    raw = open(f"{ROOT}/{f}", encoding="utf-8").read().split("\n")
    if raw and raw[-1] == "":
        raw = raw[:-1]
    code = [strip_comment(l) for l in raw]
    blank = sum(1 for l in raw if not l.strip())
    comment = sum(1 for l, c in zip(raw, code) if l.strip() and not c.strip())
    toks = sum(len(tokens(c)) for c in code)
    # test region lines: lines inside `test "..." {` blocks and `test fn` items
    in_test, depth, test_lines, test_toks = False, 0, 0, 0
    for c in code:
        s = c.strip()
        if not in_test and (s.startswith("test ") ):
            in_test, depth = True, 0
        if in_test:
            test_lines += 1
            test_toks += len(tokens(c))
            depth += c.count("{") - c.count("}")
            # a test fn with expression body may have no braces; end at a blank line
            if depth <= 0 and (s.endswith("}") or (not s.endswith("=") and "{" not in s and depth == 0 and not s.startswith("test fn") )):
                in_test = False
            elif depth <= 0 and s.startswith("test fn") and "{" not in s and not s.endswith("="):
                in_test = False
    per_file[f] = dict(lines=len(raw), blank=blank, comment=comment,
                       code=len(raw) - blank - comment, tokens=toks,
                       test_lines=test_lines, test_tokens=test_toks)

    n = len(code)
    for i, c in enumerate(code):
        s = c.strip()
        ln = i + 1
        nxt = code[i + 1].strip() if i + 1 < n else ""
        nxt2 = code[i + 2].strip() if i + 2 < n else ""
        nxt3 = code[i + 3].strip() if i + 3 < n else ""

        # 1 for loop whose first statement destructures the loop variable
        m = re.match(r"for (\w+) in .*\{$", s)
        if m and re.match(r"let \(.*\) = " + m.group(1) + r"$", nxt):
            hit("for_destructure", f, ln, 1, 4, s + " / " + nxt)

        # 2 counter loops: `x = x + 1` inside a while over x
        m = re.match(r"(\w+) = \1 \+ 1$", s)
        if m:
            name = m.group(1)
            back = "\n".join(x.strip() for x in code[max(0, i - 12):i])
            if re.search(r"while " + name + r" [<>]", back) and re.search(r"var " + name + r" = ", back):
                hit("counter_while", f, ln, 2, 8, s)

        # 3 compound assignment
        m = re.match(r"([\w.]+) = \1 ([+\-*]) ", s)
        if m:
            hit("compound_assign", f, ln, 0, 2, s)

        # 4 guard: `if c {` / `return ..` / `}` on three lines
        if re.match(r"if .*\{$", s) and nxt.startswith("return") and nxt2 == "}" and not nxt3.startswith("else") and not code[i + 2].strip().startswith("} else"):
            hit("guard_three_lines", f, ln, 2, 0, s + " " + nxt)

        # 5 let x = match e { P => x / Q => return ..} (let-else)
        if re.match(r"let .* = match .*\{$", s):
            arms = [nxt, nxt2]
            if any("return" in a for a in arms) and nxt3 == "}":
                hit("let_else", f, ln, 3, 4, s)
        # 5b match with one diverging arm and one `=> {}` arm (if-is)
        if re.match(r"match .*\{$", s) and ("=> {}" in nxt + nxt2) and ("return" in nxt + nxt2) and nxt3 == "}":
            hit("if_is_statement", f, ln, 3, 6, s)

        # 6 Bool match: arms ending `=> true` and `_ => false`
        if re.search(r"match .*\{$", s) and nxt.endswith("=> true") and nxt2 == "_ => false":
            hit("bool_match", f, ln, 3, 7, s)

        # 7 Option default: Some(x) => x / None => d  (slice 9 unwrap_or)
        m = re.match(r"Some\((\w+)\) => \1$", s)
        if m and nxt.startswith("None =>") and "return" not in nxt:
            hit("option_default_s9", f, ln, 3, 6, s + " / " + nxt)

        # 8 None => None / _ => None arms in Option code (Option `?`)
        if re.match(r"(None|_) => None$", s):
            hit("none_to_none", f, ln, 1, 4, s)

        # 9 empty arms
        if s.endswith("=> {}"):
            hit("empty_arm", f, ln, 1, 4, s)

        # 10 state threading: let (next, e) = f(state..) / state = next
        m = re.match(r"let \((\w+), \w+\) = .*", s)
        if m and re.match(r"\w+ = " + m.group(1) + "$", nxt):
            hit("state_thread", f, ln, 1, 4, s + " / " + nxt)

        # 11 append runs after `var x = ..`
        m = re.match(r"var (\w+)(: [^=]+)? = (.*)$", s)
        if m:
            name, j = m.group(1), i + 1
            while j < n and re.match(re.escape(name) + r"\.(append|push)\(", code[j].strip()):
                j += 1
            run = j - i - 1
            if run >= 1 and not code[j].strip().startswith(("for", "while", "loop")):
                hit("append_run", f, ln, run, 2 * run, s)

        # 12 read loops
        if re.match(r"match .*\.read\([\w_]+\)\?? \{$", s):
            hit("read_match", f, ln, 0, 0, s)

        # 13 pair lists
        for _ in re.finditer(r'(?<=[\[ ])\("[^"]*", ', s):
            hit("pair_literal", f, ln, 0, 2, s[:60])

        # 14 .at( indexing
        for _ in re.finditer(r"\.at\(", s):
            hit("at_call", f, ln, 0, 1, s[:60])

        # 15 slice to the end
        for _ in re.finditer(r"\.slice\(([^,()]+(\([^()]*\))?), (\w+)\.len\(\)\)", s):
            hit("slice_to_len", f, ln, 0, 4, s[:70])
        for _ in re.finditer(r"\.slice\(", s):
            hit("slice_any", f, ln, 0, 0, s[:70])

        # 16 escaped quotes and doubled braces (slice 9 raw strings / row 1)
        q = len(re.findall(r'\\"', s)) + len(re.findall(r"\{\{|\}\}", s))
        if q and '"' in s:
            hit("escapes_s9", f, ln, 0, q, s[:60])

        # 17 const with a literal type
        if re.match(r"const \w+: (Int|Text|Bool) = (-?\d|\")", s):
            hit("const_literal_type", f, ln, 0, 2, s)

        # 18 assert lines and their callee
        m = re.match(r"assert !?(\w+)\(", s)
        if m:
            hit("assert_call", f, ln, 0, 0, m.group(1))

        # 19 role close pattern
        if s == "_ = stream.close()" and nxt in ("result",):
            hit("close_then_result", f, ln, 2, 6, s)

        # 20 Stdin.set_raw(false) restores
        if "Stdin.set_raw(false)" in s:
            hit("raw_restore", f, ln, 0, 0, s)

        # 21 accumulator loops: var X = <empty>; for ..; X
        m = re.match(r"var (\w+)(: [^=]+)? = (Bytes\.empty\(\)|List\.empty\(\)|\"\"|\[\])$", s)
        if m:
            name = m.group(1)
            for j in range(i + 1, min(n, i + 16)):
                t = code[j].strip()
                if t in (name, name + ".to_text()", "return " + name, "Ok(" + name + ")"):
                    body = [x.strip() for x in code[i + 1:j]]
                    if any(b.startswith("for ") for b in body):
                        hit("accumulator_loop", f, ln, j - i, 0, s)
                    break

        # 22 test fns
        if s.startswith("test fn"):
            hit("test_fn", f, ln, 0, 0, s[:60])

        # 23 Ok(()) tails and `return Ok(())`
        if s in ("Ok(())", "return Ok(())") or s.endswith("=> return Ok(())"):
            hit("ok_unit", f, ln, 0, 0, s)

        # 24 to_bytes / to_text conversions
        for _ in re.finditer(r"\.to_bytes\(\)|\.to_text\(\)", s):
            hit("conversion", f, ln, 0, 0, s[:60])

        # 25 requires lists
        if "requires" in s:
            hit("requires", f, ln, 0, 0, s[:70])

        # 26 two-arm match whose second arm is a wildcard or None with a one-line value:
        #    `match e { P => a  _ => b }` over 4 lines -> `if e is P { a } else { b }`
        if re.search(r"match .*\{$", s) and nxt3 == "}" and re.match(r"(_|None) => ", nxt2) \
                and "=>" in nxt and not nxt.endswith("{") and not nxt.endswith("=> true"):
            hit("two_arm_match", f, ln, 3, 2, s + " / " + nxt + " / " + nxt2)

        # 27 None-propagating matches: `None => return None` or `None => None` first arm
        if re.match(r"None => (return )?None$", s):
            hit("option_question", f, ln, 2, 6, s)

        # 28 record literal copying fields from one value: `f = x.f`
        for m in re.finditer(r"(\w+) = (\w+)\.\1\b", s):
            hit("record_copy_field", f, ln, 0, 3, s[:60])

        # 29 Object([...]) JSON literals
        for _ in re.finditer(r"Object\(\[", s):
            hit("json_object", f, ln, 0, 0, s[:60])

# assert runs with the same callee on consecutive lines
runs = []
by_file = defaultdict(list)
for f, n, _, _, callee in hits["assert_call"]:
    by_file[f].append((n, callee))
for f, items in by_file.items():
    cur = []
    for n, c in items:
        if cur and cur[-1][1] == c and n == cur[-1][0] + 1:
            cur.append((n, c))
        else:
            if len(cur) >= 3:
                runs.append((f, cur[0][0], len(cur), cur[0][1]))
            cur = [(n, c)]
    if len(cur) >= 3:
        runs.append((f, cur[0][0], len(cur), cur[0][1]))

tot = defaultdict(int)
print("| file | lines | blank | comment | code | tokens | test lines | test tokens |")
print("|---|---|---|---|---|---|---|---|")
for f, d in per_file.items():
    for k, v in d.items():
        tot[k] += v
    print(f"| `{f}` | {d['lines']} | {d['blank']} | {d['comment']} | {d['code']} | {d['tokens']} | {d['test_lines']} | {d['test_tokens']} |")
print(f"| total | {tot['lines']} | {tot['blank']} | {tot['comment']} | {tot['code']} | {tot['tokens']} | {tot['test_lines']} | {tot['test_tokens']} |")
print()
print("| pattern | count | lines saved | tokens saved | first example |")
print("|---|---|---|---|---|")
for name, items in sorted(hits.items(), key=lambda kv: -len(kv[1])):
    ls = sum(x[2] for x in items)
    ts = sum(x[3] for x in items)
    f, n, _, _, text = items[0]
    print(f"| {name} | {len(items)} | {ls} | {ts} | `{f}:{n}` {text[:50]} |")
print()
print("assert runs (>=3 consecutive asserts on one callee):", len(runs), "covering", sum(r[2] for r in runs), "asserts")
for r in runs:
    print("  ", r)
print()
for name in ("accumulator_loop", "let_else", "if_is_statement", "bool_match", "guard_three_lines", "append_run",
             "counter_while", "for_destructure", "state_thread", "option_default_s9", "none_to_none",
             "close_then_result", "slice_to_len", "read_match", "empty_arm", "two_arm_match", "option_question", "pair_literal"):
    print(name)
    for f, n, ls, ts, text in hits[name]:
        print(f"   {f}:{n} saves {ls}: {text[:80]}")
