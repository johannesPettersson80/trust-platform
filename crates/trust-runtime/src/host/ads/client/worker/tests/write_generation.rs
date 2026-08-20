use std::sync::{Arc, Mutex};

use super::*;

#[test]
fn write_response_cardinality_and_failure_quality_fail_closed() {
    let bindings = vec![
        binding(point(
            "first",
            "MAIN.First",
            PointAccess::Write,
            UpdateMode::Poll,
        )),
        binding(point(
            "second",
            "MAIN.Second",
            PointAccess::Write,
            UpdateMode::Poll,
        )),
    ];
    let transport = ScriptedAdsTransport::for_bindings(&bindings);
    let (shared, mut worker) = worker_fixture(bindings, transport);
    worker.connect(0).expect("connect malformed-write worker");
    shared.queue_write("first".to_string(), Value::Real(1.0));
    shared.queue_write("second".to_string(), Value::Real(2.0));
    worker
        .transport_mut()
        .script_writes(vec![PointQuality::good(1)]);

    assert!(
        worker.tick(1).is_err(),
        "write response with missing result must fail"
    );
    let snapshot = shared.snapshot();
    assert_eq!(
        snapshot.pending_writes.get("first"),
        Some(&Value::Real(1.0))
    );
    assert_eq!(
        snapshot.pending_writes.get("second"),
        Some(&Value::Real(2.0))
    );

    let failed = binding(point(
        "failed",
        "MAIN.Failed",
        PointAccess::Write,
        UpdateMode::Poll,
    ));
    let transport = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&failed));
    let (shared, mut worker) = worker_fixture(vec![failed], transport);
    worker.connect(0).expect("connect failed-write worker");
    shared.queue_write("failed".to_string(), Value::Real(3.0));
    worker
        .transport_mut()
        .script_writes(vec![PointQuality::error(1, "access denied")]);
    worker
        .tick(1)
        .expect("point-level write failure is correlated");

    let snapshot = shared.snapshot();
    let quality = &snapshot.qualities["failed"];
    assert_eq!(quality.state, QualityState::Error);
    assert_eq!(
        quality.detail.as_deref(),
        Some("ADS write failed: access denied"),
        "failed write must expose the ADS write boundary"
    );
    assert_eq!(
        snapshot.pending_writes.get("failed"),
        None,
        "a correlated failed generation must be terminal"
    );

    let mut storage = crate::memory::VariableStorage::new();
    storage.set_global("in_flight", Value::Real(4.0));
    storage.set_global("rejected", Value::Real(3.0));
    let mut bindings = vec![
        binding(point(
            "in_flight",
            "MAIN.InFlight",
            PointAccess::Write,
            UpdateMode::Poll,
        )),
        binding(point(
            "rejected",
            "MAIN.Rejected",
            PointAccess::Write,
            UpdateMode::Poll,
        )),
    ];
    for binding in &mut bindings {
        binding.reference = storage
            .ref_for_global(binding.point.point_name.as_str())
            .expect("multi-output reference");
    }
    let transport = ScriptedAdsTransport::for_bindings(&bindings);
    let (mut bridge, mut worker) =
        AdsConnectionBridge::with_transport(transport, bindings).expect("multi-output bridge");
    worker
        .connect(0)
        .expect("connect malformed generation worker");
    bridge
        .capture_outputs(&mut storage, 0)
        .expect("prime both output generations");
    worker.tick(1).expect("acknowledge priming generations");
    bridge
        .shared
        .queue_write("in_flight".to_string(), Value::Real(4.0));
    let rejected_during_write = Arc::new(Mutex::new((storage, bridge)));
    let rejection_hook_state = Arc::clone(&rejected_during_write);
    worker.transport_mut().on_write(move || {
        let mut guard = rejection_hook_state
            .lock()
            .expect("different-output rejection fixture lock");
        let (storage, bridge) = &mut *guard;
        let rejected_ref = storage.ref_for_global("rejected").unwrap();
        assert!(storage.write_by_ref(rejected_ref, Value::Real(f32::NAN)));
        bridge
            .capture_outputs(storage, 2)
            .expect("reject different output while write is in flight");
    });
    worker.transport_mut().script_writes(Vec::new());

    assert!(
        worker.tick(2).is_err(),
        "malformed write cardinality must fail closed"
    );
    let guard = rejected_during_write
        .lock()
        .expect("different-output rejection fixture lock");
    let (_storage, bridge) = &*guard;
    assert_eq!(bridge.state(), AdsConnectionState::Faulted);
    assert!(worker.handles.is_empty());
    assert!(worker.subscriptions.is_empty());
    assert_eq!(worker.symbol_version, None);
    assert_eq!(bridge.pending_write("in_flight"), Some(Value::Real(4.0)));
    let rejected = bridge.status("rejected").unwrap().quality;
    assert_eq!(rejected.state, QualityState::Error);
    assert!(
        rejected
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("non-finite")),
        "global malformed-response projection overwrote a newer different-output rejection"
    );
}

