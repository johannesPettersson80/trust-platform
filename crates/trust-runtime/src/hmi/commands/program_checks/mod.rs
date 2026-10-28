//! Method commands and settings checked against the running program (specification 39, §21.2
//! item 2, §21.4 item 4): the runtime reads what a check needs from the image of the program that
//! runs, its symbol and type tables (specification 44 §5.4, §5.7, §5.8, §13.3), never from a
//! description of the project's sources. The instance's type lists the interfaces it implements,
//! itself or through a base. The interface declares a method when the image has the method's tool
//! symbol `@TOOL.<I>.<M>`, whose parts name the method's inputs with their types and its result.
//! A method without a tool entry takes another parameter than an input and is refused. A text
//! longer than its input holds is refused, never cut, as an HMI write is. An input a command does
//! not give is left out of the call: the program gives it the initial value the interface declares
//! (§5.4).

use serde_json::Value as Json;
use smol_str::SmolStr;
use trust_runtime_core::native::image::{
    type_kind, EntryKind, EntryRecord, Image, SymbolRecord, NONE, SYMBOL_INPUT,
};
use trust_runtime_core::native::memory::{layout, type_name, type_shape, Layout, TypeShape};
use trust_runtime_core::native::reflect::{resolve_path, type_implements};
use trust_runtime_core::native::tool::tool_symbol;

use super::capability::{Argument, MethodCommand};
use super::capability_checks::scalar;
use super::write::{enumeration_values, fits_capacity, path_text, WriteRefused};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::write_value::value_for_target_with;
use crate::runtime::program_access::Located;
use crate::value::{Duration, EnumValue, Value};

#[cfg(test)]
mod tests;

/// Why a method without a tool entry is refused.
const ONLY_INPUTS: &str = "HMI methods accept only input parameters";

/// The function block or class instance a method command names (§21.2 item 1), where it lies in
/// the running program.
///
/// # Errors
///
/// "the equipment is not available" for a path that names no instance of the program.
pub(crate) fn equipment(image: &Image<'_>, instance: &str) -> Result<Located, String> {
    let path = MemberPath::parse(instance)?;
    locate(image, &path)
        .filter(|at| matches!(layout(image, at.type_index), Ok(Layout::Instance(_))))
        .ok_or_else(|| "the equipment is not available".to_string())
}

/// The inputs a method command gives the instance at `at`, each converted to its input's type,
/// after the checks of §21.2 item 2: the instance's type implements the command's interface,
/// itself or through a base; the interface declares the method; the method has a tool entry, so
/// its parameters are inputs only; it returns BOOL, TRUE when the instance accepts the command;
/// every argument names one of its inputs; at most one takes `entered`, the value the control
/// sends, already checked against the control; and every value fits its input's type, a text
/// within its input's capacity, refused and never cut, as an HMI write is (§12.8 item 5). The
/// inputs come in their declared order. An input the command does not name is left out: the
/// program gives it the initial value the interface declares.
///
/// # Errors
///
/// The [`WriteRefused`] of the first check that fails: its reason, and for a text longer than its
/// input holds the stable code `runtime_string_capacity_exceeded`.
pub(crate) fn method_inputs(
    image: &Image<'_>,
    at: &Located,
    command: &MethodCommand,
    entered: Option<&Json>,
) -> Result<Vec<(SmolStr, Value)>, WriteRefused> {
    command.validate()?;
    if !type_implements(image, at.type_index, &command.interface).unwrap_or(false) {
        return Err(format!(
            "{} does not implement {}",
            type_name(image, at.type_index),
            command.interface
        )
        .into());
    }
    let method = Signature::of(image, &command.interface, &command.method)?;
    let returns_bool = method.result.is_some_and(|result| {
        matches!(
            type_shape(image, result),
            Ok(TypeShape::Elementary(type_kind::BOOL))
        )
    });
    if !returns_bool {
        return Err(format!(
            "{}.{} must return BOOL, TRUE when the command is accepted",
            command.interface, command.method
        )
        .into());
    }
    let mut given = Vec::with_capacity(command.arguments.len());
    for (name, argument) in &command.arguments {
        let input = method
            .inputs
            .iter()
            .position(|(input, _)| input.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("{} has no argument {name}", command.method))?;
        given.push((input, argument));
    }
    let takes_value = given
        .iter()
        .filter(|(_, argument)| matches!(argument, Argument::Input { .. }))
        .count();
    if takes_value > 1 {
        return Err("One visual control supplies one entered value".into());
    }
    if entered.is_some() && takes_value == 0 {
        return Err("this control sends no value".into());
    }
    given.sort_unstable_by_key(|(input, _)| *input);
    given
        .into_iter()
        .map(|(input, argument)| {
            let (name, type_index) = &method.inputs[input];
            let requested = match argument {
                Argument::Input { .. } => entered.cloned().ok_or("this control needs a value")?,
                Argument::Constant(value) => scalar(value),
            };
            let value = input_value(image, *type_index, &requested)
                .map_err(|reason| format!("{name}: {reason}"))?;
            fits_capacity(image, *type_index, &value, name)?;
            Ok((name.clone(), value))
        })
        .collect()
}

/// The values of the enumeration `name` of the member at `path`, by their names and numbers as the
/// running program's type table declares them (specification 44 §5.8): what a setting of an
/// enumeration is checked against (§21.4 item 4). `None` for a path that names nothing and for a
/// member of another type.
pub(crate) fn member_enumeration(
    image: &Image<'_>,
    path: &MemberPath,
    name: &str,
) -> Option<Vec<(SmolStr, i64)>> {
    let at = locate(image, path)?;
    enumeration_values(image, at.type_index, name)
}

