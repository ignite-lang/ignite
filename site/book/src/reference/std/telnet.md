# std.protocol.telnet

Telnet over one byte stream (RFC 854): a Network Virtual Terminal with every option refused.
The module holds `protocol Telnet` with its two roles, the byte decoder both roles use, and
the three component kinds that speak Telnet: the server `TelnetServer`, the client
`TelnetClient` and the probe `TelnetPrompt`. Both roles are generic over the stream, so the
same code runs over a `TcpStream` in a program and a `PipeStream` in a test.

- Layer: protocol.
- Source: [`std/protocol/telnet.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/protocol/telnet.ignite).
- Import: `import std.protocol.telnet.{Telnet, TelnetServer, TelnetClient, SessionLimits}`.

## The protocol

```ignite,fragment
type IoResult<T> = Result<T, IoError>

protocol Telnet {
  fn serve<S: ByteStream>(stream: S, limits: SessionLimits, application: Console) -> IoResult<Unit> requires [
    stream.read, stream.write
  ]

  fn connect<S: ByteStream>(stream: S) -> IoResult<Unit> requires [
    stream.read, stream.write, stdin.read, stdin.mode, stdout.write, process.exit
  ]
}
```

**`Telnet.serve`**, the server role, runs one session of a [`Console`](console.md):

- It sets the stream's idle timeout from `limits.idle_timeout`, shows the console's prompt,
  passes each input line to `respond` and sends the reply, each line ending in CR LF, then
  the next prompt. The prompt and the answer to AYT are each followed by IAC GA (Go Ahead,
  RFC 854 p5), since the server then waits for input.
- CR LF, CR NUL, a lone CR and a lone LF each end a line. IAC IAC is the data byte 255.
- DO is answered WONT and WILL is answered DONT. IAC AYT is answered `[yes]`.
- A Synch is honoured: when urgent data is pending after a read (`urgent_pending`, see
  [std.net](net.md)), the line so far is dropped and data is discarded up to the Data Mark
  (IAC DM), while commands are still obeyed.
- The session ends on the console's `Close` action, end of input, IAC IP, a line longer than
  the console's `maximum_line` (answered `line too long`), or a subnegotiation longer than
  `limits.maximum_subnegotiation`.
- A client silent for `limits.idle_timeout` reads as end of input, so an abandoned connection
  does not hold its task forever.
- It closes the stream when the session ends.

**`Telnet.connect`**, the client role, joins standard input and output to a session:

- The server's text, with Telnet commands removed and line ends as LF, goes to standard
  output.
- When standard input is a terminal, it is switched to raw mode and a local line editor
  sends each line: Enter sends the line, backspace erases the last character, Ctrl+D on a
  partial line sends it without a line end, Ctrl+C sends IAC IP followed by the Synch (IAC
  DM as urgent data), Ctrl+D on an empty line closes the write half, and Ctrl+] opens the
  local `telnet> ` prompt, which knows `close`, `quit` and `help`.
- Otherwise standard input is forwarded as NVT lines until it ends.
- It ends with `Ok` when the server closes the connection, and with `Err` on a subnegotiation
  longer than `MAXIMUM_SUBNEGOTIATION`.

A role is called like an associated function: `Telnet.serve(stream, limits, console)`,
usually through `spawn`.

## Records

```ignite,fragment
type SessionLimits {
  maximum: Int = 1000
  idle_timeout: Duration = 10 minutes
  input_buffer: ByteSize = 16 KiB
  output_buffer: ByteSize = 64 KiB
  slow_reader_deadline: Duration = 30 seconds
  maximum_subnegotiation: ByteSize = 1 KiB
}

type TelnetProfile {
  baseline: Text = "nvt"
  negotiation: Text = "q_method"
  unsupported_options: Text = "refuse"
  character_encoding: Text = "nvt_ascii"
}
```

- `SessionLimits` is Telnet's limits record. `Telnet.serve` reads `idle_timeout` and
  `maximum_subnegotiation`. The other fields are declared and validated, without behaviour
  yet.
- `TelnetProfile` is the Telnet dialect a server speaks. Each field is a closed set written
  as text; the defaults are what `Telnet` implements.

## Component kinds

```ignite,fragment
type TelnetServer {
  listener: TcpListener
  application: CommandConsole
  executable: Text
  protocol: TelnetProfile = TelnetProfile {}
  sessions: SessionLimits = SessionLimits {}
  recovery: Restart = Restart {}
  rollout: DrainRollout = DrainRollout {}
}

fn TelnetServer.main(self: TelnetServer) -> IoResult<Unit>
fn TelnetServer.machine(self: TelnetServer, id: Text) -> Machine
fn TelnetServer.validate(self: TelnetServer) -> List<Text>
```

`TelnetServer` serves a declared console ([`CommandConsole`](console.md)) on a listener.

- `main` accepts connections and runs `Telnet.serve` under `sessions` in one task per
  connection. A session that fails ends its task, and the server goes on accepting.
- `machine` gives `ignite run` a supervisor that restarts the server under `recovery`
  ([std.policy](policy.md)); the `rollout` policy brings its rollout machine
  ([std.control](control.md)).
- `validate` refuses session limits that admit no session: `sessions.maximum` below 1 or a
  non-positive `sessions.idle_timeout` (E120).

```ignite,fragment
type TelnetClient {
  target: Endpoint
  executable: Text
}

fn TelnetClient.main(self: TelnetClient) -> IoResult<Unit>
```

`TelnetClient` connects to `target` and runs `Telnet.connect`: standard input and output
become the session. The process exits with status 3 when connecting fails and 4 when the
session fails.

```ignite,fragment
type TelnetPrompt {
  target: Ref<TelnetServer>
  interval: Duration = 10 seconds
  timeout: Duration = 2 seconds
  expected_prompt: Text
  close_after_probe: Bool = true
  attempts: Signal<ProbeAttempt>
  executable: Text
}

fn TelnetPrompt.main(self: TelnetPrompt) -> IoResult<Unit>
fn TelnetPrompt.machine(self: TelnetPrompt, id: Text) -> Machine
fn TelnetPrompt.validate(self: TelnetPrompt) -> List<Text>
```

`TelnetPrompt` probes a running Telnet server: every `interval` it connects, waits up to
`timeout` for `expected_prompt`, and reports each attempt on its output signal `attempts`.

- `main` runs one attempt. It resolves `target` with `Endpoint.of`, connects, reads through
  the option negotiation until the prompt, measures the latency, closes if asked, and prints
  one line: the [`ProbeAttempt`](sli.md) in the model encoding. A refused connection, a
  stream closed before the prompt and a read past `timeout` are bad attempts
  (`received = false`) and exit 0. A target with no `--ref` binding, or a failed
  `set_idle_timeout` or write of the line, ends `main` with the error: exit 1
  (exit 2 for a malformed `--ref`).
- `machine` gives `ignite run` the probe scheduler: one attempt per `interval` from the
  target's first readiness, each with a deadline of `timeout`.
- `validate` refuses an empty `expected_prompt` (E120). The timing rules (positive `interval`
  and `timeout`, `timeout < interval`) are the compiler's E123.

In a `system`, the three kinds look like this (from
[`terminal.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.ignite)
and the full declaration
[`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite)):

```ignite,fragment
component server: TelnetServer {
  listener = ingress
  application = console
  executable = "terminald"
}

component client: TelnetClient {
  target = ingress
  executable = "tn"
}

component promptProbe: TelnetPrompt {
  target = server
  interval = 10 seconds
  timeout = 2 seconds
  expected_prompt = "terminal> "
  close_after_probe = true
}
```

## Functions

```ignite,fragment
fn await_prompt(stream: TcpStream, expected_prompt: Text) -> IoResult<Text>
fn connect(stream: TcpStream) -> IoResult<Unit>
fn decode(state: Decoder, b: Byte) -> (Decoder, Event)
fn decode_within(state: Decoder, b: Byte, maximum: Int) -> (Decoder, Event)
fn render(reply: Reply) -> Bytes
fn encode_input(bytes: Bytes) -> Bytes
fn encode_attempt(attempt: ProbeAttempt) -> Text
```

| function | what it does |
|---|---|
| `await_prompt` | the probe's side of a session: reads the server's bytes, answers option negotiation as `connect` does, and returns the text before `expected_prompt` once the text ends with it; `Err` when the server closes first, on an overlong subnegotiation, or when a read fails |
| `connect` | the client role for a `TcpStream`, the handler a [`std.cli`](cli.md) command names with `module("std.protocol.telnet")` |
| `decode` | one byte of the incoming stream: the decoder's next state and the byte's event; subnegotiation is skipped up to `MAXIMUM_SUBNEGOTIATION` bytes |
| `decode_within` | `decode` with the subnegotiation bound `maximum`, counting the IAC of IAC SE; one byte more is `SubnegotiationTooLong` |
| `render` | each reply line followed by CR LF |
| `encode_input` | standard input as NVT data: LF becomes CR LF, a CR before LF is dropped, any other CR becomes CR NUL, the data byte 255 is doubled |
| `encode_attempt` | the model encoding of a probe attempt, one line, as the probe prints it |

The decoder's types:

```ignite,fragment
enum Decoder {
  Ground,
  CarriageReturn,
  Interpret,
  Negotiation(Byte),
  Subnegotiation(Int),
  SubnegotiationInterpret(Int)
}

enum Event {
  Nothing,
  Literal(Byte),
  EndOfLine,
  Answer(Byte, Byte),
  AreYouThere,
  Interrupt,
  SubnegotiationTooLong,
  DataMark
}
```

Constants: the command bytes `IAC` (255), `DONT`, `DO`, `WONT`, `WILL`, `SB`, `GA`, `AYT`, `IP`,
`DM`, `SE`, and `MAXIMUM_SUBNEGOTIATION` (1024), the bound of the client and the probe.

## Examples

The decoder and the encoders are pure, so they can be checked byte by byte:

```ignite
import std.protocol.telnet.*
import std.console.Reply

test "the decoder answers negotiation and finds line ends" {
  assert decode(Ground, b'x') == (Ground, Literal(b'x'))
  assert decode(Ground, b'\r') == (CarriageReturn, EndOfLine)
  assert decode(CarriageReturn, b'\n') == (Ground, Nothing)
  assert decode(Ground, IAC) == (Interpret, Nothing)
  assert decode(Interpret, IAC) == (Ground, Literal(IAC))
  assert decode(Interpret, DO) == (Negotiation(DO), Nothing)
  assert decode(Negotiation(DO), 1) == (Ground, Answer(WONT, 1))
}

test "replies and input in NVT form" {
  assert render(Reply { lines = ["a", "", "é"] }) == "a\r\n\r\né\r\n".to_bytes()
  assert encode_input(b"a\nb") == b"a\r\nb"
  assert encode_input(b"a\rb") == b"a\r\0b"
  assert encode_input([1, IAC, 2]) == [1, IAC, IAC, 2]
}
```

A whole session over a pipe, as the module's own tests run it:

```ignite
import std.protocol.telnet.{Telnet, SessionLimits}
import std.console.{Console, Reply, Action}

// A console that echoes each line and closes on `bye`.
fn echo(line: Text) -> Reply = if line == "bye" {
  Reply { lines = ["bye"], action = Close }
} else {
  Reply { lines = [line] }
}

// Everything the server sends after `input`, until it closes the stream.
test fn exchange(input: Bytes) -> Result<Bytes, IoError> {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn Telnet.serve(server, SessionLimits {}, Console { prompt = "> ", respond = echo, maximum_line = 8 })
  client.write_all(input)?
  var out = Bytes.empty()
  loop {
    match client.read(4096)? {
      Eof => return Ok(out)
      Data(bytes) => out.append(bytes)
    }
  }
}

test "the server answers each line and ends on the console's close" {
  assert exchange(b"hi\r\n\r\nbye\r\nlost\r\n")? == b"> \xff\xf9hi\r\n> \xff\xf9\r\n> \xff\xf9bye\r\n"
  assert exchange(b"12345678\r\n123456789")? == b"> \xff\xf912345678\r\n> \xff\xf9line too long\r\n"
}
```

The [terminal walkthrough](../../guide/terminal.md) builds and runs `terminald` and `tn`.
