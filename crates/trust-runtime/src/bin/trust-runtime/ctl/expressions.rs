//! The debug expressions of `ctl` (specification 11 §6.13.2): a breakpoint's condition and the
//! parts of its log message, and the expression of `evaluate`, which the tool compiles into the
//! portable form the runtime evaluates, with the types of the project when one is named or the
//! current folder holds one.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use base64::Engine as _;
use serde_json::{json, Value as JsonValue};
use trust_hir::types::TypeRegistry;
use trust_runtime::harness::{compile_debug_expression, project_expression_types};
use trust_runtime::value::DateTimeProfile;
use trust_runtime_services::debug::expression::EXPR_VERSION;

use crate::cli::ControlAction;

/// Compiles debug expressions with the types of one project, or with none.
pub(crate) struct Expressions {
    registry: TypeRegistry,
    profile: DateTimeProfile,
}

impl Expressions {
    /// With the types of the project at `project`, else of the current folder when it holds a
    /// `runtime.toml`, else with no project's types, which serve names, literals and operators.
    ///
    /// # Errors
    ///
    /// The errors of a project whose sources do not check.
    pub(crate) fn for_project(project: Option<&Path>) -> anyhow::Result<Self> {
        let root = project.map(Path::to_path_buf).or_else(|| {
            let here = PathBuf::from(".");
            here.join("runtime.toml").is_file().then_some(here)
        });
        let Some(root) = root else {
            return Ok(Self {
                registry: TypeRegistry::new(),
                profile: DateTimeProfile::default(),
            });
        };
        let sources = trust_runtime::bundle_builder::collect_project_sources_labelled(&root, None)?;
        let conversions = trust_runtime::native::conversion_profile(&root)?;
        let (registry, profile) = project_expression_types(&sources, conversions)
            .map_err(|error| anyhow!("the project's types for debug expressions: {error}"))?;
        Ok(Self { registry, profile })
    }

    /// The compiled form of `text` as the protocol carries it: `expr_version` and `expr`.
    ///
    /// # Errors
    ///
    /// The error of text that is no debug expression of version 1.
    pub(crate) fn compile(&mut self, text: &str) -> anyhow::Result<JsonValue> {
        let bytes = compile_debug_expression(text, &mut self.registry, self.profile, &[])
            .map_err(|error| anyhow!("the debug expression `{text}`: {error}"))?;
        Ok(json!({
            "expr_version": EXPR_VERSION,
            "expr": base64::engine::general_purpose::STANDARD.encode(bytes),
        }))
    }
}

/// Adds to `request`, built for `action`, the compiled expressions it carries: the options of a
/// `breakpoints-set` that has any, one for each line, and the expression of `evaluate`.
///
/// # Errors
///
/// The errors of [`Expressions::for_project`], of an expression that does not compile, and of a
/// hit condition or a log message that is not as the command takes it.
pub(crate) fn attach(
    request: &mut JsonValue,
    action: &ControlAction,
    project: Option<&Path>,
) -> anyhow::Result<()> {
    match action {
        ControlAction::BreakpointsSet {
            lines,
            condition,
            hit_condition,
            log_message,
            ..
        } => {
            if condition.is_none() && hit_condition.is_none() && log_message.is_none() {
                return Ok(());
            }
            let mut expressions = Expressions::for_project(project)?;
            let mut options = serde_json::Map::new();
            if let Some(text) = condition {
                options.insert("condition".into(), expressions.compile(text)?);
            }
            if let Some(text) = hit_condition {
                options.insert("hit_condition".into(), hit(text)?);
            }
            if let Some(text) = log_message {
                options.insert("log_message".into(), message_parts(text, &mut expressions)?);
            }
            request["params"]["options"] = JsonValue::Array(vec![options.into(); lines.len()]);
        }
        ControlAction::Evaluate { expr, .. } => {
            let compiled = Expressions::for_project(project)?.compile(expr)?;
            request["params"]["expr_version"] = compiled["expr_version"].clone();
            request["params"]["expr"] = compiled["expr"].clone();
        }
        _ => {}
    }
    Ok(())
}

