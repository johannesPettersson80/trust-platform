//! One TYPE_TABLE-driven traversal for default, coercion and explicit initialization.
use crate::bytecode::{Field, TypeData, TypeEntry, TypeTable};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::layout::ByteRange;
use crate::value::{ArrayValue, DateTimeProfile, EnumValue, StructValue, Value};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::mem::size_of;
use smol_str::SmolStr;

pub(crate) mod scalar;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueOperation {
    Default,
    Coerce,
    Apply,
}

/// Where a construction puts the value it builds (C1-13).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Target {
    /// A `Value`, which the construction returns.
    Value,
    /// The bytes of a value of the constructed type on the flat store: each leaf is written where it
    /// lies and no container is built, so the construction allocates nothing, and it returns
    /// `Value::Null`. It charges and checks what building the value does, in the same order
    /// (TIME-08), since both run this one traversal.
    Bytes(ByteRange),
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
    fn strings(&self) -> &crate::vm::Names;
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
    /// Take back the container of an array whose elements a construction consumed, for the next
    /// raw array an initializer body builds (spec 34 ARCH-05; C1-13). By default it is dropped.
    fn recycle_array(&mut self, array: Box<ArrayValue>) {
        drop(array);
    }
    /// Take back the container of a structure whose fields a construction consumed, as
    /// [`ValueConstructionContext::recycle_array`] takes back an array's.
    fn recycle_struct(&mut self, structure: Arc<StructValue>) {
        drop(structure);
    }
    /// Write `value`, of the `TYPE_TABLE` type `type_id`, into its bytes `at` on the flat store, as a
    /// construction in place writes each leaf it builds (C1-13). Only the engine's flat store has
    /// such bytes; any other context refuses.
    fn write_in_place(
        &mut self,
        _at: ByteRange,
        _type_id: u32,
        _value: &Value,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
    /// Write the enumeration value `numeric` of the type `type_id` into its leaf `at` (C1-13).
    fn write_enum_in_place(
        &mut self,
        _at: ByteRange,
        _type_id: u32,
        _numeric: i64,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
    /// The bytes of element `index` of the array of `type_id` whose bytes are `at` (C1-13).
    fn element_in_place(
        &self,
        _at: ByteRange,
        _type_id: u32,
        _index: usize,
    ) -> Result<ByteRange, RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
    /// The bytes of member `ordinal` of the structure of `type_id` whose bytes are `at` (C1-13).
    fn member_in_place(
        &self,
        _at: ByteRange,
        _type_id: u32,
        _ordinal: usize,
    ) -> Result<ByteRange, RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
    /// The checks [`ValueConstructionContext::check_value`] makes of the value of `type_id` a
    /// construction in place wrote at `at`, charged as it charges them for that value (C1-13).
    fn check_in_place(&mut self, _at: ByteRange, _type_id: u32) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
}

pub fn construct_value(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    type_id: u32,
    mode: ValueConstructionMode,
) -> Result<Value, RuntimeError> {
    construct(types, ctx, Target::Value, type_id, mode, false, 0)
}

/// Return slots and intrinsic-only action stages share traversal but skip recipes.
pub fn construct_intrinsic_value(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    type_id: u32,
) -> Result<Value, RuntimeError> {
    construct(
        types,
        ctx,
        Target::Value,
        type_id,
        ValueConstructionMode::Default,
        true,
        0,
    )
}

/// Construct the value of `type_id` into its bytes `at` on the flat store (C1-13): what
/// [`construct_value`], or with `intrinsic` [`construct_intrinsic_value`], builds, charged and
/// checked as they charge and check it, written where it lies.
pub(crate) fn construct_in_place(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    at: ByteRange,
    type_id: u32,
    mode: ValueConstructionMode,
    intrinsic: bool,
) -> Result<(), RuntimeError> {
    construct(types, ctx, Target::Bytes(at), type_id, mode, intrinsic, 0).map(drop)
}

fn construct<C: ValueConstructionContext>(
    types: &TypeTable,
    ctx: &mut C,
    target: Target,
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
                // In place, the layout holds the dimensions, so none are copied.
                Some(match target {
                    Target::Value => dims.clone(),
                    Target::Bytes(_) => Vec::new(),
                })
            }
            _ => None,
        };
        let input = match mode {
            ValueConstructionMode::Default if !intrinsic => {
                if let Some(recipe) = ctx.type_recipe(type_id) {
                    let value = ctx.evaluate_recipe(recipe)?;
                    return emit(ctx, target, type_id, value);
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
            target,
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
        match target {
            Target::Value => ctx.check_value(type_id, &value)?,
            Target::Bytes(at) => ctx.check_in_place(at, type_id)?,
        }
        Ok(value)
    });
    ctx.leave_type();
    result
}

