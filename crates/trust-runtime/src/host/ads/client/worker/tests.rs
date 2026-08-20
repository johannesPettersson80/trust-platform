mod support;
mod write_generation;

use std::time::{Duration, Instant};

use trust_ads_core::{
    AdsDataTypeDescriptor, ArrayDimension, IecDataType, PointAccess, PointQuality, QualityState,
    SymbolDescriptor, SymbolFlag, UpdateMode,
};
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};

use crate::value::{ArrayValue, Value};

use super::super::{
    ads_descriptor_matches_type_id, validate_bindings, validate_remote_symbols, AdsConnectionBridge,
};
use super::*;
use crate::ads::transport::{AdsPointAddress, AdsSubscription};
use support::*;

fn assert_value_matches_with_nonfinite(actual: &Value, expected: &Value, case: &str) {
    match (actual, expected) {
        (Value::Real(actual), Value::Real(expected)) if expected.is_nan() => {
            assert!(actual.is_nan(), "{case} changed the rejected REAL NaN")
        }
        (Value::LReal(actual), Value::LReal(expected)) if expected.is_nan() => {
            assert!(actual.is_nan(), "{case} changed the rejected LREAL NaN")
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.dimensions(), expected.dimensions(), "{case} shape");
            assert_eq!(
                actual.elements().len(),
                expected.elements().len(),
                "{case} length"
            );
            for (actual, expected) in actual.elements().iter().zip(expected.elements()) {
                assert_value_matches_with_nonfinite(actual, expected, case);
            }
        }
        _ => assert_eq!(actual, expected, "{case}"),
    }
}

#[test]
fn handle_resolution_requires_exact_response_bijection_and_cleanup() {
    let first = binding(point(
        "first",
        "MAIN.First",
        PointAccess::Read,
        UpdateMode::Poll,
    ));
    let second = binding(point(
        "second",
        "MAIN.Second",
        PointAccess::Read,
        UpdateMode::Poll,
    ));
    let mut extra = resolved(&first, 1);
    extra.point_name = "extra".to_string();
    let mut wrong_address = resolved(&first, 1);
    wrong_address.address = AdsPointAddress::Symbol("MAIN.Wrong".to_string());
    let mut wrong_descriptor = resolved(&first, 1);
    wrong_descriptor.data_type = AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal);
    let cases = [
        (
            "extra",
            vec![resolved(&first, 1), resolved(&second, 2), extra],
        ),
        (
            "duplicate point",
            vec![resolved(&first, 1), resolved(&first, 2)],
        ),
        ("wrong address", vec![wrong_address, resolved(&second, 2)]),
        (
            "wrong descriptor",
            vec![wrong_descriptor, resolved(&second, 2)],
        ),
        ("missing", vec![resolved(&first, 1)]),
    ];

    for (case, handles) in cases {
        let bindings = vec![first.clone(), second.clone()];
        let mut transport = ScriptedAdsTransport::for_bindings(&bindings);
        transport.script_handles(handles);
        let probe = transport.probe();
        let (_shared, mut worker) = worker_fixture(bindings, transport);

        let result = worker.connect(0);

        assert!(
            result.is_err(),
            "{case} handle response must fail exact bijection validation"
        );
        assert!(
            probe.disconnects() >= 1,
            "{case} post-connect validation failure must attempt disconnect"
        );
        assert!(worker.handles.is_empty(), "{case} leaked worker handles");
        assert!(
            worker.subscriptions.is_empty(),
            "{case} leaked worker subscriptions"
        );
        assert_eq!(worker.symbol_version, None, "{case} leaked symbol token");
    }

    let bindings = vec![first.clone(), second.clone()];
    let mut transport = ScriptedAdsTransport::for_bindings(&bindings);
    transport.script_handles(vec![resolved(&second, 7), resolved(&first, 7)]);
    let (_shared, mut worker) = worker_fixture(bindings, transport);
    worker
        .connect(0)
        .expect("response order is not authority and numeric handles need not be unique");
}

