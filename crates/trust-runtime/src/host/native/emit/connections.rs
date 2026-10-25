//! The connections of the program instances' inputs and outputs in the configuration (IEC 61131-3
//! §6.8.2, Table 62 8b, 9b; specification 18 §2.4): an instance's entry gives each input that the
//! configuration connects to a global variable or a constant its value before the instance's body,
//! and each output's value to the global it is connected to after the body. An input or an output
//! connected to a directly represented variable is a binding instead (`model::bindings`).

use std::fmt::Write as _;

use smol_str::SmolStr;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::frame_init::omitted_scalar;
use super::super::front::{Front, StatementIds};
use super::super::lower::{ident_of, Frame, Lowerer, Place};
use super::super::model::Model;
use super::super::types::{rust_field, Kind};
use super::super::NativeError;
use super::bodies::find_pou;
use crate::harness::{AccessPart, ConnectionEnd, ProgramConnection};

/// Checks every connection of every program instance and writes into `source` the functions
/// `connect_in_<i>` and `connect_out_<i>` of each instance `i` with a connection to a global or a
/// constant, which its entry calls (`entries`).
///
/// # Errors
///
/// [`NativeError::Compile`] for a connection that names no input or output of its program, an
/// input connected with `=>` or an output with `:=`, or a value that the parameter does not take.
pub(super) fn connections(
    front: &Front,
    ids: &StatementIds,
    model: &mut Model<'_>,
    pointer_bytes: u64,
    source: &mut String,
) -> Result<(), NativeError> {
    for index in 0..model.instances.len() {
        if model.instances[index].connections.is_empty() {
            continue;
        }
        let instance = model.instances[index].name.clone();
        let program = model.instances[index].program;
        let connections = model.instances[index].connections.clone();
        let def_name = model.program_types[program].def.name.clone();
        let Some((file, pou)) = find_pou(front, SyntaxKind::Program, &def_name, None) else {
            return Err(NativeError::Compile(format!(
                "program {def_name} has no declaration"
            )));
        };
        let (inputs, outputs) = parameters(&pou);
        for connection in &connections {
            let (declared, sign, section) = if connection.output {
                (&outputs, "=>", "VAR_OUTPUT")
            } else {
                (&inputs, ":=", "VAR_INPUT")
            };
            if !declared
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&connection.parameter))
            {
                return Err(NativeError::Compile(format!(
                    "the connection {} {sign} of program instance {instance} names no {section} \
                     of program {def_name} (IEC 61131-3 §6.8.2, Table 62)",
                    connection.parameter
                )));
            }
        }
        for output in [false, true] {
            let made: Vec<&ProgramConnection> = connections
                .iter()
                .filter(|connection| {
                    connection.output == output
                        && !matches!(connection.end, ConnectionEnd::Address { .. })
                })
                .collect();
            if made.is_empty() {
                continue;
            }
            let mut lowerer = Lowerer::new(
                model,
                &front.sources[file],
                file,
                ids,
                Frame::Function,
                false,
                pointer_bytes,
            );
            for connection in made {
                connect(&mut lowerer, &pou, &instance, program, connection)?;
            }
            let (text, _, _) = lowerer.finish();
            let name = if output { "connect_out" } else { "connect_in" };
            let _ = write!(
                source,
                "fn {name}_{index}(cx: &mut Cx, w: &World<'_>) -> R<()> {{\n{text}    Ok(())\n}}\n\n"
            );
        }
    }
    Ok(())
}

/// The names the program `pou` declares in `VAR_INPUT` and in `VAR_OUTPUT`.
fn parameters(pou: &SyntaxNode) -> (Vec<SmolStr>, Vec<SmolStr>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for block in pou
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        let keyword = block
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
            .map(|token| token.kind());
        let names = match keyword {
            Some(SyntaxKind::KwVarInput) => &mut inputs,
            Some(SyntaxKind::KwVarOutput) => &mut outputs,
            _ => continue,
        };
        for declaration in block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            names.extend(
                declaration
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Name)
                    .map(|name| SmolStr::new(ident_of(&name))),
            );
        }
    }
    (inputs, outputs)
}

