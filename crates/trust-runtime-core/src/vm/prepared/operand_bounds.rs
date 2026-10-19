//! The most a scan keeps beside its operands (specification 34 MEM-09; C1-19c2).
//!
//! The texts a built-in text function makes, the `WSTRING` constants a load pushes and the copies a
//! call makes of the places it leaves on the operand stack lie in the operand stack's temporary
//! area (C1-12, C1-19b); a text function writes its result into a result buffer first and decodes
//! the `WSTRING`s it reads; a conversion from a text parses it in a scratch buffer. Construction
//! reserves each for the most the program can need, so that no scan grows one, the first included.
//!
//! Preparation finds those maxima by walking every POU body and every initializer body as the
//! validator walks it, block by block with the operands each block starts with. Each operand keeps
//! an upper bound of the UTF-8 bytes of its text, of the bytes it holds in the temporary area, of
//! the bytes a call copies of it when it is a place, and its type when the walk knows it. A text
//! function's result is bounded by its arguments: `CONCAT` by their sum, `LEFT`, `RIGHT`, `MID`,
//! `DELETE` and the cut by the first, `INSERT` and `REPLACE` by the first two, a conversion to a
//! text by the longest text its source type gives. A block the walk keeps joining new operands
//! into, as a loop that carries a growing text across its back edge does, is widened to the
//! longest text a function may return, 65,535 characters, so the walk ends.

use alloc::vec;
use alloc::vec::Vec;

use crate::bytecode::{opcodes, StorageOwner, TypeData, NATIVE_CALL_KIND_STDLIB};
use crate::memory::cells::{is_reference_type, text_geometry};
use crate::stdlib::conversions::{ConversionSpec, ConversionType};
use crate::stdlib::text_kernels::TextKernel;
use crate::value::Value;
use crate::vm::module::VmNativeSymbolSpec;
use crate::vm::prepared::byte_types::{backing_place, copies_as_bytes, resolve};
use crate::vm::PreparedModule;

/// The most UTF-8 bytes of a text a function returns: 65,535 characters of at most three bytes.
const LONGEST_TEXT: u32 = 65_535 * 3;

/// The most UTF-8 bytes a conversion from a scalar writes: an `LREAL` written out without an
/// exponent, its sign, `0.`, 323 leading zeros and 17 significant digits.
const LONGEST_SCALAR_TEXT: u32 = 1 + 2 + 323 + 17;

/// How often the walk joins new operands into a block's before it widens them to their bounds, so
/// that a loop that carries a growing text across its back edge still ends.
const JOINS_BEFORE_WIDENING: u8 = 8;

/// The maxima construction reserves from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OperandBounds {
    /// The most bytes one activation holds in the temporary area at once.
    pub(crate) temporaries: usize,
    /// The most bytes an activation keeps in the temporary area below a call of a user function,
    /// function block or method while the callee runs.
    pub(crate) temporaries_below_call: usize,
    /// The most bytes one initializer body holds in the temporary area at once.
    pub(crate) initializer_temporaries: usize,
    /// The longest text one text function writes, in UTF-8 bytes.
    pub(crate) text_result: usize,
    /// The most UTF-8 bytes of the texts one text function reads.
    pub(crate) text_arguments: usize,
    /// The most arguments of one text function.
    pub(crate) text_count: usize,
}

impl OperandBounds {
    /// The bounds of every body of `prepared`.
    pub(crate) fn of(prepared: &PreparedModule) -> Self {
        let types = Types::of(prepared);
        let mut bounds = Self::default();
        let code = &prepared.vm.code;
        for (pou, entry) in prepared.vm.pou_by_id.iter() {
            let body = Body {
                code: code
                    .get(entry.code_start..entry.code_end)
                    .unwrap_or_default(),
                pou: Some(*pou),
            };
            let walked = walk(prepared, &types, &body, &mut bounds);
            bounds.temporaries = bounds.temporaries.max(walked.peak);
            bounds.temporaries_below_call = bounds.temporaries_below_call.max(walked.below_call);
        }
        for entry in &prepared.initializers.entries {
            let start = entry.code_offset as usize;
            let Some(end) = start.checked_add(entry.code_length as usize) else {
                continue;
            };
            let body = Body {
                code: code.get(start..end).unwrap_or_default(),
                pou: entry.owner_pou_id,
            };
            let walked = walk(prepared, &types, &body, &mut bounds);
            bounds.initializer_temporaries = bounds.initializer_temporaries.max(walked.peak);
        }
        bounds
    }
}

