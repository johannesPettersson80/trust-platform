//! Breakpoint and logpoint evaluation.

#![allow(missing_docs)]

use std::collections::HashSet;
use std::sync::mpsc::Sender;

use crate::value::Value;
use trust_runtime_services::debug::{
    count_hit, observe_hit, BreakpointRecord, HitCondition, Span, StatementId,
};

use super::hook::DebugRuntimeContext;
use super::{DebugBreakpoint, DebugLog, LogFragment, SourceLocation};

pub(crate) type BreakpointCycleKey = (u32, u32, u32, u64);

pub(crate) fn matches_breakpoint(
    breakpoints: &mut [DebugBreakpoint],
    stopped_this_cycle: &HashSet<BreakpointCycleKey>,
    logs: &mut Vec<DebugLog>,
    log_tx: Option<&Sender<DebugLog>>,
    location: &SourceLocation,
    ctx: &mut Option<&mut DebugRuntimeContext<'_>>,
) -> Option<(u64, BreakpointCycleKey)> {
    for breakpoint in breakpoints.iter_mut() {
        // The shared hit rule decides the match, counts the hit and applies the hit condition, in
        // the order of specification 13's hook contract ("Breakpoint evaluation, source resolution,
        // and hook contract"). A statement matches when it starts inside the breakpoint's half-open
        // span, so an enclosing statement, such as a FOR, IF, WHILE or CASE whose span covers its
        // body, does not match a breakpoint on a body statement. A cycle-suppressed hit counts
        // nothing, and a logpoint is never suppressed.
        let is_logpoint = breakpoint.log_message.is_some();
        let passes = observe_hit(
            breakpoint,
            |id| ordinary_stop_suppressed(stopped_this_cycle, is_logpoint, id),
            location.file_id,
            location.start,
        );
        if !passes {
            continue;
        }
        let cycle_key = cycle_key_of(&breakpoint.statement());
        if let Some(condition) = &breakpoint.condition {
            let Some(eval_ctx) = ctx.as_deref_mut() else {
                continue;
            };
            if !condition_matches_with_registry(
                eval_ctx,
                breakpoint.type_registry.as_deref(),
                condition,
            ) {
                continue;
            }
        }
        if let Some(message) = &breakpoint.log_message {
            if let Some(eval_ctx) = ctx.as_deref_mut() {
                let formatted = format_log_message_with_registry(
                    eval_ctx,
                    breakpoint.type_registry.as_deref(),
                    message,
                );
                let log = DebugLog {
                    message: formatted,
                    location: Some(*location),
                };
                if let Some(sender) = log_tx {
                    if sender.send(log.clone()).is_err() {
                        logs.push(log);
                    }
                } else {
                    logs.push(log);
                }
            }
            continue;
        }
        return Some((breakpoint.generation, cycle_key));
    }
    None
}

/// The shared hit rule reads a `DebugBreakpoint` through this view: the statement it names at its
/// generation, its hit condition and its count (specification 13).
impl BreakpointRecord<u32> for DebugBreakpoint {
    fn statement(&self) -> StatementId<u32> {
        StatementId {
            file_id: self.location.file_id,
            span: Span::new(self.location.start, self.location.end),
            generation: self.generation,
        }
    }

    fn hit_condition(&self) -> Option<HitCondition> {
        self.hit_condition
    }

    fn hits(&self) -> u64 {
        self.hits
    }

    fn set_hits(&mut self, hits: u64) {
        self.hits = hits;
    }
}

/// The cycle-suppression key of a statement identity: the `(file_id,start,end,generation)` tuple
/// the cycle set stores (specification 13). The set keeps its tuple type, which its tests build.
fn cycle_key_of(id: &StatementId<u32>) -> BreakpointCycleKey {
    (id.file_id, id.span.start, id.span.end, id.generation)
}

/// Whether the ordinary stop of `id` is cycle-suppressed in this scan. A logpoint never stops, so
/// the cycle guard never suppresses its output (specification 13).
fn ordinary_stop_suppressed(
    stopped_this_cycle: &HashSet<BreakpointCycleKey>,
    is_logpoint: bool,
    id: &StatementId<u32>,
) -> bool {
    !is_logpoint && stopped_this_cycle.contains(&cycle_key_of(id))
}

/// Whether a breakpoint at `location` evaluates a condition or a logpoint, which needs the storage.
/// It reads the same hit rule as `matches_breakpoint` without counting a hit, so the storage is
/// built exactly when a condition or logpoint runs.
pub(crate) fn any_breakpoint_needs_context(
    breakpoints: &[DebugBreakpoint],
    stopped_this_cycle: &HashSet<BreakpointCycleKey>,
    location: &SourceLocation,
) -> bool {
    breakpoints.iter().any(|breakpoint| {
        let is_logpoint = breakpoint.log_message.is_some();
        (breakpoint.condition.is_some() || is_logpoint)
            && count_hit(
                breakpoint,
                |id| ordinary_stop_suppressed(stopped_this_cycle, is_logpoint, id),
                location.file_id,
                location.start,
            )
            .is_some_and(|hit| hit.passes)
    })
}

