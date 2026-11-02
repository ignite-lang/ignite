import sys

io = "/home/timo/projects/telos/crates/telos-io/src/lib.rs"
rt = "/home/timo/projects/telos/crates/telos-rt/src/lib.rs"

io_blocks = [
    '#[cfg(feature = "report")]\nmod report;\n',
    '#[cfg(feature = "runner")]\nmod test;\n',
    '#[cfg(feature = "report")]\npub use report::{FAILURE_EXIT_CODE, assert_failed, assert_report, try_failed, try_report};\n',
    '#[cfg(feature = "runner")]\npub use test::{\n    Arguments, DEFAULT_TIMEOUT, Test, USAGE_EXIT_CODE, Verdict, test_main, test_main_with,\n};\n',
]
rt_blocks = [
    "mod test;\n",
    '#[cfg(feature = "runner")]\npub use test::telos_rt_test_main;\n',
    "pub use test::{TelosTest, telos_rt_assert_failed, telos_rt_try_failed};\n",
]

TAIL = (
    "\n// Tests (`docs/slice.md` section 12) come last, so the line numbers of\n"
    "// everything above, and with them the debug info of a program without\n"
    "// tests, stay as they were before tests existed.\n"
)


def move(path, blocks):
    text = open(path).read()
    for block in blocks:
        if text.count(block) != 1:
            sys.exit(f"{path}: block not found once: {block!r}")
        text = text.replace(block, "", 1)
    text = text.rstrip("\n") + "\n" + TAIL + "".join(blocks)
    open(path, "w").write(text)


move(io, io_blocks)
move(rt, rt_blocks)
