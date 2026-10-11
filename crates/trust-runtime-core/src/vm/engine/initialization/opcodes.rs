//! Initialization instructions routed by the common stack dispatcher.
use super::super::super::{
    construction::values::{construct_value, ValueConstructionMode},
    OperandStack, VmTrap,
};
use super::*;
use crate::bytecode::opcodes::*;
use crate::collections::OrderedMap;
use crate::value::{ArrayValue, StructValue};
use alloc::{boxed::Box, sync::Arc};

impl EngineState<'_> {
    pub(in crate::vm::engine) fn initialization_opcode(
        &mut self,
        frame: &mut VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut OperandStack,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        if opcode != DEFAULT_VALUE && self.construction.initializers.is_empty() {
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
                    construct_value(context, operand, mode)
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
                let mut values = Vec::new();
                values
                    .try_reserve_exact(count)
                    .map_err(|_| RuntimeError::Overflow)?;
                values.resize(count, Value::Null);
                Value::Array(Box::new(ArrayValue::from_canonical_parts(
                    values,
                    vec![(0, i64::from(operand) - 1)],
                )))
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
                Value::Struct(Arc::new(StructValue::from_canonical_parts(
                    "".into(),
                    OrderedMap::default(),
                )))
            }
            STRUCT_SET => {
                let value = stack.pop().map_err(VmTrap::into_runtime_error)?;
                let Value::Struct(structure) = stack.pop().map_err(VmTrap::into_runtime_error)?
                else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let name = self
                    .prepared
                    .vm
                    .strings
                    .get(operand as usize)
                    .cloned()
                    .ok_or(RuntimeError::TypeMismatch)?;
                self.charge_work_units(structure.fields().len())?;
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
                let (type_name, mut fields) =
                    Arc::unwrap_or_clone(structure).into_canonical_parts();
                fields.insert(name, value);
                Value::Struct(Arc::new(StructValue::from_canonical_parts(
                    type_name, fields,
                )))
            }
            _ => return Err(VmTrap::InvalidOpcode(opcode).into_runtime_error()),
        };
        stack.push(value).map_err(VmTrap::into_runtime_error)
    }
}
