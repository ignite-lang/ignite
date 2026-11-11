# Your first program

This page builds a small program end to end: a function, a test for it, and an executable
that prints a greeting.

## A function and its test

Create `hello.ignite`:

```ignite
package hello

fn greet(name: Text) -> Text = "Hello, {name}!"

test "greet puts the name in the sentence" {
  assert greet("Ignite") == "Hello, Ignite!"
}
```

- `package hello` names the module. A file without a `package` line is the module `main`.
- `fn greet(name: Text) -> Text = …` is a function with an expression body. `{name}` inside
  a string is interpolation.
- A `test` block sits next to the code it checks. `assert a == b` compares structurally and,
  when it fails, prints both sides and the source position.

Check it:

```sh
ignite check hello.ignite
```

No output and exit status 0 mean the program is well formed. Run the test:

```sh
ignite test hello.ignite
```

```text
running 1 test
test hello/greet puts the name in the sentence ... ok

test result: ok. 1 passed; 0 failed; 0 filtered out; finished in 0.01s
```

A test's full name is `<module>/<name>`. [Test it](testing.md) has more.

## An executable

An executable comes from a `system`. Each component of a system has a kind, a record type;
a kind `K` with an associated function `fn K.main(self: K) -> Result<(), IoError>` produces
an executable named by the component's `executable` field. Extend `hello.ignite`:

```ignite
package hello

type Greeter {
  greeting: Text
  executable: Text
}

fn greet(name: Text) -> Text = "Hello, {name}!"

fn Greeter.main(self: Greeter) -> Result<(), IoError> {
  Stdout.write_all("{greet(self.greeting)}\n".to_bytes())
}

system Hello {
  component greeter: Greeter {
    greeting = "Ignite"
    executable = "hello"
  }
}

test "greet puts the name in the sentence" {
  assert greet("Ignite") == "Hello, Ignite!"
}
```

- `type Greeter { … }` is a record type, here used as a component kind.
- `Greeter.main` is an associated function. Writing to standard output is an effect
  (`stdout.write`); `main` has no `requires` clause, so its effects are inferred. See
  [Effects and capabilities](../design/effects.md).
- `system Hello` declares one component of kind `Greeter` with its field values.

Build and run it:

```sh
ignite build hello.ignite
target/ignite/hello/bin/hello
```

The output of both commands:

```text
built target/ignite/hello/bin/hello
Hello, Ignite!
```

`ignite build` compiles the program on the default target, LLVM. It writes LLVM IR under
`target/ignite/hello/llvm/`, and clang compiles and links it. It prints one `built <path>` line
per executable. The [command-line reference](../reference/cli.md#ignite-build) has its
options.

Next: [the terminal system](terminal.md), a real system with a protocol, a console and two
executables.
