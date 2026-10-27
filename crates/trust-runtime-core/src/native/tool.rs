//! A tool's call of a method of a live function block or class instance between scans
//! (specification 44 §5.4, §5.7, §7.1, §13.3): the instance found by its path, the method by an
//! interface the instance's type implements, the inputs written into the call block that the
//! method's tool symbol names in the stack area, the method's tool entry called with the tool's
//! statement allowance, and the result read back from the block. The instance's body does not run,
//! and a call that uses its allowance up is undone. The kernel makes the call, so that every device
//! takes it alike.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_plc_abi::module::GENERATION;
use trust_plc_abi::{RefValue, AREA_STACK};

use super::image::{EntryKind, EntryRecord, Image, SymbolRecord, SYMBOL_INPUT};
use super::memory::Place;
use super::program::{Memory, ModuleCode, NativeProgram};
use super::reflect::{resolve_path, type_implements, Located, PathError};
use super::values::{damaged, holds_non_finite, Types};
use super::variables::CompiledVariables;
use crate::error::{ExecutionLimitKind, RuntimeError};
use crate::value::{Duration, Value};

/// The start of a tool symbol's canonical name (§5.7), which no variable's name can have.
pub const TOOL_SYMBOL_PREFIX: &str = "@TOOL.";

/// The part of a call block that names the instance, as a value of the interface (§7.3).
pub const THIS_PART: &str = "THIS";

/// The part of a call block that marks the inputs a call gives: bit n % 8 of its byte n / 8 for
/// the n-th input in the order of the inputs' offsets, their declaration order. Its name begins
/// with `__`, as no identifier does, so that it is no input's name.
pub const GIVEN_PART: &str = "__GIVEN";

/// The refusal of a method that takes another parameter than an input whose value its call block
/// holds.
pub const ONLY_INPUTS: &str = "a host method call accepts only input parameters";

/// The refusal of a call while a scan runs.
const DURING_A_SCAN: &str = "a method is called between scans, and a scan is running";

/// The canonical name of the tool symbol of method `method` of interface `interface` (§5.7):
/// `@TOOL.LIB.IRUN.START` for `Lib.IRun.Start`.
#[must_use]
pub fn tool_symbol(interface: &str, method: &str) -> String {
    format!(
        "{TOOL_SYMBOL_PREFIX}{}.{}",
        interface.to_ascii_uppercase(),
        method.to_ascii_uppercase()
    )
}

/// Why a tool's call of a method did not complete.
#[derive(Debug, Clone, PartialEq)]
pub enum MethodCallError {
    /// The call was refused, before the method ran or, its allowance used up, undone: the
    /// program's memory is as it was before the call.
    Refused(RuntimeError),
    /// The method faulted: the program's memory is as the fault left it, and the resource faults
    /// as it does at a faulted scan (specification 44 §8.6).
    Fault(RuntimeError),
}

/// The call block of a tool entry, as the parts of its tool symbol name it in the stack area.
struct Block {
    /// The tool entry's index in the entry table.
    entry: usize,
    /// The offset of `THIS`.
    this: u32,
    /// The offset and the bytes of the mask of the given inputs.
    given: (u32, usize),
    /// The inputs by their canonical names, in the order of their offsets.
    inputs: Vec<(String, Located)>,
    /// The result, for a method that has one.
    result: Option<Located>,
}

/// The mask of a call's given inputs, and the place of each with its value.
type Given<'v> = (Vec<u8>, Vec<(Located, &'v Value)>);

