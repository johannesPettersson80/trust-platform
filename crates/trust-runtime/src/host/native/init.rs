//! The starting memory of a compiled program (specification 44 §6.7, §7.4): every variable of the
//! globals, retain and instance areas at its initial value, as bytes at its offset.
//!
//! The build works the values out from the declarations (IEC §6.5.1, Table 14), with the runtime's
//! evaluator of initial values on the engineering computer and no program constructed or run: a
//! variable's initial value, else its type's default; an instance's variables the same way from
//! its type's declarations, its base's first, an initial value given to the instance's variable
//! (`(PT := T#1s)`) in place of a declared one, and its class id. The constants are worked out
//! first, in declaration order, so that an initial value may name one.

use smol_str::SmolStr;
use trust_hir::TypeId;

use super::model::{AreaLayout, Model};
use super::types::{Elem, Kind, Ty};
use super::NativeError;
use crate::harness::LoweredApplication;
use crate::memory::VariableStorage;
use crate::program_model::Expr;
use crate::value::Value;
use trust_hir::symbols::ParamDirection;

/// The starting bytes of the marker, globals, retain and instance areas.
pub(super) struct StartingMemory {
    pub(super) markers: Vec<u8>,
    pub(super) globals: Vec<u8>,
    pub(super) retain: Vec<u8>,
    pub(super) instances: Vec<u8>,
}

/// Computes the starting memory of `model`, whose declarations `lowered` holds.
///
/// # Errors
///
/// [`NativeError`] when an initial value cannot be worked out at build, such as one that names a
/// variable that is no constant (NX-09), or a value does not fit its place.
pub(super) fn starting_memory(
    lowered: &LoweredApplication,
    model: &mut Model<'_>,
) -> Result<StartingMemory, NativeError> {
    let declared: Vec<_> = lowered
        .globals
        .iter()
        .chain(
            lowered
                .configuration
                .iter()
                .flat_map(|configuration| configuration.globals.iter()),
        )
        .collect();
    let mut start = Start {
        constants: VariableStorage::new(),
    };
    // The constants first, so that the initial values after them may name them.
    for global in declared.iter().filter(|global| global.constant) {
        let value = start.evaluate(
            model,
            global.type_id,
            global.initializer.as_ref(),
            &global.name,
        )?;
        start.constants.set_global(global.name.clone(), value);
    }
    let mut globals = vec![0u8; model.area_g.size as usize];
    let mut retain = vec![0u8; model.area_r.size as usize];
    let mut instances = vec![0u8; model.area_p.size as usize];
    let fields: Vec<(SmolStr, Ty, bool)> = model
        .globals
        .iter()
        .map(|global| (global.name.clone(), global.ty, global.retained))
        .collect();
    for (name, ty, retained) in fields {
        let Some(global) = declared
            .iter()
            .find(|global| global.name.eq_ignore_ascii_case(&name))
        else {
            continue;
        };
        let (offset, bytes) = if retained {
            (field_offset(&model.area_r, &name)?, &mut retain)
        } else {
            (field_offset(&model.area_g, &name)?, &mut globals)
        };
        let initializer = global.initializer.as_ref();
        start.write(
            model,
            bytes,
            offset as usize,
            ty,
            global.type_id,
            initializer,
            &name,
        )?;
    }
    let programs: Vec<(SmolStr, usize)> = model
        .instances
        .iter()
        .map(|instance| (instance.name.clone(), instance.program))
        .collect();
    for (name, program) in programs {
        let p_offset = field_offset(&model.area_p, &name)? as usize;
        let r_offset = field_offset(&model.area_r, &name).map_or(0, |offset| offset as usize);
        let variables: Vec<(SmolStr, TypeId, Option<Expr>)> = model.program_types[program]
            .def
            .vars
            .iter()
            .filter(|var| !var.external)
            .map(|var| (var.name.clone(), var.type_id, var.initializer.clone()))
            .collect();
        for (variable, type_id, initializer) in variables {
            let Some((member, in_retain)) = model.program_types[program].member(&variable) else {
                continue;
            };
            let (place, ty) = (member.offset as usize, member.ty);
            let (bytes, offset) = if in_retain {
                (&mut retain, r_offset + place)
            } else {
                (&mut instances, p_offset + place)
            };
            let path = format!("{name}.{variable}");
            start.write(
                model,
                bytes,
                offset,
                ty,
                type_id,
                initializer.as_ref(),
                &path,
            )?;
        }
    }
    let markers = vec![0u8; model.images[2] as usize];
    Ok(StartingMemory {
        markers,
        globals,
        retain,
        instances,
    })
}