#[test]
fn poll_results_require_exact_order_and_cardinality_before_cache_commit() {
    let bindings = vec![
        binding(point(
            "first",
            "MAIN.First",
            PointAccess::Read,
            UpdateMode::Poll,
        )),
        binding(point(
            "second",
            "MAIN.Second",
            PointAccess::Read,
            UpdateMode::Poll,
        )),
    ];
    let malformed = [
        (
            "reordered",
            vec![
                read_result("second", Value::Real(22.0), PointQuality::good(1)),
                read_result("first", Value::Real(11.0), PointQuality::good(1)),
            ],
        ),
        (
            "missing",
            vec![read_result(
                "first",
                Value::Real(11.0),
                PointQuality::good(1),
            )],
        ),
        (
            "extra",
            vec![
                read_result("first", Value::Real(11.0), PointQuality::good(1)),
                read_result("second", Value::Real(22.0), PointQuality::good(1)),
                read_result("extra", Value::Real(33.0), PointQuality::good(1)),
            ],
        ),
    ];

    for (case, reads) in malformed {
        let transport = ScriptedAdsTransport::for_bindings(&bindings);
        let (shared, mut worker) = worker_fixture(bindings.clone(), transport);
        worker.connect(0).expect("connect poll worker");
        shared.set_value("first", Value::Real(1.0));
        shared.set_value("second", Value::Real(2.0));
        worker.transport_mut().script_reads(reads);

        let result = worker.tick(1);
        let snapshot = shared.snapshot();

        assert!(
            result.is_err(),
            "{case} poll response must fail structural validation"
        );
        assert_eq!(snapshot.values.get("first"), Some(&Value::Real(1.0)));
        assert_eq!(snapshot.values.get("second"), Some(&Value::Real(2.0)));
        assert!(!snapshot.values.contains_key("extra"));
    }

    let transport = ScriptedAdsTransport::for_bindings(&bindings);
    let (shared, mut worker) = worker_fixture(bindings, transport);
    worker.connect(0).expect("connect point-quality worker");
    worker.transport_mut().script_reads(vec![
        read_result(
            "first",
            Value::Real(11.0),
            PointQuality::error(1, "point-level read failure"),
        ),
        read_result("second", Value::Real(f32::NAN), PointQuality::good(1)),
    ]);
    worker
        .tick(1)
        .expect("validly correlated point-level failures stay per-point");
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.qualities["first"].state, QualityState::Error);
    assert_eq!(snapshot.qualities["second"].state, QualityState::Error);
    assert!(!snapshot.values.contains_key("first"));
    assert!(!snapshot.values.contains_key("second"));
}

#[test]
fn subscription_and_notification_results_require_active_correlation() {
    let notify = binding(point(
        "notify",
        "MAIN.Notify",
        PointAccess::Read,
        UpdateMode::Notify,
    ));
    let mut mismatched = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&notify));
    mismatched.script_subscription(AdsSubscription {
        point_name: "other".to_string(),
        subscription_id: 1,
    });
    let mismatch_probe = mismatched.probe();
    let (_shared, mut worker) = worker_fixture(vec![notify.clone()], mismatched);
    assert!(
        worker.connect(0).is_err(),
        "subscription response for another point must fail"
    );
    assert!(mismatch_probe.disconnects() >= 1);
    assert!(worker.subscriptions.is_empty());

    let other = binding(point(
        "other",
        "MAIN.Other",
        PointAccess::Read,
        UpdateMode::Notify,
    ));
    let bindings = vec![notify.clone(), other.clone()];
    let mut duplicate = ScriptedAdsTransport::for_bindings(&bindings);
    duplicate.script_subscription(AdsSubscription {
        point_name: "notify".to_string(),
        subscription_id: 7,
    });
    duplicate.script_subscription(AdsSubscription {
        point_name: "other".to_string(),
        subscription_id: 7,
    });
    let (_shared, mut worker) = worker_fixture(bindings.clone(), duplicate);
    assert!(
        worker.connect(0).is_err(),
        "duplicate active subscription IDs must fail candidate validation"
    );
    assert!(worker.subscriptions.is_empty());

    let transport = ScriptedAdsTransport::for_bindings(&bindings);
    let (shared, mut worker) = worker_fixture(bindings, transport);
    worker.connect(0).expect("connect notification worker");
    let notify_id = worker.subscription_for_point("notify").unwrap();
    let other_id = worker.subscription_for_point("other").unwrap();
    worker.transport_mut().script_notifications(vec![
        notification("notify", notify_id, Value::Real(10.0)),
        notification("notify", notify_id, Value::Real(11.0)),
        notification("notify", other_id, Value::Real(99.0)),
    ]);

    let result = worker.tick(1);
    let snapshot = shared.snapshot();

    assert!(
        result.is_err(),
        "cross-point subscription correlation must fail closed"
    );
    assert_eq!(
        snapshot.values.get("notify"),
        Some(&Value::Real(11.0)),
        "repeated valid samples before an invalid sample remain accepted in order"
    );
    assert_eq!(shared.state(), AdsConnectionState::Faulted);
    assert_eq!(snapshot.qualities["notify"].state, QualityState::Error);
}

