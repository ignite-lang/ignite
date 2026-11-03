# R5 security review of the Telos compiler (read-only)

Scope: crates/telos-rt/src (unsafe), crates/telos-io/src (net, task), crates/telos-llvm/src, crates/telos-rustgen/src, crates/telos/src (driver), std/protocol/telnet.telos, std/json.telos, and the HTTP draft t3-drafts/http.telos.draft. The tree was being edited while I read; line numbers are as read on 2026-11-01.
Method: static reading and grep only. Nothing was built, run or edited. "confirmed" means the path was traced in the code end to end; "suspected" means one step was not run or depends on a deployment fact.
Counts: high 3, medium 4, low 3.

1. [high, confirmed] A failed accept ends the whole service, and a peer can cause the failure.
   where: crates/telos-llvm/src/lib.rs:368-370 (LLVM service accept loop); crates/telos-rustgen/src/lib.rs:2070 (Rust target); crates/telos-io/src/net.rs:163-172 (accept retries only Interrupted, no backoff)
   quote (LLVM): "call i32 @telos_rt_tcp_accept(i64 {listener}, ptr {stream_slot}, ptr {err})" is followed by fail_unless_ok(&mut l, &status, &err, 1);
   quote (Rust): let stream = {resource}.accept()?;
   scenario: a peer opens connections until the process reaches its descriptor limit (EMFILE). The next accept fails; the LLVM loop exits with status 1 and the Rust main returns the error through the question mark. Every session dies and nothing restarts the service. There is no connection cap (see 2 and 3).

2. [high, confirmed] A failed thread start in the accept loop panics the process, and a peer can cause it.
   where: crates/telos-io/src/task.rs:57-58 (spawn); crates/telos-rt/src/task.rs:76 and 86 (telos_rt_spawn_copy calls telos_io::spawn); the generated accept loop calls spawn_copy once per accepted connection
   quote: panic!("cannot start a task: {error}");
   scenario: a peer opens many connections and holds them open. Each gets its own OS thread; LINGER reuse helps only after a task ends. When the thread or pid limit is reached, Builder::spawn fails, the panic goes through the hook installed by telos-rt process init, die() exits with 101, and every session ends. No thread or connection cap exists in telos-io or telos-rt (grep).