/// The module-wide fallbacks for an operand whose type the walk does not know.
struct Types {
    /// The longest text of any text type, in UTF-8 bytes.
    longest_text: u32,
    /// The bytes of the largest place a call may copy.
    largest_place: u32,
}

impl Types {
    fn of(prepared: &PreparedModule) -> Self {
        let mut types = Self {
            longest_text: 0,
            largest_place: 0,
        };
        for type_id in 0..prepared.vm.types.entries.len() {
            let Ok(type_id) = u32::try_from(type_id) else {
                break;
            };
            let loaded = Operand::loaded(prepared, None, Some(type_id));
            types.longest_text = types.longest_text.max(loaded.text);
            types.largest_place = types.largest_place.max(loaded.copy);
        }
        types
    }
}

/// One body: its code and the POU it runs for.
struct Body<'a> {
    code: &'a [u8],
    pou: Option<u32>,
}

/// What a body's walk found.
#[derive(Default)]
struct Walked {
    peak: usize,
    below_call: usize,
}

/// The walk's bound of one operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Operand {
    /// The most UTF-8 bytes of the operand's text, when it can be a text.
    text: u32,
    /// The most bytes the operand holds in the temporary area.
    temporary: u32,
    /// The most bytes a call copies of the operand when it is a place it leaves on the stack.
    copy: u32,
    /// The operand's type, or a reference's target type, when the walk knows it.
    ty: Option<u32>,
    /// The POU of an instance the operand is, when the walk knows it.
    pou: Option<u32>,
}

impl Operand {
    /// A value that holds no text and takes no room.
    const VALUE: Self = Self {
        text: 0,
        temporary: 0,
        copy: 0,
        ty: None,
        pou: None,
    };

    /// The operand a load of a value of `ty` pushes: its text, and its bytes when a store may copy
    /// it as a place (C1-10c). A load whose type the walk does not know is bounded by `fallback`.
    fn loaded(prepared: &PreparedModule, fallback: Option<&Types>, ty: Option<u32>) -> Self {
        let Some(ty) = ty else {
            return match fallback {
                Some(types) => Self {
                    text: types.longest_text,
                    copy: types.largest_place,
                    ..Self::VALUE
                },
                None => Self::VALUE,
            };
        };
        let text =
            text_geometry(prepared, ty).map_or(
                0,
                |(_, capacity, wide)| {
                    if wide {
                        capacity / 2 * 3
                    } else {
                        capacity
                    }
                },
            );
        let copy = if copies_as_bytes(prepared, ty) || backing_place(prepared, ty) {
            prepared
                .type_layout(ty)
                .map_or(0, |layout| layout.shape.bytes)
        } else {
            0
        };
        let pou = match resolve(prepared, ty).map(|entry| &entry.data) {
            Some(TypeData::Pou { pou_id }) => Some(*pou_id),
            _ => None,
        };
        Self {
            text,
            copy,
            ty: Some(ty),
            pou,
            ..Self::VALUE
        }
    }

    /// The operand a reference to a value of `target` is.
    fn reference(target: Option<u32>) -> Self {
        Self {
            ty: target,
            ..Self::VALUE
        }
    }

    /// The text `text` UTF-8 bytes long that a text function or a constant puts in the temporary
    /// area. A function writes all of a longer text than it may return before it refuses it, so
    /// the bound is not cut.
    fn temporary_text(text: u32) -> Self {
        Self {
            text,
            temporary: text,
            ..Self::VALUE
        }
    }

