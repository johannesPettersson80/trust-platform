//! One TYPE_TABLE-driven traversal for default, coercion and explicit initialization.
use crate::bytecode::{Field, TypeData, TypeEntry, TypeTable};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::value::{ArrayValue, DateTimeProfile, EnumValue, StructValue, Value};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::mem::size_of;
use smol_str::SmolStr;

pub(in crate::vm) mod scalar;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueOperation {
    Default,
    Coerce,
    Apply,
}

#[derive(Debug)]
pub enum ValueConstructionMode {
    Default,
    Coerce(Value),
    Apply(Value),
}
impl ValueConstructionMode {
    fn operation(&self) -> ValueOperation {
        match self {
            Self::Default => ValueOperation::Default,
            Self::Coerce(_) => ValueOperation::Coerce,
            Self::Apply(_) => ValueOperation::Apply,
        }
    }
}

/// Immutable artifact metadata and charged callbacks supplied by the engine.
/// Recipe results are already typed by their own COERCE/APPLY bytecode. Their
/// evaluation inherits the active action context; the helper never commits storage.
pub trait ValueConstructionContext {
    fn strings(&self) -> &[SmolStr];
    /// Bind rebuilt fields to the active installation's immutable physical layout.
    fn bind_structure_layout(
        &mut self,
        _type_id: u32,
        value: StructValue,
    ) -> Result<StructValue, RuntimeError> {
        Ok(value)
    }
    /// Preserve unnamed physical bits when rebuilding a compatible typed value.
    /// A field-only initializer has no physical source and uses ordinary binding.
    fn bind_structure_layout_preserving(
        &mut self,
        type_id: u32,
        value: StructValue,
        _source: &StructValue,
    ) -> Result<StructValue, RuntimeError> {
        self.bind_structure_layout(type_id, value)
    }
    /// Construct fresh default storage in declaration order. Unlike binding a
    /// supplied value, overlapping default fields need not already agree.
    fn initialize_structure_layout(
        &mut self,
        type_id: u32,
        value: StructValue,
    ) -> Result<StructValue, RuntimeError> {
        self.bind_structure_layout(type_id, value)
    }
    fn profile(&self) -> DateTimeProfile;
    fn type_recipe(&self, type_id: u32) -> Option<u32>;
    fn member_recipe(&self, type_id: u32, member: u32) -> Option<u32>;
    fn evaluate_recipe(&mut self, recipe_id: u32) -> Result<Value, RuntimeError>;
    fn construct_instance(&mut self, pou_id: u32, intrinsic: bool) -> Result<Value, RuntimeError>;
    fn charge_value(&mut self) -> Result<(), RuntimeError>;
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    fn charge_work(&mut self, units: usize) -> Result<(), RuntimeError>;
    fn charge_allocation(&mut self, bytes: usize) -> Result<(), RuntimeError>;
    /// Enforce actual instance/reference compatibility and recursive reference lifetime.
    fn check_value(&mut self, type_id: u32, value: &Value) -> Result<(), RuntimeError>;
    /// This stack spans helper/dispatcher callbacks; Default and Coerce are distinct.
    fn enter_type(&mut self, type_id: u32, operation: ValueOperation) -> Result<(), RuntimeError>;
    fn leave_type(&mut self);
}

pub fn construct_value(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    type_id: u32,
    mode: ValueConstructionMode,
) -> Result<Value, RuntimeError> {
    construct(types, ctx, type_id, mode, false, 0)
}

/// Return slots and intrinsic-only action stages share traversal but skip recipes.
pub fn construct_intrinsic_value(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    type_id: u32,
) -> Result<Value, RuntimeError> {
    construct(types, ctx, type_id, ValueConstructionMode::Default, true, 0)
}

