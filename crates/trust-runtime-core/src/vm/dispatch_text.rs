//! Built-in text functions run on their arguments where they lie (specification 34 ARCH-05; C1-12).
//!
//! A `CALL_NATIVE` of a built-in text function reads each argument in place: a text or a scalar in
//! the flat store, named by a place or by a target reference, a temporary text on the operand
//! stack, or a value. The function writes its text result into a buffer the engine keeps, and the
//! result goes onto the operand stack's text area. Every charge, result and refusal is the one a
//! call that binds values gives (TIME-08), so such a call allocates nothing once the engine is warm.

use super::call::bindings::VmWriteTarget;
use super::call::context::RegisterValueOpKind;
use super::call::stdlib::formal_matches;
use super::context::ExecutionContext;
use super::helpers::borrowed_value_is_cloned;
use super::module::{VmModule, VmNativeSymbolSpec};
use super::{Operand, OperandStack, VmFrame, VmTrap};
use crate::bytecode::NATIVE_CALL_KIND_STDLIB;
use crate::error::RuntimeError;
use crate::memory::cell_access::CellsView;
use crate::memory::flat::TextBytes;
use crate::memory::layout::ByteRange;
use crate::memory::view::ValueView;
use crate::numeric::to_i64;
use crate::stdlib::text_kernels::{self, TextArg, TextKernel, TextResult};
use crate::stdlib::StdParams;
use crate::value::Value;
use alloc::string::String;
use alloc::vec::Vec;

/// The most arguments a text call by name binds here; one by position takes any number.
const MAX_NAMED: usize = 4;

/// The most arguments a text call with a target argument takes here.
const MAX_TARGETS: usize = 64;

/// The buffers a text call works in, kept by the engine between calls (C1-12): the result, the
/// characters of `WSTRING` arguments decoded from their code units, and where each lies.
#[derive(Debug, Default)]
pub struct TextBuffers {
    result: String,
    decoded: String,
    spans: Vec<(usize, usize)>,
}

/// A call of a built-in text function that runs on its arguments where they lie.
#[derive(Debug, Clone, Copy)]
pub(super) struct TextCall {
    kernel: TextKernel,
    count: usize,
    /// For a call by name, the position among the call's arguments of the one each formal takes.
    named: Option<[u8; MAX_NAMED]>,
    /// The arguments passed as target references, one bit for each position.
    targets: u64,
}

impl TextCall {
    /// The position among the call's arguments of the one formal `formal` takes.
    fn source(&self, formal: usize) -> usize {
        self.named
            .and_then(|named| named.get(formal).copied())
            .map_or(formal, usize::from)
    }

    /// Whether the argument at position `source` is passed as a target reference.
    fn is_target(&self, source: usize) -> bool {
        source < MAX_TARGETS && self.targets & (1 << source) != 0
    }
}

/// The text call of the `CALL_NATIVE` instruction whose operands start at `pc`: a call of a
/// built-in text function that no custom registration replaces, bound one to one as the call that
/// binds values binds it. `None` for any other call, which binds values.
pub(super) fn text_call_at(
    runtime: &impl ExecutionContext,
    module: &VmModule,
    pc: usize,
) -> Option<TextCall> {
    let word = |at: usize| -> Option<u32> {
        let bytes = module
            .code
            .get(pc.checked_add(at)?..pc.checked_add(at + 4)?)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    };
    if word(0)? != NATIVE_CALL_KIND_STDLIB {
        return None;
    }
    let count = usize::try_from(word(8)?).ok()?;
    let VmNativeSymbolSpec::Parsed {
        normalized_target_name,
        conversion_spec: None,
        arg_specs,
        ..
    } = module.native_symbol_spec(word(4)?).ok()?
    else {
        return None;
    };
    if arg_specs.len() != count {
        return None;
    }
    let mut targets = 0u64;
    for (source, spec) in arg_specs.iter().enumerate() {
        if spec.is_target {
            if source >= MAX_TARGETS {
                return None;
            }
            targets |= 1 << source;
        }
    }
    let kernel = TextKernel::of(normalized_target_name)?;
    let builtin = runtime.stdlib().builtin(normalized_target_name)?;
    let named = if arg_specs.iter().all(|spec| spec.name.is_none()) {
        // A count the formals do not admit is refused by the call that binds values.
        let admitted = match builtin.params {
            StdParams::Fixed(params) => params.len() == count,
            StdParams::Variadic { fixed, min, .. } => count >= fixed.len().checked_add(*min)?,
        };
        if !admitted {
            return None;
        }
        None
    } else {
        let StdParams::Fixed(params) = builtin.params else {
            return None;
        };
        if params.len() != count || count > MAX_NAMED {
            return None;
        }
        let mut formals = [u8::MAX; MAX_NAMED];
        for (source, spec) in arg_specs.iter().enumerate() {
            let name = spec.name.as_ref()?;
            let formal = params
                .iter()
                .position(|param| formal_matches(param.as_str(), name.as_str()))?;
            if formals[formal] != u8::MAX {
                return None;
            }
            formals[formal] = u8::try_from(source).ok()?;
        }
        Some(formals)
    };
    Some(TextCall {
        kernel,
        count,
        named,
        targets,
    })
}