    /// The bound of an operand that is `self` on one path and `other` on another.
    fn join(self, other: Self) -> Self {
        Self {
            text: self.text.max(other.text),
            temporary: self.temporary.max(other.temporary),
            copy: self.copy.max(other.copy),
            ty: if self.ty == other.ty { self.ty } else { None },
            pou: if self.pou == other.pou {
                self.pou
            } else {
                None
            },
        }
    }

    /// `self` with every bound at its largest, as the walk widens a block it keeps joining.
    fn widened(self, types: &Types) -> Self {
        Self {
            text: LONGEST_TEXT,
            temporary: LONGEST_TEXT.max(types.largest_place),
            copy: types.largest_place,
            ty: None,
            pou: None,
        }
        .join(self)
    }
}

/// One decoded instruction: where it starts in the body, its opcode and its operands.
#[derive(Clone, Copy)]
struct Instruction {
    at: usize,
    opcode: u8,
    operands: [u32; 3],
}

impl Instruction {
    /// Where the next instruction starts.
    fn next(&self) -> usize {
        self.at + 1 + crate::vm::opcode_operand_len(self.opcode).unwrap_or(0)
    }

    /// Where a jump goes, in the body: relative to the next instruction.
    fn target(&self) -> Option<usize> {
        if !matches!(self.opcode, 0x02..=0x04) {
            return None;
        }
        let next = i64::try_from(self.at).ok()? + 5;
        usize::try_from(next + i64::from(self.operands[0] as i32)).ok()
    }
}

/// The body's instructions, or none when its code does not decode (the validator refused it).
fn decode(code: &[u8]) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    let mut at = 0;
    while at < code.len() {
        let opcode = code[at];
        let Some(width) = crate::vm::opcode_operand_len(opcode) else {
            return Vec::new();
        };
        let mut operands = [0u32; 3];
        for (index, operand) in operands.iter_mut().enumerate().take(width / 4) {
            let from = at + 1 + 4 * index;
            let Some(bytes) = code.get(from..from + 4) else {
                return Vec::new();
            };
            *operand = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        instructions.push(Instruction {
            at,
            opcode,
            operands,
        });
        at += 1 + width;
    }
    instructions
}

/// Walk `body`, block by block, and fold its text functions into `bounds`.
fn walk(
    prepared: &PreparedModule,
    types: &Types,
    body: &Body<'_>,
    bounds: &mut OperandBounds,
) -> Walked {
    let instructions = decode(body.code);
    let mut walked = Walked::default();
    if instructions.is_empty() {
        return walked;
    }
    let index_of = |at: usize| {
        instructions
            .binary_search_by_key(&at, |instruction| instruction.at)
            .ok()
    };
    // A block starts at the first instruction, at a jump's target and after a jump or a return,
    // as the validator's blocks do.
    let mut leader = vec![false; instructions.len()];
    leader[0] = true;
    for (index, instruction) in instructions.iter().enumerate() {
        if let Some(target) = instruction.target().and_then(index_of) {
            leader[target] = true;
        }
        if matches!(instruction.opcode, 0x02..=0x04 | 0x06) && index + 1 < instructions.len() {
            leader[index + 1] = true;
        }
    }
    let mut entries: Vec<Option<(Vec<Operand>, u8)>> = vec![None; instructions.len()];
    entries[0] = Some((Vec::new(), 0));
    let mut work = vec![0usize];
    while let Some(first) = work.pop() {
        let Some((mut stack, _)) = entries[first].clone() else {
            continue;
        };
        let mut index = first;
        loop {
            let instruction = instructions[index];
            step(
                prepared,
                types,
                body,
                instruction,
                &mut stack,
                &mut walked,
                bounds,
            );
            walked.peak = walked.peak.max(held(&stack));
            let ends = matches!(instruction.opcode, 0x02..=0x04 | 0x06);
            if !ends && index + 1 < instructions.len() && !leader[index + 1] {
                index += 1;
                continue;
            }
            let successors = match instruction.opcode {
                0x06 => [None, None],
                0x02 => [instruction.target(), None],
                0x03 | 0x04 => [instruction.target(), Some(instruction.next())],
                _ => [Some(instruction.next()), None],
            };
            for at in successors.into_iter().flatten() {
                if let Some(target) = index_of(at)
                    && merge(&mut entries[target], &stack, types)
                {
                    work.push(target);
                }
            }
            break;
        }
    }
    walked
}

