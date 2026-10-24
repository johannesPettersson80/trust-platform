//! Debug expressions for a runtime that runs a compiled program (specification 11 §6.13.2): the
//! adapter compiles each one into the portable form with the types of the project the source
//! options name, loaded from its declarations, and the runtime evaluates it. A breakpoint's
//! condition and a logpoint's expressions go to the runtime so compiled, with its hit condition.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_hir::types::TypeRegistry;
use trust_runtime::debug::HitCondition;
use trust_runtime::harness::compile_debug_expression;
use trust_runtime::value::DateTimeProfile;

use crate::protocol::SourceBreakpoint;
use crate::session::{parse_hit_condition, split_log_message, LogText};

use super::super::remote::{compiled_expression, RemoteSession};
use super::super::DebugAdapter;

/// Why a breakpoint's options do not go to a runtime that runs its program on the bytecode engine,
/// which evaluates no compiled expression and takes no options (specification 11 §6.13.2).
const ENGINE_OPTIONS: &str = "this runtime runs its program on the bytecode engine, which takes \
     no conditions, hit counts or logpoints while attached: build the project compiled";

/// The shortest time after which an expression the loaded types do not compile loads them again:
/// a type declared since they were loaded compiles after an online change, and a mistyped watch
/// does not check the project at every evaluation.
const RELOAD_AFTER: Duration = Duration::from_secs(2);

/// The project's types, loaded from its declarations.
pub(in crate::adapter) struct ExpressionTypes {
    registry: TypeRegistry,
    profile: DateTimeProfile,
    loaded: Instant,
}

impl DebugAdapter {
    /// The options of `breakpoint` as `breakpoints.set` carries them (specification 11 §6.13.2):
    /// its condition and its logpoint's expressions compiled, and its hit condition; `None` for a
    /// breakpoint without any.
    ///
    /// # Errors
    ///
    /// The reason the breakpoint cannot go to the runtime with its options: a runtime on the
    /// bytecode engine, an expression that does not compile, a hit condition or a message
    /// template that does not parse.
    pub(in crate::adapter) fn remote_options(
        &mut self,
        breakpoint: &SourceBreakpoint,
    ) -> Result<Option<Value>, String> {
        let condition = breakpoint
            .condition
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let hit = breakpoint
            .hit_condition
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let template = breakpoint
            .log_message
            .as_deref()
            .filter(|text| !text.is_empty());
        if condition.is_none() && hit.is_none() && template.is_none() {
            return Ok(None);
        }
        if !self
            .remote_session
            .as_mut()
            .is_some_and(RemoteSession::compiled)
        {
            return Err(String::from(ENGINE_OPTIONS));
        }
        let mut options = serde_json::Map::new();
        if let Some(condition) = condition {
            let bytes = self.compile_for_runtime(condition)?;
            options.insert(String::from("condition"), compiled_expression(&bytes));
        }
        if let Some(hit) = hit {
            let (op, count) = match parse_hit_condition(hit) {
                Some(HitCondition::Equal(count)) => ("=", count),
                Some(HitCondition::AtLeast(count)) => (">=", count),
                Some(HitCondition::GreaterThan(count)) => (">", count),
                None => return Err(format!("invalid hit condition '{hit}'")),
            };
            options.insert(
                String::from("hit_condition"),
                json!({ "op": op, "count": count }),
            );
        }
        if let Some(template) = template {
            let mut parts = Vec::new();
            for part in split_log_message(template)? {
                parts.push(match part {
                    LogText::Text(text) => Value::String(text),
                    LogText::Expression(expression) => {
                        compiled_expression(&self.compile_for_runtime(&expression)?)
                    }
                });
            }
            options.insert(String::from("log_message"), Value::Array(parts));
        }
        Ok(Some(Value::Object(options)))
    }

    /// The compiled form of `expression`, version 1, compiled with the project's types; an
    /// expression they do not compile loads them again once they are older than [`RELOAD_AFTER`].
    /// A project that does not check gives the elementary types alone.
    pub(in crate::adapter) fn compile_for_runtime(
        &mut self,
        expression: &str,
    ) -> Result<Vec<u8>, String> {
        let compile = |types: &ExpressionTypes| {
            let mut registry = types.registry.clone();
            compile_debug_expression(expression, &mut registry, types.profile, &[])
                .map_err(|error| error.to_string())
        };
        if let Some(types) = self.expression_types.as_ref() {
            let compiled = compile(types);
            if compiled.is_ok() || types.loaded.elapsed() < RELOAD_AFTER {
                return compiled;
            }
        }
        let (registry, profile) = self.session.project_expression_types().unwrap_or_else(|_| {
            let metadata = self.session.metadata();
            (metadata.registry().clone(), metadata.profile())
        });
        let types = ExpressionTypes {
            registry,
            profile,
            loaded: Instant::now(),
        };
        let compiled = compile(&types);
        self.expression_types = Some(types);
        compiled
    }
}
