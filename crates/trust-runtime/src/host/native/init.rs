//! The starting memory of a compiled program (specification 44 §6.7, §7.4): every variable of the
//! globals, retain and instance areas at its initial value, as bytes at its offset.
//!
//! The initial values are the ones the runtime gives the same program today: the build constructs
//! the program on the computer with the runtime's own initialization (IEC §6.5.1, Table 14; the
//! declared initial values, a type's defaults, a function block's defaults) and writes each value
//! at the place this program's layout gives it, so that a compiled program starts where an
//! interpreted one started.

use smol_str::SmolStr;

use super::model::{AreaLayout, Model};
use super::types::{Elem, Kind, Ty};
use super::NativeError;
use crate::harness::{CompileSession, SourceFile};
use crate::memory::InstanceId;
use crate::value::Value;
use crate::Runtime;

/// The starting bytes of the marker, globals, retain and instance areas.
pub(super) struct StartingMemory {
    pub(super) markers: Vec<u8>,
    pub(super) globals: Vec<u8>,
    pub(super) retain: Vec<u8>,
    pub(super) instances: Vec<u8>,
}

/// Computes the starting memory of `model`, whose sources are `sources`.
///
/// # Errors
///
/// [`NativeError`] when the runtime cannot construct the program or a value does not fit its
/// place.
pub(super) fn starting_memory(
    sources: &[SourceFile],
    model: &mut Model<'_>,
) -> Result<StartingMemory, NativeError> {
    let runtime = CompileSession::from_sources(sources.to_vec())
        .build_runtime()
        .map_err(|error| NativeError::Compile(error.to_string()))?;
    let mut writer = Writer {
        runtime: &runtime,
        model,
        top: false,
    };
    let mut globals = vec![0u8; writer.model.area_g.size as usize];
    let mut retain = vec![0u8; writer.model.area_r.size as usize];
    let mut instances = vec![0u8; writer.model.area_p.size as usize];
    let global_fields: Vec<(SmolStr, Ty, bool)> = writer
        .model
        .globals
        .iter()
        .map(|global| (global.name.clone(), global.ty, global.retained))
        .collect();
    for (name, ty, retained) in global_fields {
        let (layout, bytes) = if retained {
            (&writer.model.area_r, &mut retain)
        } else {
            (&writer.model.area_g, &mut globals)
        };
        let offset = field_offset(layout, &name)?;
        let Some(value) = writer.runtime.storage().get_global(&name).cloned() else {
            continue;
        };
        writer.top = true;
        writer.value(&value, ty, bytes, offset as usize)?;
    }
    let instance_fields: Vec<(SmolStr, usize)> = writer
        .model
        .instances
        .iter()
        .map(|instance| (instance.name.clone(), instance.program))
        .collect();
    for (name, program) in instance_fields {
        let Some(Value::Instance(id)) = writer.runtime.storage().get_global(&name).cloned() else {
            return Err(NativeError::Compile(format!(
                "program instance {name} was not constructed"
            )));
        };
        let p_offset = field_offset(&writer.model.area_p, &name)? as usize;
        let r_offset = field_offset(&writer.model.area_r, &name)? as usize;
        let (members, retained) = {
            let program = &writer.model.program_types[program];
            (program.members.clone(), program.retained.clone())
        };
        for member in members {
            writer.top = true;
            writer.member(
                id,
                &member.name,
                member.ty,
                &mut instances,
                p_offset + member.offset as usize,
            )?;
        }
        for member in retained {
            writer.top = true;
            writer.member(
                id,
                &member.name,
                member.ty,
                &mut retain,
                r_offset + member.offset as usize,
            )?;
        }
    }
    let markers = vec![0u8; model_markers(writer.model)];
    Ok(StartingMemory {
        markers,
        globals,
        retain,
        instances,
    })
}

fn model_markers(model: &Model<'_>) -> usize {
    model.images[2] as usize
}

fn field_offset(layout: &AreaLayout, name: &str) -> Result<u32, NativeError> {
    layout
        .field(name)
        .map(|field| field.offset)
        .ok_or_else(|| NativeError::Compile(format!("{name} has no place in its area")))
}

/// Writes values at their places.
struct Writer<'r, 'm, 'a> {
    runtime: &'r Runtime,
    model: &'m mut Model<'a>,
    /// Whether the next value written is a global or a program's variable itself, not a part of
    /// one.
    top: bool,
}