#[test]
fn symbol_version_change_invalidates_old_state_and_requires_stable_candidate() {
    let input = binding(point(
        "input",
        "MAIN.Input",
        PointAccess::Read,
        UpdateMode::Poll,
    ));
    let mut unstable_initial = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&input));
    unstable_initial.script_symbol_versions([10, 11]);
    let initial_probe = unstable_initial.probe();
    let (_shared, mut worker) = worker_fixture(vec![input.clone()], unstable_initial);

    assert!(
        worker.connect(0).is_err(),
        "initial candidate must reject a token that changes during construction"
    );
    assert_eq!(
        initial_probe.symbol_version_calls(),
        2,
        "candidate construction must bracket symbol-derived work"
    );
    assert!(initial_probe.disconnects() >= 1);
    assert!(worker.handles.is_empty());
    assert_eq!(worker.symbol_version, None);

    let mut unstable_refresh = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&input));
    unstable_refresh.script_symbol_versions([7, 7, 8, 9, 10]);
    let (shared, mut worker) = worker_fixture(vec![input], unstable_refresh);
    worker.connect(0).expect("stable initial candidate");
    shared.set_value("input", Value::Real(42.0));
    shared.set_quality("input", PointQuality::good(1));

    let result = worker.tick(100);
    let snapshot = shared.snapshot();

    assert!(
        result.is_err(),
        "refresh candidate must reject an unstable symbol token"
    );
    assert!(
        worker.handles.is_empty(),
        "old handles remained authoritative"
    );
    assert!(worker.subscriptions.is_empty());
    assert_eq!(worker.symbol_version, None);
    assert_eq!(snapshot.values.get("input"), Some(&Value::Real(42.0)));
    assert_ne!(
        snapshot.qualities["input"].state,
        QualityState::Good,
        "raw diagnostic value may remain but Good authority must be revoked"
    );

    let bindings = ["good", "pending", "rejected"]
        .into_iter()
        .map(|point_name| {
            let symbol_name = format!("MAIN.{point_name}");
            binding(point(
                point_name,
                symbol_name.as_str(),
                PointAccess::ReadWrite,
                UpdateMode::Notify,
            ))
        })
        .collect::<Vec<_>>();
    let mut transport = ScriptedAdsTransport::for_bindings(&bindings);
    transport.script_symbol_versions([7, 7, 8, 8, 8]);
    let (shared, mut worker) = worker_fixture(bindings, transport);
    worker
        .connect(0)
        .expect("connect read/write refresh worker");
    shared.set_quality("good", PointQuality::good(1));
    shared.queue_write("pending".to_string(), Value::Real(7.0));
    shared.reject_write(
        "rejected",
        2,
        "ADS output 'rejected' contains a non-finite REAL/LREAL value".to_string(),
    );

    assert!(
        worker
            .refresh_handles_if_symbol_version_due(100)
            .expect("stable symbol refresh"),
        "changed token must rebuild correlation"
    );
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.qualities["good"].state, QualityState::Stale);
    assert_eq!(
        snapshot.qualities["pending"].detail.as_deref(),
        Some("ADS write pending"),
        "symbol refresh overwrote a read/write point's newer pending generation"
    );
    assert_eq!(snapshot.qualities["rejected"].state, QualityState::Error);
    assert!(
        snapshot.qualities["rejected"]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("non-finite")),
        "symbol refresh overwrote a read/write point's newer non-finite rejection"
    );
}

