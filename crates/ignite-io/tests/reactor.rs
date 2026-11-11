//! The reactor contract (`docs/readiness-io.md` 3.4, `ignite_io::reactor`)
//! run against every implementation: `EpollReactor`, `PollReactor` and the
//! test double `ScriptedReactor`, which delivers the readiness a test
//! scripts without asking the kernel about any descriptor. That the double
//! implements the trait from outside the crate, with nothing but the public
//! waiter, is the proof that the seam carries no epoll type.
//!
//! Every case parks a thread waiter, the thread-per-task implementation of
//! the waiter seam, so the suite also runs over it.

use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
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
    fn descriptor(&self) -> BorrowedFd<'_>;
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
    fn descriptor(&self) -> BorrowedFd<'_> {
        self.watched.as_fd()
    }
    fn make_readable(&self) {
        self.peer.lock().unwrap().write_all(b"x").unwrap();
    }
}

/// The double and the descriptor the harness lends it: one end of a
/// socket pair with a byte waiting, which the kernel reports readable from
/// the start. A double that asked the kernel would report it in
/// `a_deadline_expires` and after `deregistration_stops_events`; only the
/// script makes it readable for the double.
struct Scripted {
    reactor: ScriptedReactor,
    lent: UnixStream,
    _peer: UnixStream,
}

impl Scripted {
    fn new() -> Scripted {
        let (lent, mut peer) = UnixStream::pair().unwrap();
        peer.write_all(b"x").unwrap();
        Scripted {
            reactor: ScriptedReactor::default(),
            lent,
            _peer: peer,
        }
    }
}

impl Harness for Scripted {
    type R = ScriptedReactor;
    fn reactor(&self) -> &ScriptedReactor {
        &self.reactor
    }
    fn descriptor(&self) -> BorrowedFd<'_> {
        self.lent.as_fd()
    }
    fn make_readable(&self) {
        self.reactor.ready(self.lent.as_raw_fd(), Interest::READ);
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
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        waiter: Waiter,
    ) -> std::io::Result<Token> {
        let mut script = self.state.lock().unwrap();
        script.next += 1;
        let token = Token(script.next);
        script
            .registered
            .push((token, descriptor.as_raw_fd(), interest, waiter));
        Ok(token)
    }

    fn deregister(&self, token: Token, descriptor: BorrowedFd<'_>) -> std::io::Result<()> {
        let mut script = self.state.lock().unwrap();
        let Some(at) = script
            .registered
            .iter()
            .position(|(known, ..)| *known == token)
        else {
            return Ok(());
        };
        let (_, registered, _, _) = script.registered.remove(at);
        if registered != descriptor.as_raw_fd() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "deregistered with another descriptor than the registered one",
            ));
        }
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
    harness
        .reactor()
        .deregister(token, harness.descriptor())
        .unwrap();
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
    harness
        .reactor()
        .deregister(token, harness.descriptor())
        .unwrap();
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
    harness
        .reactor()
        .deregister(token, harness.descriptor())
        .unwrap();
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

/// A deregistration names the descriptor it is for: another one than the
/// registered one is refused, and the token gives no more events.
fn another_descriptor_is_refused<H: Harness>(harness: H) {
    let token = harness
        .reactor()
        .register(harness.descriptor(), Interest::READ, Waiter::current())
        .unwrap();
    let (other, _peer) = UnixStream::pair().unwrap();
    let refused = harness
        .reactor()
        .deregister(token, other.as_fd())
        .expect_err("another descriptor deregistered");
    assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput);
    harness.make_readable();
    let mut events = Vec::new();
    harness
        .reactor()
        .wait(Some(Instant::now() + SHORT), &mut events)
        .unwrap();
    assert!(
        events.iter().all(|event| event.token != token),
        "an event after the refused deregistration: {events:?}"
    );
}

/// A registration after a deregistration has a token of its own: the
/// earlier token, deregistered again, leaves the new registration in place
/// (an `EpollReactor` reuses the slot of an ended registration, story
/// `binary-size-back`).
fn a_reused_registration_has_a_token_of_its_own<H: Harness>(harness: H) {
    let reactor = harness.reactor();
    let first = reactor
        .register(harness.descriptor(), Interest::READ, Waiter::current())
        .unwrap();
    reactor.deregister(first, harness.descriptor()).unwrap();
    let second = reactor
        .register(harness.descriptor(), Interest::READ, Waiter::current())
        .unwrap();
    assert_ne!(first, second, "a reused registration got the earlier token");
    reactor.deregister(first, harness.descriptor()).unwrap();
    harness.make_readable();
    let events = events_within(reactor);
    assert!(
        events.iter().any(|event| event.token == second),
        "the earlier token ended the new registration: {events:?}"
    );
    reactor.deregister(second, harness.descriptor()).unwrap();
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

            #[test]
            fn another_descriptor_is_refused() {
                super::another_descriptor_is_refused($harness);
            }

            #[test]
            fn a_reused_registration_has_a_token_of_its_own() {
                super::a_reused_registration_has_a_token_of_its_own($harness);
            }
        }
    };
}

contract!(epoll, Kernel::new(EpollReactor::new().unwrap()));
contract!(poll, Kernel::new(PollReactor::new().unwrap()));
contract!(scripted, Scripted::new());