/// `compile-expression`: prints the compiled form of `text`, as a request carries it.
///
/// # Errors
///
/// As [`attach`].
pub(crate) fn print_compiled(text: &str, project: Option<&Path>) -> anyhow::Result<()> {
    let compiled = Expressions::for_project(project)?.compile(text)?;
    println!("{}", serde_json::to_string(&compiled)?);
    Ok(())
}

/// A hit condition as the command takes it, `=N`, `>=N` or `>N` with N from 1 (`==` for `=`, and
/// spaces allowed), as the protocol carries it: `op` and `count`.
///
/// # Errors
///
/// Text that is no such condition.
pub(crate) fn hit(text: &str) -> anyhow::Result<JsonValue> {
    let text = text.trim();
    let (op, rest) = [(">=", ">="), ("==", "="), ("=", "="), (">", ">")]
        .into_iter()
        .find_map(|(spelling, op)| text.strip_prefix(spelling).map(|rest| (op, rest)))
        .ok_or_else(|| anyhow!("the hit condition `{text}` is none of =N, >=N and >N"))?;
    let count: u64 = rest
        .trim()
        .parse()
        .with_context(|| format!("the hit condition `{text}` has no count"))?;
    if count == 0 {
        bail!("the hit condition `{text}` counts from 1");
    }
    Ok(json!({ "op": op, "count": count }))
}