fn condition_matches(
    ctx: &mut DebugRuntimeContext<'_>,
    condition: &crate::program_model::Expr,
) -> bool {
    match crate::helper_eval::eval_storage_expr_with_abi(
        ctx.storage,
        ctx.registry,
        ctx.storage_abi,
        &ctx.profile,
        ctx.current_instance,
        ctx.stdlib,
        condition,
    ) {
        Ok(Value::Bool(true)) => true,
        Ok(Value::Bool(false)) => false,
        Ok(_) => false,
        Err(_) => false,
    }
}

fn condition_matches_with_registry(
    ctx: &mut DebugRuntimeContext<'_>,
    registry: Option<&trust_hir::types::TypeRegistry>,
    condition: &crate::program_model::Expr,
) -> bool {
    let mut scoped = scoped_context(ctx, registry);
    condition_matches(&mut scoped, condition)
}

fn format_log_message(ctx: &mut DebugRuntimeContext<'_>, fragments: &[LogFragment]) -> String {
    let mut output = String::new();
    for fragment in fragments {
        match fragment {
            LogFragment::Text(text) => output.push_str(text),
            LogFragment::Expr(expr) => match crate::helper_eval::eval_storage_expr_with_abi(
                ctx.storage,
                ctx.registry,
                ctx.storage_abi,
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib,
                expr,
            ) {
                Ok(value) => output.push_str(&format_log_value(&value)),
                Err(err) => output.push_str(&format!("<error: {err}>")),
            },
        }
    }
    output
}

fn format_log_message_with_registry(
    ctx: &mut DebugRuntimeContext<'_>,
    registry: Option<&trust_hir::types::TypeRegistry>,
    fragments: &[LogFragment],
) -> String {
    let mut scoped = scoped_context(ctx, registry);
    format_log_message(&mut scoped, fragments)
}

fn scoped_context<'a>(
    ctx: &'a mut DebugRuntimeContext<'_>,
    registry: Option<&'a trust_hir::types::TypeRegistry>,
) -> DebugRuntimeContext<'a> {
    DebugRuntimeContext {
        storage: &mut *ctx.storage,
        registry: registry.unwrap_or(ctx.registry),
        stdlib: ctx.stdlib,
        profile: ctx.profile,
        current_instance: ctx.current_instance,
        storage_abi: ctx.storage_abi,
        now: ctx.now,
    }
}