fn field_offset(layout: &AreaLayout, name: &str) -> Result<u32, NativeError> {
    layout
        .field(name)
        .map(|field| field.offset)
        .ok_or_else(|| NativeError::Compile(format!("{name} has no place in its area")))
}

/// The values the build has worked out: the constants an initial value may name.
struct Start {
    constants: VariableStorage,
}

impl Start {
    /// The value a variable of trust-hir's type `type_id`, named `name`, starts at: its initial
    /// value `initializer`, else its type's default.
    fn evaluate(
        &self,
        model: &Model<'_>,
        type_id: TypeId,
        initializer: Option<&Expr>,
        name: &str,
    ) -> Result<Value, NativeError> {
        let builder = model.builder;
        let abi = crate::value::StorageAbi::default();
        match initializer {
            Some(expr) => crate::harness::initializer::evaluate_initializer_with_abi(
                &self.constants,
                builder.registry(),
                abi,
                builder.initializer_catalog(),
                &builder.profile(),
                None,
                builder.stdlib(),
                expr,
                type_id,
            ),
            None => crate::harness::initializer::default_value_for_type_id_with_abi(
                &self.constants,
                builder.registry(),
                abi,
                builder.initializer_catalog(),
                &builder.profile(),
                None,
                builder.stdlib(),
                type_id,
            ),
        }
        .map_err(|error| NativeError::Compile(format!("the starting value of {name}: {error}")))
    }

    /// Writes the starting value of the variable `name`, of the program's type `ty` and trust-hir's
    /// type `type_id`, at `offset` of `bytes`.
    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        model: &mut Model<'_>,
        bytes: &mut [u8],
        offset: usize,
        ty: Ty,
        type_id: TypeId,
        initializer: Option<&Expr>,
        name: &str,
    ) -> Result<(), NativeError> {
        let kind = model.types.info(ty).kind.clone();
        match kind {
            // A reference starts `NULL`; a `REF(variable)` initializer is set by the cold-start and
            // the warm-start initialization (`entries::reference_initializers`).
            Kind::Ref { .. } => Ok(()),
            // An interface names no instance until one is assigned.
            Kind::Interface => match initializer {
                None | Some(Expr::Literal(Value::Null | Value::Reference(None))) => Ok(()),
                Some(_) => Err(NativeError::unsupported(format!(
                    "the initial value of the interface {name}"
                ))),
            },
            Kind::Fb { .. } => self.instance(model, bytes, offset, ty, initializer, name),
            Kind::StdFb(_) => self.standard_instance(model, bytes, offset, ty, initializer, name),
            Kind::Array { elem, dims } if model.types.info(elem).kind.is_instance() => {
                if initializer.is_some() {
                    return Err(NativeError::unsupported(format!(
                        "the initial value of the array of instances {name}"
                    )));
                }
                let size = model.types.info(elem).size as usize;
                let count: usize = dims
                    .iter()
                    .map(|(lower, upper)| usize::try_from(upper - lower + 1).unwrap_or(0))
                    .product();
                for index in 0..count {
                    let element = format!("{name}[{index}]");
                    self.write(
                        model,
                        bytes,
                        offset + index * size,
                        elem,
                        type_id,
                        None,
                        &element,
                    )?;
                }
                Ok(())
            }
            _ if holds_instance(model, ty) => Err(NativeError::unsupported(format!(
                "the starting value of {name}, a structure that holds an instance"
            ))),
            _ => {
                let value = self.evaluate(model, type_id, initializer, name)?;
                encode(model, &value, ty, bytes, offset)
            }
        }
    }

    /// Writes the variables of the instance of a user function block or class at `offset`: its
    /// base's, then its class id in the first field of its hierarchy's root, which lies first
    /// (specification 44 §6.4), then its own; a `VAR_IN_OUT` starts `NULL`.
    fn instance(
        &self,
        model: &mut Model<'_>,
        bytes: &mut [u8],
        offset: usize,
        ty: Ty,
        initializer: Option<&Expr>,
        name: &str,
    ) -> Result<(), NativeError> {
        let Kind::Fb {
            def,
            members,
            base,
            class,
            ..
        } = model.types.info(ty).kind.clone()
        else {
            return Ok(());
        };
        let given = given_values(initializer, name)?;
        if let Some(base) = base {
            self.instance(model, bytes, offset, base, None, name)?;
        }
        if class != 0 {
            put(bytes, offset, &class.to_le_bytes())?;
        }
        let owner = model
            .types
            .owner(&def)
            .ok_or_else(|| NativeError::Compile(format!("{def} has no declaration")))?;
        let declared: Vec<(SmolStr, TypeId, Option<Expr>)> = owner
            .params()
            .iter()
            .filter(|param| param.direction != ParamDirection::InOut)
            .map(|param| (param.name.clone(), param.type_id, param.default.clone()))
            .chain(
                owner
                    .vars()
                    .iter()
                    .filter(|var| !var.external && !var.in_out)
                    .map(|var| (var.name.clone(), var.type_id, var.initializer.clone())),
            )
            .collect();
        for (variable, type_id, initializer) in declared {
            // A variable the instance does not hold (a `VAR_TEMP`) lies in a frame.
            let Some(member) = members.iter().find(|member| {
                member.via.is_empty() && member.name.eq_ignore_ascii_case(&variable)
            }) else {
                continue;
            };
            let (place, member_ty) = (member.offset as usize, member.ty);
            let value = given
                .iter()
                .find(|(given, _)| given.eq_ignore_ascii_case(&variable))
                .map(|(_, value)| value)
                .or(initializer.as_ref());
            let path = format!("{name}.{variable}");
            self.write(
                model,
                bytes,
                offset + place,
                member_ty,
                type_id,
                value,
                &path,
            )?;
        }
        Ok(())
    }

    /// Writes the variables of a standard function block's instance at `offset`: zero, which is
    /// each input's and output's default and the block's own starting state, and the values its
    /// variable's initial value gives its inputs (`(PT := T#1s)`).
    fn standard_instance(
        &self,
        model: &mut Model<'_>,
        bytes: &mut [u8],
        offset: usize,
        ty: Ty,
        initializer: Option<&Expr>,
        name: &str,
    ) -> Result<(), NativeError> {
        let Kind::StdFb(fb) = model.types.info(ty).kind.clone() else {
            return Ok(());
        };
        let given = given_values(initializer, name)?;
        if given.is_empty() {
            return Ok(());
        }
        let members = model.types.std_fb_members(fb);
        let builder = model.builder;
        for (variable, expr) in given {
            let Some(member) = members.iter().find(|member| {
                !member.name.starts_with("__") && member.name.eq_ignore_ascii_case(&variable)
            }) else {
                return Err(NativeError::Compile(format!(
                    "{name} has no variable {variable}"
                )));
            };
            let value = crate::helper_eval::eval_storage_expr_with_abi(
                &self.constants,
                builder.registry(),
                crate::value::StorageAbi::default(),
                &builder.profile(),
                None,
                Some(builder.stdlib()),
                &expr,
            )
            .map_err(|error| {
                NativeError::Compile(format!("the starting value of {name}.{variable}: {error}"))
            })?;
            encode(
                model,
                &value,
                member.ty,
                bytes,
                offset + member.offset as usize,
            )?;
        }
        Ok(())
    }
}

