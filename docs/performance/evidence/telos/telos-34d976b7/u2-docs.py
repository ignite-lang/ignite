import sys

path = "/home/timo/projects/telos/docs/llvm.md"
text = open(path).read()


def replace_once(old, new):
    global text
    if text.count(old) != 1:
        sys.exit(f"not found once: {old[:60]!r}")
    text = text.replace(old, new, 1)


replace_once(
    "| driver | `crates/telos` | `telos build --target llvm` |\n",
    "| driver | `crates/telos` | `telos build --target llvm`, `telos test --target llvm` |\n",
)

replace_once(
    "| `i8 store_update(ptr store, i64 id, ptr f, ptr thunk, ptr out)` | 0 when `id` holds nothing (`f` is not called); otherwise replaces the value by `thunk(f, old)` under the store's lock, 1 and a copy of the new value at `out` |\n",
    "| `i8 store_update(ptr store, i64 id, ptr f, ptr thunk, ptr out)` | 0 when `id` holds nothing (`f` is not called); otherwise replaces the value by `thunk(f, old)` under the store's lock, 1 and a copy of the new value at `out` |\n"
    "| `void assert_failed(Text* cond, Text* message, Text* left, Text* right, Text* at)` | `telos_io::assert_failed`: the report of `docs/slice.md` section 12.5 to stderr, restore the terminal, exit 101. `message` is null without a message; `left` and `right` are null unless the condition is `a == b` or `a != b` |\n"
    "| `void try_failed(Text* error, Text* at)` | `telos_io::try_failed`: `` `?` failed on Err: <error> `` (`error` not null) or `` `?` failed on None ``, then `at`, exit 101 |\n"
    "| `void test_main(TelosTest* table, i64 count)` | `telos_io::test_main_with` over the `count` entries at `table` and the arguments `init` stored: the runner of `docs/slice.md` section 12.7. Only with the feature `runner` (below) |\n",
)

replace_once(
    "`exit`, `fail` and `panic` do not return; the generator emits `unreachable` after them.\n",
    "`exit`, `fail`, `panic`, `assert_failed`, `try_failed` and `test_main` do not return; the\n"
    "generator emits `unreachable` after them.\n"
    "\n"
    "Tests. `TelosTest` is `{ Text name, ptr run }` (`{ ptr, i64, i64, ptr }`), `run` a\n"
    "`void ()`. `test_main` exists only in an archive built with\n"
    "`cargo build -p telos-rt --features runner`, which enables the runner in `telos-io`; the\n"
    "default `libtelos_rt.a` leaves it out, so a program without tests links the same members\n"
    "as before tests existed. `telos test --target llvm` checks the archive for the symbol and\n"
    "otherwise fails with exit 3 naming that command. `assert_failed` and `try_failed` are in\n"
    "every archive, because `assert` is allowed outside tests; they call no generic function,\n"
    "and the report functions of `telos-io` are `#[inline]`, which keeps the runtime's other\n"
    "codegen units as they were. Measured on `terminald` and `tn` (2026-11-01): the Rust target's\n"
    "debug executables are byte-identical; the LLVM target's grow by about 1 KB in debug (the\n"
    "two adapters' line tables and constants) and by 16 and 40 bytes with `--release`.\n",
)

replace_once(
    "The role comes from the protocol's instance when the protocol is generic.\n",
    "The role comes from the protocol's instance when the protocol is generic.\n"
    "\n"
    "Test build (`telos test --target llvm`, `docs/slice.md` sections 12.6 and 12.7). The\n"
    "selected modules (`Options::test_modules`) keep their test-only items, and each `test`\n"
    "block becomes `define void @\"<module>.test.<decl index>\"()`. The only executable is\n"
    "`telos-test`; its module `main_telos-test.ll` holds `@telos.tests`, a constant\n"
    "`[N x { ptr, i64, i64, ptr }]` of the full test name (a `Text` constant) and the test\n"
    "function, in the order of the selected modules and then declaration order, and\n"
    "`main(argc, argv)` calls `init` and `test_main(@telos.tests, N)`. The runner starts the\n"
    "same executable with `--run-one <name>` for each test, and `test_main` then runs that\n"
    "one function under `telos_io::run`. Lowering:\n"
    "- `assert cond`: evaluate `cond`; branch to a block that calls `assert_failed` with the\n"
    "  source text of `cond`, the message (when given) and the position of `assert`.\n"
    "- `assert a == b`, `assert a != b`: evaluate `a`, then `b`, once each. A scalar is\n"
    "  compared with `icmp`; an aggregate through the glue `i1 \"eq.<mangled type>\"(ptr a,\n"
    "  ptr b)`, structural and deep (lists element by element, enums by tag then payload),\n"
    "  emitted once per type like `copy` (section 4). On failure both sides are shown by the\n"
    "  `Show` the typer chose (`to_text`, an `IoError`'s message, or `<T>`) and passed as\n"
    "  `left` and `right`.\n"
    "- `?` in a test body (`TypedProgram::test_try`): the `Err` or `None` branch calls\n"
    "  `try_failed` with the shown error (null for `None`) and the position of the `?`,\n"
    "  instead of returning.\n"
    "A position is `<path>:<line>:<column>` from the module's `SourceFile`\n"
    "(`Options::sources`), `<module>:?:?` when the driver passed none.\n",
)

open(path, "w").write(text)