fn format_log_value(value: &Value) -> String {
    match value {
        Value::Bool(value) => {
            if *value {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        Value::String(value) => value.to_string(),
        Value::WString(value) => value.clone(),
        Value::Char(value) => (*value as char).to_string(),
        Value::WChar(value) => char::from_u32((*value).into()).unwrap_or('?').to_string(),
        Value::Array(value) => format!("[{}]", value.elements().len()),
        Value::Struct(value) => format!("{} {{...}}", value.type_name()),
        Value::Enum(value) => format!("{}::{}", value.type_name(), value.variant_name()),
        Value::Reference(Some(_)) => "REF".to_string(),
        Value::Reference(None) => "NULL_REF".to_string(),
        Value::Instance(_) => "Instance".to_string(),
        Value::Null => "NULL".to_string(),
        _ => format!("{value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_match_a_statement_that_starts_inside_their_span() {
        let outer = SourceLocation::new(0, 0, 20);
        let inner = SourceLocation::new(0, 5, 10);
        let mut breakpoints = vec![DebugBreakpoint::new(inner)];
        let stopped_this_cycle = HashSet::new();
        let mut logs = Vec::new();
        let mut ctx = None;

        // A01 decision (spec 13, "Breakpoint evaluation, source resolution, and hook contract"): a
        // statement matches only when it starts inside the breakpoint's half-open span, so the
        // enclosing span 0..20 does not match the breakpoint at 5..10.
        assert!(matches_breakpoint(
            &mut breakpoints,
            &stopped_this_cycle,
            &mut logs,
            None,
            &outer,
            &mut ctx
        )
        .is_none());
        assert!(matches_breakpoint(
            &mut breakpoints,
            &stopped_this_cycle,
            &mut logs,
            None,
            &inner,
            &mut ctx
        )
        .is_some());
    }

    #[test]
    fn breakpoints_do_not_match_non_overlapping_location() {
        let left = SourceLocation::new(0, 0, 5);
        let right = SourceLocation::new(0, 6, 10);
        let mut breakpoints = vec![DebugBreakpoint::new(left)];
        let stopped_this_cycle = HashSet::new();
        let mut logs = Vec::new();
        let mut ctx = None;

        assert!(matches_breakpoint(
            &mut breakpoints,
            &stopped_this_cycle,
            &mut logs,
            None,
            &right,
            &mut ctx
        )
        .is_none());
    }

    #[test]
    fn breakpoint_stop_suppression_is_identity_scoped() {
        let first = SourceLocation::new(0, 0, 20);
        let second = SourceLocation::new(0, 30, 40);
        let mut first_breakpoint = DebugBreakpoint::new(first);
        first_breakpoint.generation = 7;
        let mut second_breakpoint = DebugBreakpoint::new(second);
        second_breakpoint.generation = 7;
        let mut breakpoints = vec![first_breakpoint, second_breakpoint];
        let stopped_this_cycle = HashSet::from([(0, 0, 20, 7)]);
        let mut logs = Vec::new();
        let mut ctx = None;

        assert!(matches_breakpoint(
            &mut breakpoints,
            &stopped_this_cycle,
            &mut logs,
            None,
            &first,
            &mut ctx
        )
        .is_none());
        assert_eq!(breakpoints[0].hits, 0);

        let matched = matches_breakpoint(
            &mut breakpoints,
            &stopped_this_cycle,
            &mut logs,
            None,
            &second,
            &mut ctx,
        );
        assert_eq!(matched, Some((7, (0, 30, 40, 7))));
        assert_eq!(breakpoints[1].hits, 1);
    }

    // Spans worked out by hand from the hosted probe program: the FOR statement spans 88..156 and
    // its body statement `Scan := Scan + i;` spans 131..149.

    #[test]
    fn an_enclosing_statement_that_starts_before_a_body_breakpoint_does_not_match() {
        // Wrong behaviour caught: the old overlap rule matched the enclosing FOR statement
        // first, so a breakpoint on the body stopped at the loop header.
        let body = SourceLocation::new(0, 131, 149);
        let for_statement = SourceLocation::new(0, 88, 156);
        let mut breakpoint = DebugBreakpoint::new(body);
        breakpoint.generation = 1;
        let mut breakpoints = vec![breakpoint];

        assert_eq!(
            matches_breakpoint(
                &mut breakpoints,
                &HashSet::new(),
                &mut Vec::new(),
                None,
                &for_statement,
                &mut None,
            ),
            None
        );
        assert_eq!(breakpoints[0].hits, 0);
    }

    #[test]
    fn a_statement_starting_at_131_matches_the_body_breakpoint_at_131_to_149() {
        // Wrong behaviour caught: a rule that needs the statement to start strictly after the
        // breakpoint start misses the breakpoint's own body statement.
        let body = SourceLocation::new(0, 131, 149);
        let mut breakpoint = DebugBreakpoint::new(body);
        breakpoint.generation = 1;
        let mut breakpoints = vec![breakpoint];

        assert_eq!(
            matches_breakpoint(
                &mut breakpoints,
                &HashSet::new(),
                &mut Vec::new(),
                None,
                &body,
                &mut None,
            ),
            Some((1, (0, 131, 149, 1)))
        );
    }

    #[test]
    fn a_statement_starting_at_the_breakpoint_end_does_not_match() {
        // Wrong behaviour caught: an inclusive end (`start <= end`) matches the next statement,
        // so the breakpoint stops one statement late. The span is half-open.
        let body = SourceLocation::new(0, 131, 149);
        let next = SourceLocation::new(0, 149, 160);
        let mut breakpoint = DebugBreakpoint::new(body);
        breakpoint.generation = 1;
        let mut breakpoints = vec![breakpoint];

        assert_eq!(
            matches_breakpoint(
                &mut breakpoints,
                &HashSet::new(),
                &mut Vec::new(),
                None,
                &next,
                &mut None,
            ),
            None
        );
    }

    #[test]
    fn a_statement_starting_inside_an_if_line_breakpoint_matches() {
        // Wrong behaviour caught: a rule that matches only a breakpoint's exact span would
        // miss the IF body, which the documented IF-line case relies on.
        let if_statement = SourceLocation::new(0, 88, 156);
        let body = SourceLocation::new(0, 131, 149);
        let mut breakpoint = DebugBreakpoint::new(if_statement);
        breakpoint.generation = 1;
        let mut breakpoints = vec![breakpoint];

        assert_eq!(
            matches_breakpoint(
                &mut breakpoints,
                &HashSet::new(),
                &mut Vec::new(),
                None,
                &body,
                &mut None,
            ),
            Some((1, (0, 88, 156, 1)))
        );
    }
}

#[cfg(test)]
#[path = "breakpoints/contract_tests.rs"]
mod contract_tests;
