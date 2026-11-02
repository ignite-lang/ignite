import pathlib

C = pathlib.Path("/home/timo/projects/telos/crates")

HELPER = '''
/// The harness name of `test` in this file: its path without the crate
/// name (`stdio::child_x` when the file is a module of `tests/main.rs`).
fn harness_name(test: &str) -> String {
    match module_path!().split_once("::") {
        Some((_, module)) => format!("{module}::{test}"),
        None => test.to_string(),
    }
}
'''


def edit(path, pairs, anchor):
    p = C / path
    text = p.read_text()
    for old, new in pairs:
        assert text.count(old) == 1, (path, old)
        text = text.replace(old, new)
    assert text.count(anchor) == 1, (path, anchor)
    text = text.replace(anchor, anchor + HELPER, 1)
    p.write_text(text)


edit(
    "telos-io/tests/stdio.rs",
    [('.args([test, "--exact", "--test-threads=1", "--nocapture"])',
      '.args([&harness_name(test), "--exact", "--test-threads=1", "--nocapture"])')],
    'const CHILD: &str = "TELOS_IO_CHILD";\n',
)
edit(
    "telos-io/tests/terminal.rs",
    [('.args([test, "--exact", "--test-threads=1", "--nocapture"])',
      '.args([&harness_name(test), "--exact", "--test-threads=1", "--nocapture"])'),
     ('''        .args([
            "child_not_a_terminal",
            "--exact",''',
      '''        .args([
            &harness_name("child_not_a_terminal"),
            "--exact",''')],
    'const CHILD: &str = "TELOS_IO_CHILD";\n',
)
edit(
    "telos-rt/tests/terminal.rs",
    [('.args(["--exact", "child", "--nocapture", "--test-threads=1"])',
      '.args(["--exact", &harness_name("child"), "--nocapture", "--test-threads=1"])')],
    'const MODE: &str = "TELOS_RT_CHILD";\n',
)