fn construct<C: ValueConstructionContext>(
    types: &TypeTable,
    ctx: &mut C,
    type_id: u32,
    mode: ValueConstructionMode,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    ctx.charge_work(1)?;
    ctx.enter_type(type_id, mode.operation())?;
    let result = (|| {
        let entry = entry(types, type_id)?;
        // ARRAY dimensions are copied into the resulting ArrayValue. Keep that
        // real copy's admission at the old pre-recipe point; other TYPE_TABLE
        // vectors are only read and remain borrowed throughout construction.
        let array_dims = match &entry.data {
            TypeData::Array { dims, .. } => {
                let bytes = dims
                    .len()
                    .checked_mul(size_of::<(i64, i64)>())
                    .ok_or_else(|| RuntimeError::Overflow)?;
                ctx.charge_allocation(bytes)?;
                Some(dims.clone())
            }
            _ => None,
        };
        let input = match mode {
            ValueConstructionMode::Default if !intrinsic => {
                if let Some(recipe) = ctx.type_recipe(type_id) {
                    return ctx.evaluate_recipe(recipe);
                }
                None
            }
            ValueConstructionMode::Apply(Value::Struct(overrides)) if !intrinsic => {
                Some(apply_struct_recipe(ctx, type_id, overrides)?)
            }
            ValueConstructionMode::Apply(value) | ValueConstructionMode::Coerce(value) => {
                Some(value)
            }
            ValueConstructionMode::Default => None,
        };
        node(
            types,
            ctx,
            TypeNode {
                type_id,
                entry,
                array_dims,
            },
            input,
            intrinsic,
            depth,
        )
    })()
    .and_then(|value| {
        ctx.check_value(type_id, &value)?;
        Ok(value)
    });
    ctx.leave_type();
    result
}

// Recipe overlay maps belong only to aggregate Apply. Do not retain their
// temporaries in every scalar/POU construction across nested initializer calls.
#[inline(never)]
fn apply_struct_recipe(
    ctx: &mut impl ValueConstructionContext,
    type_id: u32,
    overrides: Arc<StructValue>,
) -> Result<Value, RuntimeError> {
    if let Some(recipe) = ctx.type_recipe(type_id) {
        let base = ctx.evaluate_recipe(recipe)?;
        ctx.check_value(type_id, &base)?;
        if let Value::Struct(base) = base {
            let name = base.type_name().clone();
            let physical_source = if overrides.physical_layout().is_some() {
                Some(Arc::clone(&overrides))
            } else if base.physical_layout().is_some() {
                Some(Arc::clone(&base))
            } else {
                None
            };
            let mut fields = owned_fields(ctx, base)?;
            let overrides = owned_fields(ctx, overrides)?;
            reserve_fields(ctx, overrides.len())?;
            for (name, value) in overrides {
                fields.insert(name, value);
            }
            let value = StructValue::from_canonical_parts(name, fields);
            let value = if let Some(source) = physical_source {
                ctx.bind_structure_layout_preserving(type_id, value, &source)?
            } else {
                ctx.bind_structure_layout(type_id, value)?
            };
            Ok(Value::Struct(Arc::new(value)))
        } else {
            Ok(Value::Struct(overrides))
        }
    } else {
        Ok(Value::Struct(overrides))
    }
}

struct TypeNode<'a> {
    type_id: u32,
    entry: &'a TypeEntry,
    array_dims: Option<Vec<(i64, i64)>>,
}

struct StructType<'a> {
    type_id: u32,
    name_idx: Option<u32>,
    fields: &'a [Field],
}

struct SubrangeType {
    base_type_id: u32,
    lower: i64,
    upper: i64,
}