/// The node `value` of `type_id` built for `target`: returned, or written into its bytes.
fn emit(
    ctx: &mut impl ValueConstructionContext,
    target: Target,
    type_id: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    match target {
        Target::Value => Ok(value),
        Target::Bytes(at) => {
            ctx.write_in_place(at, type_id, &value)?;
            Ok(Value::Null)
        }
    }
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

struct ArrayType<'a> {
    type_id: u32,
    elem_type_id: u32,
    dims: &'a [(i64, i64)],
}

struct StructType<'a> {
    type_id: u32,
    name_idx: Option<u32>,
    fields: &'a [Field],
}

struct SubrangeType {
    type_id: u32,
    base_type_id: u32,
    lower: i64,
    upper: i64,
}

fn node<C: ValueConstructionContext>(
    types: &TypeTable,
    ctx: &mut C,
    target: Target,
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
            target,
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
            let value = if let Some(value) = input {
                charge_leaf_copy(ctx, &value)?;
                scalar::coerce(*prim_id, *max_length, value)?
            } else {
                scalar::default(*prim_id, ctx.profile())?
            };
            emit(ctx, target, type_id, value)
        }
        TypeData::Array { elem_type_id, dims } => {
            let owned_dims = array_dims.ok_or_else(|| RuntimeError::TypeMismatch)?;
            construct_array(
                types,
                ctx,
                target,
                ArrayType {
                    type_id,
                    elem_type_id: *elem_type_id,
                    dims,
                },
                owned_dims,
                input,
                intrinsic,
                depth,
            )
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => construct_struct(
            types,
            ctx,
            target,
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
            target,
            SubrangeType {
                type_id,
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
            target,
            type_id,
            entry.name_idx,
            *base_type_id,
            variants,
            input,
            depth,
        ),
        TypeData::Reference { .. } => {
            let value = match input {
                None | Some(Value::Null) => Value::Reference(None),
                Some(value @ Value::Reference(_)) => value,
                Some(_) => return Err(RuntimeError::TypeMismatch),
            };
            emit(ctx, target, type_id, value)
        }
        TypeData::Pou { pou_id } => {
            let value = match input {
                None => ctx.construct_instance(*pou_id, intrinsic)?,
                Some(value @ Value::Instance(_)) => value,
                Some(_) => return Err(RuntimeError::TypeMismatch),
            };
            emit(ctx, target, type_id, value)
        }
        TypeData::Interface { .. } => {
            let value = match input {
                None | Some(Value::Null) => Value::Null,
                Some(value @ Value::Instance(_)) => value,
                Some(_) => return Err(RuntimeError::TypeMismatch),
            };
            emit(ctx, target, type_id, value)
        }
    }
}