impl Writer<'_, '_, '_> {
    /// Writes the variable `name` of instance `id` at `offset`, when the runtime holds it.
    fn member(
        &mut self,
        id: InstanceId,
        name: &str,
        ty: Ty,
        bytes: &mut [u8],
        offset: usize,
    ) -> Result<(), NativeError> {
        let value = self
            .runtime
            .storage()
            .get_instance_var(id, name)
            .cloned()
            .or_else(|| {
                self.runtime
                    .storage()
                    .get_instance(id)
                    .and_then(|instance| {
                        instance
                            .variables
                            .iter()
                            .find(|(key, _)| key.eq_ignore_ascii_case(name))
                            .map(|(_, value)| value.clone())
                    })
            });
        match value {
            Some(value) => self.value(&value, ty, bytes, offset),
            // A hidden variable of a standard function block starts at zero.
            None => Ok(()),
        }
    }

    /// Writes `value` of type `ty` at `offset`.
    fn value(
        &mut self,
        value: &Value,
        ty: Ty,
        bytes: &mut [u8],
        offset: usize,
    ) -> Result<(), NativeError> {
        let top = std::mem::take(&mut self.top);
        let info = self.model.types.info(ty).clone();
        let put = |bytes: &mut [u8], data: &[u8]| -> Result<(), NativeError> {
            bytes
                .get_mut(offset..offset + data.len())
                .ok_or_else(|| NativeError::Compile("a starting value outside its area".into()))?
                .copy_from_slice(data);
            Ok(())
        };
        match (&info.kind, value) {
            (_, Value::Null) => Ok(()),
            // A reference starts `NULL`; a variable's `REF` initializer is set by the cold-start
            // and the warm-start initialization (`emit::reference_initializers`), which is only
            // there for a global or a program's variable.
            (Kind::Ref { .. }, Value::Reference(None)) => Ok(()),
            (Kind::Ref { .. }, Value::Reference(Some(_))) if top => Ok(()),
            (Kind::Ref { .. }, Value::Reference(Some(_))) => Err(NativeError::unsupported(
                "a reference initializer inside a structure or a function block".into(),
            )),
            (
                Kind::Elem(elem)
                | Kind::Enum { base: elem, .. }
                | Kind::Subrange { base: elem, .. },
                value,
            ) => {
                let data = scalar(*elem, value).ok_or_else(|| {
                    NativeError::Compile(format!("a starting value {value:?} of {}", elem.name()))
                })?;
                put(bytes, &data[..elem.size() as usize])
            }
            (Kind::Str(capacity), Value::String(text)) => {
                let mut end = text.len().min(*capacity as usize);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                let length = u16::try_from(end).unwrap_or(u16::MAX);
                put(bytes, &length.to_le_bytes())?;
                bytes
                    .get_mut(offset + 2..offset + 2 + end)
                    .ok_or_else(|| NativeError::Compile("a starting text outside its area".into()))?
                    .copy_from_slice(&text.as_bytes()[..end]);
                Ok(())
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
                put(bytes, &length.to_le_bytes())?;
                bytes
                    .get_mut(offset + 2..offset + 2 + units.len())
                    .ok_or_else(|| NativeError::Compile("a starting text outside its area".into()))?
                    .copy_from_slice(&units);
                Ok(())
            }
            (Kind::Array { elem, .. }, Value::Array(array)) => {
                let size = self.model.types.info(*elem).size as usize;
                for (index, element) in array.elements().iter().enumerate() {
                    self.value(element, *elem, bytes, offset + index * size)?;
                }
                Ok(())
            }
            (Kind::Struct { members }, Value::Struct(structure)) => {
                for member in members {
                    if let Some(field) = structure.field(&member.name) {
                        self.value(field, member.ty, bytes, offset + member.offset as usize)?;
                    }
                }
                Ok(())
            }
            (Kind::Fb { members, .. }, Value::Instance(id)) => {
                // A `VAR_IN_OUT` holds a reference to its argument from the first call on; it
                // starts `NULL`, whatever value the runtime keeps for it before a call.
                let held = members.iter().filter(|member| {
                    member.flags & trust_runtime_core::native::image::SYMBOL_IN_OUT == 0
                });
                for member in held.cloned().collect::<Vec<_>>() {
                    self.member(
                        *id,
                        &member.name,
                        member.ty,
                        bytes,
                        offset + member.offset as usize,
                    )?;
                }
                Ok(())
            }
            (Kind::StdFb(fb), Value::Instance(id)) => {
                let members = self.model.types.std_fb_members(*fb);
                for member in members {
                    self.member(
                        *id,
                        &member.name,
                        member.ty,
                        bytes,
                        offset + member.offset as usize,
                    )?;
                }
                Ok(())
            }
            (kind, value) => Err(NativeError::Compile(format!(
                "a starting value {value:?} for a {kind:?}"
            ))),
        }
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