3. [high, confirmed] No read timeout anywhere: a silent peer holds a thread and a descriptor indefinitely (slowloris).
   where: crates/telos-io/src/net.rs:216-227 (read_into, blocking, no deadline); std/protocol/telnet.telos:348 (session read loop); crates/telos-llvm/src/lower.rs:1974-1984 (set_idle_timeout arm)
   quote: match stream.read(4096)? {
   scenario: a client connects and sends nothing, or one byte every 29 s. The task blocks in read_into forever, holding its thread and descriptor. With no cap (findings 1 and 2) enough such connections exhaust descriptors or threads. The only timeout in the language, TcpStream.set_idle_timeout, is typed (crates/telos-types/src/builtins.rs:53-54) and effect-checked (crates/telos-check/src/lib.rs:251), but nothing implements it: lower.rs:1977 sends TcpStreamSetIdleTimeout to unimplemented! at line 1984, telos-rustgen never mentions it (grep), and telos-io and telos-rt have no timeout code. docs/slice.md:1028 (section 11.5) defines a read that waits longer than the timeout as ending with Eof. The HTTP draft calls it at line 49 and so cannot build.

4. [medium, confirmed] HTTP draft: a tab before the colon defeats the Transfer-Encoding check.
   where: t3-drafts/http.telos.draft:154 (name check), 109-110 (exact-match header lookup), 217-220 (Transfer-Encoding rejection)
   quote: Some((name, value)) => if name.is_empty() || name.contains(" ") {
   scenario: the header line "Transfer-Encoding<TAB>: chunked" passes parse_head, since only a space is rejected. Its key becomes "transfer-encoding<TAB>", which never equals "transfer-encoding", so the 400 at line 217 never fires and the body is framed by Content-Length alone. Behind a front end that trims the tab and honours chunked, the chunked body becomes a second request on the same keep-alive connection. Medium because the draft server is standalone; it becomes high if deployed behind such a proxy. The front-end behaviour is suspected, not verified.

5. [medium, confirmed] HTTP draft: a duplicate Content-Length is framed by its first value.
   where: t3-drafts/http.telos.draft:106-114 (header() returns the first match), 221-227 (Content-Length read)
   quote: if key == name {  /  return Some(value)
   scenario: "Content-Length: 5" followed by "Content-Length: 50" is framed as 5. A front end that uses the last value frames the body as 50, so the bytes after 5 are read as the next request. RFC 9112 section 6.3 requires rejecting differing repeated Content-Length values with 400. The smuggling needs a front end (suspected for that part).

6. [medium, confirmed] Int.parse and set_idle_timeout pass check and typing, then abort the LLVM build; the Rust target has no lowering.
   where: crates/telos-llvm/src/lower.rs:1974 and 1977 (arm), 1984 (unimplemented!); crates/telos-types/src/builtins.rs:110 (Int.parse typed)
   quote: unimplemented!("slice 7: builtin {b:?} in the LLVM target")
   scenario: a program that calls Int.parse(text) or set_idle_timeout(ms) passes telos-check and telos-types, then the LLVM target panics at compile time with no diagnostic. telos-rustgen never mentions either name (grep). The HTTP draft uses both, so it cannot build on either target. The arm is a slice 7 placeholder, so the defect is the gap between checker and backends. Medium because the panic is in the author own compile, not on a runtime path.

7. [medium, confirmed] A record that contains itself without indirection recurses without bound in the LLVM layout code.
   where: crates/telos-llvm/src/layout.rs:147-148 (layout), 157-161 (record_layout), 193-198 (enum_layout), 237-240 (needs_glue); no record cycle check in crates/telos-check/src/lib.rs (only const cycles, 1049) or crates/telos-types/src/typer.rs (only alias cycles, 228 and 677)
   quote: let layouts: Vec<Layout> = fields.iter().map(|t| self.layout(t)).collect();
   scenario: type Node { next: Option<Node> } is accepted by the checker. Any use of Node in a value or a function signature makes layout call record_layout(Node), which calls enum_layout(Option<Node>), which calls layout(Node) again, with no base case. The compiler aborts with a stack overflow and no diagnostic. The Rust target would instead emit an infinite-size type that cargo rejects. A List<Node> is fine, since List layout stops at 24 bytes (layout.rs:145). The brief lists crash under high; I graded this medium because the crash is in the author own compile.

8. [low, suspected] The parser has no nesting depth limit.
   where: crates/telos-syntax/src/parser.rs:1144 (fn expr, recursive descent entry); 333 (reset_nesting is error recovery, not a limit)
   quote: fn expr(&mut self) -> PResult<Expr> {
   scenario: a source file with about 100k nested parentheses or unary operators recurses once per level on the driver main thread (crates/telos/src/main.rs sets no stack size) and overflows the 8 MiB stack. Local input only; not measured.

9. [low, suspected] HTTP draft: header names and values are written with no CR/LF check.
   where: t3-drafts/http.telos.draft:92-94 (render)
   quote: head = head + "{name}: {value}\r\n"
   scenario: if an application copies request data into a response header (a redirect Location built from the query, for example), a value holding CR LF injects headers or a second response. The draft has no such call itself, so this depends on the application.

10. [low, confirmed] HTTP draft: reject paths close without reading the rest of the request.
   where: t3-drafts/http.telos.draft:232 (413 returned before read_body), 249 (Reject writes then returns), 51 (serve closes the stream)
   quote: Reject(status) => return stream.write_all(render(status_response(status), false))
   scenario: a client sends a body larger than 1 MiB; the server answers 413 and closes with unread bytes in its receive buffer. The kernel sends RST on close, and the client may never read the 413, so it sees a reset instead.

Checked, no finding:
- telos-rt unsafe code: arena alloc, trim and release (arena.rs:307-335, 386-390; DUE is 64 KiB, so 64 KiB read blocks are reclaimed at loop back-edges); handle table generation (rt net.rs:131-160); spawn handshake and spawn_copy (rt task.rs:40-91); extern C unwinding (the panic hook exits first).
- LLVM c_string and global escaping (ir.rs:57-67); list index checks (glue.rs check_index).
- Driver: executable-name gate (rustgen is_executable_name) blocks path and TOML injection; toml_string and escape_str (rustgen 421-438, 2101-2115); clang and cargo run through Command::args with no shell (llvm_build.rs:36-51; lib.rs:584-590).
- telnet subnegotiation (MAXIMUM_SUBNEGOTIATION 1024) and line caps; json.telos depth 64, digit cap 18, surrogate pairs.
- telos-io read_buffer clamp (1 to 1 MiB) and Interrupted handling on read and accept.
Not reported: generic-instance and polymorphic recursion. telos-llvm does not consume TypedProgram instances yet (grep finds no match in crates/telos-llvm/src).