/// Join `stack` into the operands a block starts with; whether they changed.
fn merge(entry: &mut Option<(Vec<Operand>, u8)>, stack: &[Operand], types: &Types) -> bool {
    match entry {
        None => {
            *entry = Some((stack.to_vec(), 0));
            true
        }
        Some((current, joins)) => {
            if current.len() != stack.len() {
                // The validator refuses operand stacks of different depths at a merge.
                return false;
            }
            let widen = *joins >= JOINS_BEFORE_WIDENING;
            let mut changed = false;
            for (current, incoming) in current.iter_mut().zip(stack) {
                let mut joined = current.join(*incoming);
                if widen && joined != *current {
                    joined = joined.widened(types);
                }
                changed |= joined != *current;
                *current = joined;
            }
            if changed {
                *joins = joins.saturating_add(1);
            }
            changed
        }
    }
}

/// The bytes the operands `stack` hold in the temporary area.
fn held(stack: &[Operand]) -> usize {
    stack
        .iter()
        .map(|operand| operand.temporary as usize)
        .fold(0usize, usize::saturating_add)
}

/// Pop one operand; a stack the validator admitted holds it.
fn pop(stack: &mut Vec<Operand>) -> Operand {
    stack.pop().unwrap_or(Operand::VALUE)
}

/// Apply `instruction` to the walk's operands.
fn step(
    prepared: &PreparedModule,
    types: &Types,
    body: &Body<'_>,
    instruction: Instruction,
    stack: &mut Vec<Operand>,
    walked: &mut Walked,
    bounds: &mut OperandBounds,
) {
    let first = instruction.operands[0];
    match instruction.opcode {
        0x03 | 0x04 | 0x12 | 0x21 => {
            pop(stack);
        }
        0x09 => call(prepared, types, instruction, stack, walked, bounds),
        0x10 => stack.push(match prepared.vm.consts.get(first as usize) {
            // A `WSTRING` constant is loaded into the temporary area (C1-12); a `STRING`
            // constant's copy shares its characters.
            Some(Value::WString(text)) => Operand::temporary_text(len(text.len())),
            Some(Value::String(text)) => Operand {
                text: len(text.len()),
                ..Operand::VALUE
            },
            _ => Operand::VALUE,
        }),
        0x11 => {
            let top = stack.last().copied().unwrap_or(Operand::VALUE);
            stack.push(top);
        }
        0x13 => {
            let len = stack.len();
            if len >= 2 {
                stack.swap(len - 1, len - 2);
            }
        }
        0x20 => stack.push(load(prepared, prepared.vm.ref_type(first))),
        0x22 => stack.push(Operand::reference(prepared.vm.ref_type(first))),
        0x23 | 0x24 => stack.push(Operand {
            pou: body.pou,
            ..Operand::VALUE
        }),
        0x30 => {
            let base = pop(stack);
            stack.push(Operand::reference(member_type(prepared, base, first)));
        }
        0x31 => {
            pop(stack);
            let base = pop(stack);
            let element =
                base.ty
                    .and_then(|ty| match resolve(prepared, ty).map(|entry| &entry.data) {
                        Some(TypeData::Array { elem_type_id, .. }) => Some(*elem_type_id),
                        _ => None,
                    });
            stack.push(Operand::reference(element));
        }
        0x32 => {
            let reference = pop(stack);
            stack.push(match reference.ty {
                Some(ty) => load(prepared, Some(ty)),
                None => Operand::loaded(prepared, Some(types), None),
            });
        }
        0x33 => {
            pop(stack);
            pop(stack);
        }
        0x45 | 0x49 | 0x61 | 0x62 | 0x64 => {
            pop(stack);
            stack.push(Operand::VALUE);
        }
        0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 | 0x63 => {
            pop(stack);
            pop(stack);
            stack.push(Operand::VALUE);
        }
        0x60
        | opcodes::DEFAULT_VALUE
        | opcodes::DEFAULT_TYPED
        | opcodes::ARRAY_NEW
        | opcodes::STRUCT_NEW => stack.push(Operand::VALUE),
        opcodes::COERCE_INIT_VALUE | opcodes::APPLY_INIT_VALUE => {
            let value = pop(stack);
            stack.push(Operand {
                temporary: 0,
                copy: 0,
                ..value
            });
        }
        opcodes::ARRAY_SET | opcodes::STRUCT_SET => {
            pop(stack);
            let aggregate = pop(stack);
            stack.push(Operand {
                temporary: 0,
                copy: 0,
                ..aggregate
            });
        }
        _ => {}
    }
}

