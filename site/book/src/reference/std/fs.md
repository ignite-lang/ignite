# std.fs

Files under one root directory. The module is the capability `Directory`: a handle to a root
that a system declares as a resource and passes to the code that may read or write under it.
Every path is relative to that root, and no operation leaves it.

- Layer: base.
- Source: [`std/fs.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/fs.ignite).
- Import: `import std.fs.{Directory, read_text_file, write_text_file, join_path}`.

## The resource

```ignite,fragment
resource files: Directory { path = "workspace", create = true, flag = "--root" }
```

| field | default | meaning |
|---|---|---|
| `path` | required | the root, relative to the working directory of the process |
| `create` | `false` | create the root at launch when it is missing |
| `flag` | `""`, none | a command-line flag whose value replaces `path` at launch |

Tests make a `Directory` with the test-only `Directory.temporary()`, a new empty directory
inside the test's scratch directory.

## Methods

Each method returns `Result<_, IoError>`. Its effect is on the receiver: a function that calls
`files.read_file` declares `requires [files.read]`.

```ignite,fragment
read_file(path: Text) -> Bytes
write_file(path: Text, data: Bytes) -> Unit
read_directory(path: Text) -> List<Text>
create_directory(path: Text) -> Unit
remove(path: Text) -> Unit
remove_tree(path: Text) -> Unit
absolute_path() -> Text
```

| method | effect | what it does |
|---|---|---|
| `read_file` | read | the bytes of a file; at most 16 MiB |
| `write_file` | write | writes a file, replacing it atomically |
| `read_directory` | read | the names in a directory in byte order; a directory's name ends with `/` |
| `create_directory` | write | creates a directory and its missing parents |
| `remove` | write | removes a file, a link or an empty directory |
| `remove_tree` | write | removes a directory and everything under it; never follows a link |
| `absolute_path` | read | the root as resolved at launch |

## Paths

Paths are relative and separated by `/`; `""` and `.` name the root. An absolute path, a `..`
component or a NUL byte is refused before any IO, and is [E140](../diagnostics.md#e140) when the path is a literal. A
symbolic link at any component is refused too.

## Helpers

```ignite,fragment
fn read_text_file(files: Directory, path: Text) -> Result<Text, IoError> requires [files.read]
fn write_text_file(files: Directory, path: Text, text: Text) -> Result<Unit, IoError> requires [files.write]
fn join_path(left: Text, right: Text) -> Text
```

| function | what it does |
|---|---|
| `read_text_file` | the file as text; bytes that are not UTF-8 are an error naming the path |
| `write_text_file` | writes `text` as UTF-8, replacing the file atomically |
| `join_path` | `left` and `right` with one `/` between them when both are non-empty |
