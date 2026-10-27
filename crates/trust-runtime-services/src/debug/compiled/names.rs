//! The debug expressions of a compiled program (specification 11 §6.13.2, version 1; specification
//! 44 §13.2): `debug.evaluate` decodes the tool's compiled form and evaluates it against the
//! published state or the held scan, and `eval` reads one exact name. A dotted name's first
//! identifier is a variable of the held frame first, its own and then its instance's, and
//! otherwise the name's longest leading part is a global or a program instance's variable
//! (`Instance.Variable`), found through the symbol table; each further identifier is a member of
//! the instance before it, through the type table. An enumeration literal takes its number from
//! the type table.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use base64::Engine as _;
use serde::Deserialize;
use trust_runtime_core::native::memory::{
    enum_value, pou_symbol, read_place, symbol_place, Place, ProgramMemory,
};
use trust_runtime_core::value::Value;

use super::super::expression::{self, ExpressionError, Names, EXPR_VERSION};
use super::super::projection::{format_value, value_type_name};
use super::super::replies;
use super::super::requests::{failed, parse};
use super::{frame_index, Reader};
use crate::protocol::ControlResponse;

/// The parameters of `debug.evaluate` with a compiled expression (specification 11 §6.13.2).
#[derive(Debug, Deserialize)]
struct EvaluateParams {
    /// The version of the compiled form.
    expr_version: u32,
    /// The compiled form, standard base64.
    expr: String,
    /// The frame the names are resolved in; none between scans.
    #[serde(default)]
    frame_id: Option<u32>,
}

/// The parameters of `eval`: one exact name (specification 11 §6.8.1).
#[derive(Debug, Deserialize)]
struct EvalParams {
    expr: String,
}

/// The names of an expression evaluated in frame `frame` of the held call stack, innermost first,
/// or between scans with no frame.
pub(crate) struct FrameNames<'m> {
    reader: Reader<'m>,
    frame: Option<usize>,
}

impl<'m> FrameNames<'m> {
    /// The names of `memory` in frame `frame`, an index from the innermost.
    pub(crate) fn new(memory: &'m dyn ProgramMemory, frame: Option<usize>) -> Self {
        Self {
            reader: Reader::new(memory),
            frame,
        }
    }

    /// Where the variable the dotted `name` names lies, and its type.
    fn locate(&self, name: &str) -> Option<(Place, u32)> {
        let parts: Vec<&str> = name.split('.').collect();
        let (first, rest) = parts.split_first()?;
        if let Some(index) = self.frame
            && let Some((place, type_index)) = self.in_frame(index, first)
        {
            return self.walk(place, type_index, rest);
        }
        let image = &self.reader.image;
        let (place, type_index, named) = (1..=parts.len()).rev().find_map(|named| {
            let symbol = image.find_variable(&parts[..named].join(".")).ok()?;
            let (place, type_index) = symbol_place(image, symbol).ok()?;
            Some((place, type_index, named))
        })?;
        self.walk(place, type_index, &parts[named..])
    }

    /// The variable `name`, one identifier, of frame `index`: one of the frame's own variables, the
    /// child symbols of its POU's symbol, or else a member of the frame's instance.
    fn in_frame(&self, index: usize, name: &str) -> Option<(Place, u32)> {
        let frames = self.reader.memory.frames();
        let frame = frames.get(index)?;
        let (pou, symbol) = pou_symbol(&self.reader.image, frame.pou)?;
        if frame.locals
            && let Some((_, variable)) = self.reader.symbols().find(|(_, variable)| {
                let display = self.reader.display(variable);
                let own = display.rsplit_once('.').map_or(display, |(_, own)| own);
                variable.parent == pou && own.eq_ignore_ascii_case(name)
            })
        {
            let place = Place::Frame {
                frame: index,
                offset: variable.offset,
            };
            return Some((place, variable.type_index));
        }
        self.walk(frame.instance?, symbol.type_index, &[name])
    }

    /// The member `parts` names, one identifier after the other, of the instance of type
    /// `type_index` at `place`; the variable itself without parts.
    fn walk(&self, mut place: Place, mut type_index: u32, parts: &[&str]) -> Option<(Place, u32)> {
        for part in parts {
            let members = self.reader.members(place, type_index).ok()?;
            let child = self
                .reader
                .children(&members, place)
                .find(|child| child.name.eq_ignore_ascii_case(part))?;
            place = child.place;
            type_index = child.type_index;
        }
        Some((place, type_index))
    }
}

impl Names for FrameNames<'_> {
    fn read(&self, name: &str) -> Option<Value> {
        let (place, type_index) = self.locate(name)?;
        read_place(self.reader.memory, place, type_index).ok()
    }

    fn enumeration(&self, type_name: &str, value: &str) -> Option<Value> {
        enum_value(&self.reader.image, type_name, value)
    }
}

/// The answer to `eval` or `debug.evaluate` from the compiled program's `memory`.
pub(in crate::debug) fn evaluate(
    id: u64,
    kind: &str,
    params: Option<&serde_json::Value>,
    memory: &dyn ProgramMemory,
) -> ControlResponse {
    if kind == "eval" {
        let params: EvalParams = match parse(params) {
            Ok(params) => params,
            Err(error) => return failed(id, error),
        };
        return match FrameNames::new(memory, None).read(params.expr.trim()) {
            Some(value) => {
                ControlResponse::ok(id, serde_json::json!({ "value": format!("{value:?}") }))
            }
            None => failed(id, String::from("unknown identifier")),
        };
    }
    let params: EvaluateParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    if params.expr_version != EXPR_VERSION {
        return replies::expr_version_unsupported(id, params.expr_version);
    }
    let frame = match params.frame_id {
        Some(frame_id) => match frame_index(memory.frames().len(), frame_id) {
            Some(index) => Some(index),
            None => return failed(id, String::from("unknown frame id")),
        },
        None => None,
    };
    let result = base64::engine::general_purpose::STANDARD
        .decode(params.expr.as_bytes())
        .map_err(|_| ExpressionError::Invalid(String::from("the expression is not base64")))
        .and_then(|bytes| expression::decode(&bytes))
        .and_then(|expression| expression::evaluate(&expression, &FrameNames::new(memory, frame)));
    match result {
        Ok(value) => ControlResponse::ok(
            id,
            serde_json::json!({
                "result": format_value(&value),
                "type": value_type_name(&value),
                "variables_reference": 0,
            }),
        ),
        Err(error) => {
            ControlResponse::error_with_code(id, String::from(error.detail()), error.code())
        }
    }
}