/// The operand a load of a value of `ty` pushes; a reference variable's value is a reference to
/// its target type.
fn load(prepared: &PreparedModule, ty: Option<u32>) -> Operand {
    match ty.and_then(|ty| resolve(prepared, ty).map(|entry| (ty, &entry.data))) {
        Some((_, TypeData::Reference { target_type_id })) => {
            Operand::reference(Some(*target_type_id))
        }
        Some((ty, _)) if is_reference_type(prepared, ty) => Operand::reference(None),
        _ => Operand::loaded(prepared, None, ty),
    }
}

/// The type of the member named by string `name` of what `base` refers to: a structure's field or
/// a function block's or class's variable, its own or an ancestor's.
fn member_type(prepared: &PreparedModule, base: Operand, name: u32) -> Option<u32> {
    let name = prepared.vm.strings.get(name as usize)?;
    let named = |name_idx: u32| {
        prepared
            .vm
            .strings
            .get(name_idx as usize)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
    };
    let pou = match base
        .ty
        .and_then(|ty| resolve(prepared, ty))
        .map(|entry| &entry.data)
    {
        Some(TypeData::Struct { fields } | TypeData::Union { fields }) => {
            return fields
                .iter()
                .find(|field| named(field.name_idx))
                .map(|field| field.type_id);
        }
        Some(TypeData::Pou { pou_id }) => Some(*pou_id),
        _ => base.pou,
    };
    let mut level = pou;
    for _ in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        let current = level?;
        for &id in prepared.declarations(StorageOwner::Instance, Some(current)) {
            let declaration = prepared.layout.entries.get(id as usize)?;
            if named(declaration.name_idx) {
                return declaration.type_id;
            }
        }
        level = prepared.vm.parent_pou_ids.get(&current).copied();
    }
    None
}