#[test]
fn write_ack_does_not_drop_newer_queued_value() {
    let output = binding(point(
        "out",
        "MAIN.Out",
        PointAccess::Write,
        UpdateMode::Poll,
    ));
    let transport = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&output));
    let (shared, mut worker) = worker_fixture(vec![output], transport);
    worker.connect(0).expect("connect generation worker");
    shared.queue_write("out".to_string(), Value::Real(1.0));
    let queued_during_write = shared.clone();
    worker.transport_mut().on_write(move || {
        queued_during_write.queue_write("out".to_string(), Value::Real(2.0));
    });

    worker.tick(1).expect("correlated old-generation ack");

    let snapshot = shared.snapshot();
    assert_eq!(
        snapshot.pending_writes.get("out"),
        Some(&Value::Real(2.0)),
        "older completion erased the newer generation"
    );
    assert_eq!(
        snapshot.qualities["out"].detail.as_deref(),
        Some("ADS write pending"),
        "older completion overwrote newer pending quality"
    );

    let (mut storage, mut bridge, mut worker) = output_fixture(Value::Real(1.0));
    worker
        .connect(0)
        .expect("connect non-finite generation worker");
    bridge
        .capture_outputs(&mut storage, 1)
        .expect("queue in-flight finite output");
    let rejection_state = Arc::new(Mutex::new((storage, bridge)));
    let rejection_during_write = Arc::clone(&rejection_state);
    worker.transport_mut().on_write(move || {
        let mut guard = rejection_during_write
            .lock()
            .expect("non-finite rejection fixture lock");
        let (storage, bridge) = &mut *guard;
        let output_ref = storage.ref_for_global("out").unwrap();
        assert!(storage.write_by_ref(output_ref, Value::Real(f32::NAN)));
        bridge
            .capture_outputs(storage, 2)
            .expect("reject output while older generation is in flight");
    });

    worker.tick(1).expect("complete older in-flight generation");

    let guard = rejection_state
        .lock()
        .expect("non-finite rejection fixture lock");
    let (_storage, bridge) = &*guard;
    assert_eq!(
        bridge.pending_write("out"),
        None,
        "non-finite rejection must cancel local pending intent"
    );
    let quality = bridge.status("out").unwrap().quality;
    assert_eq!(quality.state, QualityState::Error);
    assert!(
        quality
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("non-finite")),
        "older completion overwrote the newer non-finite rejection"
    );

    let bindings = vec![
        binding(point(
            "in_flight",
            "MAIN.InFlight",
            PointAccess::Write,
            UpdateMode::Poll,
        )),
        binding(point(
            "newer",
            "MAIN.Newer",
            PointAccess::Write,
            UpdateMode::Poll,
        )),
        binding(point(
            "input",
            "MAIN.Input",
            PointAccess::Read,
            UpdateMode::Notify,
        )),
    ];
    let transport = ScriptedAdsTransport::for_bindings(&bindings);
    let (shared, mut worker) = worker_fixture(bindings, transport);
    worker
        .connect(0)
        .expect("connect transport-error generation worker");
    shared.queue_write("in_flight".to_string(), Value::Real(5.0));
    shared.set_quality("input", PointQuality::good(1));
    let queued_during_write = shared.clone();
    worker.transport_mut().on_write(move || {
        queued_during_write.queue_write("newer".to_string(), Value::Real(6.0));
    });
    worker
        .transport_mut()
        .script_write_error("scripted transport write failure");

    assert!(
        worker.tick(2).is_err(),
        "transport write failure must reconnect"
    );
    let snapshot = shared.snapshot();
    assert_eq!(shared.state(), AdsConnectionState::Reconnecting);
    assert!(worker.handles.is_empty());
    assert!(worker.subscriptions.is_empty());
    assert_eq!(worker.symbol_version, None);
    assert_eq!(
        snapshot.pending_writes.get("newer"),
        Some(&Value::Real(6.0))
    );
    assert_eq!(
        snapshot.qualities["newer"].detail.as_deref(),
        Some("ADS write pending"),
        "global transport projection overwrote a newer different-output generation"
    );
    assert_ne!(
        snapshot.qualities["input"].state,
        QualityState::Good,
        "transport failure left readable input authority Good"
    );
}
