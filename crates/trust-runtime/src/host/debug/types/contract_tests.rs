use super::*;
use crate::error::RuntimeError;
use crate::memory::{FrameId, VariableStorage};
use crate::program_model::{Expr, LValue};
use crate::value::{DateTimeProfile, Duration, Value};
use trust_hir::types::TypeRegistry;

fn snapshot_with_two_frames() -> (DebugSnapshot, FrameId, FrameId) {
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(99));
    let first = storage.push_frame("first");
    assert!(storage.set_local("value", Value::DInt(1)));
    let second = storage.push_frame("second");
    assert!(storage.set_local("value", Value::DInt(2)));
    (
        DebugSnapshot {
            storage,
            now: Duration::from_millis(7),
        },
        first,
        second,
    )
}

#[test]
fn source_location_constructor_preserves_exact_coordinates() {
    let location = SourceLocation::new(u32::MAX, 0, u32::MAX);
    assert_eq!(location.file_id, u32::MAX);
    assert_eq!(location.start, 0);
    assert_eq!(location.end, u32::MAX);
}

#[test]
fn breakpoint_constructor_starts_unconditional_and_unversioned() {
    let location = SourceLocation::new(1, 10, 20);
    let breakpoint = DebugBreakpoint::new(location);

    assert_eq!(breakpoint.location, location);
    assert!(breakpoint.condition.is_none());
    assert!(breakpoint.hit_condition.is_none());
    assert!(breakpoint.log_message.is_none());
    assert_eq!(breakpoint.hits, 0);
    assert_eq!(breakpoint.generation, 0);
}

#[test]
fn equality_hit_condition_matches_only_exact_target() {
    let condition = HitCondition::Equal(7);
    assert!(!condition.is_met(6));
    assert!(condition.is_met(7));
    assert!(!condition.is_met(8));
}

#[test]
fn at_least_hit_condition_includes_target() {
    let condition = HitCondition::AtLeast(7);
    assert!(!condition.is_met(6));
    assert!(condition.is_met(7));
    assert!(condition.is_met(8));
}

#[test]
fn greater_than_hit_condition_excludes_target() {
    let condition = HitCondition::GreaterThan(7);
    assert!(!condition.is_met(6));
    assert!(!condition.is_met(7));
    assert!(condition.is_met(8));
}

#[test]
fn hit_conditions_are_defined_at_unsigned_boundaries() {
    assert!(HitCondition::Equal(0).is_met(0));
    assert!(!HitCondition::GreaterThan(0).is_met(0));
    assert!(HitCondition::AtLeast(0).is_met(0));

    assert!(HitCondition::Equal(u64::MAX).is_met(u64::MAX));
    assert!(HitCondition::AtLeast(u64::MAX).is_met(u64::MAX));
    assert!(!HitCondition::GreaterThan(u64::MAX).is_met(u64::MAX));
}

#[test]
fn debug_snapshot_expression_uses_selected_frame_and_restores_stack_order() {
    let (mut snapshot, first, second) = snapshot_with_two_frames();
    let registry = TypeRegistry::new();
    let value = snapshot
        .evaluate_expression(
            &Expr::Name("value".into()),
            &registry,
            None,
            &DateTimeProfile::default(),
            Some(first),
        )
        .expect("evaluate first frame");

    assert_eq!(value, Value::DInt(1));
    assert_eq!(
        snapshot.storage.current_frame().map(|frame| frame.id),
        Some(second)
    );
}

#[test]
fn debug_snapshot_expression_without_frame_uses_current_context() {
    let (mut snapshot, _first, second) = snapshot_with_two_frames();
    let registry = TypeRegistry::new();
    let value = snapshot
        .evaluate_expression(
            &Expr::Name("value".into()),
            &registry,
            None,
            &DateTimeProfile::default(),
            None,
        )
        .expect("evaluate current frame");

    assert_eq!(value, Value::DInt(2));
    assert_eq!(
        snapshot.storage.current_frame().map(|frame| frame.id),
        Some(second)
    );
}

#[test]
fn debug_snapshot_expression_rejects_unknown_frame_without_fallback() {
    let (mut snapshot, _first, second) = snapshot_with_two_frames();
    let registry = TypeRegistry::new();
    let error = snapshot
        .evaluate_expression(
            &Expr::Name("value".into()),
            &registry,
            None,
            &DateTimeProfile::default(),
            Some(FrameId(99)),
        )
        .expect_err("unknown frame");

    assert!(matches!(error, RuntimeError::InvalidFrame(99)));
    assert_eq!(
        snapshot.storage.current_frame().map(|frame| frame.id),
        Some(second)
    );
}

#[test]
fn debug_snapshot_lvalue_read_uses_selected_frame() {
    let (mut snapshot, first, second) = snapshot_with_two_frames();
    let value = snapshot
        .read_lvalue(
            &LValue::Name("value".into()),
            &TypeRegistry::new(),
            &DateTimeProfile::default(),
            Some(first),
        )
        .expect("read first frame");

    assert_eq!(value, Value::DInt(1));
    assert_eq!(
        snapshot.storage.current_frame().map(|frame| frame.id),
        Some(second)
    );
}

#[test]
fn debug_snapshot_lvalue_read_rejects_unknown_frame_without_fallback() {
    let (mut snapshot, _first, second) = snapshot_with_two_frames();
    let error = snapshot
        .read_lvalue(
            &LValue::Name("value".into()),
            &TypeRegistry::new(),
            &DateTimeProfile::default(),
            Some(FrameId(99)),
        )
        .expect_err("unknown frame");

    assert!(matches!(error, RuntimeError::InvalidFrame(99)));
    assert_eq!(
        snapshot.storage.current_frame().map(|frame| frame.id),
        Some(second)
    );
}

