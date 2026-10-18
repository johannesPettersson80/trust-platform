#![cfg(feature = "full")]
//! Spec 34 PORT-05 and ARCH-05: the heap that a chip's control link uses. Its line link moves
//! requests and replies in its fixed storage and allocates nothing. On its 8,192-byte frames,
//! `events.tail` and `faults` with `limit = 1000` over a full event store are written event by
//! event into one buffer of the frame's size and refused at its end, under 16 KiB of heap; the
//! unbounded pipeline, as trust-runtime answers them, builds the whole reply, measured too.
//!
//! The tests live here rather than in `trust-runtime-services`, which forbids the global
//! allocator's `GlobalAlloc` implementation. A counting global allocator records the bytes live
//! on this thread while it is armed, and the most that were live at once. A reallocation counts
//! its new block before the old one is released, as a moving reallocation holds both.
//!
//! Run with `-- --nocapture` to see the peaks.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::VecDeque;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass, WallClock};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Duration;
use trust_runtime_services::access::{AccessRole, AuthFailure};
use trust_runtime_services::dispatch::{prepare_line, prepare_line_within};
use trust_runtime_services::events::{RuntimeEvent, EVENT_STORE_CAPACITY};
use trust_runtime_services::link::{DeviceLineLink, DEVICE_FRAME_LEN};
use trust_runtime_services::metrics::RuntimeMetricsSnapshot;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch, RecordsPort, ResourcePort,
    StatusPort,
};
use trust_runtime_services::protocol::{ControlRequest, ControlResponse};
use trust_runtime_services::resource::commands::CommandSendError;
use trust_runtime_services::status::{IoDriverStatus, StatusSettings};

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
}

/// Adds `change` to the bytes live on this thread while it is armed. Only const-initialized
/// `Cell`s are touched, so counting never allocates, blocks or unwinds.
fn record(change: i64) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = LIVE.try_with(|live| {
                let now = live.get().saturating_add(change);
                live.set(now);
                let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
            });
        }
    });
}