fn node<C: ValueConstructionContext>(
    types: &TypeTable,
    ctx: &mut C,
    node: TypeNode<'_>,
    input: Option<Value>,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let TypeNode {
        type_id,
        entry,
        array_dims,
    } = node;
    // Aliases add no value; instances are charged exactly once at reservation.
    if !matches!(&entry.data, TypeData::Alias { .. } | TypeData::Pou { .. }) {
        ctx.charge_value()?;
    }
    match &entry.data {
        TypeData::Alias { target_type_id } => construct(
            types,
            ctx,
            *target_type_id,
            input.map_or(
                ValueConstructionMode::Default,
                ValueConstructionMode::Coerce,
            ),
            intrinsic,
            depth + 1,
        ),
        TypeData::Primitive {
            prim_id,
            max_length,
        } => {
            if let Some(value) = input {
                charge_leaf_copy(ctx, &value)?;
                scalar::coerce(*prim_id, *max_length, value)
            } else {
                scalar::default(*prim_id, ctx.profile())
            }
        }
        TypeData::Array { elem_type_id, .. } => {
            let dims = array_dims.ok_or_else(|| RuntimeError::TypeMismatch)?;
            construct_array(types, ctx, *elem_type_id, dims, input, intrinsic, depth)
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => construct_struct(
            types,
            ctx,
            StructType {
                type_id,
                name_idx: entry.name_idx,
                fields,
            },
            input,
            intrinsic,
            depth,
        ),
        TypeData::Subrange {
            base_type_id,
            lower,
            upper,
        } => construct_subrange(
            types,
            ctx,
            SubrangeType {
                base_type_id: *base_type_id,
                lower: *lower,
                upper: *upper,
            },
            input,
            intrinsic,
            depth,
        ),
        TypeData::Enum {
            base_type_id,
            variants,
        } => construct_enum(
            types,
            ctx,
            entry.name_idx,
            *base_type_id,
            variants,
            input,
            depth,
        ),
        TypeData::Reference { .. } => match input {
            None | Some(Value::Null) => Ok(Value::Reference(None)),
            Some(value @ Value::Reference(_)) => Ok(value),
            Some(_) => Err(RuntimeError::TypeMismatch),
        },
        TypeData::Pou { pou_id } => match input {
            None => ctx.construct_instance(*pou_id, intrinsic),
            Some(value @ Value::Instance(_)) => Ok(value),
            Some(_) => Err(RuntimeError::TypeMismatch),
        },
        TypeData::Interface { .. } => match input {
            None | Some(Value::Null) => Ok(Value::Null),
            Some(value @ Value::Instance(_)) => Ok(value),
            Some(_) => Err(RuntimeError::TypeMismatch),
        },
    }
}

// Each branch owns its recursive inputs and result validation. A common POU
// node must not carry these range/variant temporaries into instance initialization.
#[inline(never)]
fn construct_subrange(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    subrange: SubrangeType,
    input: Option<Value>,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let value = input.unwrap_or(Value::LInt(subrange.lower));
    let value = construct(
        types,
        ctx,
        subrange.base_type_id,
        ValueConstructionMode::Coerce(value),
        intrinsic,
        depth + 1,
    )?;
    let numeric = scalar::integer(&value).ok_or_else(|| RuntimeError::TypeMismatch)?;
    if numeric < i128::from(subrange.lower) || numeric > i128::from(subrange.upper) {
        return Err(RuntimeError::SubrangeViolation {
            value: numeric,
            lower: i128::from(subrange.lower),
            upper: i128::from(subrange.upper),
        });
    }
    Ok(value)
}

#[inline(never)]
fn construct_enum(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    name_idx: Option<u32>,
    base_type_id: u32,
    variants: &[crate::bytecode::EnumVariant],
    input: Option<Value>,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let name = name_idx
        .map(|idx| string(ctx, idx))
        .transpose()?
        .unwrap_or_default();
    let variant = match input {
        None => variants.first().ok_or_else(|| RuntimeError::TypeMismatch)?,
        Some(Value::Enum(value)) => {
            if !value.type_name().eq_ignore_ascii_case(&name) {
                return Err(RuntimeError::TypeMismatch);
            }
            variants
                .iter()
                .find(|v| {
                    v.value == value.numeric_value()
                        && ctx
                            .strings()
                            .get(v.name_idx as usize)
                            .is_some_and(|n| n.eq_ignore_ascii_case(value.variant_name()))
                })
                .ok_or_else(|| RuntimeError::TypeMismatch)?
        }
        Some(_) => return Err(RuntimeError::TypeMismatch),
    };
    // Named-value groups are aliases in the artifact. Ordinary enums keep
    // their closed name/value identity and must fit their declared base.
    construct(
        types,
        ctx,
        base_type_id,
        ValueConstructionMode::Coerce(Value::LInt(variant.value)),
        true,
        depth + 1,
    )?;
    ctx.charge_allocation(size_of::<EnumValue>())?;
    Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
        name,
        string(ctx, variant.name_idx)?,
        variant.value,
    ))))
}

