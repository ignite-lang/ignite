# Keep data in a directory

A program keeps data across a restart in a `Directory`: a root that the system declares as a
resource and passes to the code that may read or write under it. Every path is relative to
that root, and no operation leaves it. The methods and their limits are in
[`std.fs`](../reference/std/fs.md).

## Declare the directory and use it

`visits.ignite` counts how often it was started, in the file `count`:

```ignite
package visits

import std.fs.{read_text_file, write_text_file}

type Counter {
  files: Directory
  executable: Text
}

fn count_of(text: Text) -> Int = match Int.parse(text.trim()) {
  Some(n) => n
  None => 0
}

fn Counter.main(self: Counter) -> Result<(), IoError> {
  let before = match read_text_file(self.files, "count") {
    Ok(text) => count_of(text)
    Err(_) => 0
  }
  let now = before + 1
  write_text_file(self.files, "count", "{now}\n")?
  Stdout.write_all("visit {now}, stored under {self.files.absolute_path()?}\n".to_bytes())
}

system Visits {
  resource state: Directory { path = "visits-data", create = true, flag = "--data" }

  component counter: Counter {
    files = state
    executable = "visits"
  }
}

test "a count survives a fresh read" {
  let files = Directory.temporary()?
  write_text_file(files, "count", "41\n")?
  assert count_of(read_text_file(files, "count")?) == 41
  assert count_of("") == 0
}
```

- `path = "visits-data"` is relative to the working directory of the process.
- `create = true` creates the root at launch when it is missing.
- `flag = "--data"` lets the command line replace `path`.
- `files = state` hands the resource to the component. Only code that receives the
  `Directory` can touch the files.
- `write_text_file` replaces the file atomically: a reader sees the old content or the new
  one, never a part.

## Build and run it twice

```sh
ignite build visits.ignite
./target/ignite/visits/bin/visits
./target/ignite/visits/bin/visits
cat visits-data/count
```

```text
built target/ignite/visits/bin/visits
visit 1, stored under /home/user/visits/visits-data
visit 2, stored under /home/user/visits/visits-data
2
```

The flag points the same program at another root:

```sh
./target/ignite/visits/bin/visits --data other
```

```text
visit 1, stored under /home/user/visits/other
```

## Test without touching your files

`Directory.temporary()` gives a test a new, empty directory of its own:

```sh
ignite test visits.ignite
```

```text
running 1 test
test visits/a count survives a fresh read ... ok

test result: ok. 1 passed; 0 failed; 0 filtered out; finished in 0.00s
```

## Paths that leave the root

An absolute path, a `..` component or a NUL byte is refused before any IO, and so is a
symbolic link at any component. When the path is a literal, `ignite check` refuses it:

```ignite,fails=E140
fn secret(files: Directory) -> Result<Text, IoError> requires [files.read] =
  Ok(files.read_file("../etc/passwd")?.to_text())
```

```text
escape.ignite:2:22: error[E140]: path `../etc/passwd` of `read_file`: leaves its root
```

## A larger example

The [todo service](../guide/http.md) writes one file per change, `<shard>/<id>/<version>.json`
under its `Directory` `todo-data`, and loads the newest version of each todo on its first
request. Its source,
[`examples/todo/todo.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/todo/todo.ignite),
explains why that layout survives concurrent writes and a SIGKILL. Writes do not `fsync`, so a
power loss can lose an acknowledged change.
