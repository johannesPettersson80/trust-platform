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
use crate::stdlib::conversions::{ConversionSpec, ConversionType};
use crate::stdlib::text_kernels::{self, TextArg, TextKernel, TextResult};
use crate::stdlib::{StandardLibrary, StdParams};
use crate::value::Value;
use alloc::string::String;
use alloc::vec::Vec;

/// The most arguments a text call by name binds here; one by position takes any number.
const MAX_NAMED: usize = 4;

/// The most arguments a text call with a target argument takes here.
const MAX_TARGETS: usize = 64;

/// The most arguments of a text call kept where they lie between the uses of one pass.
const KEPT_ARGUMENTS: usize = 8;

/// The buffers a text call works in, kept by the engine between calls (C1-12): the result, the
/// characters of `WSTRING` arguments decoded from their code units, where each lies, and a
/// conversion's cleaned digits.
#[derive(Debug, Default)]
pub struct TextBuffers {
    result: String,
    decoded: String,
    spans: Vec<(usize, usize)>,
    scratch: String,
}

impl TextBuffers {
    /// Room for a result of `result` UTF-8 bytes, for `arguments` UTF-8 bytes of the texts one
    /// call reads, decoded or parsed, and for `count` arguments, so that no call grows a buffer
    /// (spec 34 MEM-09; C1-19c2).
    pub(crate) fn reserve(
        &mut self,
        result: usize,
        arguments: usize,
        count: usize,
    ) -> Result<(), RuntimeError> {
        let refused = |_| RuntimeError::Overflow;
        self.result.try_reserve_exact(result).map_err(refused)?;
        self.decoded.try_reserve_exact(arguments).map_err(refused)?;
        self.spans.try_reserve_exact(count).map_err(refused)?;
        self.scratch.try_reserve_exact(arguments).map_err(refused)
    }

    /// The bytes [`TextBuffers::reserve`] takes.
    pub(crate) fn reservation_bytes(
        result: usize,
        arguments: usize,
        count: usize,
    ) -> Option<usize> {
        arguments
            .checked_mul(2)?
            .checked_add(result)?
            .checked_add(count.checked_mul(core::mem::size_of::<(usize, usize)>())?)
    }
}

/// A call of a built-in text function that runs on its arguments where they lie.
#[derive(Debug, Clone, Copy)]
pub struct TextCall {
    kernel: TextKernel,
    count: usize,
    /// For a call by name, the position among the call's arguments of the one each formal takes.
    named: Option<[u8; MAX_NAMED]>,
    /// The arguments passed as target references, one bit for each position.
    targets: u64,
}

impl TextCall {
    /// The number of the call's arguments.
    #[cfg(feature = "register-ir")]
    pub(crate) fn count(&self) -> usize {
        self.count
    }

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
    text_call_of(
        runtime.stdlib(),
        module.native_symbol_spec(word(4)?).ok()?,
        count,
    )
}

