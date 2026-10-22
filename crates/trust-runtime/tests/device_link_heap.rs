#![cfg(feature = "full")]
//! Spec 34 PORT-05 and ARCH-05: the heap that a chip's control link uses. Its line link moves
//! requests and replies in its fixed storage and allocates nothing. On a chip without a transfer
//! session, `events.tail` and `faults` with `limit = 1000` over a full event store are written
//! event by event into parts of 4,096 bytes and refused at the frame, 8,192 bytes, under 16 KiB of
//! heap; the unbounded pipeline, as trust-runtime answers them, builds the whole reply, measured
//! too. On a chip with the ESP32-C6's session, a read of 200 events, its store's maximum, is held
//! in parts and never takes a block of the heap longer than a part, and a short read takes one part
//! and nothing ahead of its text (Appendix D.2, long replies).
//!
//! The tests live here rather than in `trust-runtime-services`, which forbids the global
//! allocator's `GlobalAlloc` implementation. A counting global allocator records the bytes live
//! on this thread while it is armed, the most that were live at once, and the longest single block
//! taken. A reallocation counts its new block before the old one is released, as a moving
//! reallocation holds both.
//!
//! Run with `-- --nocapture` to see the peaks.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass, WallClock};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_services::access::{AccessRole, AuthFailure};
use trust_runtime_services::dispatch::{prepare_line, prepare_line_within};
use trust_runtime_services::events::{RuntimeEvent, EVENT_STORE_CAPACITY};
use trust_runtime_services::link::{DeviceLineLink, DEVICE_FRAME_LEN};
use trust_runtime_services::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use trust_runtime_services::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, PendingVarTarget,
};
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::live_values::RequestQueueFull;
use trust_runtime_services::metrics::RuntimeMetricsSnapshot;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, EventSink, ExtensionDispatch, ProgramControl, RecordsPort,
    ResourcePort, StatusPort, TransferSessionError,
};
use trust_runtime_services::protocol::{ControlRequest, ControlResponse};
use trust_runtime_services::resource::commands::CommandSendError;
use trust_runtime_services::runner::EngineLiveValues;
use trust_runtime_services::status::{IoDriverStatus, StatusSettings};
use trust_runtime_services::transfer::{TransferSession, PART_BYTES};

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
    static LARGEST: Cell<usize> = const { Cell::new(0) };
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

/// Notes a block of `size` bytes taken while armed, if it is the longest so far. Only
/// const-initialized `Cell`s are touched, as in `record`.
fn note_block(size: usize) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = LARGEST.try_with(|largest| largest.set(largest.get().max(size)));
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
        note_block(layout.size());
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
        note_block(new_size);
        record(bytes(new_size));
        // SAFETY: `System` returned `pointer` for `layout`; `new_size` is the caller's new size.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        record(-bytes(layout.size()));
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// What the heap did while a piece of work ran on this thread.
struct HeapUse {
    /// The most heap bytes live at once, above those live before the work.
    peak: i64,
    /// The length of the longest single block the work took, in bytes.
    largest_block: usize,
}

/// What `work` returned, and what it did to the heap on this thread.
fn heap_use<T>(work: impl FnOnce() -> T) -> (T, HeapUse) {
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
    LARGEST.with(|largest| largest.set(0));
    ARMED.with(|armed| armed.set(true));
    let output = work();
    ARMED.with(|armed| armed.set(false));
    let used = HeapUse {
        peak: PEAK.with(Cell::get),
        largest_block: LARGEST.with(Cell::get),
    };
    (output, used)
}

/// What `work` returned, and the most heap bytes live at once on this thread while it ran, above
/// those live before it.
fn heap_peak<T>(work: impl FnOnce() -> T) -> (T, i64) {
    let (output, used) = heap_use(work);
    (output, used.peak)
}

/// A runtime whose event store is full; nothing else of it is read by the event reads.
struct FullStore {
    events: VecDeque<RuntimeEvent>,
    /// The Live Values the control pipeline answers `io.*` and `var.*` from; the event reads
    /// never touch them.
    live: EngineLiveValues,
    /// The transfer session of a chip that holds long replies; `None` for a platform without one.
    transfers: Option<RefCell<TransferSession>>,
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

    fn platform(&self) -> &str {
        "esp32c6"
    }

    fn runtime_id(&self) -> &str {
        "esp32c6-test"
    }

    /// The store stands for a chip, which keeps control after `shutdown` (spec 34 STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
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

    fn with_transfers<R>(
        &self,
        use_session: impl FnOnce(&mut TransferSession) -> R,
    ) -> Result<R, TransferSessionError> {
        let Some(session) = &self.transfers else {
            return Err(TransferSessionError::Unsupported);
        };
        match session.try_borrow_mut() {
            Ok(mut session) => Ok(use_session(&mut session)),
            Err(_) => Err(TransferSessionError::Unavailable),
        }
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

impl IoQueuePort for FullStore {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.live.io_snapshot()
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live.enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live.force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.live.release_io(address);
    }
}

impl VariableQueuePort for FullStore {
    fn generation(&self) -> Generation {
        self.live.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.live.enqueue_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.live.force_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.live.release_global(name);
    }