/// Run `call` on the top operands of `stack`, whose target arguments name the variables of the
/// caller's `frame`, and replace them with its result (C1-12). `Ok(false)`, with nothing charged
/// or changed, when the composition keeps no text buffers or an argument is not read in place: a
/// place that holds no text, or a target the call that binds values reads another way. The call
/// then binds values.
pub(super) fn run_text_call(
    runtime: &mut impl ExecutionContext,
    stack: &mut OperandStack,
    frame: &VmFrame,
    call: &TextCall,
) -> Result<bool, RuntimeError> {
    let Some(mut buffers) = runtime.take_text_buffers() else {
        return Ok(false);
    };
    let outcome = run_with_buffers(runtime, stack, frame, call, &mut buffers);
    runtime.return_text_buffers(buffers);
    outcome
}

/// One argument where it lies: a value, a leaf of the flat store, or a temporary text.
enum Lying<'a> {
    Value(&'a Value),
    Cells {
        cells: CellsView<'a>,
        range: ByteRange,
        type_id: u32,
    },
    Text(&'a str, bool),
}

impl<'a> Lying<'a> {
    /// The argument a view shows.
    fn of_view(view: ValueView<'a>) -> Self {
        match view {
            ValueView::Value(value) => Lying::Value(value),
            ValueView::Cells {
                cells,
                range,
                type_id,
            } => Lying::Cells {
                cells,
                range,
                type_id,
            },
        }
    }

    /// The characters of a text leaf, in place, or `None` for any other argument.
    fn cell_text(&self) -> Option<TextBytes<'a>> {
        match *self {
            Lying::Cells {
                cells,
                range,
                type_id,
            } => cells.text(range, type_id),
            _ => None,
        }
    }

    /// Whether a copy of the argument copies a payload, as `materialize_flagged` reports it.
    fn copy_is_clone(&self) -> Result<bool, RuntimeError> {
        Ok(match *self {
            Lying::Value(value) => borrowed_value_is_cloned(value),
            Lying::Cells {
                cells,
                range,
                type_id,
            } => match cells.scalar(range, type_id)? {
                Some(scalar) => borrowed_value_is_cloned(&scalar),
                None => !cells.is_struct(type_id),
            },
            Lying::Text(..) => true,
        })
    }
}

/// The argument at position `source` where it lies, or `None` when it is not read in place.
fn lying<'a>(
    runtime: &'a impl ExecutionContext,
    stack: &'a OperandStack,
    frame: &'a VmFrame,
    call: &TextCall,
    operands: &'a [Operand],
    source: usize,
) -> Result<Option<Lying<'a>>, RuntimeError> {
    let operand = operands
        .get(source)
        .ok_or_else(|| VmTrap::StackUnderflow.into_runtime_error())?;
    Ok(match operand {
        Operand::Value(Value::Reference(Some(reference))) if call.is_target(source) => {
            VmWriteTarget::peek_reference(runtime, frame, reference)
                .ok()
                .map(Lying::of_view)
        }
        // A target that is no reference is refused by the call that binds values.
        Operand::Value(_) if call.is_target(source) => None,
        Operand::Value(value) => Some(Lying::Value(value)),
        Operand::Place(place) => runtime
            .place_view(*place)
            .map(Lying::of_view)
            .filter(|lying| lying.cell_text().is_some()),
        Operand::Text(temp) => Some(Lying::Text(
            stack.text(*temp).map_err(VmTrap::into_runtime_error)?,
            temp.wide(),
        )),
    })
}