/// A log message as the command takes it, its `{expression}` parts compiled, as the protocol
/// carries it: the texts and the compiled expressions in their order.
///
/// # Errors
///
/// A brace that opens no part or closes none, and an expression that does not compile.
pub(crate) fn message_parts(
    text: &str,
    expressions: &mut Expressions,
) -> anyhow::Result<JsonValue> {
    let mut parts = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['{', '}']) {
            None => {
                parts.push(JsonValue::String(rest.to_string()));
                break;
            }
            Some(at) if rest[at..].starts_with('}') => {
                bail!("the log message `{text}` closes a brace it did not open")
            }
            Some(at) => {
                if at > 0 {
                    parts.push(JsonValue::String(rest[..at].to_string()));
                }
                let end = rest[at..].find('}').ok_or_else(|| {
                    anyhow!("the log message `{text}` opens a brace it does not close")
                })?;
                parts.push(expressions.compile(&rest[at + 1..at + end])?);
                rest = &rest[at + end + 1..];
            }
        }
    }
    Ok(JsonValue::Array(parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime::program_model::BinaryOp;
    use trust_runtime_services::debug::expression::{decode, Expression};

    /// The expression a compiled form carries, decoded as the runtime decodes it.
    fn decoded(compiled: &JsonValue) -> Expression {
        assert_eq!(compiled["expr_version"], json!(1), "{compiled}");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(compiled["expr"].as_str().expect("the compiled bytes"))
            .expect("standard base64");
        decode(&bytes).expect("the runtime decodes it")
    }

    fn name(text: &str) -> Expression {
        Expression::Name(text.into())
    }

    /// Specification 11 §6.13.2: `breakpoints-set` with a condition, a hit condition and a log
    /// message sends each line the same options: the condition `count > limit` compiled, as the
    /// runtime decodes it a comparison of the two names; `>=3` as `{op: ">=", count: 3}`; and the
    /// message `count is {count}!` as its text, the compiled name and its text. A breakpoint
    /// without options sends none. Catches options sent to one line only, a condition sent as
    /// text, and a message's expression left in its text.
    #[test]
    fn breakpoints_set_sends_each_line_its_compiled_options() {
        let action = ControlAction::BreakpointsSet {
            source: "src/main.st".into(),
            lines: vec![3, 8],
            condition: Some("count > limit".into()),
            hit_condition: Some(">=3".into()),
            log_message: Some("count is {count}!".into()),
        };
        let mut request = json!({ "params": { "source": "src/main.st", "lines": [2, 7] } });
        attach(&mut request, &action, None).expect("the options compile");
        let options = request["params"]["options"]
            .as_array()
            .expect("options")
            .clone();
        assert_eq!(options.len(), 2);
        assert_eq!(options[0], options[1]);
        let option = &options[0];
        assert_eq!(
            decoded(&option["condition"]),
            Expression::Binary(
                BinaryOp::Gt,
                Box::new(name("count")),
                Box::new(name("limit"))
            )
        );
        assert_eq!(option["hit_condition"], json!({ "op": ">=", "count": 3 }));
        let parts = option["log_message"].as_array().expect("message parts");
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert_eq!(parts[0], json!("count is "));
        assert_eq!(decoded(&parts[1]), name("count"));
        assert_eq!(parts[2], json!("!"));

        let plain = ControlAction::BreakpointsSet {
            source: "src/main.st".into(),
            lines: vec![3],
            condition: None,
            hit_condition: None,
            log_message: None,
        };
        let mut request = json!({ "params": { "source": "src/main.st", "lines": [2] } });
        attach(&mut request, &plain, None).expect("nothing to compile");
        assert_eq!(
            request,
            json!({ "params": { "source": "src/main.st", "lines": [2] } })
        );
    }

    /// Specification 11 §6.13.2: `evaluate` sends `debug.evaluate` the compiled expression beside
    /// the frame it names. Catches the expression sent as text, which the runtime refuses.
    #[test]
    fn evaluate_sends_the_compiled_expression_with_its_frame() {
        let action = ControlAction::Evaluate {
            expr: "P1.Counter".into(),
            frame: Some(2),
        };
        let mut request = json!({ "type": "debug.evaluate", "params": { "frame_id": 2 } });
        attach(&mut request, &action, None).expect("the expression compiles");
        assert_eq!(request["params"]["frame_id"], json!(2));
        assert_eq!(
            decoded(&request["params"]),
            Expression::Field(Box::new(name("P1")), "Counter".into())
        );
    }

    /// The hit conditions the command takes, as the protocol's `=`, `>=` and `>` (specification 11
    /// §6.13.2), and the ones it refuses: no operator, another operator, no count and a count of
    /// 0. Catches `==` or spaces refused, and a count of 0 sent, which the runtime refuses.
    #[test]
    fn hit_conditions_are_the_protocols_operators_with_a_count_from_one() {
        assert_eq!(hit("=5").unwrap(), json!({ "op": "=", "count": 5 }));
        assert_eq!(hit("== 5").unwrap(), json!({ "op": "=", "count": 5 }));
        assert_eq!(hit(">=3").unwrap(), json!({ "op": ">=", "count": 3 }));
        assert_eq!(hit(" > 2 ").unwrap(), json!({ "op": ">", "count": 2 }));
        for refused in ["5", "<3", ">=", "=0", ">= x"] {
            assert!(hit(refused).is_err(), "{refused}");
        }
    }

    /// A log message's parts: texts and `{expression}`s in their order, an expression next to
    /// another, and a brace that opens or closes no part refused. Catches a part lost between two
    /// expressions and an unbalanced brace sent as text.
    #[test]
    fn a_log_message_is_split_into_its_texts_and_expressions() {
        let mut expressions = Expressions::for_project(None).expect("no project");
        let parts = message_parts("{a}{b}", &mut expressions).expect("two expressions");
        let parts = parts.as_array().expect("parts");
        assert_eq!(parts.len(), 2);
        assert_eq!(decoded(&parts[0]), name("a"));
        assert_eq!(decoded(&parts[1]), name("b"));
        assert_eq!(
            message_parts("no values", &mut expressions).unwrap(),
            json!(["no values"])
        );
        assert!(message_parts("oops }", &mut expressions).is_err());
        assert!(message_parts("{ open", &mut expressions).is_err());
    }
}