/// A call: its kind, its symbol and its argument count are the instruction's operands.
fn call(
    prepared: &PreparedModule,
    types: &Types,
    instruction: Instruction,
    stack: &mut Vec<Operand>,
    walked: &mut Walked,
    bounds: &mut OperandBounds,
) {
    let [kind, symbol, count] = instruction.operands;
    let count = (count as usize).min(stack.len());
    let below = stack.len() - count;
    if kind != NATIVE_CALL_KIND_STDLIB {
        // A call of a user function, function block or method copies the places it leaves on
        // the stack (C1-19b), which stay there while the callee runs.
        for operand in &mut stack[..below] {
            operand.temporary = operand.temporary.max(operand.copy);
        }
        walked.below_call = walked.below_call.max(held(&stack[..below]));
        walked.peak = walked.peak.max(held(stack));
        stack.truncate(below);
        // A user function's result is a value: bounded as any text, it holds no room.
        stack.push(Operand {
            text: types.longest_text,
            ..Operand::VALUE
        });
        return;
    }
    let arguments = stack.split_off(below);
    let Ok(VmNativeSymbolSpec::Parsed {
        normalized_target_name,
        conversion_spec,
        ..
    }) = prepared.vm.native_symbol_spec(symbol)
    else {
        stack.push(Operand::VALUE);
        return;
    };
    let text = |index: usize| arguments.get(index).map_or(0, |operand| operand.text);
    let sum = |indexes: core::ops::Range<usize>| indexes.map(text).fold(0u32, u32::saturating_add);
    let result = match conversion_spec {
        Some(ConversionSpec::Convert { src, dst }) if is_text(*dst) => {
            let from = arguments.first().copied().unwrap_or(Operand::VALUE);
            Some(if from.text > 0 || src.is_some_and(is_text) {
                from.text
            } else {
                LONGEST_SCALAR_TEXT
            })
        }
        Some(_) => None,
        None => match TextKernel::of(normalized_target_name) {
            Some(TextKernel::Concat) => Some(sum(0..arguments.len())),
            Some(TextKernel::Insert | TextKernel::Replace) => Some(sum(0..2)),
            Some(
                TextKernel::Left
                | TextKernel::Right
                | TextKernel::Mid
                | TextKernel::Delete
                | TextKernel::Limit,
            ) => Some(text(0)),
            Some(TextKernel::Len | TextKernel::Find | TextKernel::Convert { .. }) | None => None,
        },
    };
    let read = sum(0..arguments.len());
    if TextKernel::of(normalized_target_name).is_some() || conversion_spec.is_some() {
        bounds.text_arguments = bounds.text_arguments.max(read as usize);
        bounds.text_count = bounds.text_count.max(arguments.len());
    }
    match result {
        Some(text) => {
            let pushed = Operand::temporary_text(text);
            bounds.text_result = bounds.text_result.max(pushed.text as usize);
            stack.push(pushed);
        }
        // Any other standard function returns one of its arguments at most, or no text.
        None => stack.push(Operand {
            text: arguments
                .iter()
                .map(|operand| operand.text)
                .max()
                .unwrap_or(0),
            ..Operand::VALUE
        }),
    }
}

/// Whether a conversion type is a text.
fn is_text(ty: ConversionType) -> bool {
    matches!(ty, ConversionType::String | ConversionType::WString)
}

/// A length as the walk counts bytes.
fn len(bytes: usize) -> u32 {
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::PreparationLimits;

    const COPIES: &[u8] =
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/copies/program.stbc");

    /// Spec 34 MEM-09 (C1-19c2): the walk bounds what a scan of the `copies` corpus program keeps
    /// beside its operands, worked out by hand from `copies/src/main.st`. Its text calls are the
    /// cuts the compiler puts before an assignment to a text of another capacity: the largest is
    /// `wide_short := wide`, whose text, a `WSTRING[8]`, is 8 code units of at most 3 UTF-8 bytes,
    /// 24 bytes, and whose other argument is the capacity, a number; so the longest result, the
    /// most bytes one call reads and the most the area holds at once are 24, with 2 arguments.
    /// Below a user call lie the places a call copies (C1-19b): `pairs[1]`, a `Pair` of a DINT and a
    /// REAL, 8 bytes, below `Bump`, and `tags[1]`, a `Tagged` of an INT and a `STRING[8]` (a 2-byte
    /// length and 8 bytes), 12 bytes, below `Flip`. Catches a walk that misses the copies a call
    /// makes (0 below a call), one that bounds a `WSTRING` by its code units (16) and one that adds
    /// the capacity argument to the text (more than 24).
    #[test]
    fn the_copies_program_keeps_at_most_its_largest_cut_and_copy() {
        let prepared = PreparedModule::from_bytes(COPIES, PreparationLimits::default())
            .expect("the copies program prepares");
        let bounds = prepared.operand_bounds;
        assert_eq!(bounds.temporaries, 24);
        assert_eq!(bounds.temporaries_below_call, 12);
        assert_eq!(bounds.text_result, 24);
        assert_eq!(bounds.text_arguments, 24);
        assert_eq!(bounds.text_count, 2);
    }
}