impl Block {
    /// The call block of method `method` of interface `interface`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::UndefinedField`] for a method the image has no tool symbol of, and
    /// [`RuntimeError::InvalidConfig`] ([`ONLY_INPUTS`]) for one whose tool symbol no entry names.
    fn of(image: &Image<'_>, interface: &str, method: &str) -> Result<Self, RuntimeError> {
        let name = tool_symbol(interface, method);
        let Some((index, _)) = image.find_symbol(&name) else {
            return Err(RuntimeError::UndefinedField(SmolStr::new(format!(
                "{interface}.{method}"
            ))));
        };
        let entries = image
            .table::<EntryRecord>()
            .map_err(|_| damaged("entry table"))?;
        let Some(entry) = entries
            .iter()
            .position(|entry| entry.kind == EntryKind::Method as u32 && entry.index == index)
        else {
            return Err(RuntimeError::InvalidConfig(SmolStr::new_static(
                ONLY_INPUTS,
            )));
        };
        let symbols = image
            .table::<SymbolRecord>()
            .map_err(|_| damaged("symbol table"))?;
        // The parts follow the tool symbol in the sorted table, each `<tool symbol>.<PART>`.
        let prefix = format!("{name}.");
        let result_name = format!("{prefix}{}", method.to_ascii_uppercase());
        let (mut this, mut given, mut result) = (None, None, None);
        let mut inputs = Vec::new();
        for at in index.saturating_add(1)..symbols.len() {
            let part = symbols.get(at).ok_or_else(|| damaged("symbol table"))?;
            let canonical = image
                .string(part.canonical)
                .ok_or_else(|| damaged("string pool"))?;
            let Some(short) = canonical.strip_prefix(prefix.as_str()) else {
                break;
            };
            if part.parent != index {
                continue;
            }
            let located = Located {
                place: Place::Area {
                    area: AREA_STACK,
                    offset: part.offset,
                },
                type_index: part.type_index,
            };
            if short == THIS_PART {
                this = Some(part.offset);
            } else if short == GIVEN_PART {
                given = Some((part.offset, part.size as usize));
            } else if part.flags & SYMBOL_INPUT != 0 {
                inputs.push((part.offset, String::from(short), located));
            } else if canonical == result_name {
                result = Some(located);
            }
        }
        let (Some(this), Some(given)) = (this, given) else {
            return Err(damaged("symbol table"));
        };
        if inputs.len() > given.1.saturating_mul(8) {
            return Err(damaged("symbol table"));
        }
        inputs.sort_unstable_by_key(|(offset, _, _)| *offset);
        Ok(Self {
            entry,
            this,
            given,
            inputs: inputs
                .into_iter()
                .map(|(_, name, located)| (name, located))
                .collect(),
            result,
        })
    }

    /// The mask of `inputs` and each one's place: an input the block holds, given once, with a
    /// value that holds no `REAL` or `LREAL` NaN or infinity (specification 34 §4.3.1).
    fn admit<'v>(&self, inputs: &'v [(SmolStr, Value)]) -> Result<Given<'v>, RuntimeError> {
        let mut mask = vec![0u8; self.given.1];
        let mut places = Vec::with_capacity(inputs.len());
        for (name, value) in inputs {
            let position = self
                .inputs
                .iter()
                .position(|(input, _)| input.eq_ignore_ascii_case(name))
                .filter(|position| mask[position / 8] & (1 << (position % 8)) == 0);
            let Some(position) = position else {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            };
            if holds_non_finite(value) {
                return Err(RuntimeError::NonFiniteValue);
            }
            mask[position / 8] |= 1 << (position % 8);
            places.push((self.inputs[position].1, value));
        }
        Ok((mask, places))
    }
}

/// The value of an interface that names the instance at `at` (specification 44 §7.3): a reference
/// of kind instance, its extent the instance's type's, in this program's generation.
fn instance_reference(image: &Image<'_>, at: Located) -> Result<RefValue, RuntimeError> {
    let Place::Area { area, offset } = at.place else {
        return Err(damaged("symbol table"));
    };
    Ok(RefValue {
        area,
        kind: RefValue::INSTANCE,
        char_index: 0,
        offset,
        extent: Types::of(image)?.get(at.type_index)?.size,
        generation: GENERATION,
    })
}

/// The bytes of `reference`, laid out as `RefValue` is (specification 44 §7.3).
fn reference_bytes(reference: &RefValue) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0] = reference.area;
    bytes[1] = reference.kind;
    bytes[2..4].copy_from_slice(&reference.char_index.to_le_bytes());
    bytes[4..8].copy_from_slice(&reference.offset.to_le_bytes());
    bytes[8..12].copy_from_slice(&reference.extent.to_le_bytes());
    bytes[12..16].copy_from_slice(&reference.generation.to_le_bytes());
    bytes
}

