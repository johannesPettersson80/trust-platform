//! Bounded queues under concurrent producers (specification 11, the runtime event buffer and the
//! control I/O and variable mutation contract; specification 34 section 4.3.1). Each queue keeps
//! its declared bound and drop policy, its dropped count is exact, and no producer or reply waits
//! forever.
//!
//! The control command channel is not declared bounded by the specification, so for it these tests
//! check only that every command gets exactly one reply.

mod concurrency_support;

use std::sync::mpsc;
use std::thread;

use smol_str::SmolStr;
use trust_runtime::debug::{DebugControl, ForcedVarTarget, RequestQueueFull, RuntimeEvent};
use trust_runtime::io::IoAddress;
use trust_runtime::scheduler::{
    ResourceCommand, ResourceHandle, ResourceRunner, ResourceState, StdClock,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RuntimeBuilder;

use concurrency_support::{stop_and_join, wait_until, WAIT};

const PRODUCERS: usize = 8;
/// Events per producer for the event flood: 2,400 in all, more than the buffer keeps.
const EVENTS: usize = 300;
/// Requests per producer for the queue floods: 1,600 in all, more than each queue keeps.
const REQUESTS: usize = 200;
/// The bound of the runtime event buffer and of each request queue (specification 11).
const CAPACITY: usize = 1_024;

/// Spawns an empty resource with debugging enabled and returns its debug handle.
fn spawn_idle(name: &str) -> (ResourceHandle<StdClock>, DebugControl) {
    let mut runtime = RuntimeBuilder::new()
        .build()
        .expect("install declared test runtime");
    let debug = runtime.enable_debug();
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .spawn(name)
        .expect("spawn resource");
    (handle, debug)
}

/// Pushes `EVENTS` runtime events from each of `PRODUCERS` threads. Each event names its producer
/// and its sequence number.
fn flood_events(debug: &DebugControl) {
    thread::scope(|scope| {
        for producer in 0..PRODUCERS {
            let debug = debug.clone();
            scope.spawn(move || {
                for sequence in 0..EVENTS {
                    debug.push_runtime_event(RuntimeEvent::WriteRejected {
                        target: SmolStr::new(format!("p{producer}-{sequence}")),
                        error: String::from("flood"),
                        time: Duration::ZERO,
                    });
                }
            });
        }
    });
}

/// The producer and sequence number that a flood event names.
fn flood_position(event: &RuntimeEvent) -> (usize, usize) {
    let RuntimeEvent::WriteRejected { target, .. } = event else {
        panic!("only flood events are buffered, found {event:?}");
    };
    let body = target.strip_prefix('p').expect("flood target prefix");
    let (producer, sequence) = body.split_once('-').expect("flood target separator");
    (
        producer.parse::<usize>().expect("producer index"),
        sequence.parse::<usize>().expect("sequence number"),
    )
}

/// Checks that the buffer kept the newest events. A drop always takes the oldest event first, so
/// each producer keeps its last sequence numbers, in order.
fn assert_newest_kept(events: &[RuntimeEvent]) {
    let mut kept = vec![Vec::new(); PRODUCERS];
    for event in events {
        let (producer, sequence) = flood_position(event);
        kept[producer].push(sequence);
    }
    for (producer, sequences) in kept.iter().enumerate() {
        let first = EVENTS - sequences.len();
        for (offset, sequence) in sequences.iter().enumerate() {
            assert_eq!(
                *sequence,
                first + offset,
                "producer {producer} keeps its newest events in order"
            );
        }
    }
}

/// Makes `PRODUCERS * REQUESTS` attempts from concurrent threads. Returns the accepted attempts and
/// the number that were refused.
fn flood_requests<F>(attempt: F) -> (Vec<(usize, usize)>, usize)
where
    F: Fn(usize, usize) -> Result<(), RequestQueueFull> + Sync,
{
    let attempt = &attempt;
    let outcomes: Vec<Vec<((usize, usize), bool)>> = thread::scope(|scope| {
        let producers: Vec<_> = (0..PRODUCERS)
            .map(|producer| {
                scope.spawn(move || {
                    (0..REQUESTS)
                        .map(|request| ((producer, request), attempt(producer, request).is_ok()))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        producers
            .into_iter()
            .map(|producer| producer.join().expect("producer thread"))
            .collect()
    });
    let mut accepted = Vec::new();
    let mut refused = 0;
    for ((producer, request), ok) in outcomes.into_iter().flatten() {
        if ok {
            accepted.push((producer, request));
        } else {
            refused += 1;
        }
    }
    (accepted, refused)
}

#[test]
fn concurrent_producers_keep_the_newest_1024_events_and_count_every_drop() {
    let debug = DebugControl::new();
    flood_events(&debug);

    let dropped = debug.runtime_events_dropped();
    let kept = debug.drain_runtime_events();
    assert_eq!(kept.len(), CAPACITY, "the event buffer is bounded at 1,024");
    assert_eq!(
        dropped,
        (PRODUCERS * EVENTS - CAPACITY) as u64,
        "every event beyond the bound is counted as dropped"
    );
    assert_newest_kept(&kept);
    assert_eq!(
        debug.runtime_events_dropped(),
        dropped,
        "draining does not reset the dropped count"
    );
}

#[test]
fn a_flood_while_the_scan_is_paused_keeps_exactly_the_newest_events() {
    let (handle, debug) = spawn_idle("saturation-events");
    let control = handle.control();
    control.pause().expect("pause is queued");
    wait_until("the scan pauses", || {
        control.state() == ResourceState::Paused
    });

    // A paused scan runs no cycle, so after this drain only the flood reaches the buffer.
    let _ = debug.drain_runtime_events();
    let before = debug.runtime_events_dropped();
    flood_events(&debug);

    assert_eq!(
        debug.runtime_events_dropped() - before,
        (PRODUCERS * EVENTS - CAPACITY) as u64,
        "the flood drops exactly the events beyond the bound"
    );
    let kept = debug.drain_runtime_events();
    assert_eq!(kept.len(), CAPACITY);
    assert_newest_kept(&kept);
    stop_and_join(handle);
}

#[test]
fn request_queues_accept_exactly_1024_entries_under_concurrent_producers() {
    let (handle, debug) = spawn_idle("saturation-queues");
    let control = handle.control();
    control.pause().expect("pause is queued");
    wait_until("the scan pauses", || {
        control.state() == ResourceState::Paused
    });

    // I/O writes are an append-only queue, so duplicates are distinct requests.
    let (accepted, refused) = flood_requests(|producer, request| {
        let address = IoAddress::parse("%MW0").expect("memory word address");
        debug.try_enqueue_io_write(address, Value::LInt((producer * REQUESTS + request) as i64))
    });
    assert_eq!(
        accepted.len(),
        CAPACITY,
        "the I/O write queue accepts exactly its bound"
    );
    assert_eq!(
        refused,
        PRODUCERS * REQUESTS - CAPACITY,
        "every I/O write past the bound is refused"
    );
    assert_eq!(
        debug.drain_io_writes().len(),
        CAPACITY,
        "a refused I/O write queues nothing"
    );

    // Forces: a new target past the bound is refused, and a forced target is replaced in place.
    let (forced, refused) = flood_requests(|producer, request| {
        debug.try_force_global(
            format!("t{producer}_{request}"),
            Value::DInt(request as i32),
        )
    });
    assert_eq!(
        forced.len(),
        CAPACITY,
        "the force table accepts exactly its bound"
    );
    assert_eq!(refused, PRODUCERS * REQUESTS - CAPACITY);
    let (producer, request) = forced[0];
    let target = ForcedVarTarget::Global(SmolStr::new(format!("t{producer}_{request}")));
    debug
        .try_force_global(format!("t{producer}_{request}"), Value::DInt(9))
        .expect("re-forcing an accepted target is never refused");
    assert_eq!(
        debug.forced_var_value(&target),
        Some(Value::DInt(9)),
        "re-forcing replaces the value in place"
    );
    assert_eq!(
        debug.try_force_global("fresh", Value::DInt(1)),
        Err(RequestQueueFull),
        "a new force past the bound is refused"
    );

    // Variable writes coalesce per target, so only a new target counts against the bound.
    let (written, refused) = flood_requests(|producer, request| {
        debug.try_enqueue_global_write(
            format!("v{producer}_{request}"),
            Value::DInt(request as i32),
        )
    });
    assert_eq!(
        written.len(),
        CAPACITY,
        "the variable write queue accepts exactly its bound"
    );
    assert_eq!(refused, PRODUCERS * REQUESTS - CAPACITY);
    let (producer, request) = written[0];
    debug
        .try_enqueue_global_write(format!("v{producer}_{request}"), Value::DInt(5))
        .expect("a pending target is replaced in place, never refused");
    assert_eq!(
        debug.try_enqueue_global_write("fresh", Value::DInt(1)),
        Err(RequestQueueFull),
        "a new variable write past the bound is refused"
    );
    stop_and_join(handle);
}

#[test]
fn a_command_flood_gets_exactly_one_reply_per_command() {
    let (handle, _debug) = spawn_idle("saturation-commands");
    let control = handle.control();

    let mut clients = Vec::new();
    for _ in 0..PRODUCERS {
        let control = control.clone();
        clients.push(thread::spawn(move || {
            for _ in 0..REQUESTS {
                let (tx, rx) = mpsc::channel();
                control
                    .send_command(ResourceCommand::MeshSnapshot {
                        names: Vec::new(),
                        respond_to: tx,
                    })
                    .expect("command channel open");
                rx.recv_timeout(WAIT)
                    .expect("every command is answered within the bound");
                // The scan thread drops the reply's sender after it replies, so no second reply
                // can follow.
                match rx.recv_timeout(WAIT) {
                    Err(mpsc::RecvTimeoutError::Disconnected) => {}
                    other => panic!("a command gets exactly one reply, found {other:?}"),
                }
            }
            REQUESTS
        }));
    }
    let answered: usize = clients
        .into_iter()
        .map(|client| client.join().expect("client thread"))
        .sum();
    assert_eq!(answered, PRODUCERS * REQUESTS);
    stop_and_join(handle);
}
