# The Ignite Book

**Say what your program is for. One file, one build, one native executable.**

Ignite is a compiled, intent-oriented language for fast, resource-efficient software
development by humans and AI agents. One typed file holds the program, its components, its
contracts and its policies. The compiler turns it into native executables, then runs and
checks them.

A system declares what it is made of. An excerpt of the terminal example:

```ignite,fragment
system Terminal {
  resource ingress: TcpListener { address = "127.0.0.1", port = 2323 }

  component server: TelnetServer {
    listener = ingress
    application = console
    executable = "terminald"
  }
}
```

`ignite build` turns the whole file into a server `terminald` and a client `tn`. The Telnet
protocol is Ignite code in the standard library. [The terminal system](guide/terminal.md) walks
through the full program.

## What works today

Ignite is an early prototype, at release v0.16.4. It builds native executables on two targets,
LLVM by default and Rust, and runs and applies systems on one local machine. The syntax is not
frozen. The [roadmap](roadmap.md) lists what is released, what is being built and what is only
designed.

## How this book is organised

| part | read it when you want to |
|---|---|
| [Start](guide/install.md) | install Ignite, write and test a program, build the terminal and the HTTP examples, plan, run, replay and apply a system |
| [How-to](guide/targets.md) | do one task: another target, a container build, a component kind, a protocol, a probe and a contract, data in a directory, a rollout, a recovery, a conformance check |
| [Concepts](design/index.md) | understand the ideas: intent and properties, effects, protocols, components, systems, contracts, the run-time model, the evaluator, the substrate |
| [Reference](reference/index.md) | look something up: syntax, types, effects, modules, system declarations, the standard library, every command and every diagnostic |
| [Project](roadmap.md) | see the roadmap, where the ideas come from, the FAQ and the measured quality of each release |

Every Ignite block carries a label in its corner. A **checked** block is a complete program,
and the repository's test suite runs `ignite check` on it. An **excerpt** is part of a larger
file. A block that **fails to check, on purpose** must produce the diagnostic it shows. The
language design is a versioned
proposal, [`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md)
version 0.3. The exact contract of what is implemented is
[`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md).