#[test]
fn symbol_version_comparison_is_equality_only_across_wrap() {
    let input = binding(point(
        "input",
        "MAIN.Input",
        PointAccess::Read,
        UpdateMode::Poll,
    ));
    let mut transport = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&input));
    transport.script_symbol_versions([u32::MAX, u32::MAX, u32::MAX, 0, 0, 0]);
    let probe = transport.probe();
    let (_shared, mut worker) = worker_fixture(vec![input], transport);
    worker.connect(0).expect("stable maximum token");
    let first_handle = worker.handle_for_point("input").unwrap();

    worker.tick(100).expect("equal token does not refresh");
    assert_eq!(worker.handle_for_point("input"), Some(first_handle));
    worker
        .tick(200)
        .expect("unequal wrapped token refreshes without ordering semantics");
    assert_ne!(worker.handle_for_point("input"), Some(first_handle));
    assert!(probe.connects() >= 2);
}

#[test]
fn reconnect_invalidates_local_correlation_and_marks_cached_inputs_stale() {
    let input = binding(point(
        "input",
        "MAIN.Input",
        PointAccess::ReadWrite,
        UpdateMode::Poll,
    ));
    let transport = ScriptedAdsTransport::for_bindings(std::slice::from_ref(&input));
    let (shared, mut worker) = worker_fixture(vec![input.clone()], transport);
    worker.connect(0).expect("initial connection");
    shared.queue_write("input".to_string(), Value::Real(41.0));
    worker
        .tick(1)
        .expect("acknowledge read/write generation before read fault");
    assert_eq!(
        shared.snapshot().qualities["input"].state,
        QualityState::Good,
        "write acknowledgement must establish the regression precondition"
    );
    shared.set_value("input", Value::Real(42.0));

    worker
        .transport_mut()
        .script_read_error("transport recovery");
    assert!(
        worker.tick(10).is_err(),
        "transport recovery must reconnect"
    );

    assert_eq!(shared.state(), AdsConnectionState::Reconnecting);
    assert_eq!(
        shared.snapshot().qualities["input"].state,
        QualityState::Stale
    );
    assert!(
        worker.handles.is_empty(),
        "reconnect retained stale handles"
    );
    assert!(
        worker.subscriptions.is_empty(),
        "reconnect retained stale subscriptions"
    );
    assert_eq!(
        worker.symbol_version, None,
        "reconnect retained stale token"
    );
    assert_eq!(
        worker.next_symbol_version_check_ms, None,
        "reconnect retained stale symbol schedule"
    );

    let mut extra = resolved(&input, 2);
    extra.point_name = "extra".to_string();
    worker
        .transport_mut()
        .script_handles(vec![resolved(&input, 1), extra]);
    assert!(
        worker.tick(35).is_err(),
        "post-connect validation failure must fail reconnect"
    );
    assert_eq!(shared.state(), AdsConnectionState::Faulted);
    assert_eq!(
        shared.snapshot().qualities["input"].state,
        QualityState::Error
    );
    assert!(worker.handles.is_empty());
    assert!(worker.subscriptions.is_empty());
    assert_eq!(worker.symbol_version, None);
}

