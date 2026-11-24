//! A tool's method call over the control link: `method.call` (specification 11 §6.8.1, "Calling a
//! method"). It makes the method call between scans that specification 44 §13.3 defines and a
//! screen's command uses (specification 39 §21.2), alike on every runtime (specification 34
//! STATE-08): a machine test presses a sequence's Start with it (specification 48 RU-58).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::tool::method_inputs;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Value;

use crate::live_values::value_grammar::parse_value;
use crate::live_values::value_grammar::typed::typed_value;
use crate::live_values::var_requests::VariableQueuePort;
use crate::ports::ResourcePort;
use crate::protocol::ControlResponse;
use crate::resource::replies::{busy_refusal, hung_refusal, resource_not_running};
use crate::status::invalid_params;

#[cfg(test)]
mod tests;

/// The statements and loop passes one call may use: a screen's command's allowance (specification
/// 11 §6.8.1, "Calling a method"; specification 39 §21.2).
pub const ALLOWANCE: u32 = 32_768;

/// The members a `method.call` request's parameters may have.
const MEMBERS: &[&str] = &["instance", "interface", "method", "inputs"];

/// A method call that `method.call` asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodCall {
    /// The instance's path, as `variables.read` names a variable.
    pub instance: SmolStr,
    /// The interface's qualified name, which the instance's type implements.
    pub interface: SmolStr,
    /// A method the interface declares.
    pub method: SmolStr,
    /// The given inputs by name; the kernel converts each value to its input's type.
    pub inputs: Vec<(SmolStr, Value)>,
    /// Each input's Structured Text literal as the request gave it, in the order of `inputs`: a
    /// cycle trace records the call with them (specification 48 Appendix C, C.6).
    pub literals: Vec<SmolStr>,
}

/// Why a runtime made no call, or why the call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodCallRefusal {
    /// The resource is in a state that takes no call: stopped or faulted.
    NotRunning(ResourceState),
    /// The resource's command queue is full; the call was not queued.
    Busy,
    /// The kernel refused the call and left the program as it was; the text is its reason.
    Refused(String),
    /// The method faulted, and the resource with it, as a faulted scan does; the text names the
    /// fault.
    Faulted(String),
}

/// Answers `method.call`: `{"value": v}`, the method's result as `variables.read` gives a value,
/// `null` for a method without one; the kernel's refusal or the method's fault as the request's
/// refusal. A resource that is stopped or faulted refuses the call as it refuses a pause
/// (`resource_not_running`), and a hung one as it refuses every request to its scan thread.
#[must_use]
pub fn request<H: ResourcePort + VariableQueuePort>(
    id: u64,
    params: Option<&JsonValue>,
    host: &H,
) -> ControlResponse {
    let asked = match parse(params) {
        Ok(asked) => asked,
        Err(reason) => return invalid_params(id, &reason),
    };
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if !takes_calls(resource) {
        return resource_not_running(id, resource);
    }
    let call = match typed(asked, host) {
        Ok(call) => call,
        Err(reason) => return ControlResponse::error(id, reason),
    };
    match host.call_method(&call) {
        Ok(value) => {
            ControlResponse::ok(id, json!({ "value": crate::variables::json_value(&value) }))
        }
        Err(MethodCallRefusal::NotRunning(state)) => resource_not_running(id, state),
        Err(MethodCallRefusal::Busy) => busy_refusal(id),
        Err(MethodCallRefusal::Refused(reason)) => ControlResponse::error(id, reason),
        Err(MethodCallRefusal::Faulted(fault)) => {
            ControlResponse::error(id, format!("the method faulted: {fault}"))
        }
    }
}

/// Whether a resource in `state` takes a call between two scans: one that runs or is paused, and
/// one still starting, whose loop makes the call before its first scan. A stopped resource has no
/// scans to fault, and a faulted one refuses every call until a restart (specification 44 §13.3).
#[must_use]
pub fn takes_calls(state: ResourceState) -> bool {
    matches!(
        state,
        ResourceState::Running | ResourceState::Paused | ResourceState::Boot
    )
}

/// A `method.call` as its request gives it: the inputs are the literals' texts.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    instance: SmolStr,
    interface: SmolStr,
    method: SmolStr,
    inputs: Vec<(SmolStr, String)>,
}

fn parse(params: Option<&JsonValue>) -> Result<Asked, String> {
    let params = params.ok_or("missing params")?;
    let object = params.as_object().ok_or("parameters must be an object")?;
    if let Some(member) = object.keys().find(|key| !MEMBERS.contains(&key.as_str())) {
        return Err(format!("unknown member {member}"));
    }
    let name = |member: &str| {
        object
            .get(member)
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(SmolStr::new)
            .ok_or_else(|| format!("{member} must be a name"))
    };
    let inputs = match object.get("inputs") {
        None | Some(JsonValue::Null) => Vec::new(),
        Some(JsonValue::Object(inputs)) => inputs
            .iter()
            .map(|(input, value)| {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("inputs.{input} must be a literal"))?;
                Ok((SmolStr::new(input), text.to_string()))
            })
            .collect::<Result<Vec<_>, String>>()?,
        Some(_) => return Err("inputs must be an object of literals".to_string()),
    };
    Ok(Asked {
        instance: name("instance")?,
        interface: name("interface")?,
        method: name("method")?,
        inputs,
    })
}

