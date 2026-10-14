//! Reconstruct nominal HIR identities from the admitted portable type table.
use crate::error::RuntimeError;
use smol_str::SmolStr;
use trust_hir::{
    types::{StructField, TypeRegistry, UnionVariant},
    Type, TypeId,
};
use trust_runtime_core::{
    bytecode::{StorageDeclaration, StorageRole, TypeData, TypeKind},
    vm::PreparedModule,
};

type StorageDeclarations<'a> =
    std::collections::HashMap<(u32, u8, bool), Vec<&'a StorageDeclaration>>;
type MethodOwners = std::collections::HashMap<u32, Vec<u32>>;

pub(super) struct Projection<'a> {
    pub(super) prepared: &'a PreparedModule,
    pub(super) registry: TypeRegistry,
    pub(super) wire_types: Vec<TypeId>,
    pub(super) declarations: StorageDeclarations<'a>,
    pub(super) methods: MethodOwners,
}

impl<'a> Projection<'a> {
    pub(super) fn new(prepared: &'a PreparedModule) -> Result<Self, RuntimeError> {
        let (registry, wire_types) = reserve_wire_types(prepared)?;
        let (declarations, methods) = collect_owner_indexes(prepared);
        let mut projection = Self {
            prepared,
            registry,
            wire_types,
            declarations,
            methods,
        };
        projection.populate_types()?;
        Ok(projection)
    }

    fn populate_types(&mut self) -> Result<(), RuntimeError> {
        for (index, entry) in self.prepared.type_table().entries.iter().enumerate() {
            if matches!(entry.data, TypeData::Primitive { max_length: 0, .. }) {
                continue;
            }
            let name = entry
                .name_idx
                .map(|id| self.name(id))
                .transpose()?
                .unwrap_or_else(|| format!("@stbc/type/{index}").into());
            let ty = match &entry.data {
                TypeData::Primitive {
                    prim_id: 24,
                    max_length,
                } => Type::String {
                    max_len: Some(u32::from(*max_length)),
                },
                TypeData::Primitive {
                    prim_id: 25,
                    max_length,
                } => Type::WString {
                    max_len: Some(u32::from(*max_length)),
                },
                TypeData::Primitive { .. } => return Err(RuntimeError::InvalidExecutionState),
                TypeData::Array { elem_type_id, dims } => Type::Array {
                    element: self.ty(*elem_type_id)?,
                    dimensions: dims.clone(),
                },
                TypeData::Struct { fields } => Type::Struct {
                    name,
                    fields: fields
                        .iter()
                        .map(|field| {
                            Ok(StructField {
                                name: self.name(field.name_idx)?,
                                type_id: self.ty(field.type_id)?,
                                address: field.bit_offset.map(|offset| {
                                    format!("%X{}.{}", offset / 8, offset % 8).into()
                                }),
                                default_initializer: None,
                            })
                        })
                        .collect::<Result<_, RuntimeError>>()?,
                },
                TypeData::Union { fields } => Type::Union {
                    name,
                    variants: fields
                        .iter()
                        .map(|field| {
                            Ok(UnionVariant {
                                name: self.name(field.name_idx)?,
                                type_id: self.ty(field.type_id)?,
                                address: None,
                                default_initializer: None,
                            })
                        })
                        .collect::<Result<_, RuntimeError>>()?,
                },
                TypeData::Enum {
                    base_type_id,
                    variants,
                } => Type::Enum {
                    name,
                    base: self.ty(*base_type_id)?,
                    values: variants
                        .iter()
                        .map(|variant| Ok((self.name(variant.name_idx)?, variant.value)))
                        .collect::<Result<_, RuntimeError>>()?,
                },
                TypeData::Alias { target_type_id } => Type::Alias {
                    name,
                    target: self.ty(*target_type_id)?,
                },
                TypeData::Subrange {
                    base_type_id,
                    lower,
                    upper,
                } => Type::Subrange {
                    base: self.ty(*base_type_id)?,
                    lower: *lower,
                    upper: *upper,
                },
                TypeData::Reference { target_type_id } => Type::Reference {
                    target: self.ty(*target_type_id)?,
                },
                TypeData::Pou { pou_id } => {
                    let name = self
                        .prepared
                        .pou_name(*pou_id)
                        .ok_or(RuntimeError::InvalidExecutionState)?
                        .into();
                    match entry.kind {
                        TypeKind::FunctionBlock => Type::FunctionBlock { name },
                        TypeKind::Class => Type::Class { name },
                        _ => return Err(RuntimeError::InvalidExecutionState),
                    }
                }
                TypeData::Interface { .. } => Type::Interface { name },
            };
            self.registry.replace(self.wire_types[index], ty);
            if entry.kind == TypeKind::OverlappingStruct {
                self.registry
                    .register_overlapping_struct_type(self.wire_types[index]);
            }
        }
        Ok(())
    }