/// Writes the connection `connection` of the instance `instance` of the program type `program`;
/// `node`, the program's declaration, is where a refusal points.
fn connect(
    lowerer: &mut Lowerer<'_, '_>,
    node: &SyntaxNode,
    instance: &str,
    program: usize,
    connection: &ProgramConnection,
) -> Result<(), NativeError> {
    let label = format!("{instance}.{}", connection.parameter);
    let parameter = {
        let Some((member, retained)) =
            lowerer.model.program_types[program].member(&connection.parameter)
        else {
            return Err(NativeError::Compile(format!("{label} has no storage")));
        };
        let area = if retained { "w.r" } else { "w.p" };
        Place {
            expr: format!("{area}.{}.{}", rust_field(instance), member.field()),
            ty: member.ty,
            is_ref: false,
        }
    };
    match &connection.end {
        ConnectionEnd::Address { .. } => Ok(()),
        ConnectionEnd::Global(parts) => {
            if let Some(global) = lowerer.global_place(node, &label, parts)? {
                let (source, target) = if connection.output {
                    (parameter, global)
                } else {
                    (global, parameter)
                };
                return assign(lowerer, node, &label, &source, &target);
            }
            // A name alone that names no global is a value of the input's enumeration.
            if let (false, [AccessPart::Name(name)]) = (connection.output, parts.as_slice())
                && let Kind::Enum { base, values, .. } = &lowerer.info(parameter.ty).kind
                && let Some((_, value)) = values
                    .iter()
                    .find(|(value, _)| value.eq_ignore_ascii_case(name))
            {
                let literal = format!("({value}{})", base.rust());
                lowerer.store_scalar(&parameter, &literal);
                return Ok(());
            }
            Err(NativeError::Compile(format!(
                "{label} is connected to {}, which is no global variable (IEC 61131-3 Table 62 \
                 8b, 9b)",
                path_text(parts)
            )))
        }
        ConnectionEnd::Constant(expr) => {
            let type_id = lowerer.model.program_types[program]
                .def
                .vars
                .iter()
                .find(|var| var.name.eq_ignore_ascii_case(&connection.parameter))
                .map(|var| var.type_id)
                .ok_or_else(|| NativeError::Compile(format!("{label} has no declaration")))?;
            if lowerer.info(parameter.ty).scalar().is_none() {
                return Err(NativeError::Compile(format!(
                    "{label} is connected to a constant, which only an input of an elementary \
                     type takes (IEC 61131-3 Table 62 8b)"
                )));
            }
            let literal = omitted_scalar(lowerer.model, parameter.ty, type_id, Some(expr), &label)?;
            lowerer.store_scalar(&parameter, &literal);
            Ok(())
        }
    }
}

/// Writes the value at `source` into `target` as an assignment does (IEC §6.4.2): of the same
/// type, a text cut at the target's capacity, an elementary value converted implicitly to the
/// target's type (Table 11) or to a subrange of its type.
fn assign(
    lowerer: &mut Lowerer<'_, '_>,
    node: &SyntaxNode,
    label: &str,
    source: &Place,
    target: &Place,
) -> Result<(), NativeError> {
    let (from, to) = (
        lowerer.info(source.ty).kind.clone(),
        lowerer.info(target.ty).kind.clone(),
    );
    match (&from, &to) {
        (Kind::Str(_), Kind::Str(_)) => {
            lowerer.line(&format!(
                "lib::assign(({}).view(), ({}).view());",
                target.reference(),
                source.reference()
            ));
            return Ok(());
        }
        (Kind::WStr(_), Kind::WStr(_)) => {
            lowerer.line(&format!(
                "lib::assign_wide(({}).view(), ({}).view());",
                target.reference(),
                source.reference()
            ));
            return Ok(());
        }
        _ => {}
    }
    if source.ty == target.ty {
        if lowerer.info(target.ty).scalar().is_some() {
            let value = lowerer.load(source);
            lowerer.store_scalar(target, &value);
        } else {
            lowerer.line(&format!(
                "Assign::assign({}, {});",
                target.reference(),
                source.reference()
            ));
        }
        return Ok(());
    }
    // An enumeration takes only its own values; a subrange is a value of its base.
    let elementary = |kind: &Kind| match kind {
        Kind::Elem(elem) | Kind::Subrange { base: elem, .. } => Some(*elem),
        _ => None,
    };
    let implicit = match (elementary(&from), elementary(&to)) {
        (Some(from), Some(to)) => {
            let registry = lowerer.model.types.registry();
            match (registry.lookup(to.name()), registry.lookup(from.name())) {
                (Some(to), Some(from)) => registry.is_assignable(to, from),
                _ => false,
            }
        }
        _ => false,
    };
    if !implicit {
        return Err(NativeError::Compile(format!(
            "{label} is connected to a value of another type, which no implicit conversion \
             gives it (IEC 61131-3 §6.4.2, Table 11)"
        )));
    }
    let value = lowerer.load(source);
    let converted = lowerer.convert(node, &value, source.ty, target.ty)?;
    lowerer.store_scalar(target, &converted);
    Ok(())
}

/// The text of a global's path, `G.member[1]`, as a message names it.
fn path_text(parts: &[AccessPart]) -> String {
    let mut text = String::new();
    for part in parts {
        match part {
            AccessPart::Name(name) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(name);
            }
            AccessPart::Index(indices) => {
                let indices: Vec<String> = indices.iter().map(i64::to_string).collect();
                let _ = write!(text, "[{}]", indices.join(", "));
            }
            AccessPart::Partial(_) => text.push_str(".%"),
        }
    }
    text
}
