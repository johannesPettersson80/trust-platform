use smol_str::SmolStr;

use crate::program_model::{MethodDef, Param, VarDef};
use trust_hir::{Type, TypeId};

use super::util::normalize_name;
use crate::bytecode::{EnumVariant, Field, TypeData, TypeEntry, TypeKind};

use super::{BytecodeEncoder, BytecodeError};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn collect_decl_types(&mut self) -> Result<(), BytecodeError> {
        for meta in self.runtime.globals().values() {
            self.type_index(meta.type_id)?;
        }
        for program in self.runtime.programs().values() {
            self.collect_var_types(&program.vars)?;
            self.collect_var_types(&program.temps)?;
        }
        for func in self.runtime.functions().values() {
            self.type_index(func.return_type)?;
            self.collect_param_types(&func.params)?;
            self.collect_var_types(&func.locals)?;
            self.collect_var_types(&func.static_locals)?;
        }
        for fb in self.runtime.function_blocks().values() {
            if self.is_stdlib_fb(&fb.name) {
                continue;
            }
            self.collect_param_types(&fb.params)?;
            self.collect_var_types(&fb.vars)?;
            self.collect_var_types(&fb.temps)?;
            for method in &fb.methods {
                self.collect_method_types(method)?;
            }
        }
        for class in self.runtime.classes().values() {
            self.collect_var_types(&class.vars)?;
            for method in &class.methods {
                self.collect_method_types(method)?;
            }
        }
        for binding in self.runtime.io().bindings() {
            if let Some(type_id) = binding.value_type {
                self.type_index(type_id)?;
            }
        }
        Ok(())
    }

    fn collect_param_types(&mut self, params: &[Param]) -> Result<(), BytecodeError> {
        for param in params {
            self.type_index(param.type_id)?;
        }
        Ok(())
    }

    fn collect_var_types(&mut self, vars: &[VarDef]) -> Result<(), BytecodeError> {
        for var in vars {
            self.type_index(var.type_id)?;
        }
        Ok(())
    }

    fn collect_method_types(&mut self, method: &MethodDef) -> Result<(), BytecodeError> {
        if let Some(return_type) = method.return_type {
            self.type_index(return_type)?;
        }
        self.collect_param_types(&method.params)?;
        self.collect_var_types(&method.locals)?;
        self.collect_var_types(&method.static_locals)?;
        Ok(())
    }

    pub(super) fn is_stdlib_fb(&self, name: &SmolStr) -> bool {
        let key = normalize_name(name);
        self.stdlib_fbs.contains(&key)
    }

    pub(super) fn type_index(&mut self, type_id: TypeId) -> Result<u32, BytecodeError> {
        if let Some(idx) = self.type_map.get(&type_id) {
            return Ok(*idx);
        }
        let idx = self.types.len() as u32;
        self.type_map.insert(type_id, idx);
        self.types.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 0,
                max_length: 0,
            },
        });
        let ty = self
            .runtime
            .registry()
            .get(type_id)
            .ok_or_else(|| BytecodeError::InvalidSection("unknown type id".into()))?
            .clone();
        let entry = self.encode_type_entry(type_id, &ty)?;
        self.types[idx as usize] = entry;
        Ok(idx)
    }

    fn encode_type_entry(
        &mut self,
        type_id: TypeId,
        ty: &Type,
    ) -> Result<TypeEntry, BytecodeError> {
        let name_idx = self
            .runtime
            .registry()
            .type_name(type_id)
            .map(|name| self.strings.intern(name));
        let (kind, data) = match ty {
            Type::Bool => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 1,
                    max_length: 0,
                },
            ),
            Type::Byte => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 2,
                    max_length: 0,
                },
            ),
            Type::Word => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 3,
                    max_length: 0,
                },
            ),
            Type::DWord => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 4,
                    max_length: 0,
                },
            ),
            Type::LWord => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 5,
                    max_length: 0,
                },
            ),
            Type::SInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 6,
                    max_length: 0,
                },
            ),
            Type::Int => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 7,
                    max_length: 0,
                },
            ),
            Type::DInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 8,
                    max_length: 0,
                },
            ),
            Type::LInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 9,
                    max_length: 0,
                },
            ),
            Type::USInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 10,
                    max_length: 0,
                },
            ),
            Type::UInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 11,
                    max_length: 0,
                },
            ),
            Type::UDInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 12,
                    max_length: 0,
                },
            ),
            Type::ULInt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 13,
                    max_length: 0,
                },
            ),
            Type::Real => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 14,
                    max_length: 0,
                },
            ),
            Type::LReal => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 15,
                    max_length: 0,
                },
            ),
            Type::Time => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 16,
                    max_length: 0,
                },
            ),
            Type::LTime => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 17,
                    max_length: 0,
                },
            ),
            Type::Date => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 18,
                    max_length: 0,
                },
            ),
            Type::LDate => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 19,
                    max_length: 0,
                },
            ),
            Type::Tod => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 20,
                    max_length: 0,
                },
            ),
            Type::LTod => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 21,
                    max_length: 0,
                },
            ),
            Type::Dt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 22,
                    max_length: 0,
                },
            ),
            Type::Ldt => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 23,
                    max_length: 0,
                },
            ),
            Type::String { max_len } => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 24,
                    max_length: max_len
                        .map(|len| {
                            u16::try_from(len).map_err(|_| {
                                BytecodeError::InvalidSection("STRING length overflow".into())
                            })
                        })
                        .transpose()?
                        .unwrap_or(0),
                },
            ),
            Type::WString { max_len } => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 25,
                    max_length: max_len
                        .map(|len| {
                            u16::try_from(len).map_err(|_| {
                                BytecodeError::InvalidSection("WSTRING length overflow".into())
                            })
                        })
                        .transpose()?
                        .unwrap_or(0),
                },
            ),
            Type::Char => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 26,
                    max_length: 0,
                },
            ),
            Type::WChar => (
                TypeKind::Primitive,
                TypeData::Primitive {
                    prim_id: 27,
                    max_length: 0,
                },
            ),
            Type::Array {
                element,
                dimensions,
            } => {
                let elem_type_id = self.type_index(*element)?;
                (
                    TypeKind::Array,
                    TypeData::Array {
                        elem_type_id,
                        dims: dimensions.clone(),
                    },
                )
            }
            Type::Struct { fields, .. } => {
                let mut out_fields = Vec::with_capacity(fields.len());
                for field in fields {
                    let name_idx = self.strings.intern(field.name.clone());
                    let type_id = self.type_index(field.type_id)?;
                    out_fields.push(Field { name_idx, type_id });
                }
                (TypeKind::Struct, TypeData::Struct { fields: out_fields })
            }
            Type::Union { variants, .. } => {
                let mut out_fields = Vec::with_capacity(variants.len());
                for variant in variants {
                    let name_idx = self.strings.intern(variant.name.clone());
                    let type_id = self.type_index(variant.type_id)?;
                    out_fields.push(Field { name_idx, type_id });
                }
                (TypeKind::Union, TypeData::Union { fields: out_fields })
            }
            Type::Enum { base, values, .. } => {
                let base_type_id = self.type_index(*base)?;
                let mut variants = Vec::with_capacity(values.len());
                for (name, value) in values {
                    let name_idx = self.strings.intern(name.clone());
                    variants.push(EnumVariant {
                        name_idx,
                        value: *value,
                    });
                }
                (
                    TypeKind::Enum,
                    TypeData::Enum {
                        base_type_id,
                        variants,
                    },
                )
            }
            Type::Alias { target, .. } => {
                let target_type_id = self.type_index(*target)?;
                (TypeKind::Alias, TypeData::Alias { target_type_id })
            }
            Type::Subrange { base, lower, upper } => {
                let base_type_id = self.type_index(*base)?;
                (
                    TypeKind::Subrange,
                    TypeData::Subrange {
                        base_type_id,
                        lower: *lower,
                        upper: *upper,
                    },
                )
            }
            Type::Reference { target } | Type::Pointer { target } => {
                let target_type_id = self.type_index(*target)?;
                (TypeKind::Reference, TypeData::Reference { target_type_id })
            }
            Type::FunctionBlock { name } => {
                let pou_id = self.pou_ids.function_block_id(name).ok_or_else(|| {
                    BytecodeError::InvalidSection("unknown function block".into())
                })?;
                (TypeKind::FunctionBlock, TypeData::Pou { pou_id })
            }
            Type::Class { name } => {
                let pou_id = self
                    .pou_ids
                    .class_id(name)
                    .ok_or_else(|| BytecodeError::InvalidSection("unknown class".into()))?;
                (TypeKind::Class, TypeData::Pou { pou_id })
            }
            Type::Interface { name } => {
                let methods = self.interface_methods_for(name)?;
                (TypeKind::Interface, TypeData::Interface { methods })
            }
            Type::Unknown
            | Type::Void
            | Type::Null
            | Type::Any
            | Type::AnyDerived
            | Type::AnyElementary
            | Type::AnyMagnitude
            | Type::AnyUnsigned
            | Type::AnySigned
            | Type::AnyDuration
            | Type::AnyChars
            | Type::AnyChar
            | Type::AnyInt
            | Type::AnyReal
            | Type::AnyNum
            | Type::AnyBit
            | Type::AnyString
            | Type::AnyDate => {
                return Err(BytecodeError::InvalidSection(
                    "unsupported generic type".into(),
                ))
            }
        };
        Ok(TypeEntry {
            kind,
            name_idx,
            data,
        })
    }
}