// Each branch owns its recursive inputs and result validation. A common POU
// node must not carry these range/variant temporaries into instance initialization.
#[inline(never)]
fn construct_subrange(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    target: Target,
    subrange: SubrangeType,
    input: Option<Value>,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let value = input.unwrap_or(Value::LInt(subrange.lower));
    // The base is a scalar, built as a value even in place, so that its range can be checked.
    let value = construct(
        types,
        ctx,
        Target::Value,
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
    emit(ctx, target, subrange.type_id, value)
}

#[inline(never)]
#[allow(
    clippy::too_many_arguments,
    reason = "the enumeration's type, name, base and variants are read once by node"
)]
fn construct_enum(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    target: Target,
    type_id: u32,
    name_idx: Option<u32>,
    base_type_id: u32,
    variants: &[crate::bytecode::EnumVariant],
    input: Option<Value>,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let name = name_idx
        .map(|idx| name_of(ctx, target, idx))
        .transpose()?
        .unwrap_or_default();
    let variant = match input {
        None => variants.first().ok_or_else(|| RuntimeError::TypeMismatch)?,
        Some(Value::Enum(value)) => {
            // The type's name, which `name` holds only when a value is built.
            let type_name = name_idx
                .and_then(|idx| ctx.strings().get(idx as usize))
                .unwrap_or_default();
            if !value.type_name().eq_ignore_ascii_case(type_name) {
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
        Target::Value,
        base_type_id,
        ValueConstructionMode::Coerce(Value::LInt(variant.value)),
        true,
        depth + 1,
    )?;
    ctx.charge_allocation(size_of::<EnumValue>())?;
    let variant_name = name_of(ctx, target, variant.name_idx)?;
    match target {
        Target::Value => Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
            name,
            variant_name,
            variant.value,
        )))),
        Target::Bytes(at) => {
            ctx.write_enum_in_place(at, type_id, variant.value)?;
            Ok(Value::Null)
        }
    }
}

// Keep aggregate containers out of the common node frame.
#[inline(never)]
#[allow(
    clippy::too_many_arguments,
    reason = "the array's type and its owned dimensions come from node"
)]
fn construct_array(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    target: Target,
    array: ArrayType<'_>,
    owned_dims: Vec<(i64, i64)>,
    input: Option<Value>,
    intrinsic: bool,
    depth: usize,
) -> Result<Value, RuntimeError> {
    let count = array_len(array.dims)?;
    reserve_values(ctx, count)?;
    let mut supplied = match input {
        None => None,
        Some(Value::Array(array)) => Some(array),
        Some(_) => return Err(RuntimeError::TypeMismatch),
    };
    // In place, each element is written where it lies, so no element list is built.
    let mut elements = Vec::new();
    if let Target::Value = target {
        elements
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
    }
    {
        // The supplied elements leave their container in order, so that it can be kept (C1-13).
        let mut drained = supplied.as_mut().map(|array| array.elements.drain(..));
        for index in 0..count {
            let mode = drained.as_mut().and_then(Iterator::next).map_or(
                ValueConstructionMode::Default,
                ValueConstructionMode::Coerce,
            );
            let element = match target {
                Target::Value => Target::Value,
                Target::Bytes(at) => {
                    Target::Bytes(ctx.element_in_place(at, array.type_id, index)?)
                }
            };
            let value = construct(
                types,
                ctx,
                element,
                array.elem_type_id,
                mode,
                intrinsic,
                depth + 1,
            )?;
            if let Target::Value = target {
                elements.push(value);
            }
        }
    }
    if let Some(array) = supplied {
        ctx.recycle_array(array);
    }
    match target {
        Target::Value => Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
            elements, owned_dims,
        )))),
        Target::Bytes(_) => Ok(Value::Null),
    }
}