/// The refusal of a call whose allowance of `allowance` statements is used up, worded as the
/// budget of a scan is (specification 44 NX-10).
fn used_up(allowance: u32) -> RuntimeError {
    let allowance = usize::try_from(allowance).unwrap_or(usize::MAX);
    RuntimeError::ExecutionLimit {
        kind: ExecutionLimitKind::Work,
        used: allowance,
        requested: 1,
        limit: allowance,
    }
}

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// Calls method `method` of interface `interface` on the function block or class instance at
    /// `path` (a path as [`resolve_path`] resolves it), between scans at the logical time `now`,
    /// with the inputs `inputs` by their names and an allowance of `allowance` statements and loop
    /// passes (specification 44 §13.3). The instance's type implements the interface; the method's
    /// tool entry calls it through the interface, as a call through an interface reference binds
    /// it, so a derived type's override is the one that runs; an input the call does not give
    /// takes its declared initial value (IEC 61131-3 §6.6.1); the instance's body does not run.
    /// Each given input is converted as a Live Values write converts it. Returns the method's
    /// result, `Value::Null` for a method without one, and the statements and loop passes it used.
    ///
    /// # Errors
    ///
    /// [`MethodCallError::Refused`], the program unchanged: for an allowance of 0; a path that
    /// names nothing ([`RuntimeError::UndefinedVariable`]); an instance whose type does not
    /// implement the interface ([`RuntimeError::ControlError`]); a method the interface does not
    /// have ([`RuntimeError::UndefinedField`]) or that takes another parameter than an input whose
    /// value its call block holds ([`ONLY_INPUTS`]); an input the method does not have or given
    /// twice ([`RuntimeError::InvalidArgumentName`]); a value its input refuses, a NaN or an
    /// infinity among them; a call during a scan; and a call that used its allowance up, which is
    /// undone. [`MethodCallError::Fault`] for a fault of the method.
    pub fn call_method(
        &mut self,
        path: &str,
        interface: &str,
        method: &str,
        inputs: &[(SmolStr, Value)],
        allowance: u32,
        now: Duration,
    ) -> Result<(Value, u32), MethodCallError> {
        self.may_call(allowance)?;
        let (block, this) = {
            let image = self.image();
            let instance = resolve_path(&image, path).map_err(|error| {
                MethodCallError::Refused(match error {
                    PathError::Damaged => damaged("symbol table"),
                    _ => RuntimeError::UndefinedVariable(SmolStr::new(path)),
                })
            })?;
            if !type_implements(&image, instance.type_index, interface)
                .map_err(MethodCallError::Refused)?
            {
                return Err(MethodCallError::Refused(RuntimeError::ControlError(
                    SmolStr::new(format!("{path} does not implement {interface}")),
                )));
            }
            let block = Block::of(&image, interface, method).map_err(MethodCallError::Refused)?;
            let this = instance_reference(&image, instance).map_err(MethodCallError::Refused)?;
            (block, this)
        };
        let (mask, places) = block.admit(inputs).map_err(MethodCallError::Refused)?;
        // The block lies in the stack area, which no scan uses between scans.
        if !self.write(AREA_STACK, block.this as usize, &reference_bytes(&this))
            || !self.write(AREA_STACK, block.given.0 as usize, &mask)
        {
            return Err(MethodCallError::Refused(damaged("symbol table")));
        }
        for (place, value) in places {
            self.write_located(&place, value)
                .map_err(MethodCallError::Refused)?;
        }
        let used = self.call_tool_entry(block.entry, allowance, now)?;
        let result = match &block.result {
            Some(result) => self.read_located(result).map_err(MethodCallError::Fault)?,
            None => Value::Null,
        };
        Ok((result, used))
    }

    /// Calls the tool entry `entry` between scans at the logical time `now` with an allowance of
    /// `allowance` statements and loop passes, as a program body runs with no task (specification
    /// 44 §8.3, §8.4): it does not stop at a breakpoint, and returns the statements and loop passes
    /// it used. A call that uses its allowance up is undone: every area but the stack is as it was
    /// before the call.
    ///
    /// # Errors
    ///
    /// [`MethodCallError::Refused`], the program unchanged, while a scan runs, for an allowance of
    /// 0, for an entry that is no tool entry, and for a call that used its allowance up;
    /// [`MethodCallError::Fault`] for the fault the call ended with.
    pub fn call_tool_entry(
        &mut self,
        entry: usize,
        allowance: u32,
        now: Duration,
    ) -> Result<u32, MethodCallError> {
        self.may_call(allowance)?;
        let tool = u32::try_from(entry)
            .ok()
            .and_then(|index| self.image().table::<EntryRecord>().ok()?.get(index))
            .is_some_and(|record| record.kind == EntryKind::Method as u32);
        if !tool {
            return Err(MethodCallError::Refused(
                RuntimeError::InvalidExecutionState,
            ));
        }
        // What a call that uses its allowance up is undone to: every area but the stack, whose
        // block is the call's own.
        let stack = usize::from(AREA_STACK) - 1;
        let kept: Vec<Vec<u8>> = self.areas[..stack].iter().map(Memory::bytes).collect();
        match self.call_within(entry, u32::MAX, now, allowance, None) {
            Ok(()) => Ok(allowance.saturating_sub(self.cx.budget)),
            Err(
                error @ RuntimeError::ExecutionLimit {
                    kind: ExecutionLimitKind::Work,
                    ..
                },
            ) => {
                for (memory, bytes) in self.areas.iter_mut().zip(&kept) {
                    memory.write(0, bytes);
                }
                Err(MethodCallError::Refused(error))
            }
            Err(error) => Err(MethodCallError::Fault(error)),
        }
    }

    /// Refuses a call while a scan runs, before anything is written, and a call without an
    /// allowance.
    fn may_call(&self, allowance: u32) -> Result<(), MethodCallError> {
        if self.scanning {
            return Err(MethodCallError::Refused(RuntimeError::ControlError(
                SmolStr::new_static(DURING_A_SCAN),
            )));
        }
        if allowance == 0 {
            return Err(MethodCallError::Refused(used_up(0)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