#[test]
fn debug_snapshot_lvalue_write_changes_only_selected_frame() {
    let (mut snapshot, first, second) = snapshot_with_two_frames();
    let registry = TypeRegistry::new();
    snapshot
        .write_lvalue(
            &LValue::Name("value".into()),
            Value::DInt(7),
            &registry,
            &DateTimeProfile::default(),
            Some(first),
        )
        .expect("write first frame");

    let first_value = snapshot
        .read_lvalue(
            &LValue::Name("value".into()),
            &registry,
            &DateTimeProfile::default(),
            Some(first),
        )
        .expect("read first frame");
    let second_value = snapshot
        .read_lvalue(
            &LValue::Name("value".into()),
            &registry,
            &DateTimeProfile::default(),
            Some(second),
        )
        .expect("read second frame");
    assert_eq!(first_value, Value::DInt(7));
    assert_eq!(second_value, Value::DInt(2));
    assert_eq!(snapshot.storage.get_global("value"), Some(&Value::DInt(99)));
}

#[test]
fn debug_snapshot_lvalue_write_rejects_unknown_frame_without_mutation() {
    let (mut snapshot, first, _second) = snapshot_with_two_frames();
    let registry = TypeRegistry::new();
    let error = snapshot
        .write_lvalue(
            &LValue::Name("value".into()),
            Value::DInt(7),
            &registry,
            &DateTimeProfile::default(),
            Some(FrameId(99)),
        )
        .expect_err("unknown frame");
    assert!(matches!(error, RuntimeError::InvalidFrame(99)));

    assert_eq!(
        snapshot
            .read_lvalue(
                &LValue::Name("value".into()),
                &registry,
                &DateTimeProfile::default(),
                Some(first),
            )
            .expect("read original"),
        Value::DInt(1)
    );
}

#[test]
fn snapshot_write_is_isolated_from_the_source_storage_clone() {
    let (mut snapshot, first, _second) = snapshot_with_two_frames();
    let original = snapshot.storage.clone();
    snapshot
        .write_lvalue(
            &LValue::Name("value".into()),
            Value::DInt(7),
            &TypeRegistry::new(),
            &DateTimeProfile::default(),
            Some(first),
        )
        .expect("snapshot write");

    let mut original = DebugSnapshot {
        storage: original,
        now: Duration::ZERO,
    };
    assert_eq!(
        original
            .read_lvalue(
                &LValue::Name("value".into()),
                &TypeRegistry::new(),
                &DateTimeProfile::default(),
                Some(first),
            )
            .expect("original read"),
        Value::DInt(1)
    );
}

#[test]
fn runtime_event_variants_preserve_exact_payloads_and_identity() {
    let events = vec![
        RuntimeEvent::CycleStart {
            cycle: 1,
            time: Duration::from_millis(1),
        },
        RuntimeEvent::CycleEnd {
            cycle: 1,
            time: Duration::from_millis(2),
        },
        RuntimeEvent::TaskStart {
            name: "Fast".into(),
            priority: 0,
            time: Duration::from_millis(3),
        },
        RuntimeEvent::TaskEnd {
            name: "Fast".into(),
            priority: 0,
            time: Duration::from_millis(4),
        },
        RuntimeEvent::TaskOverrun {
            name: "Fast".into(),
            missed: u64::MAX,
            time: Duration::from_millis(5),
        },
        RuntimeEvent::Fault {
            error: "fault".into(),
            time: Duration::from_millis(6),
        },
        RuntimeEvent::SafeStateFailed {
            root: "fault".into(),
            error: "safe state".into(),
            time: Duration::from_millis(7),
        },
        RuntimeEvent::RetainOrphanDropped {
            name: "old".into(),
            time: Duration::from_millis(8),
        },
        RuntimeEvent::RetainMigrationApplied {
            name: "kept".into(),
            detail: "DINT to LINT".into(),
            time: Duration::from_millis(9),
        },
        RuntimeEvent::AuditDropped {
            request_id: u64::MAX,
            request_type: "set".into(),
            error: "offline".into(),
            time: Duration::from_millis(10),
        },
        RuntimeEvent::FeatureDisabled {
            feature: "opcua".into(),
            request_type: Some("opcua.status".into()),
            time: Duration::from_millis(11),
        },
    ];

    assert_eq!(events, events.clone());
    assert_ne!(events[0], events[1]);
    assert!(matches!(
        &events[4],
        RuntimeEvent::TaskOverrun {
            missed: u64::MAX,
            ..
        }
    ));
    assert!(matches!(
        &events[9],
        RuntimeEvent::AuditDropped {
            request_id: u64::MAX,
            request_type,
            ..
        } if request_type == "set"
    ));
}

#[test]
fn stop_reason_variants_remain_distinct() {
    assert_ne!(DebugStopReason::Breakpoint, DebugStopReason::Step);
    assert_ne!(DebugStopReason::Step, DebugStopReason::Pause);
    assert_ne!(DebugStopReason::Pause, DebugStopReason::Entry);
}

#[test]
fn log_fragments_preserve_text_and_expression_kinds() {
    let fragments = [
        LogFragment::Text("literal".into()),
        LogFragment::Expr(Expr::Literal(Value::DInt(7))),
    ];

    assert!(matches!(&fragments[0], LogFragment::Text(text) if text == "literal"));
    assert!(matches!(
        &fragments[1],
        LogFragment::Expr(Expr::Literal(Value::DInt(7)))
    ));
}
