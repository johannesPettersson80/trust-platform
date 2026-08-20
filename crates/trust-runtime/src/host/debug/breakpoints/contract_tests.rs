use std::sync::mpsc::{channel, TryRecvError};

use crate::memory::VariableStorage;
use crate::program_model::Expr;
use crate::value::{DateTimeProfile, Duration};
use trust_hir::types::TypeRegistry;

use super::*;

struct EvalFixture {
    storage: VariableStorage,
    registry: TypeRegistry,
}

impl EvalFixture {
    fn new() -> Self {
        Self {
            storage: VariableStorage::new(),
            registry: TypeRegistry::new(),
        }
    }

    fn context(&mut self) -> DebugRuntimeContext<'_> {
        DebugRuntimeContext {
            storage: &mut self.storage,
            registry: &self.registry,
            stdlib: None,
            profile: DateTimeProfile::default(),
            current_instance: None,
            now: Duration::from_millis(7),
        }
    }
}

fn matched(
    breakpoints: &mut [DebugBreakpoint],
    stopped: &HashSet<BreakpointCycleKey>,
    logs: &mut Vec<DebugLog>,
    location: SourceLocation,
) -> Option<(u64, BreakpointCycleKey)> {
    matches_breakpoint(breakpoints, stopped, logs, None, &location, &mut None)
}

#[test]
fn breakpoint_requires_same_file_identity() {
    let mut breakpoints = vec![DebugBreakpoint::new(SourceLocation::new(1, 10, 20))];
    assert_eq!(
        matched(
            &mut breakpoints,
            &HashSet::new(),
            &mut Vec::new(),
            SourceLocation::new(2, 10, 20),
        ),
        None
    );
    assert_eq!(breakpoints[0].hits, 0);
}

#[test]
fn touching_half_open_spans_do_not_overlap() {
    let mut breakpoints = vec![DebugBreakpoint::new(SourceLocation::new(1, 10, 20))];
    for location in [
        SourceLocation::new(1, 0, 10),
        SourceLocation::new(1, 20, 30),
    ] {
        assert_eq!(
            matched(&mut breakpoints, &HashSet::new(), &mut Vec::new(), location),
            None
        );
    }
    assert_eq!(breakpoints[0].hits, 0);
}

#[test]
fn one_byte_overlap_matches_and_returns_generation_identity() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.generation = 7;
    let mut breakpoints = vec![breakpoint];
    assert_eq!(
        matched(
            &mut breakpoints,
            &HashSet::new(),
            &mut Vec::new(),
            SourceLocation::new(1, 19, 30),
        ),
        Some((7, (1, 10, 20, 7)))
    );
}

#[test]
fn first_eligible_breakpoint_in_registration_order_owns_stop() {
    let mut first = DebugBreakpoint::new(SourceLocation::new(1, 0, 20));
    first.generation = 1;
    let mut second = DebugBreakpoint::new(SourceLocation::new(1, 10, 30));
    second.generation = 2;
    let mut breakpoints = vec![first, second];

    assert_eq!(
        matched(
            &mut breakpoints,
            &HashSet::new(),
            &mut Vec::new(),
            SourceLocation::new(1, 15, 16),
        ),
        Some((1, (1, 0, 20, 1)))
    );
    assert_eq!(breakpoints[0].hits, 1);
    assert_eq!(breakpoints[1].hits, 0);
}

#[test]
fn exact_cycle_guard_suppresses_stop_without_incrementing_hit() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.generation = 7;
    let mut breakpoints = vec![breakpoint];
    let stopped = HashSet::from([(1, 10, 20, 7)]);

    assert_eq!(
        matched(
            &mut breakpoints,
            &stopped,
            &mut Vec::new(),
            SourceLocation::new(1, 10, 20),
        ),
        None
    );
    assert_eq!(breakpoints[0].hits, 0);
}

#[test]
fn cycle_guard_does_not_suppress_later_generation() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.generation = 8;
    let mut breakpoints = vec![breakpoint];
    let stopped = HashSet::from([(1, 10, 20, 7)]);

    assert_eq!(
        matched(
            &mut breakpoints,
            &stopped,
            &mut Vec::new(),
            SourceLocation::new(1, 10, 20),
        ),
        Some((8, (1, 10, 20, 8)))
    );
}

