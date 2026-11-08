//! The reactor contract (`docs/readiness-io.md` 3.4, `ignite_io::reactor`)
//! run against every implementation: `EpollReactor`, `PollReactor` and the
//! test double `ScriptedReactor`, which delivers the readiness a test
//! scripts without a kernel descriptor. That the double implements the
//! trait from outside the crate, with nothing but the public waiter, is the
//! proof that the seam carries no epoll type.
//!
//! Every case parks a thread waiter, the thread-per-task implementation of
//! the waiter seam, so the suite also runs over it.

use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use ignite_io::reactor::{EpollReactor, Event, Interest, PollReactor, Reactor, Token};
use ignite_io::wait::{WaitList, Waiter, Woken, park, park_in, wake};

const LIMIT: Duration = Duration::from_secs(5);
const SHORT: Duration = Duration::from_millis(50);

/// A reactor under test and one descriptor it can make readable.
trait Harness: Send + Sync + 'static {
    type R: Reactor + 'static;
    fn reactor(&self) -> &Self::R;
    fn descriptor(&self) -> RawFd;
    fn make_readable(&self);
}

/// A kernel reactor over one end of a socket pair; writing to the other
/// end makes it readable.
struct Kernel<R> {
    reactor: R,
    watched: UnixStream,
    peer: Mutex<UnixStream>,
}

impl<R: Reactor + 'static> Kernel<R> {
    fn new(reactor: R) -> Kernel<R> {
        let (watched, peer) = UnixStream::pair().unwrap();
        Kernel {
            reactor,
            watched,
            peer: Mutex::new(peer),
        }
    }
}

impl<R: Reactor + 'static> Harness for Kernel<R> {
    type R = R;
    fn reactor(&self) -> &R {
        &self.reactor
    }
    fn descriptor(&self) -> RawFd {
        self.watched.as_raw_fd()
    }
    fn make_readable(&self) {
        self.peer.lock().unwrap().write_all(b"x").unwrap();
    }
}

/// A descriptor number that is no open descriptor of the process: the
/// double never hands it to the kernel.
const SCRIPTED_DESCRIPTOR: RawFd = 1_000_000;

struct Scripted {
    reactor: ScriptedReactor,
}

impl Harness for Scripted {
    type R = ScriptedReactor;
    fn reactor(&self) -> &ScriptedReactor {
        &self.reactor
    }
    fn descriptor(&self) -> RawFd {
        SCRIPTED_DESCRIPTOR
    }
    fn make_readable(&self) {
        self.reactor.ready(SCRIPTED_DESCRIPTOR, Interest::READ);
    }
}

/// The test double: readiness is what the test scripts with
/// [`ScriptedReactor::ready`]; a wait blocks through the public waiter.
#[derive(Default)]
pub struct ScriptedReactor {
    state: Mutex<Script>,
}

#[derive(Default)]
struct Script {
    next: u64,
    registered: Vec<(Token, RawFd, Interest, Waiter)>,
    /// Readiness scripted and not yet delivered.
    pending: Vec<(RawFd, Interest)>,
    woken: bool,
    waiting: WaitList,
}

impl ScriptedReactor {
    /// Script `descriptor` ready for `ready`, as the kernel would report it.
    pub fn ready(&self, descriptor: RawFd, ready: Interest) {
        let mut script = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        script.pending.push((descriptor, ready));
        script.waiting.wake_all();
    }
}

impl Reactor for ScriptedReactor {
    fn register(
        &self,
        descriptor: RawFd,
        interest: Interest,
        waiter: Waiter,
    ) -> std::io::Result<Token> {
        let mut script = self.state.lock().unwrap();
        script.next += 1;
        let token = Token(script.next);
        script
            .registered
            .push((token, descriptor, interest, waiter));
        Ok(token)
    }

    fn deregister(&self, token: Token) -> std::io::Result<()> {
        let mut script = self.state.lock().unwrap();
        script.registered.retain(|(known, ..)| *known != token);
        Ok(())
    }

    fn wait(&self, deadline: Option<Instant>, events: &mut Vec<Event>) -> std::io::Result<()> {
        let mut script = self.state.lock().unwrap();
        loop {
            let pending = std::mem::take(&mut script.pending);
            for (descriptor, ready) in pending {
                for (token, known, interest, waiter) in &script.registered {
                    if *known == descriptor && interest.intersects(ready) {
                        events.push(Event {
                            token: *token,
                            waiter: waiter.clone(),
                            ready,
                        });
                    }
                }
            }
            if !events.is_empty() || std::mem::take(&mut script.woken) {
                return Ok(());
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Ok(());
            }
            script = park_in(&self.state, script, |s| &mut s.waiting, deadline).0;
        }
    }