/// The call `asked`, each input's literal typed by the input's declared type in the installed
/// program's image, as `set` types a literal by its variable's (specification 11 §6.8.1; "Live
/// Values"). For a method the image does not declare, the call takes no inputs: the kernel
/// refuses it by its path, its interface or the method, before any input. A platform without the
/// image at hand leaves each literal to the untyped grammar of [`literal`], and the kernel refuses
/// what an input does not take.
///
/// # Errors
///
/// The refusal of an input the method does not have, as the kernel refuses it, and of a literal
/// the input's type does not take, which names the input.
fn typed<Q: VariableQueuePort>(asked: Asked, host: &Q) -> Result<MethodCall, String> {
    let mut typed = None;
    host.with_program_image(&mut |image| {
        typed = Some(image_typed_inputs(image, &asked));
    });
    let inputs = match typed {
        Some(inputs) => inputs?,
        None => asked
            .inputs
            .iter()
            .map(|(input, text)| {
                literal(text)
                    .map(|value| (input.clone(), value))
                    .map_err(|reason| format!("inputs.{input}: {reason}"))
            })
            .collect::<Result<Vec<_>, String>>()?,
    };
    let literals = asked
        .inputs
        .iter()
        .map(|(_, text)| SmolStr::new(text))
        .collect();
    Ok(MethodCall {
        instance: asked.instance,
        interface: asked.interface,
        method: asked.method,
        inputs,
        literals,
    })
}

/// The call of `instance`'s `method` of `interface` with `inputs`, each an input's name and
/// Structured Text literal, typed as `image` declares the method's inputs: what `method.call`
/// makes of its request, and what a replay makes of a recorded call (specification 48 RU-60).
///
/// # Errors
///
/// As `method.call` refuses its inputs: an input the method does not have, and a literal the
/// input's type does not take, which names the input.
pub fn typed_call(
    image: &Image<'_>,
    instance: &str,
    interface: &str,
    method: &str,
    inputs: &[(&str, &str)],
) -> Result<MethodCall, String> {
    let asked = Asked {
        instance: SmolStr::new(instance),
        interface: SmolStr::new(interface),
        method: SmolStr::new(method),
        inputs: inputs
            .iter()
            .map(|(input, literal)| (SmolStr::new(input), (*literal).to_string()))
            .collect(),
    };
    let inputs = image_typed_inputs(image, &asked)?;
    let literals = asked
        .inputs
        .iter()
        .map(|(_, text)| SmolStr::new(text))
        .collect();
    Ok(MethodCall {
        instance: asked.instance,
        interface: asked.interface,
        method: asked.method,
        inputs,
        literals,
    })
}

/// The inputs of the call `asked`, typed as `image` declares its method's; none for a method the
/// image does not declare, which the kernel refuses by its path, its interface or the method.
fn image_typed_inputs(image: &Image<'_>, asked: &Asked) -> Result<Vec<(SmolStr, Value)>, String> {
    match method_inputs(image, &asked.interface, &asked.method) {
        Ok(declared) => typed_inputs(image, &declared, &asked.inputs),
        Err(_) => Ok(Vec::new()),
    }
}

/// Each of `given`, an input's name and literal, as a value of the input `declared` names alike,
/// ignoring case as the kernel does.
fn typed_inputs(
    image: &Image<'_>,
    declared: &[(String, u32)],
    given: &[(SmolStr, String)],
) -> Result<Vec<(SmolStr, Value)>, String> {
    given
        .iter()
        .map(|(input, text)| {
            let Some((_, type_index)) = declared
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(input))
            else {
                return Err(RuntimeError::InvalidArgumentName(input.clone()).to_string());
            };
            typed_value(image, *type_index, text)
                .map(|value| (input.clone(), value))
                .map_err(|reason| format!("inputs.{input}: {reason}"))
        })
        .collect()
}

/// A Structured Text literal by the untyped grammar, for a platform without the program's image at
/// hand: a boolean, an integer, a real with a decimal point or an exponent, or a text in single
/// quotes, where `$'` is a quote and `$$` a dollar sign. The kernel types an integer for its input
/// and refuses another value its input does not take.
fn literal(text: &str) -> Result<Value, String> {
    if let Ok(value) = parse_value(text) {
        return Ok(value);
    }
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return unescape(inner).map(|text| Value::String(SmolStr::new(text)));
    }
    if trimmed.contains(['.', 'e', 'E'])
        && let Ok(real) = trimmed.parse::<f64>()
        && real.is_finite()
    {
        return Ok(Value::LReal(real));
    }
    Err(format!("unsupported value '{text}'"))
}

fn unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('$') => out.push('$'),
            Some('\'') => out.push('\''),
            Some(other) => return Err(format!("unsupported escape ${other}")),
            None => return Err("a text cannot end with $".to_string()),
        }
    }
    Ok(out)
}