fn bytes(size: usize) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System`; the counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(bytes(layout.size()));
        // SAFETY: the caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(-bytes(layout.size()));
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(bytes(new_size));
        // SAFETY: `System` returned `pointer` for `layout`; `new_size` is the caller's new size.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        record(-bytes(layout.size()));
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// What `work` returned, and the most heap bytes live at once on this thread while it ran, above
/// those live before it.
fn heap_peak<T>(work: impl FnOnce() -> T) -> (T, i64) {
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
    ARMED.with(|armed| armed.set(true));
    let output = work();
    ARMED.with(|armed| armed.set(false));
    (output, PEAK.with(Cell::get))
}

/// A runtime whose event store is full; nothing else of it is read by the event reads.
struct FullStore {
    events: VecDeque<RuntimeEvent>,
}

impl AccessPort for FullStore {
    fn request_role(
        &self,
        _request: &ControlRequest,
        _client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        Ok(AccessRole::Admin)
    }

    fn debug_enabled(&self) -> bool {
        true
    }
}

impl AuditSink for FullStore {
    fn record_audit(&self, _record: AuditRecord<'_>) -> Option<SmolStr> {
        Some(SmolStr::new("audit-1"))
    }
}

impl EventSink for FullStore {
    fn record_event(&self, _event: RuntimeEvent) {}
}

impl WallClock for FullStore {
    fn utc_ms(&self) -> Option<u64> {
        None
    }
}

impl ResourcePort for FullStore {
    fn resource_state(&self) -> ResourceState {
        ResourceState::Running
    }

    fn last_error(&self) -> Option<RuntimeError> {
        None
    }

    fn is_hung(&self) -> bool {
        false
    }

    fn pause_resource(&self) -> Result<(), CommandSendError> {
        Ok(())
    }

    fn resume_resource(&self) -> Result<(), CommandSendError> {
        Ok(())
    }

    fn debug_control_mode(&self) -> bool {
        false
    }

    fn debugger_pause(&self) {}

    fn debugger_continue(&self) {}

    fn update_pending_restart(&self, _update: impl FnOnce(&mut Option<RestartMode>)) {}

    fn starts_when_stopped(&self) -> bool {
        true
    }
}

impl StatusPort for FullStore {
    fn resource_name(&self) -> &str {
        "RESOURCE"
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        None
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        None
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        None
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        None
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        None
    }
}

impl RecordsPort for FullStore {
    fn debug_hold(&self) -> Option<&'static str> {
        None
    }

    fn control_mode_label(&self) -> Option<String> {
        None
    }

    fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot> {
        None
    }

    fn event_capacity(&self) -> Option<usize> {
        Some(EVENT_STORE_CAPACITY)
    }

    fn read_events<R>(&self, read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R) -> Option<R> {
        Some(read(&self.events))
    }
}

impl ExtensionDispatch for FullStore {
    fn dispatch_extension(&self, _request: &ControlRequest) -> Option<ControlResponse> {
        None
    }

    fn audit_details(&self, _request: &ControlRequest) -> Option<serde_json::Value> {
        None
    }
}

/// A full store of the events the runner records for task `MainTask` every 10 ms.
fn scan_events() -> VecDeque<RuntimeEvent> {
    let name = SmolStr::new("MainTask");
    (0..EVENT_STORE_CAPACITY)
        .map(|index| {
            let cycle = u64::try_from(index / 4).expect("a small number");
            let time =
                Duration::from_millis(i64::try_from(index / 4 * 10).expect("a small number"));
            match index % 4 {
                0 => RuntimeEvent::CycleStart { cycle, time },
                1 => RuntimeEvent::TaskStart {
                    name: name.clone(),
                    priority: 1,
                    time,
                },
                2 => RuntimeEvent::TaskEnd {
                    name: name.clone(),
                    priority: 1,
                    time,
                },
                _ => RuntimeEvent::CycleEnd { cycle, time },
            }
        })
        .collect()
}

/// A full store of faults.
fn fault_events() -> VecDeque<RuntimeEvent> {
    (0..EVENT_STORE_CAPACITY)
        .map(|index| RuntimeEvent::Fault {
            error: format!("scan {index}: array index out of range"),
            time: Duration::from_millis(i64::try_from(index * 10).expect("a small number")),
        })
        .collect()
}

const SIXTEEN_KIB: i64 = 16 * 1024;

#[test]
fn a_thousand_events_on_a_chips_frame_are_refused_within_sixteen_kib_of_heap() {
    for (events, line) in [
        (
            scan_events(),
            r#"{"id":1,"type":"events.tail","params":{"limit":1000}}"#,
        ),
        (
            fault_events(),
            r#"{"id":1,"type":"faults","params":{"limit":1000}}"#,
        ),
    ] {
        let host = FullStore { events };
        let (bounded, bounded_peak) =
            heap_peak(|| prepare_line_within(line, &host, None, DEVICE_FRAME_LEN));
        let (whole, whole_peak) = heap_peak(|| prepare_line(line, &host, None));
        println!(
            "{line}: bounded peak {bounded_peak} bytes; unbounded peak {whole_peak} bytes for a \
             reply of {} bytes",
            whole.response.len()
        );
        assert_eq!(
            bounded.response,
            r#"{"id":1,"ok":false,"error":"reply longer than 8192 bytes","audit_id":"audit-1"}"#
        );
        assert!(
            whole.response.len() > DEVICE_FRAME_LEN,
            "the whole reply is longer than a frame: {} bytes",
            whole.response.len()
        );
        assert!(
            bounded_peak < SIXTEEN_KIB,
            "{line}: the bounded read peaks at {bounded_peak} bytes of heap"
        );
        assert!(
            whole_peak > 4 * SIXTEEN_KIB,
            "{line}: the unbounded read builds the whole reply, {whole_peak} bytes, so the \
             measurement would see a bounded read that did the same"
        );
    }
}

/// Moves `bytes` into the link as a receive interrupt does: what the link does not take stays with
/// the hardware, and a full ring is emptied by the loop's next poll, which takes no request then.
fn receive_all(link: &mut DeviceLineLink, bytes: &[u8], buffer: &mut [u8], now: MonotonicInstant) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let taken = link.receive(rest);
        rest = &rest[taken..];
        if taken == 0 {
            assert_eq!(link.poll_frame(buffer, now), None);
        }
    }
}

#[test]
fn a_chips_line_link_works_in_its_fixed_storage_without_allocating() {
    let mut link = Box::new(DeviceLineLink::new(PeerClass::Local));
    let mut buffer = vec![0_u8; DEVICE_FRAME_LEN];
    let request = [&[b'r'; 600][..], &b"\r\n"[..]].concat();
    let oversized = [&[b'x'; DEVICE_FRAME_LEN + 10][..], &b"\n"[..]].concat();
    let reply = vec![b'y'; DEVICE_FRAME_LEN];
    let at = |ms: u64| MonotonicInstant::from_origin(std::time::Duration::from_millis(ms));
    let ((), peak) = heap_peak(|| {
        for round in 0..10_u64 {
            let start = round * 10_000;
            receive_all(&mut link, &request, &mut buffer, at(start));
            assert_eq!(
                link.poll_frame(&mut buffer, at(start)),
                Some(LinkFrame::Frame {
                    len: 600,
                    peer: PeerClass::Local
                })
            );
            link.send_frame(&reply, at(start))
                .expect("one reply at a time, within the frame");
            while link.reply_in_flight() {
                let count = link.unsent().len().min(64);
                link.mark_sent(count);
            }
            receive_all(&mut link, &oversized, &mut buffer, at(start + 1));
            assert_eq!(
                link.poll_frame(&mut buffer, at(start + 1)),
                Some(LinkFrame::Oversized {
                    peer: PeerClass::Local
                })
            );
            link.send_frame(&reply[..100], at(start + 2))
                .expect("one reply at a time");
            link.mark_sent(10);
            assert_eq!(link.poll_frame(&mut buffer, at(start + 3)), None);
            assert_eq!(link.poll_frame(&mut buffer, at(start + 1_003)), None);
            assert!(!link.reply_in_flight(), "the stalled reply is dropped");
            link.reset_session();
        }
    });
    let counters = link.counters();
    assert_eq!(
        (
            counters.requests,
            counters.oversized_requests,
            counters.replies,
            counters.stalled_replies,
            counters.sessions
        ),
        (10, 10, 10, 10, 10)
    );
    assert_eq!(peak, 0, "the line link allocated {peak} bytes");
}