    fn wake(&self) -> std::io::Result<()> {
        let mut script = self.state.lock().unwrap();
        script.woken = true;
        script.waiting.wake_all();
        Ok(())
    }
}

/// Wait on `reactor` until an event arrives or [`LIMIT`] passes.
fn events_within<R: Reactor>(reactor: &R) -> Vec<Event> {
    let deadline = Instant::now() + LIMIT;
    let mut events = Vec::new();
    while events.is_empty() && Instant::now() < deadline {
        reactor.wait(Some(deadline), &mut events).unwrap();
    }
    events
}

fn readiness_wakes_a_parked_waiter<H: Harness>(harness: H) {
    let (send, waiter) = mpsc::channel();
    let (done, parked) = mpsc::channel();
    std::thread::spawn(move || {
        let me = Waiter::current();
        send.send(me.clone()).unwrap();
        done.send(park(&me, Some(Instant::now() + LIMIT))).unwrap();
    });
    let waiter = waiter.recv_timeout(LIMIT).unwrap();
    let token = harness
        .reactor()
        .register(harness.descriptor(), Interest::READ, waiter.clone())
        .unwrap();
    harness.make_readable();
    let events = events_within(harness.reactor());
    let event = events
        .iter()
        .find(|event| event.token == token)
        .expect("an event for the registered descriptor");
    assert!(event.ready.contains(Interest::READ), "{:?}", event.ready);
    assert!(event.waiter.same(&waiter), "the event names the waiter");
    wake(&event.waiter);
    assert_eq!(parked.recv_timeout(LIMIT).unwrap(), Woken::Notified);
    harness.reactor().deregister(token).unwrap();
}

fn a_deadline_expires<H: Harness>(harness: H) {
    let token = harness
        .reactor()
        .register(harness.descriptor(), Interest::READ, Waiter::current())
        .unwrap();
    let start = Instant::now();
    let mut events = Vec::new();
    harness
        .reactor()
        .wait(Some(start + SHORT), &mut events)
        .unwrap();
    let waited = start.elapsed();
    assert!(events.is_empty(), "nothing was ready: {events:?}");
    assert!(waited >= SHORT, "returned after {waited:?}");
    assert!(waited < LIMIT, "returned after {waited:?}");
    harness.reactor().deregister(token).unwrap();
}

fn a_cross_thread_wake_ends_a_wait<H: Harness>(harness: H) {
    let harness = Arc::new(harness);
    let waiting = Arc::clone(&harness);
    let (done, returned) = mpsc::channel();
    std::thread::spawn(move || {
        let mut events = Vec::new();
        waiting.reactor().wait(None, &mut events).unwrap();
        done.send(events.len()).unwrap();
    });
    std::thread::sleep(SHORT);
    harness.reactor().wake().unwrap();
    assert_eq!(
        returned.recv_timeout(LIMIT).unwrap(),
        0,
        "a wake is no event"
    );

    harness.reactor().wake().unwrap();
    let start = Instant::now();
    let mut events = Vec::new();
    harness
        .reactor()
        .wait(Some(start + LIMIT), &mut events)
        .unwrap();
    assert!(
        start.elapsed() < LIMIT,
        "a wake before the wait is not lost"
    );
    assert!(events.is_empty());
}

fn deregistration_stops_events<H: Harness>(harness: H) {
    let token = harness
        .reactor()
        .register(harness.descriptor(), Interest::READ, Waiter::current())
        .unwrap();
    harness.reactor().deregister(token).unwrap();
    harness.make_readable();
    let mut events = Vec::new();
    harness
        .reactor()
        .wait(Some(Instant::now() + SHORT), &mut events)
        .unwrap();
    assert!(
        events.iter().all(|event| event.token != token),
        "an event after deregistration: {events:?}"
    );
}

macro_rules! contract {
    ($name:ident, $harness:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn readiness_wakes_a_parked_waiter() {
                super::readiness_wakes_a_parked_waiter($harness);
            }

            #[test]
            fn a_deadline_expires() {
                super::a_deadline_expires($harness);
            }

            #[test]
            fn a_cross_thread_wake_ends_a_wait() {
                super::a_cross_thread_wake_ends_a_wait($harness);
            }

            #[test]
            fn deregistration_stops_events() {
                super::deregistration_stops_events($harness);
            }
        }
    };
}

contract!(epoll, Kernel::new(EpollReactor::new().unwrap()));
contract!(poll, Kernel::new(PollReactor::new().unwrap()));
contract!(
    scripted,
    Scripted {
        reactor: ScriptedReactor::default()
    }
);
