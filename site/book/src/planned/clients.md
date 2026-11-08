# Protocols and clients: WebSocket, RESP3, NATS <span class="badge design">design</span>

Planned: three more protocols in the standard library, written in Ignite like Telnet and HTTP.
Each runs over any byte stream, so it is tested over an in-memory pipe without a network.
Nothing on this page is implemented. The stories fix what each protocol must do. The function
names below are illustrative and do not compile today. The stories are
[`std-websocket`](https://github.com/ignite-lang/ignite/blob/main/.engineering/planning/story/std-websocket.md),
[`std-resp-client`](https://github.com/ignite-lang/ignite/blob/main/.engineering/planning/story/std-resp-client.md)
and
[`std-nats-client`](https://github.com/ignite-lang/ignite/blob/main/.engineering/planning/story/std-nats-client.md);
the protocol layering they follow is in
[`docs/protocols.md`](https://github.com/ignite-lang/ignite/blob/main/docs/protocols.md).

## WebSocket (RFC 6455)

`std.protocol.websocket` has server and client roles. The server is reached through the HTTP
upgrade of `std.protocol.http`. The role handles framing, fragmentation, masking, ping and pong,
close codes and size limits. The program sees whole messages:

```ignite,fragment
fn echo(ws: WebSocket) -> IoResult<Unit> {
  var open = true
  while open {
    match ws.receive()? {
      Text(t) => ws.send(Text(t))?
      Binary(b) => ws.send(Binary(b))?
      Close(code) => {
        ws.close(code)?
        open = false
      }
    }
  }
  Ok(())
}
```

Like Telnet and HTTP, the protocol declares which requirements of its RFC it meets
(`conforms [RFC6455]`), and `ignite conformance` reports whether the claim holds, gaps
included.

## RESP3 with Sentinel

`std.protocol.resp` is a client for Redis-compatible servers: every RESP2 and RESP3 type,
typed commands, pipelining, pub/sub, and Sentinel to find the current primary:

```ignite,fragment
let redis = Resp.connect(sentinel_primary(sentinels, "sessions")?, RespLimits {})?
redis.set("session:{id}", token, expire = 30 minutes)?
let replies = redis.pipeline([get("session:{id}"), incr("hits")])?
let events = redis.subscribe(["events"])?
```

## NATS

`std.protocol.nats` is a NATS core client: connect with user and password or a token, publish,
subscribe with wildcards and queue groups, request and reply with a deadline, keepalive:

```ignite,fragment
let nats = Nats.connect(stream, Auth.token(token), NatsLimits {})?
nats.publish("orders.created", body)?
let work = nats.subscribe("orders.*", queue = "workers")?
let price = nats.request("prices.get", body, deadline = 2 seconds)?
```

## Tested without a server

Each protocol's tests run over `Pipe.pair()` against a scripted peer, on both targets. One test
per protocol talks to a real server when one is installed and is skipped elsewhere.

## Plan

All three are planned for wave 8, after the bitwise operators that WebSocket's masking and
SHA-1 need, and after the channels and timers that request deadlines use (both in wave 7). The
[roadmap](../roadmap.md) shows the waves.