#[test]
fn output_queue_reports_latest_value_as_write_pending_until_ack() {
    let (mut storage, mut bridge, mut worker) = output_fixture(Value::Real(0.0));
    let output_ref = storage.ref_for_global("out").unwrap();
    assert!(storage.write_by_ref(output_ref.clone(), Value::Real(10.0)));
    bridge
        .capture_outputs(&mut storage, 10)
        .expect("capture first finite output");
    assert!(storage.write_by_ref(output_ref, Value::Real(11.0)));
    bridge
        .capture_outputs(&mut storage, 11)
        .expect("capture replacement output");

    assert_eq!(bridge.pending_write("out"), Some(Value::Real(11.0)));
    assert_eq!(
        bridge.status("out").unwrap().quality.detail.as_deref(),
        Some("ADS write pending"),
        "latest unacknowledged value must expose pending quality"
    );

    worker.tick(12).expect("acknowledge current generation");
    assert_eq!(bridge.pending_write("out"), None);
    assert_eq!(
        bridge.status("out").unwrap().quality.state,
        QualityState::Good
    );
}

#[test]
fn output_queue_rejects_non_finite_values_before_enqueue() {
    let mut registry = TypeRegistry::new();
    let real_array = registry.register_array(TypeId::REAL, vec![(0, 1)]);
    let lreal_array = registry.register_array(TypeId::LREAL, vec![(0, 1)]);
    let real_array_descriptor =
        real_descriptor().with_dimensions(vec![ArrayDimension { lower: 0, upper: 1 }]);
    let lreal_array_descriptor = AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal)
        .with_dimensions(vec![ArrayDimension { lower: 0, upper: 1 }]);
    let cases = [
        (
            "REAL NaN",
            real_descriptor(),
            TypeId::REAL,
            Value::Real(12.0),
            Value::Real(f32::NAN),
            Value::Real(13.0),
        ),
        (
            "REAL infinity",
            real_descriptor(),
            TypeId::REAL,
            Value::Real(12.0),
            Value::Real(f32::INFINITY),
            Value::Real(13.0),
        ),
        (
            "LREAL NaN",
            AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
            TypeId::LREAL,
            Value::LReal(12.0),
            Value::LReal(f64::NAN),
            Value::LReal(13.0),
        ),
        (
            "LREAL infinity",
            AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal),
            TypeId::LREAL,
            Value::LReal(12.0),
            Value::LReal(f64::NEG_INFINITY),
            Value::LReal(13.0),
        ),
        (
            "REAL array",
            real_array_descriptor,
            real_array,
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::Real(1.0), Value::Real(2.0)],
                vec![(0, 1)],
            ))),
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::Real(1.0), Value::Real(f32::NAN)],
                vec![(0, 1)],
            ))),
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::Real(1.0), Value::Real(2.0)],
                vec![(0, 1)],
            ))),
        ),
        (
            "LREAL array",
            lreal_array_descriptor,
            lreal_array,
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::LReal(1.0), Value::LReal(2.0)],
                vec![(0, 1)],
            ))),
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::LReal(1.0), Value::LReal(f64::INFINITY)],
                vec![(0, 1)],
            ))),
            Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![Value::LReal(1.0), Value::LReal(2.0)],
                vec![(0, 1)],
            ))),
        ),
    ];

    for (case, descriptor, type_id, initial, rejected, recovery) in cases {
        assert!(
            ads_descriptor_matches_type_id(&descriptor, type_id, &registry),
            "{case} fixture descriptor/type must be product-valid"
        );
        let (mut storage, mut bridge, _worker) = typed_output_fixture(initial, descriptor, type_id);
        let output_ref = storage.ref_for_global("out").unwrap();
        bridge
            .capture_outputs(&mut storage, 1)
            .expect("queue prior finite intent");
        assert!(storage.write_by_ref(output_ref.clone(), rejected.clone()));

        bridge
            .capture_outputs(&mut storage, 2)
            .expect("reject non-finite output without stopping scan");

        assert_value_matches_with_nonfinite(
            storage
                .get_global("out")
                .expect("output remains in PLC storage"),
            &rejected,
            case,
        );
        assert_eq!(bridge.pending_write("out"), None, "{case} reached queue");
        let quality = bridge.status("out").unwrap().quality;
        assert_eq!(quality.state, QualityState::Error, "{case}");
        assert!(
            quality
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("non-finite")),
            "{case} lacks explicit rejection detail"
        );

        assert!(storage.write_by_ref(output_ref, recovery.clone()));
        bridge
            .capture_outputs(&mut storage, 3)
            .expect("finite recovery output");
        assert_eq!(bridge.pending_write("out"), Some(recovery), "{case}");
    }
}