fn run_with_buffers(
    runtime: &mut impl ExecutionContext,
    stack: &mut OperandStack,
    frame: &VmFrame,
    call: &TextCall,
    buffers: &mut TextBuffers,
) -> Result<bool, RuntimeError> {
    let operands = stack
        .top_operands(call.count)
        .ok_or_else(|| VmTrap::StackUnderflow.into_runtime_error())?;
    for source in 0..call.count {
        if lying(&*runtime, stack, frame, call, operands, source)?.is_none() {
            return Ok(false);
        }
    }
    if runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded.into_runtime_error());
    }
    // The binding copies each argument in the order the call gives them, a target as a read of
    // it (TIME-08).
    for source in 0..call.count {
        let cloned = {
            let argument = lying(&*runtime, stack, frame, call, operands, source)?
                .ok_or(RuntimeError::InvalidExecutionState)?;
            match argument {
                Lying::Value(value) => runtime.before_value_clone(value)?,
                Lying::Cells {
                    cells,
                    range,
                    type_id,
                } => runtime.before_view_clone(&ValueView::Cells {
                    cells,
                    range,
                    type_id,
                })?,
                Lying::Text(text, wide) => runtime.before_text_clone(text, wide)?,
            }
            argument.copy_is_clone()?
        };
        if cloned {
            runtime.record_value_op(if call.is_target(source) {
                RegisterValueOpKind::ReadReference
            } else {
                RegisterValueOpKind::BindExpression
            });
        }
    }
    let TextBuffers {
        result,
        decoded,
        spans,
    } = buffers;
    decoded.clear();
    spans.clear();
    spans
        .try_reserve(call.count)
        .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
    for source in 0..call.count {
        let start = decoded.len();
        let argument = lying(&*runtime, stack, frame, call, operands, source)?;
        if let Some(wide @ TextBytes::Wide(_)) = argument.and_then(|argument| argument.cell_text())
        {
            for character in char::decode_utf16(wide.units()) {
                decoded.push(character.map_err(|_| RuntimeError::InvalidExecutionState)?);
            }
        }
        spans.push((start, decoded.len()));
    }
    let arg = |source: usize| {
        let argument = lying(&*runtime, stack, frame, call, operands, source)?
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok::<_, RuntimeError>(match argument {
            Lying::Value(value) => TextArg::of(value),
            Lying::Text(text, wide) => TextArg::Text { text, wide },
            Lying::Cells {
                cells,
                range,
                type_id,
            } => match cells.text(range, type_id) {
                Some(TextBytes::Narrow(text)) => TextArg::Text { text, wide: false },
                Some(TextBytes::Wide(_)) => {
                    let (start, end) = spans
                        .get(source)
                        .copied()
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    TextArg::Text {
                        text: decoded
                            .get(start..end)
                            .ok_or(RuntimeError::InvalidExecutionState)?,
                        wide: true,
                    }
                }
                None => TextArg::Other(match cells.scalar(range, type_id)? {
                    Some(scalar) => to_i64(&scalar),
                    None => Err(RuntimeError::TypeMismatch),
                }),
            },
        })
    };
    // The text arguments are charged together before the function runs (TIME-02, TIME-08).
    let mut text_bytes = 0usize;
    for source in 0..call.count {
        if let TextArg::Text { text, .. } = arg(source)? {
            text_bytes = text_bytes
                .checked_add(text.len())
                .ok_or(RuntimeError::Overflow)?;
        }
    }
    if text_bytes != 0 {
        runtime.charge_text_work(text_bytes)?;
    }
    result.clear();
    let outcome = text_kernels::run(
        call.kernel,
        call.count,
        |formal| arg(call.source(formal)),
        result,
    )?;
    if let TextResult::Text { wide } = outcome {
        runtime.charge_new_text(result, wide)?;
    }
    stack
        .discard_top(call.count)
        .map_err(VmTrap::into_runtime_error)?;
    match outcome {
        TextResult::Text { wide } => stack.push_text(result, wide),
        TextResult::Value(value) => stack.push(value),
    }
    .map_err(VmTrap::into_runtime_error)?;
    runtime.check_execution_deadline()?;
    Ok(true)
}