/// Where `path` lies in the running program, by its image's tables alone: its first name a global
/// or a program instance (§7.2), then members, fields and elements as
/// [`locate_member`](crate::hmi::member_paths::locate_member) walks them; `None` for a path that
/// names nothing, a remote path among them.
fn locate(image: &Image<'_>, path: &MemberPath) -> Option<Located> {
    let (_, root) = image.find_symbol(&path.root.to_ascii_uppercase())?;
    if root.parent != NONE || root.area == 0 {
        return None;
    }
    resolve_path(image, &path_text(path)).ok()
}

/// `requested` as a value of an input of type `type_index`, converted as an HMI write converts a
/// value (§12.2.4): of the input's kind and within its type's range, an enumeration's value by one
/// of the names or numbers the type table declares (§5.8).
fn input_value(image: &Image<'_>, type_index: u32, requested: &Json) -> Result<Value, String> {
    let kind = sample(image, type_index).ok_or_else(|| {
        format!(
            "a {} cannot be written from the HMI",
            type_name(image, type_index)
        )
    })?;
    value_for_target_with(&kind, requested, |name| {
        enumeration_values(image, type_index, name)
    })
}

/// A value of type `type_index`, which tells the conversion what to convert to: an elementary
/// value, a text or a value of an enumeration; `None` for a type the HMI does not write.
fn sample(image: &Image<'_>, type_index: u32) -> Option<Value> {
    Some(match type_shape(image, type_index).ok()? {
        TypeShape::Elementary(kind) | TypeShape::Subrange(kind) => match kind {
            type_kind::BOOL => Value::Bool(false),
            type_kind::SINT => Value::SInt(0),
            type_kind::INT => Value::Int(0),
            type_kind::DINT => Value::DInt(0),
            type_kind::LINT => Value::LInt(0),
            type_kind::USINT => Value::USInt(0),
            type_kind::UINT => Value::UInt(0),
            type_kind::UDINT => Value::UDInt(0),
            type_kind::ULINT => Value::ULInt(0),
            type_kind::REAL => Value::Real(0.0),
            type_kind::LREAL => Value::LReal(0.0),
            type_kind::BYTE => Value::Byte(0),
            type_kind::WORD => Value::Word(0),
            type_kind::DWORD => Value::DWord(0),
            type_kind::LWORD => Value::LWord(0),
            type_kind::TIME => Value::Time(Duration::ZERO),
            type_kind::LTIME => Value::LTime(Duration::ZERO),
            _ => return None,
        },
        TypeShape::Text { wide: false, .. } => Value::String(SmolStr::default()),
        TypeShape::Text { wide: true, .. } => Value::WString(String::new()),
        // Only the enumeration's name counts: the conversion takes its values from the type table.
        TypeShape::Enumeration { name, .. } => Value::Enum(Box::new(
            EnumValue::from_canonical_parts(name, SmolStr::default(), 0),
        )),
        _ => return None,
    })
}

/// A method as a tool calls it, by its tool symbol (specification 44 §5.4, §5.7): its inputs by
/// their declared names with their types, in their declared order, and its result's type, none
/// for a method without a result.
struct Signature {
    inputs: Vec<(SmolStr, u32)>,
    result: Option<u32>,
}

impl Signature {
    /// Method `method` of interface `interface` as a tool calls it, or why a tool cannot call it:
    /// the interface does not declare it when the image has no tool symbol of it, and it takes
    /// another parameter than an input when no tool entry reads its call block.
    fn of(image: &Image<'_>, interface: &str, method: &str) -> Result<Self, String> {
        let undeclared = || format!("{interface} has no method {method}");
        let name = tool_symbol(interface, method);
        let (index, _) = image.find_symbol(&name).ok_or_else(undeclared)?;
        let callable = image.table::<EntryRecord>().is_ok_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.kind == EntryKind::Method as u32 && entry.index == index)
        });
        if !callable {
            return Err(ONLY_INPUTS.into());
        }
        Self::parts(image, &name, index, method).ok_or_else(undeclared)
    }

    /// The parts of the call block that the tool symbol `name` at `index` names, which follow it in
    /// the sorted symbol table as `<name>.<PART>`: each input, flagged as an input, and the result
    /// by the method's name. `THIS` and the mask `__GIVEN` are the kernel's to write.
    fn parts(image: &Image<'_>, name: &str, index: u32, method: &str) -> Option<Self> {
        let symbols = image.table::<SymbolRecord>().ok()?;
        let prefix = format!("{name}.");
        let result_name = format!("{prefix}{}", method.to_ascii_uppercase());
        let mut inputs = Vec::new();
        let mut result = None;
        for at in index.checked_add(1)?..symbols.len() {
            let part = symbols.get(at)?;
            let canonical = image.string(part.canonical)?;
            if !canonical.starts_with(&prefix) {
                break;
            }
            if part.parent != index {
                continue;
            }
            if part.flags & SYMBOL_INPUT != 0 {
                // Displayed as `<Interface>.<Method>.<Input>`, each as declared.
                let display = image.string(part.display)?;
                let declared = display.rsplit('.').next().unwrap_or(display);
                inputs.push((part.offset, SmolStr::new(declared), part.type_index));
            } else if canonical == result_name {
                result = Some(part.type_index);
            }
        }
        // The inputs lie in the block in their declared order.
        inputs.sort_unstable_by_key(|(offset, _, _)| *offset);
        Some(Self {
            inputs: inputs
                .into_iter()
                .map(|(_, name, type_index)| (name, type_index))
                .collect(),
            result,
        })
    }
}