#[test]
fn worker_shutdown_and_drop_interrupt_idle_wait() {
    fn spawned_idle_worker() -> (AdsWorkerThread, TransportProbe, AdsConnectionBridge) {
        let bindings = ["good", "pending", "rejected"]
            .into_iter()
            .map(|point_name| {
                binding(point(
                    point_name,
                    format!("MAIN.{point_name}").as_str(),
                    PointAccess::ReadWrite,
                    UpdateMode::Notify,
                ))
            })
            .collect::<Vec<_>>();
        let transport = ScriptedAdsTransport::for_bindings(&bindings);
        let probe = transport.probe();
        let (bridge, worker) =
            AdsConnectionBridge::with_transport(transport, bindings).expect("shutdown bridge");
        let thread = worker
            .spawn(Duration::from_secs(2))
            .expect("spawn idle ADS worker");
        let deadline = Instant::now() + Duration::from_secs(1);
        while probe.connects() == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(probe.connects() > 0, "worker never entered its idle wait");
        std::thread::sleep(Duration::from_millis(20));
        bridge.shared.set_value("good", Value::Real(42.0));
        bridge.shared.set_quality("good", PointQuality::good(1));
        bridge
            .shared
            .queue_write("pending".to_string(), Value::Real(7.0));
        bridge.shared.reject_write(
            "rejected",
            2,
            "ADS output 'rejected' contains a non-finite REAL/LREAL value".to_string(),
        );
        (thread, probe, bridge)
    }

    let (thread, shutdown_probe, shutdown_bridge) = spawned_idle_worker();
    let started = Instant::now();
    thread.shutdown().expect("shutdown idle worker");
    let shutdown_elapsed = started.elapsed();

    let (thread, drop_probe, drop_bridge) = spawned_idle_worker();
    let started = Instant::now();
    drop(thread);
    let drop_elapsed = started.elapsed();

    assert!(
        shutdown_elapsed < Duration::from_millis(500),
        "shutdown waited for idle interval: {shutdown_elapsed:?}"
    );
    assert!(
        drop_elapsed < Duration::from_millis(500),
        "drop waited for idle interval: {drop_elapsed:?}"
    );
    assert_eq!(shutdown_probe.disconnects(), 1);
    assert_eq!(drop_probe.disconnects(), 1);
    for (path, bridge) in [("shutdown", shutdown_bridge), ("drop", drop_bridge)] {
        assert_eq!(bridge.state(), AdsConnectionState::Disconnected, "{path}");
        assert_ne!(
            bridge.status("good").unwrap().quality.state,
            QualityState::Good,
            "{path} left prior Good input authoritative"
        );
        assert_eq!(
            bridge.status("pending").unwrap().quality.detail.as_deref(),
            Some("ADS write pending"),
            "{path} overwrote a newer finite pending generation"
        );
        let rejected = bridge.status("rejected").unwrap().quality;
        assert_eq!(rejected.state, QualityState::Error, "{path}");
        assert!(
            rejected
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("non-finite")),
            "{path} overwrote the newer non-finite rejection"
        );
    }
}