#[test]
fn hit_count_increments_before_hit_condition_evaluation() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.hit_condition = Some(super::super::HitCondition::Equal(2));
    let mut breakpoints = vec![breakpoint];
    let location = SourceLocation::new(1, 10, 20);

    assert_eq!(
        matched(&mut breakpoints, &HashSet::new(), &mut Vec::new(), location),
        None
    );
    assert_eq!(breakpoints[0].hits, 1);
    assert!(matched(&mut breakpoints, &HashSet::new(), &mut Vec::new(), location).is_some());
    assert_eq!(breakpoints[0].hits, 2);
}

#[test]
fn hit_counter_saturates_at_unsigned_maximum() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.hits = u64::MAX;
    let mut breakpoints = vec![breakpoint];
    assert!(matched(
        &mut breakpoints,
        &HashSet::new(),
        &mut Vec::new(),
        SourceLocation::new(1, 10, 20),
    )
    .is_some());
    assert_eq!(breakpoints[0].hits, u64::MAX);
}

#[test]
fn true_boolean_condition_allows_stop() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.condition = Some(Expr::Literal(Value::Bool(true)));
    let mut breakpoints = vec![breakpoint];
    let mut fixture = EvalFixture::new();
    let mut eval = fixture.context();
    let mut ctx = Some(&mut eval);

    assert!(matches_breakpoint(
        &mut breakpoints,
        &HashSet::new(),
        &mut Vec::new(),
        None,
        &SourceLocation::new(1, 10, 20),
        &mut ctx,
    )
    .is_some());
}

#[test]
fn false_non_boolean_and_error_conditions_do_not_stop_but_count_hits() {
    for condition in [
        Expr::Literal(Value::Bool(false)),
        Expr::Literal(Value::DInt(1)),
        Expr::Name("missing".into()),
    ] {
        let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
        breakpoint.condition = Some(condition);
        let mut breakpoints = vec![breakpoint];
        let mut fixture = EvalFixture::new();
        let mut eval = fixture.context();
        let mut ctx = Some(&mut eval);
        assert!(matches_breakpoint(
            &mut breakpoints,
            &HashSet::new(),
            &mut Vec::new(),
            None,
            &SourceLocation::new(1, 10, 20),
            &mut ctx,
        )
        .is_none());
        assert_eq!(breakpoints[0].hits, 1);
    }
}

#[test]
fn conditional_breakpoint_without_context_does_not_stop_but_counts_hit() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.condition = Some(Expr::Literal(Value::Bool(true)));
    let mut breakpoints = vec![breakpoint];
    assert_eq!(
        matched(
            &mut breakpoints,
            &HashSet::new(),
            &mut Vec::new(),
            SourceLocation::new(1, 10, 20),
        ),
        None
    );
    assert_eq!(breakpoints[0].hits, 1);
}

#[test]
fn logpoint_formats_fragments_in_registration_order_and_never_stops() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.log_message = Some(vec![
        LogFragment::Text("value=".to_string()),
        LogFragment::Expr(Expr::Literal(Value::DInt(7))),
        LogFragment::Text(" done".to_string()),
    ]);
    let mut breakpoints = vec![breakpoint];
    let mut fixture = EvalFixture::new();
    let mut eval = fixture.context();
    let mut ctx = Some(&mut eval);
    let mut logs = Vec::new();

    assert!(matches_breakpoint(
        &mut breakpoints,
        &HashSet::new(),
        &mut logs,
        None,
        &SourceLocation::new(1, 10, 20),
        &mut ctx,
    )
    .is_none());
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].message, "value=DInt(7) done");
    assert_eq!(logs[0].location, Some(SourceLocation::new(1, 10, 20)));
}

#[test]
fn logpoint_expression_error_is_rendered_in_place() {
    let mut fixture = EvalFixture::new();
    let mut ctx = fixture.context();
    let message = format_log_message(
        &mut ctx,
        &[
            LogFragment::Text("before ".to_string()),
            LogFragment::Expr(Expr::Name("missing".into())),
            LogFragment::Text(" after".to_string()),
        ],
    );
    assert!(message.starts_with("before <error: "));
    assert!(message.ends_with("> after"));
}