/// The text call of a standard-function `CALL_NATIVE` of the symbol `spec` with `count` arguments, as
/// [`text_call_at`] finds it: it depends on the symbol and the standard library alone, so a
/// composition may find it once for each symbol (C1-20).
pub(super) fn text_call_of(
    stdlib: &StandardLibrary,
    spec: &VmNativeSymbolSpec,
    count: usize,
) -> Option<TextCall> {
    let VmNativeSymbolSpec::Parsed {
        normalized_target_name,
        conversion_spec,
        arg_specs,
        ..
    } = spec
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
    if let Some(spec) = conversion_spec {
        // A conversion from or to a text, its one argument by position or named IN, as the call
        // that binds values binds it.
        let ConversionSpec::Convert { src, dst } = *spec else {
            return None;
        };
        let text =
            |ty: ConversionType| matches!(ty, ConversionType::String | ConversionType::WString);
        if !(text(dst) || src.is_some_and(text)) || count != 1 {
            return None;
        }
        if arg_specs[0]
            .name
            .as_ref()
            .is_some_and(|name| !name.eq_ignore_ascii_case("IN"))
        {
            return None;
        }
        return Some(TextCall {
            kernel: TextKernel::Convert { src, dst },
            count,
            named: None,
            targets,
        });
    }
    let kernel = TextKernel::of(normalized_target_name)?;
    let builtin = stdlib.builtin(normalized_target_name)?;
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
#[derive(Clone, Copy)]
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
    /// The argument a view shows, or `None` for a node of a backing, which holds no text and is
    /// bound as a value (C1-15).
    fn of_view(view: ValueView<'a>) -> Option<Self> {
        match view {
            ValueView::Value(value) => Some(Lying::Value(value)),
            ValueView::Cells {
                cells,
                range,
                type_id,
            } => Some(Lying::Cells {
                cells,
                range,
                type_id,
            }),
            // A program root's instance is no text either.
            ValueView::Bits { .. } | ValueView::Instance(_) => None,
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
            // A scalar of the cells is copied without a clone; any other leaf clones unless it is
            // a structure.
            Lying::Cells { cells, type_id, .. } => {
                !cells.holds_scalar(type_id) && !cells.is_struct(type_id)
            }
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
                .and_then(Lying::of_view)
        }
        // A target that is no reference is refused by the call that binds values.
        Operand::Value(_) if call.is_target(source) => None,
        Operand::Value(value) => Some(Lying::Value(value)),
        Operand::Place(place) => runtime
            .place_view(*place)
            .and_then(Lying::of_view)
            .filter(|lying| lying.cell_text().is_some()),
        Operand::Text(temp) => Some(Lying::Text(
            stack.text(*temp).map_err(VmTrap::into_runtime_error)?,
            temp.wide(),
        )),
        // The dispatcher gives a text call's enumerations and copies their values first (C1-19a,
        // C1-19b).
        Operand::Enum(_) | Operand::Copy(_) => None,
    })
}