// Keep aggregate containers out of the common node frame.
#[inline(never)]
fn construct_array(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    elem_type_id: u32,
    dims: Vec<(i64, i64)>,
    input: Option<Value>,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let count = array_len(&dims)?;
    reserve_values(ctx, count)?;
    let mut supplied = match input {
        None => Vec::new().into_iter(),
        Some(Value::Array(array)) => array.elements.into_iter(),
        Some(_) => return Err(RuntimeError::TypeMismatch),
    };
    let mut elements = Vec::new();
    elements
        .try_reserve_exact(count)
        .map_err(|_| RuntimeError::Overflow)?;
    for _ in 0..count {
        let mode = supplied.next().map_or(
            ValueConstructionMode::Default,
            ValueConstructionMode::Coerce,
        );
        elements.push(construct(
            types,
            ctx,
            elem_type_id,
            mode,
            intrinsic,
            depth + 1,
        )?);
    }
    Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
        elements, dims,
    ))))
}

#[inline(never)]
fn construct_struct(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    structure: StructType<'_>,
    input: Option<Value>,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let StructType {
        type_id,
        name_idx,
        fields,
    } = structure;
    let default_only = input.is_none();
    // Retain the source backing through recursive coercion. Keeping this Arc
    // makes owned_fields charge any field cloning before it takes place.
    let physical_source = match &input {
        Some(Value::Struct(value)) if value.physical_layout().is_some() => Some(Arc::clone(value)),
        _ => None,
    };
    let overrides = match input {
        None => OrderedMap::default(),
        Some(Value::Struct(value)) => owned_fields(ctx, value)?,
        Some(_) => return Err(RuntimeError::TypeMismatch),
    };
    reserve_fields(ctx, fields.len())?;
    let mut values = OrderedMap::default();
    for (index, field) in fields.iter().enumerate() {
        let name = string(ctx, field.name_idx)?;
        let value = if let Some(recipe) = (!intrinsic)
            .then(|| ctx.member_recipe(type_id, index as u32))
            .flatten()
        {
            let value = ctx.evaluate_recipe(recipe)?;
            ctx.check_value(field.type_id, &value)?;
            value
        } else {
            construct(
                types,
                ctx,
                field.type_id,
                ValueConstructionMode::Default,
                intrinsic,
                depth + 1,
            )?
        };
        values.insert(name, value);
    }
    // Defaults run first even for supplied fields. Overrides retain input
    // evaluation order, while replacements retain canonical member order.
    for (name, value) in overrides {
        let field = fields
            .iter()
            .find(|field| {
                ctx.strings()
                    .get(field.name_idx as usize)
                    .is_some_and(|s| s.eq_ignore_ascii_case(&name))
            })
            .ok_or_else(|| RuntimeError::TypeMismatch)?;
        let name = string(ctx, field.name_idx)?;
        let value = construct(
            types,
            ctx,
            field.type_id,
            ValueConstructionMode::Coerce(value),
            intrinsic,
            depth + 1,
        )?;
        values.insert(name, value);
    }
    let name = name_idx
        .map(|idx| string(ctx, idx))
        .transpose()?
        .unwrap_or_default();
    let value = StructValue::from_canonical_parts(name, values);
    let value = if default_only {
        ctx.initialize_structure_layout(type_id, value)?
    } else if let Some(source) = physical_source {
        ctx.bind_structure_layout_preserving(type_id, value, &source)?
    } else {
        ctx.bind_structure_layout(type_id, value)?
    };
    Ok(Value::Struct(Arc::new(value)))
}

// Resolve the immutable metadata before entering a recipe or nested type. The
// borrow comes from PreparedModule, not the mutable callback context.
#[inline(never)]
fn entry(types: &TypeTable, type_id: u32) -> Result<&TypeEntry, RuntimeError> {
    types
        .entries
        .get(type_id as usize)
        .ok_or_else(|| RuntimeError::TypeMismatch)
}