/// The values an instance's variable's initial value gives the instance's variables, by name.
fn given_values(
    initializer: Option<&Expr>,
    name: &str,
) -> Result<Vec<(SmolStr, Expr)>, NativeError> {
    match initializer {
        None => Ok(Vec::new()),
        Some(Expr::StructInitializer(fields)) => Ok(fields.clone()),
        Some(_) => Err(NativeError::unsupported(format!(
            "the initial value of the instance {name}"
        ))),
    }
}

/// Whether a value of `ty` holds an instance of a function block or a class.
fn holds_instance(model: &Model<'_>, ty: Ty) -> bool {
    match &model.types.info(ty).kind {
        Kind::Fb { .. } | Kind::StdFb(_) => true,
        Kind::Array { elem, .. } => holds_instance(model, *elem),
        Kind::Struct { members } => members
            .iter()
            .any(|member| holds_instance(model, member.ty)),
        _ => false,
    }
}

fn put(bytes: &mut [u8], offset: usize, data: &[u8]) -> Result<(), NativeError> {
    bytes
        .get_mut(offset..offset + data.len())
        .ok_or_else(|| NativeError::Compile("a starting value outside its area".into()))?
        .copy_from_slice(data);
    Ok(())
}

/// The bytes, at the program's layout, of `value` of type `ty`: a frame variable's starting value
/// (specification 44 §6.7).
///
/// # Errors
///
/// [`NativeError`] when the value does not fit its type.
pub(super) fn value_bytes(
    model: &mut Model<'_>,
    value: &Value,
    ty: Ty,
) -> Result<Vec<u8>, NativeError> {
    let mut bytes = vec![0u8; model.types.info(ty).size as usize];
    encode(model, value, ty, &mut bytes, 0)?;
    Ok(bytes)
}