fn run_with_buffers(
    runtime: &mut impl ExecutionContext,
    stack: &mut OperandStack,
    frame: &VmFrame,
    call: &TextCall,
    buffers: &mut TextBuffers,
) -> Result<bool, RuntimeError> {
    // Each argument is found where it lies once for the binding's charges and once for the
    // function, and kept between the uses of each pass (C1-20); the arguments of a call with more
    // than `KEPT_ARGUMENTS` are found again for each use.
    let (reads, binds) = {
        let operands = stack
            .top_operands(call.count)
            .ok_or_else(|| VmTrap::StackUnderflow.into_runtime_error())?;
        let mut kept = [None; KEPT_ARGUMENTS];
        for source in 0..call.count {
            let Some(argument) = lying(&*runtime, stack, frame, call, operands, source)? else {
                return Ok(false);
            };
            if let Some(slot) = kept.get_mut(source) {
                *slot = Some(argument);
            }
        }
        if runtime.deadline_exceeded() {
            return Err(VmTrap::DeadlineExceeded.into_runtime_error());
        }
        // The binding copies each argument in the order the call gives them, a target as a read
        // of it (TIME-08).
        let (mut reads, mut binds) = (0usize, 0usize);
        for source in 0..call.count {
            let argument = match kept.get(source) {
                Some(Some(argument)) => *argument,
                _ => lying(&*runtime, stack, frame, call, operands, source)?
                    .ok_or(RuntimeError::InvalidExecutionState)?,
            };
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
            if argument.copy_is_clone()? {
                if call.is_target(source) {
                    reads += 1;
                } else {
                    binds += 1;
                }
            }
        }
        (reads, binds)
    };
    for _ in 0..reads {
        runtime.record_value_op(RegisterValueOpKind::ReadReference);
    }
    for _ in 0..binds {
        runtime.record_value_op(RegisterValueOpKind::BindExpression);
    }
    let TextBuffers {
        result,
        decoded,
        spans,
        scratch,
    } = buffers;
    let operands = stack
        .top_operands(call.count)
        .ok_or_else(|| VmTrap::StackUnderflow.into_runtime_error())?;
    let mut kept = [None; KEPT_ARGUMENTS];
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
        if let Some(slot) = kept.get_mut(source) {
            *slot = argument;
        }
    }
    let arg = |source: usize| {
        let argument = match kept.get(source) {
            Some(Some(argument)) => *argument,
            _ => lying(&*runtime, stack, frame, call, operands, source)?
                .ok_or(RuntimeError::InvalidExecutionState)?,
        };
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
                None => TextArg::Scalar(cells.scalar(range, type_id)?.unwrap_or(Value::Null)),
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
        scratch,
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

/// The characters of a text operand where it lies (C1-19b): UTF-8, or a `WSTRING` leaf's code units.
enum Chars<'a> {
    Utf8(&'a str),
    Utf16(TextBytes<'a>),
}

impl Chars<'_> {
    /// Whether `self` and `other` have the same characters.
    fn same(&self, other: &Chars<'_>) -> bool {
        match (self, other) {
            (Chars::Utf8(left), Chars::Utf8(right)) => left == right,
            (Chars::Utf16(left), Chars::Utf16(right)) => left.units().eq(right.units()),
            (Chars::Utf16(wide), Chars::Utf8(text)) | (Chars::Utf8(text), Chars::Utf16(wide)) => {
                char::decode_utf16(wide.units())
                    .map(Result::ok)
                    .eq(text.chars().map(Some))
            }
        }
    }
}

/// The text `operand` stands for where it lies, and whether it is a `WSTRING`: a text value, a
/// temporary text, or a place that is a text leaf of the flat store which a read into its value
/// takes. `None` for any other operand, and for a `WSTRING` leaf whose code units are no valid
/// UTF-16, which that read refuses.
fn operand_text<'a>(
    runtime: &'a impl ExecutionContext,
    stack: &'a OperandStack,
    operand: &'a Operand,
) -> Option<(bool, Chars<'a>)> {
    match operand {
        Operand::Value(Value::String(text)) => Some((false, Chars::Utf8(text.as_str()))),
        Operand::Value(Value::WString(text)) => Some((true, Chars::Utf8(text.as_str()))),
        Operand::Text(temp) => Some((temp.wide(), Chars::Utf8(stack.text(*temp).ok()?))),
        Operand::Place(place) => match runtime.place_view(*place)? {
            ValueView::Cells {
                cells,
                range,
                type_id,
            } => match cells.text(range, type_id)? {
                TextBytes::Narrow(text) => Some((false, Chars::Utf8(text))),
                wide @ TextBytes::Wide(_) => char::decode_utf16(wide.units())
                    .all(|unit| unit.is_ok())
                    .then_some((true, Chars::Utf16(wide))),
            },
            _ => None,
        },
        _ => None,
    }
}

/// Whether `=` or `<>` compares the top two operands where they lie (C1-19b): two texts, each a text
/// value, a temporary text or a text leaf of the flat store, so neither is read into a value.
pub(super) fn texts_on_top(runtime: &impl ExecutionContext, stack: &OperandStack) -> bool {
    matches!(
        stack.top_operands(2),
        Some([left, right])
            if operand_text(runtime, stack, left).is_some()
                && operand_text(runtime, stack, right).is_some()
    )
}

/// Whether the top two operands, two texts, are equal as their values are: of one kind, `STRING` or
/// `WSTRING`, with the same characters (C1-19b). `None` when they are not two texts.
pub(super) fn equal_texts(runtime: &impl ExecutionContext, stack: &OperandStack) -> Option<bool> {
    let Some([left, right]) = stack.top_operands(2) else {
        return None;
    };
    let (left_wide, left) = operand_text(runtime, stack, left)?;
    let (right_wide, right) = operand_text(runtime, stack, right)?;
    Some(left_wide == right_wide && left.same(&right))
}

crate::memory::figure::holds_no_heap!(TextCall);