#[inline(never)]
fn construct_struct(
    types: &TypeTable,
    ctx: &mut impl ValueConstructionContext,
    target: Target,
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
    let mut supplied = match input {
        None => None,
        Some(Value::Struct(value)) => Some(value),
        Some(_) => return Err(RuntimeError::TypeMismatch),
    };
    // A supplied structure that something else holds has its fields copied, charged; one that
    // nothing else holds gives them up, so that its container can be kept (C1-13).
    let shared = supplied
        .as_mut()
        .is_some_and(|value| Arc::get_mut(value).is_none());
    let copied = match &supplied {
        Some(value) if shared => Some(copied_fields(ctx, value)?),
        _ => None,
    };
    reserve_fields(ctx, fields.len())?;
    // In place, each member is written where it lies, so no field map is built.
    let mut values = OrderedMap::default();
    for (index, field) in fields.iter().enumerate() {
        let name = name_of(ctx, target, field.name_idx)?;
        let value = if let Some(recipe) = (!intrinsic)
            .then(|| ctx.member_recipe(type_id, index as u32))
            .flatten()
        {
            let value = ctx.evaluate_recipe(recipe)?;
            ctx.check_value(field.type_id, &value)?;
            let at = member_target(ctx, target, type_id, index)?;
            emit(ctx, at, field.type_id, value)?
        } else {
            let at = member_target(ctx, target, type_id, index)?;
            construct(
                types,
                ctx,
                at,
                field.type_id,
                ValueConstructionMode::Default,
                intrinsic,
                depth + 1,
            )?
        };
        if let Target::Value = target {
            values.insert(name, value);
        }
    }
    // Defaults run first even for supplied fields. Overrides retain input
    // evaluation order, while replacements retain canonical member order.
    let drained = supplied
        .as_mut()
        .and_then(Arc::get_mut)
        .map(|value| value.fields.drain(..));
    let overrides = copied
        .into_iter()
        .flatten()
        .chain(drained.into_iter().flatten());
    for (name, value) in overrides {
        let index = fields
            .iter()
            .position(|field| {
                ctx.strings()
                    .get(field.name_idx as usize)
                    .is_some_and(|s| s.eq_ignore_ascii_case(&name))
            })
            .ok_or_else(|| RuntimeError::TypeMismatch)?;
        let field = &fields[index];
        let name = name_of(ctx, target, field.name_idx)?;
        let at = member_target(ctx, target, type_id, index)?;
        let value = construct(
            types,
            ctx,
            at,
            field.type_id,
            ValueConstructionMode::Coerce(value),
            intrinsic,
            depth + 1,
        )?;
        if let Target::Value = target {
            values.insert(name, value);
        }
    }
    if let Some(structure) = supplied {
        ctx.recycle_struct(structure);
    }
    let name = name_idx
        .map(|idx| name_of(ctx, target, idx))
        .transpose()?
        .unwrap_or_default();
    // A structure in place has no physical layout (a plain type, C1-15), so binding one would
    // charge nothing.
    if let Target::Bytes(_) = target {
        return Ok(Value::Null);
    }
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

/// Where member `index` of a structure of `type_id` built for `target` goes.
fn member_target(
    ctx: &impl ValueConstructionContext,
    target: Target,
    type_id: u32,
    index: usize,
) -> Result<Target, RuntimeError> {
    match target {
        Target::Value => Ok(Target::Value),
        Target::Bytes(at) => ctx.member_in_place(at, type_id, index).map(Target::Bytes),
    }
}

/// The name at `idx`, charged as [`string`] charges it, built only when a value is built.
fn name_of(
    ctx: &mut impl ValueConstructionContext,
    target: Target,
    idx: u32,
) -> Result<SmolStr, RuntimeError> {
    match target {
        Target::Value => string(ctx, idx),
        Target::Bytes(_) => {
            let len = ctx
                .strings()
                .get(idx as usize)
                .ok_or_else(|| RuntimeError::TypeMismatch)?
                .len();
            ctx.charge_allocation(len)?;
            Ok(SmolStr::default())
        }
    }
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
        .map(SmolStr::new)
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
        Err(value) => copied_fields(ctx, &value),
    }
}
/// A copy of the fields of a structure that something else holds, its room and each field's copy
/// charged first.
fn copied_fields(
    ctx: &mut impl ValueConstructionContext,
    value: &StructValue,
) -> Result<OrderedMap<SmolStr, Value>, RuntimeError> {
    reserve_fields(ctx, value.fields().len())?;
    let mut fields = OrderedMap::default();
    for (name, value) in value.fields() {
        charge_clone(ctx, value, 0)?;
        fields.insert(name.clone(), value.clone());
    }
    Ok(fields)
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