/// Writes `value`, which holds no instance, of type `ty` at `offset`.
fn encode(
    model: &Model<'_>,
    value: &Value,
    ty: Ty,
    bytes: &mut [u8],
    offset: usize,
) -> Result<(), NativeError> {
    let info = model.types.info(ty).clone();
    match (&info.kind, value) {
        (_, Value::Null) => Ok(()),
        // A reference and an interface start `NULL` (see `Start::write`).
        (Kind::Ref { .. } | Kind::Interface, Value::Reference(None)) => Ok(()),
        (
            Kind::Elem(elem) | Kind::Enum { base: elem, .. } | Kind::Subrange { base: elem, .. },
            value,
        ) => {
            let data = scalar(*elem, value).ok_or_else(|| {
                NativeError::Compile(format!("a starting value {value:?} of {}", elem.name()))
            })?;
            put(bytes, offset, &data[..elem.size() as usize])
        }
        (Kind::Str(capacity), Value::String(text)) => {
            let mut end = text.len().min(*capacity as usize);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let length = u16::try_from(end).unwrap_or(u16::MAX);
            put(bytes, offset, &length.to_le_bytes())?;
            put(bytes, offset + 2, &text.as_bytes()[..end])
        }
        (Kind::WStr(capacity), Value::WString(text)) => {
            // At most `capacity` UTF-16 code units, never half of a pair (specification 10
            // §2.3.1).
            let end =
                trust_runtime_core::value::string_capacity_cut(text, true, *capacity as usize)
                    .unwrap_or(text.len());
            let units: Vec<u8> = text[..end]
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            let length = u16::try_from(units.len() / 2).unwrap_or(u16::MAX);
            put(bytes, offset, &length.to_le_bytes())?;
            put(bytes, offset + 2, &units)
        }
        (Kind::Array { elem, .. }, Value::Array(array)) => {
            let size = model.types.info(*elem).size as usize;
            for (index, element) in array.elements().iter().enumerate() {
                encode(model, element, *elem, bytes, offset + index * size)?;
            }
            Ok(())
        }
        (Kind::Struct { members }, Value::Struct(structure)) => {
            for member in members {
                if let Some(field) = structure.field(&member.name) {
                    encode(
                        model,
                        field,
                        member.ty,
                        bytes,
                        offset + member.offset as usize,
                    )?;
                }
            }
            Ok(())
        }
        (kind, value) => Err(NativeError::Compile(format!(
            "a starting value {value:?} for a {kind:?}"
        ))),
    }
}

/// The little-endian bytes of a scalar's value, eight long.
fn scalar(elem: Elem, value: &Value) -> Option<[u8; 8]> {
    let wide: i128 = match value {
        Value::Bool(value) => i128::from(*value),
        Value::SInt(value) => i128::from(*value),
        Value::Int(value) => i128::from(*value),
        Value::DInt(value) => i128::from(*value),
        Value::LInt(value) => i128::from(*value),
        Value::USInt(value) | Value::Byte(value) | Value::Char(value) => i128::from(*value),
        Value::UInt(value) | Value::Word(value) | Value::WChar(value) => i128::from(*value),
        Value::UDInt(value) | Value::DWord(value) => i128::from(*value),
        Value::ULInt(value) | Value::LWord(value) => i128::from(*value),
        Value::Time(value) | Value::LTime(value) => i128::from(value.as_nanos()),
        Value::Date(value) => i128::from(value.ticks()),
        Value::Tod(value) => i128::from(value.ticks()),
        Value::Dt(value) => i128::from(value.ticks()),
        Value::LDate(value) => i128::from(value.nanos()),
        Value::LTod(value) => i128::from(value.nanos()),
        Value::Ldt(value) => i128::from(value.nanos()),
        Value::Enum(value) => i128::from(value.numeric_value()),
        Value::Real(value) => {
            return match elem {
                Elem::Real => Some(pad(&value.to_le_bytes())),
                Elem::LReal => Some(f64::from(*value).to_le_bytes()),
                _ => None,
            };
        }
        Value::LReal(value) => {
            return match elem {
                Elem::LReal => Some(value.to_le_bytes()),
                #[allow(clippy::cast_possible_truncation)]
                Elem::Real => Some(pad(&(*value as f32).to_le_bytes())),
                _ => None,
            };
        }
        _ => return None,
    };
    if elem.is_real() {
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        return Some(if elem == Elem::Real {
            pad(&(wide as f32).to_le_bytes())
        } else {
            (wide as f64).to_le_bytes()
        });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((wide as u64).to_le_bytes())
}

fn pad(bytes: &[u8; 4]) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(bytes);
    out
}