    pub(super) fn name(&self, id: u32) -> Result<SmolStr, RuntimeError> {
        self.prepared
            .strings()
            .get(id as usize)
            .cloned()
            .ok_or(RuntimeError::InvalidExecutionState)
    }

    pub(super) fn ty(&self, id: u32) -> Result<TypeId, RuntimeError> {
        self.wire_types
            .get(id as usize)
            .copied()
            .ok_or(RuntimeError::InvalidExecutionState)
    }
}

fn reserve_wire_types(
    prepared: &PreparedModule,
) -> Result<(TypeRegistry, Vec<TypeId>), RuntimeError> {
    let mut registry = TypeRegistry::new();
    let mut wire_types = Vec::with_capacity(prepared.type_table().entries.len());
    for (index, entry) in prepared.type_table().entries.iter().enumerate() {
        let id = match entry.data {
            TypeData::Primitive {
                prim_id,
                max_length: 0,
            } => {
                let builtin = primitive(prim_id)?;
                let name = entry
                    .name_idx
                    .map(|id| {
                        prepared
                            .strings()
                            .get(id as usize)
                            .ok_or(RuntimeError::InvalidExecutionState)
                    })
                    .transpose()?;
                match name {
                    Some(name) if TypeId::from_builtin_name(name).is_none() => registry.register(
                        name.clone(),
                        Type::Alias {
                            name: name.clone(),
                            target: builtin,
                        },
                    ),
                    Some(name) if TypeId::from_builtin_name(name) != Some(builtin) => {
                        return Err(RuntimeError::InvalidExecutionState)
                    }
                    _ => builtin,
                }
            }
            _ => {
                let name = entry
                    .name_idx
                    .map(|id| {
                        prepared
                            .strings()
                            .get(id as usize)
                            .cloned()
                            .ok_or(RuntimeError::InvalidExecutionState)
                    })
                    .transpose()?
                    .or_else(|| match &entry.data {
                        TypeData::Pou { pou_id } => prepared.pou_name(*pou_id).map(SmolStr::new),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("@stbc/type/{index}").into());
                // Duplicate wire names may not replace an unrelated built-in.
                if TypeId::from_builtin_name(&name).is_some() {
                    return Err(RuntimeError::InvalidExecutionState);
                }
                registry.reserve(name)
            }
        };
        wire_types.push(id);
    }
    Ok((registry, wire_types))
}

fn collect_owner_indexes<'a>(
    prepared: &'a PreparedModule,
) -> (StorageDeclarations<'a>, MethodOwners) {
    let mut declarations: std::collections::HashMap<_, Vec<_>> = std::collections::HashMap::new();
    for entry in prepared.storage_declarations() {
        if let Some(owner) = entry.owner_pou_id
            && matches!(
                entry.role,
                StorageRole::Variable | StorageRole::External | StorageRole::Static
            )
        {
            declarations
                .entry((owner, entry.owner as u8, entry.role == StorageRole::Static))
                .or_default()
                .push(entry);
        }
    }
    let mut methods: std::collections::HashMap<_, Vec<_>> = std::collections::HashMap::new();
    for (id, _) in prepared.pous() {
        if let Some(owner) = prepared.method_owner(id) {
            methods.entry(owner).or_default().push(id);
        }
    }
    for ids in methods.values_mut() {
        ids.sort_unstable();
    }
    (declarations, methods)
}

pub(super) fn primitive(id: u16) -> Result<TypeId, RuntimeError> {
    Ok(match id {
        1 => TypeId::BOOL,
        2 => TypeId::BYTE,
        3 => TypeId::WORD,
        4 => TypeId::DWORD,
        5 => TypeId::LWORD,
        6 => TypeId::SINT,
        7 => TypeId::INT,
        8 => TypeId::DINT,
        9 => TypeId::LINT,
        10 => TypeId::USINT,
        11 => TypeId::UINT,
        12 => TypeId::UDINT,
        13 => TypeId::ULINT,
        14 => TypeId::REAL,
        15 => TypeId::LREAL,
        16 => TypeId::TIME,
        17 => TypeId::LTIME,
        18 => TypeId::DATE,
        19 => TypeId::LDATE,
        20 => TypeId::TOD,
        21 => TypeId::LTOD,
        22 => TypeId::DT,
        23 => TypeId::LDT,
        24 => TypeId::STRING,
        25 => TypeId::WSTRING,
        26 => TypeId::CHAR,
        27 => TypeId::WCHAR,
        0x0100 => TypeId::ANY_INT,
        _ => return Err(RuntimeError::InvalidExecutionState),
    })
}