#[test]
fn binding_and_remote_descriptor_validation_partition_table() {
    let mut registry = TypeRegistry::new();
    let real_alias = registry.register(
        "REAL_ALIAS",
        Type::Alias {
            name: "REAL_ALIAS".into(),
            target: TypeId::REAL,
        },
    );
    let int_subrange = registry.register(
        "INT_RANGE",
        Type::Subrange {
            base: TypeId::INT,
            lower: -10,
            upper: 10,
        },
    );
    let int_enum = registry.register_enum(
        "MODE",
        TypeId::INT,
        vec![("Idle".into(), 0), ("Run".into(), 1)],
    );
    let real_array = registry.register_array(TypeId::REAL, vec![(1, 2)]);
    let string_8 = registry.register_string_with_length(8);
    let accepted = [
        ("scalar", real_descriptor(), TypeId::REAL),
        ("alias", real_descriptor(), real_alias),
        (
            "subrange",
            AdsDataTypeDescriptor::scalar("INT", IecDataType::Int),
            int_subrange,
        ),
        (
            "enum",
            AdsDataTypeDescriptor::scalar("INT", IecDataType::Int),
            int_enum,
        ),
        (
            "array",
            real_descriptor().with_dimensions(vec![ArrayDimension { lower: 1, upper: 2 }]),
            real_array,
        ),
        (
            "STRING capacity",
            AdsDataTypeDescriptor::string("STRING(8)", 8),
            string_8,
        ),
    ];
    for (partition, descriptor, type_id) in accepted {
        assert!(
            ads_descriptor_matches_type_id(&descriptor, type_id, &registry),
            "accepted {partition} descriptor partition regressed"
        );
    }
    assert!(!ads_descriptor_matches_type_id(
        &real_descriptor(),
        TypeId::LREAL,
        &registry,
    ));

    let readable = binding(point(
        "value",
        "MAIN.Value",
        PointAccess::Read,
        UpdateMode::Poll,
    ));
    let mut retained_local = readable.clone();
    retained_local.retain = true;
    let mut writable = readable.clone();
    writable.point.access = PointAccess::Write;
    let mut index = readable.clone();
    index.point.address = AdsPointAddress::Index {
        index_group: 0x4020,
        index_offset: 0,
        size: 2,
    };
    let lreal = AdsDataTypeDescriptor::scalar("LREAL", IecDataType::Lreal);
    let cases = [
        (
            "duplicate binding identity",
            validate_bindings(&[readable.clone(), readable.clone()]),
            "bound more than once",
        ),
        (
            "local retained read opt-in",
            validate_bindings(&[retained_local]),
            "allow_retain_read=true",
        ),
        (
            "remote type",
            validate_remote_symbols(
                std::slice::from_ref(&readable),
                &[SymbolDescriptor::new("MAIN.Value", lreal, 0x4020, 0, 8)
                    .with_flag(SymbolFlag::Read)],
            ),
            "type mismatch",
        ),
        (
            "remote read access",
            validate_remote_symbols(
                std::slice::from_ref(&readable),
                &[
                    SymbolDescriptor::new("MAIN.Value", real_descriptor(), 0x4020, 0, 4)
                        .with_flag(SymbolFlag::Write),
                ],
            ),
            "not readable",
        ),
        (
            "remote write access",
            validate_remote_symbols(
                std::slice::from_ref(&writable),
                &[
                    SymbolDescriptor::new("MAIN.Value", real_descriptor(), 0x4020, 0, 4)
                        .with_flag(SymbolFlag::Read),
                ],
            ),
            "not writable",
        ),
        (
            "remote retained read opt-in",
            validate_remote_symbols(
                std::slice::from_ref(&readable),
                &[
                    SymbolDescriptor::new("MAIN.Value", real_descriptor(), 0x4020, 0, 4)
                        .with_flag(SymbolFlag::Read)
                        .with_flag(SymbolFlag::Retain),
                ],
            ),
            "allow_retain_read=true",
        ),
        (
            "index byte size",
            validate_remote_symbols(std::slice::from_ref(&index), &[]),
            "index size mismatch",
        ),
    ];

    for (partition, result, expected) in cases {
        let error = result.expect_err(partition);
        assert!(
            error.message().contains(expected),
            "{partition} returned the wrong validation partition: {error}"
        );
    }
}