fn string(ctx: &mut impl ValueConstructionContext, idx: u32) -> Result<SmolStr, RuntimeError> {
    let len = ctx
        .strings()
        .get(idx as usize)
        .ok_or_else(|| RuntimeError::TypeMismatch)?
        .len();
    ctx.charge_allocation(len)?;
    ctx.strings()
        .get(idx as usize)
        .cloned()
        .ok_or_else(|| RuntimeError::TypeMismatch)
}
fn array_len(dims: &[(i64, i64)]) -> Result<usize, RuntimeError> {
    dims.iter().try_fold(1usize, |n, (lo, hi)| {
        if hi < lo {
            return Err(RuntimeError::TypeMismatch);
        }
        let width = usize::try_from(i128::from(*hi) - i128::from(*lo) + 1)
            .map_err(|_| RuntimeError::Overflow)?;
        n.checked_mul(width).ok_or_else(|| RuntimeError::Overflow)
    })
}
fn reserve_values(
    ctx: &mut impl ValueConstructionContext,
    count: usize,
) -> Result<(), RuntimeError> {
    let bytes = count
        .checked_mul(size_of::<Value>())
        .and_then(|n| n.checked_add(size_of::<ArrayValue>()))
        .ok_or_else(|| RuntimeError::Overflow)?;
    ctx.charge_allocation(bytes)
}
fn reserve_fields(
    ctx: &mut impl ValueConstructionContext,
    count: usize,
) -> Result<(), RuntimeError> {
    // Include a conservative hash-index/capacity allowance as well as value slots.
    let bytes = count
        .checked_mul(2 * (size_of::<Value>() + size_of::<SmolStr>() + 4 * size_of::<usize>()))
        .and_then(|n| n.checked_add(size_of::<StructValue>() + 2 * size_of::<usize>()))
        .ok_or_else(|| RuntimeError::Overflow)?;
    ctx.charge_allocation(bytes)
}
fn owned_fields(
    ctx: &mut impl ValueConstructionContext,
    value: Arc<StructValue>,
) -> Result<OrderedMap<SmolStr, Value>, RuntimeError> {
    match Arc::try_unwrap(value) {
        Ok(value) => Ok(value.fields),
        Err(value) => {
            reserve_fields(ctx, value.fields().len())?;
            let mut fields = OrderedMap::default();
            for (name, value) in value.fields() {
                charge_clone(ctx, value, 0)?;
                fields.insert(name.clone(), value.clone());
            }
            Ok(fields)
        }
    }
}
// Detaching a shared structure copies its fields into the constructed value: each
// copied node is one CONSTRUCT unit and text costs per character (TIME-08). Each
// copied node also charges its allocation bytes.
fn charge_clone(
    ctx: &mut impl ValueConstructionContext,
    value: &Value,
    depth: usize,
) -> Result<(), RuntimeError> {
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    // TIME-08 CONSTRUCT: every node copied into the constructed value is one unit.
    ctx.charge_work(1)?;
    match value {
        Value::Array(array) => {
            reserve_values(ctx, array.elements().len())?;
            ctx.charge_allocation(
                array
                    .dimensions()
                    .len()
                    .checked_mul(size_of::<(i64, i64)>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            for value in array.elements() {
                charge_clone(ctx, value, depth + 1)?;
            }
        }
        Value::Enum(_) => ctx.charge_allocation(size_of::<EnumValue>())?,
        Value::Reference(Some(reference)) => {
            ctx.charge_allocation(
                reference
                    .path
                    .len()
                    .checked_mul(size_of::<crate::value::RefSegment>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            for segment in &reference.path {
                if let crate::value::RefSegment::Index(indices) = segment {
                    ctx.charge_allocation(
                        indices
                            .len()
                            .checked_mul(size_of::<i64>())
                            .ok_or_else(|| RuntimeError::Overflow)?,
                    )?;
                }
            }
        }
        _ => charge_leaf_copy(ctx, value)?,
    }
    Ok(())
}
fn charge_leaf_copy(
    ctx: &mut impl ValueConstructionContext,
    value: &Value,
) -> Result<(), RuntimeError> {
    let len = match value {
        Value::String(s) => s.len(),
        Value::WString(s) => s.len(),
        Value::Char(_) => 4,
        Value::WChar(_) => 4,
        _ => 0,
    };
    // TIME-08 TEXT: string conversion costs per character.
    ctx.charge_work(len)?;
    // String conversion and optional truncation may each materialize a buffer.
    ctx.charge_allocation(len.checked_mul(2).ok_or_else(|| RuntimeError::Overflow)?)
}

#[cfg(test)]
mod text_work_coercion_tests;
