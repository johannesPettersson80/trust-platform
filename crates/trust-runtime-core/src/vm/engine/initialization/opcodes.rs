//! Initialization instructions routed by the common stack dispatcher.
use super::super::super::{
    construction::values::{construct_value, ValueConstructionMode},
    OperandStack, VmTrap,
};
use super::*;
use crate::bytecode::opcodes::*;
use crate::value::StructValue;
use alloc::sync::Arc;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn initialization_opcode(
        &mut self,
        frame: &mut VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut OperandStack,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        if opcode != DEFAULT_VALUE && self.data.construction.initializers.is_empty() {
            return Err(VmTrap::InvalidOpcode(opcode).into_runtime_error());
        }
        let value = match opcode {
            DEFAULT_VALUE => {
                self.evaluate_initializer(operand, None, None, depth.saturating_add(1))?
            }
            DEFAULT_TYPED | COERCE_INIT_VALUE | APPLY_INIT_VALUE => {
                let mode = match opcode {
                    DEFAULT_TYPED => ValueConstructionMode::Default,
                    COERCE_INIT_VALUE => ValueConstructionMode::Coerce(
                        stack.pop().map_err(VmTrap::into_runtime_error)?,
                    ),
                    _ => ValueConstructionMode::Apply(
                        stack.pop().map_err(VmTrap::into_runtime_error)?,
                    ),
                };
                self.with_construction_frame(frame, |context| {
                    let prepared = context.prepared;
                    construct_value(&prepared.vm.types, context, operand, mode)
                })?
            }
            ARRAY_NEW => {
                let count = operand as usize;
                self.charge_constructed_values(
                    count.checked_add(1).ok_or(RuntimeError::Overflow)?,
                )?;
                self.charge_work_units(count)?;
                ValueConstructionContext::charge_allocation(
                    self,
                    count
                        .checked_mul(core::mem::size_of::<Value>())
                        .ok_or(RuntimeError::Overflow)?,
                )?;
                Value::Array(self.data.resources.raw_shells.array(operand)?)
            }
            ARRAY_SET => {
                let value = stack.pop().map_err(VmTrap::into_runtime_error)?;
                let Value::Array(mut array) = stack.pop().map_err(VmTrap::into_runtime_error)?
                else {
                    return Err(RuntimeError::TypeMismatch);
                };
                *array
                    .elements_mut()
                    .get_mut(operand as usize)
                    .ok_or(RuntimeError::TypeMismatch)? = value;
                Value::Array(array)
            }
            STRUCT_NEW => {
                self.charge_constructed_value()?;
                if operand != 0 {
                    return Err(RuntimeError::TypeMismatch);
                }
                ValueConstructionContext::charge_allocation(
                    self,
                    core::mem::size_of::<StructValue>(),
                )?;
                Value::Struct(self.data.resources.raw_shells.structure())
            }
            STRUCT_SET => {
                let value = stack.pop().map_err(VmTrap::into_runtime_error)?;
                let Value::Struct(mut structure) =
                    stack.pop().map_err(VmTrap::into_runtime_error)?
                else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let name = self
                    .prepared
                    .vm
                    .strings
                    .get(operand as usize)
                    .map(SmolStr::new)
                    .ok_or(RuntimeError::TypeMismatch)?;
                if structure
                    .fields()
                    .keys()
                    .any(|field| field.eq_ignore_ascii_case(&name))
                {
                    return Err(RuntimeError::TypeMismatch);
                }
                ValueConstructionContext::charge_allocation(
                    self,
                    core::mem::size_of::<(smol_str::SmolStr, Value)>(),
                )?;
                // A structure nothing else holds takes the field in place, in its container; a shared
                // one is copied first. Either way the result is the same structure, without a
                // physical layout.
                if let Some(raw) = Arc::get_mut(&mut structure) {
                    raw.physical = None;
                    raw.fields.insert(name, value);
                    Value::Struct(structure)
                } else {
                    let (type_name, mut fields) =
                        Arc::unwrap_or_clone(structure).into_canonical_parts();
                    fields.insert(name, value);
                    Value::Struct(Arc::new(StructValue::from_canonical_parts(
                        type_name, fields,
                    )))
                }
            }
            _ => return Err(VmTrap::InvalidOpcode(opcode).into_runtime_error()),
        };
        stack.push(value).map_err(VmTrap::into_runtime_error)
    }
}
