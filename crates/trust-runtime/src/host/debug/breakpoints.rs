//! Breakpoint and logpoint evaluation.

#![allow(missing_docs)]

use std::sync::mpsc::Sender;

use crate::value::Value;

use super::hook::DebugRuntimeContext;
use super::{DebugBreakpoint, DebugLog, LogFragment, SourceLocation};

pub(crate) fn matches_breakpoint(
    breakpoints: &mut [DebugBreakpoint],
    logs: &mut Vec<DebugLog>,
    log_tx: Option<&Sender<DebugLog>>,
    location: &SourceLocation,
    ctx: &mut Option<&mut DebugRuntimeContext<'_>>,
) -> Option<u64> {
    for breakpoint in breakpoints.iter_mut() {
        let bp_location = &breakpoint.location;
        if bp_location.file_id != location.file_id {
            continue;
        }
        // Source mapping can resolve to nearby-but-not-identical spans after
        // edits/reformatting; treat overlapping ranges as the same statement.
        let overlaps = location.start < bp_location.end && bp_location.start < location.end;
        if !overlaps {
            continue;
        }
        breakpoint.hits = breakpoint.hits.saturating_add(1);
        if let Some(hit_condition) = breakpoint.hit_condition {
            if !hit_condition.is_met(breakpoint.hits) {
                continue;
            }
        }
        if let Some(condition) = &breakpoint.condition {
            let Some(eval_ctx) = ctx.as_deref_mut() else {
                continue;
            };
            if !condition_matches(eval_ctx, condition) {
                continue;
            }
        }
        if let Some(message) = &breakpoint.log_message {
            if let Some(eval_ctx) = ctx.as_deref_mut() {
                let formatted = format_log_message(eval_ctx, message);
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
        return Some(breakpoint.generation);
    }
    None
}

fn condition_matches(
    ctx: &mut DebugRuntimeContext<'_>,
    condition: &crate::program_model::Expr,
) -> bool {
    match crate::helper_eval::eval_storage_expr_with_stdlib(
        ctx.storage,
        ctx.registry,
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

fn format_log_message(ctx: &mut DebugRuntimeContext<'_>, fragments: &[LogFragment]) -> String {
    let mut output = String::new();
    for fragment in fragments {
        match fragment {
            LogFragment::Text(text) => output.push_str(text),
            LogFragment::Expr(expr) => match crate::helper_eval::eval_storage_expr_with_stdlib(
                ctx.storage,
                ctx.registry,
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
        Value::Instance(value) => format!("Instance({})", value.0),
        Value::Null => "NULL".to_string(),
        _ => format!("{value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_match_on_overlapping_location() {
        let outer = SourceLocation::new(0, 0, 20);
        let inner = SourceLocation::new(0, 5, 10);
        let mut breakpoints = vec![DebugBreakpoint::new(inner)];
        let mut logs = Vec::new();
        let mut ctx = None;

        assert!(matches_breakpoint(&mut breakpoints, &mut logs, None, &outer, &mut ctx).is_some());
        assert!(matches_breakpoint(&mut breakpoints, &mut logs, None, &inner, &mut ctx).is_some());
    }

    #[test]
    fn breakpoints_do_not_match_non_overlapping_location() {
        let left = SourceLocation::new(0, 0, 5);
        let right = SourceLocation::new(0, 6, 10);
        let mut breakpoints = vec![DebugBreakpoint::new(left)];
        let mut logs = Vec::new();
        let mut ctx = None;

        assert!(matches_breakpoint(&mut breakpoints, &mut logs, None, &right, &mut ctx).is_none());
    }
}
