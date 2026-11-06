# The Ignite Book

**Say what your program is for. One file, one build, one native executable.**

Ignite is a compiled, intent-oriented language for fast, resource-efficient software
development by humans and AI agents. One typed file holds the program, its components, its
contracts and its policies; the compiler turns it into native executables, then runs and
checks them.

This is what a complete Ignite system looks like, a Telnet console with a server and a client:

```ignite,dir=examples/terminal
package examples.terminal

import std.console.CommandConsole
import std.protocol.telnet.{TelnetServer, TelnetClient}

system Terminal {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 2323
  }

  component console: CommandConsole {
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
    command_schema = module("examples.terminal.command_schema")
  }

  component server: TelnetServer {
    listener = ingress
    application = console
    executable = "terminald"
  }

  component client: TelnetClient {
    target = ingress
    executable = "tn"
  }
}
```

`ignite build` turns it into two programs, the server `terminald` and the client `tn`. The
console's commands are ordinary Ignite functions in a sibling file, and the Telnet protocol
itself is Ignite code in the standard library.

## How this book is organised

| part | read it when you want to |
|---|---|
| [Guide](guide/install.md) | install Ignite, write and test a program, build it for both targets, and plan, run, replay and apply a system |
| [Language design](design/index.md) | understand the ideas: intent and properties, effects, protocols, components, systems and assemblies, the substrate, the evaluator |
| [Reference](reference/index.md) | look something up: syntax, types, effects, modules, the standard library, every command and every diagnostic |
| [Roadmap](roadmap.md) and [FAQ](faq.md) | see where Ignite is going and what it does not do yet |

## Status

Ignite is an early prototype, at release v0.8.0. What works today:

- the language subset, with records, enums, pattern matching, generics with bounds,
  closures, quantities such as `4096 bytes` and `15 minutes`, and explicit effects;
- two native targets, LLVM (the default) and Rust, over one IO substrate;
- the standard library in Ignite, including Telnet and HTTP/1.1 and the control reducers;
- inline `test` blocks, run on both targets by `ignite test`;
- the system declaration: resources, components, contracts, policies and assemblies;
- `ignite plan` and `ignite explain`, `ignite run` and `ignite replay`, and local
  `ignite apply` through `ignite host`;
- protocols that state which RFC requirements they meet, checked by `ignite conformance`
  against catalogues of RFC 854, 855, 1143, 9112 and 9110.

The syntax is not frozen, and the [language design](design/index.md) is a versioned proposal
(version 0.3). Where this book describes behaviour, it follows the contract of what is
implemented, [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md).
Designs that are not built yet appear only on the [roadmap](roadmap.md).