#[test]
fn live_log_receiver_gets_one_record_without_buffer_duplicate() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.log_message = Some(vec![LogFragment::Text("message".to_string())]);
    let mut breakpoints = vec![breakpoint];
    let mut fixture = EvalFixture::new();
    let mut eval = fixture.context();
    let mut ctx = Some(&mut eval);
    let mut logs = Vec::new();
    let (tx, rx) = channel();

    assert!(matches_breakpoint(
        &mut breakpoints,
        &HashSet::new(),
        &mut logs,
        Some(&tx),
        &SourceLocation::new(1, 10, 20),
        &mut ctx,
    )
    .is_none());
    assert_eq!(rx.try_recv().expect("log").message, "message");
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    assert!(logs.is_empty());
}

#[test]
fn closed_log_receiver_falls_back_to_local_buffer() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.log_message = Some(vec![LogFragment::Text("message".to_string())]);
    let mut breakpoints = vec![breakpoint];
    let mut fixture = EvalFixture::new();
    let mut eval = fixture.context();
    let mut ctx = Some(&mut eval);
    let mut logs = Vec::new();
    let (tx, rx) = channel();
    drop(rx);

    assert!(matches_breakpoint(
        &mut breakpoints,
        &HashSet::new(),
        &mut logs,
        Some(&tx),
        &SourceLocation::new(1, 10, 20),
        &mut ctx,
    )
    .is_none());
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].message, "message");
}

#[test]
fn logpoint_cycle_guard_does_not_suppress_output() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.generation = 7;
    breakpoint.log_message = Some(vec![LogFragment::Text("message".to_string())]);
    let mut breakpoints = vec![breakpoint];
    let mut fixture = EvalFixture::new();
    let mut eval = fixture.context();
    let mut ctx = Some(&mut eval);
    let mut logs = Vec::new();

    assert!(matches_breakpoint(
        &mut breakpoints,
        &HashSet::from([(1, 10, 20, 7)]),
        &mut logs,
        None,
        &SourceLocation::new(1, 10, 20),
        &mut ctx,
    )
    .is_none());
    assert_eq!(breakpoints[0].hits, 1);
    assert_eq!(logs.len(), 1);
}

#[test]
fn logpoint_without_evaluation_context_counts_but_emits_nothing() {
    let mut breakpoint = DebugBreakpoint::new(SourceLocation::new(1, 10, 20));
    breakpoint.log_message = Some(vec![LogFragment::Text("message".to_string())]);
    let mut breakpoints = vec![breakpoint];
    let mut logs = Vec::new();
    assert_eq!(
        matched(
            &mut breakpoints,
            &HashSet::new(),
            &mut logs,
            SourceLocation::new(1, 10, 20),
        ),
        None
    );
    assert_eq!(breakpoints[0].hits, 1);
    assert!(logs.is_empty());
}

#[test]
fn log_value_boolean_string_and_character_forms_are_stable() {
    assert_eq!(format_log_value(&Value::Bool(true)), "TRUE");
    assert_eq!(format_log_value(&Value::Bool(false)), "FALSE");
    assert_eq!(format_log_value(&Value::String("text".into())), "text");
    assert_eq!(
        format_log_value(&Value::WString("wide".to_string())),
        "wide"
    );
    assert_eq!(format_log_value(&Value::Char(b'A')), "A");
    assert_eq!(format_log_value(&Value::WChar('å' as u16)), "å");
    assert_eq!(format_log_value(&Value::WChar(0xD800)), "?");
}

#[test]
fn log_value_reference_instance_and_null_forms_are_stable() {
    assert_eq!(format_log_value(&Value::Reference(None)), "NULL_REF");
    assert_eq!(format_log_value(&Value::Null), "NULL");
    assert_eq!(
        format_log_value(&Value::Instance(crate::memory::InstanceId(7))),
        "Instance"
    );
}

#[test]
fn ordinary_scalar_log_values_use_debug_representation() {
    assert_eq!(format_log_value(&Value::DInt(-7)), "DInt(-7)");
    assert_eq!(format_log_value(&Value::LReal(1.5)), "LReal(1.5)");
}