    fn release_retain(&self, name: &str) {
        self.live.release_retain(name);
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.live.release_instance(instance, name);
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.live.forced_snapshot()
    }
}

/// A store of the `count` overruns that task `MainTask` records when it overruns at each of its
/// scans every 10 ms; the store keeps no event that every scan makes (specification 11).
fn overrun_events_of(count: usize) -> VecDeque<RuntimeEvent> {
    let name = SmolStr::new("MainTask");
    (0..count)
        .map(|index| RuntimeEvent::TaskOverrun {
            name: name.clone(),
            missed: 1,
            time: Duration::from_millis(i64::try_from(index * 10).expect("a small number")),
        })
        .collect()
}

/// A store of `count` faults.
fn fault_events_of(count: usize) -> VecDeque<RuntimeEvent> {
    (0..count)
        .map(|index| {
            RuntimeEvent::fault(
                &RuntimeError::SimulationFault(SmolStr::new(format!(
                    "scan {index}: array index out of range"
                ))),
                Duration::from_millis(i64::try_from(index * 10).expect("a small number")),
            )
        })
        .collect()
}

/// A full store of the overruns of task `MainTask`.
fn overrun_events() -> VecDeque<RuntimeEvent> {
    overrun_events_of(EVENT_STORE_CAPACITY)
}

/// A full store of faults.
fn fault_events() -> VecDeque<RuntimeEvent> {
    fault_events_of(EVENT_STORE_CAPACITY)
}

const SIXTEEN_KIB: i64 = 16 * 1024;

#[test]
fn a_thousand_events_on_a_chips_frame_are_refused_within_sixteen_kib_of_heap() {
    for (events, line) in [
        (
            overrun_events(),
            r#"{"id":1,"type":"events.tail","params":{"limit":1000}}"#,
        ),
        (
            fault_events(),
            r#"{"id":1,"type":"faults","params":{"limit":1000}}"#,
        ),
    ] {
        let host = FullStore {
            events,
            live: EngineLiveValues::empty(),
            transfers: None,
        };
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

/// The ESP32-C6's transfer session (specification 34, Appendix D.2): an upload of one program slot,
/// 3,056 KiB, and a reply of the capacity of its heap, 48 KiB + 64 KiB = 114,688 bytes.
fn c6_session() -> RefCell<TransferSession> {
    RefCell::new(TransferSession::new(1, 3_129_344, 114_688))
}

#[test]
fn a_long_reply_takes_no_heap_block_longer_than_a_part() {
    // Specification 34 PORT-05, reply capacity and parts, and Appendix D.2, long replies: a chip's heap
    // is first-fit and has no free block of 30 KiB, so a reply is written in parts of 4,096 bytes and
    // never joined. A chip with the C6's session reads its event store's maximum, 200 events, with
    // `events.tail` and `faults`. The replies are longer than the 8,192-byte frame, so the session
    // holds them in parts and the answer is a head. No block that the read takes is longer than 4,096
    // bytes, and its peak heap is under the reply's length plus 16 KiB: the parts, and the working
    // memory of a few events. Catches a reply built whole or joined into one block, and a writer that
    // takes its bound ahead.
    for (events, line) in [
        (
            overrun_events_of(200),
            r#"{"id":1,"type":"events.tail","params":{"limit":200}}"#,
        ),
        (
            fault_events_of(200),
            r#"{"id":1,"type":"faults","params":{"limit":200}}"#,
        ),
    ] {
        let host = FullStore {
            events,
            live: EngineLiveValues::empty(),
            transfers: Some(c6_session()),
        };
        let (held, used) = heap_use(|| prepare_line_within(line, &host, None, DEVICE_FRAME_LEN));
        let head: serde_json::Value =
            serde_json::from_str(&held.response).expect("the answer is a JSON head");
        let reply_len = i64::try_from(
            head["result"]["bytes"]
                .as_u64()
                .expect("the reply's length"),
        )
        .expect("a small number");
        println!(
            "{line}: a reply of {reply_len} bytes, peak {} bytes, longest block {} bytes",
            used.peak, used.largest_block
        );
        assert!(
            reply_len > i64::try_from(DEVICE_FRAME_LEN).expect("a small number"),
            "{line}: the reply is longer than a frame: {reply_len} bytes"
        );
        assert!(
            used.largest_block <= PART_BYTES,
            "{line}: the read took a block of {} bytes",
            used.largest_block
        );
        assert!(
            used.peak < reply_len + SIXTEEN_KIB,
            "{line}: the read peaks at {} bytes of heap for a reply of {reply_len} bytes",
            used.peak
        );
    }
}

#[test]
fn an_event_read_reserves_nothing_ahead_of_its_text() {
    // Specification 34 PORT-05, reply capacity and parts: each part is taken when the writing reaches
    // it, never reserved ahead. A chip whose session has the computer's reply capacity, 64 MiB, reads
    // three events, a reply of a few hundred bytes. It takes one part for its text, so no block is
    // longer than 4,096 bytes and the peak is under two parts, 8,192 bytes. Catches a read that
    // reserves its whole bound up front, which for 64 MiB is a block of 67,108,864 bytes, and one
    // that takes a second part before the first is full.
    let host = FullStore {
        events: overrun_events_of(3),
        live: EngineLiveValues::empty(),
        transfers: Some(RefCell::new(TransferSession::new(
            1,
            3_129_344,
            64 * 1024 * 1024,
        ))),
    };
    let line = r#"{"id":1,"type":"events.tail","params":{"limit":3}}"#;
    let (reply, used) = heap_use(|| prepare_line_within(line, &host, None, DEVICE_FRAME_LEN));
    println!(
        "{line}: a reply of {} bytes, peak {} bytes, longest block {} bytes",
        reply.response.len(),
        used.peak,
        used.largest_block
    );
    assert!(reply
        .response
        .starts_with(r#"{"id":1,"ok":true,"result":{"events":[{"#));
    assert!(reply.response.len() < PART_BYTES, "a short read");
    assert!(
        used.largest_block <= PART_BYTES,
        "the read took a block of {} bytes",
        used.largest_block
    );
    assert!(
        used.peak < 2 * i64::try_from(PART_BYTES).expect("a small number"),
        "the read peaks at {} bytes of heap",
        used.peak
    );
}
