//! Convert non-executable embedding seeds to declarations and symbolic bindings.
use super::{RuntimeBuilder, SmolStr};
mod images;
use crate::{
    harness::{AccessDecl, AccessPart, AccessPath, CompileError, ConfigInit, GlobalInit},
    memory::{InstanceId, MemoryLocation},
    program_model::{Expr, LValue, VarDef},
    task::ProgramDef,
    value::{RefSegment, Value, ValueRef},
};
use images::image_initializers;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use trust_hir::{Type, TypeId};

fn invalid(message: impl std::fmt::Display) -> CompileError {
    CompileError::new(format!("builder seed: {message}"))
}

type SeedFields = (Option<SmolStr>, Vec<(SmolStr, TypeId)>);

struct Seeds<'a> {
    builder: &'a RuntimeBuilder,
    instances: HashMap<InstanceId, Vec<AccessPart>>,
    visited: HashSet<InstanceId>,
}

pub(super) fn lower(
    mut builder: RuntimeBuilder,
) -> Result<crate::harness::LoweredApplication, CompileError> {
    if !builder.storage.frames().is_empty() || builder.storage.execution_frame_count() != 0 {
        return Err(invalid("runtime frames cannot seed a new installation"));
    }
    if !builder.storage.retain().is_empty() {
        return Err(invalid(
            "raw retain storage has no declared binding; use retained declarations",
        ));
    }
    let mut declarations = builder.take_global_declarations();
    let mut seeds = Seeds {
        builder: &builder,
        instances: HashMap::new(),
        visited: HashSet::new(),
    };
    // Establish every persistent path before translating reference-valued seeds.
    for (name, value) in builder.storage.globals() {
        let path = vec![AccessPart::Name(name.clone())];
        if let Some(declaration) = declarations
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        {
            seeds.index_value(value, declaration.type_id, &path, 0)?;
        } else if let Some(program) = builder
            .programs
            .values()
            .find(|program| program.name.eq_ignore_ascii_case(name))
        {
            let Value::Instance(id) = value else {
                return Err(invalid("program seed must be a declared instance"));
            };
            seeds.index_instance(*id, &program.name, &path, 0)?;
        } else if let Some(local) = builder
            .functions
            .values()
            .flat_map(|function| {
                function
                    .static_locals
                    .iter()
                    .map(move |local| (function, local))
            })
            .find(|(function, local)| {
                crate::program_model::static_storage_name(&function.name, &local.name)
                    .eq_ignore_ascii_case(name)
            })
            .map(|(_, local)| local)
        {
            seeds.index_value(value, local.type_id, &path, 0)?;
        } else {
            return Err(invalid(format!("undeclared global '{name}'")));
        }
    }
    if seeds.visited.len() != builder.storage.instances().len() {
        return Err(invalid(
            "orphan instance or ambiguous ownership in seed graph",
        ));
    }
    for declaration in &mut declarations {
        if declaration.external || declaration.in_out || declaration.static_storage {
            return Err(invalid(
                "global declaration has a frame/member-only qualifier",
            ));
        }
        if let Some(value) = builder
            .storage
            .globals()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&declaration.name))
            .map(|(_, value)| value)
        {
            declaration.initializer = Some(seeds.expression(value, declaration.type_id, 0)?);
        }
    }
    let mut functions = builder.functions.clone();
    for function in functions.values_mut() {
        for local in &mut function.static_locals {
            let name = crate::program_model::static_storage_name(&function.name, &local.name);
            if let Some(value) = builder.storage.get_global(&name) {
                local.initializer = Some(seeds.expression(value, local.type_id, 0)?);
            }
        }
    }
    let mut programs = builder.programs.clone();
    for program in programs.values_mut() {
        if let Some(value) = builder
            .storage
            .globals()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&program.name))
            .map(|(_, value)| value)
        {
            let Value::Instance(id) = value else {
                return Err(invalid("program seed is not an instance"));
            };
            let instance = builder
                .storage
                .get_instance(*id)
                .ok_or_else(|| invalid("stale program instance"))?;
            for (name, value) in &instance.variables {
                let declaration = program
                    .vars
                    .iter_mut()
                    .find(|var| var.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| invalid(format!("undeclared program member '{name}'")))?;
                if declaration.external {
                    return Err(invalid("external member has no owned seed value"));
                }
                declaration.initializer = Some(seeds.expression(value, declaration.type_id, 0)?);
            }
        }
    }
    let mut aliases = Vec::new();
    for binding in builder.access.bindings() {
        let mut path = seeds.reference(&binding.reference)?;
        if let Some(partial) = binding.partial {
            path.push(AccessPart::Partial(partial));
        }
        aliases.push(AccessDecl {
            name: binding.name.clone(),
            path: AccessPath::Parts(path),
            writable: binding.writable,
        });
    }
    let mut configuration = Vec::new();
    for binding in builder.io.bindings() {
        let path = match &binding.target {
            crate::io::IoTarget::Reference(reference) => seeds.reference(reference)?,
            crate::io::IoTarget::Name(name) => vec![AccessPart::Name(name.clone())],
        };
        let ty = seeds.path_type(&path, &declarations, &programs)?;
        if let Some(declared) = binding.value_type {
            // Alias codecs may normalize to the primitive type; the common binder
            // will validate the actual declared destination and address together.
            let source = seeds.resolved_type(declared)?;
            let target = seeds.resolved_type(ty)?;
            let bounded_string = matches!((source, target, binding.address.size),
                (Type::String { max_len: None }, Type::String { max_len: Some(length) }, crate::io::IoSize::Bytes(width)) if *length == width);
            if source != target && !bounded_string {
                return Err(invalid("I/O binding type differs from declaration"));
            }
        }
        configuration.push(ConfigInit {
            display_name: binding.display_name.clone(),
            path: AccessPath::Parts(path),
            address: Some(binding.address.clone()),
            type_id: ty,
            initializer: None,
        });
    }
    let mut tasks = builder.tasks.clone();
    let mut task_bindings = Vec::new();
    for task in &mut tasks {
        for reference in std::mem::take(&mut task.fb_instances) {
            task_bindings.push((task.name.clone(), seeds.reference(&reference)?));
        }
    }
    drop(seeds);
    configuration.extend(image_initializers(&mut builder)?);
    builder.programs = programs;
    builder.functions = functions;
    builder.manual_seed_mode = true;
    let globals = declarations
        .into_iter()
        .map(|entry| GlobalInit {
            constant: entry.constant,
            name: entry.name,
            type_id: entry.type_id,
            initializer: entry.initializer,
            retain: entry.retain,
            address: entry
                .address
                .as_ref()
                .map(crate::bytecode::format_authoring_io_address),
        })
        .collect();
    crate::harness::LoweredApplication::from_builder(
        builder,
        globals,
        aliases,
        configuration,
        tasks,
        task_bindings,
    )
}

impl Seeds<'_> {
    fn resolved_type(&self, mut ty: TypeId) -> Result<&Type, CompileError> {
        for _ in 0..256 {
            let value = self
                .builder
                .registry
                .get(ty)
                .ok_or_else(|| invalid("unknown seed type"))?;
            if let Type::Alias { target, .. } = value {
                ty = *target;
            } else {
                return Ok(value);
            }
        }
        Err(invalid("type alias depth exceeded"))
    }

    fn fields(&self, name: &str) -> Result<SeedFields, CompileError> {
        if let Some(program) = self
            .builder
            .programs
            .values()
            .find(|program| program.name.eq_ignore_ascii_case(name))
        {
            return Ok((
                None,
                program
                    .vars
                    .iter()
                    .filter(|var| !var.external)
                    .map(|var| (var.name.clone(), var.type_id))
                    .collect(),
            ));
        }
        if let Some(fb) = self
            .builder
            .function_blocks
            .get(name.to_ascii_uppercase().as_str())
        {
            let parent = fb.base.as_ref().map(|base| match base {
                crate::program_model::FunctionBlockBase::Class(name)
                | crate::program_model::FunctionBlockBase::FunctionBlock(name) => name.clone(),
            });
            let mut fields: Vec<_> = fb
                .params
                .iter()
                .filter(|param| param.direction != trust_hir::symbols::ParamDirection::InOut)
                .map(|param| (param.name.clone(), param.type_id))
                .chain(
                    fb.vars
                        .iter()
                        .filter(|var| !var.external)
                        .map(|var| (var.name.clone(), var.type_id)),
                )
                .collect();
            fields.extend(method_static_fields(&fb.name, &fb.methods));
            return Ok((parent, fields));
        }
        if let Some(class) = self.builder.classes.get(name.to_ascii_uppercase().as_str()) {
            let mut fields: Vec<_> = class
                .vars
                .iter()
                .filter(|var| !var.external)
                .map(|var| (var.name.clone(), var.type_id))
                .collect();
            fields.extend(method_static_fields(&class.name, &class.methods));
            return Ok((class.base.clone(), fields));
        }
        Err(invalid(format!("undeclared instance type '{name}'")))
    }

    fn index_instance(
        &mut self,
        id: InstanceId,
        expected: &str,
        path: &[AccessPart],
        depth: usize,
    ) -> Result<(), CompileError> {
        if depth >= 256 || !self.visited.insert(id) {
            return Err(invalid("cyclic or shared owned instance"));
        }
        let builder = self.builder;
        let instance = builder
            .storage
            .get_instance(id)
            .ok_or_else(|| invalid("stale instance"))?;
        if !instance.type_name.eq_ignore_ascii_case(expected) {
            return Err(invalid("instance type differs from declaration"));
        }
        self.instances.insert(id, path.to_vec());
        let (parent, fields) = self.fields(expected)?;
        if let Some(parent_id) = instance.parent {
            let parent = parent.ok_or_else(|| invalid("undeclared parent instance"))?;
            let mut ancestor = Some(parent_id);
            for _ in 0..256 {
                let Some(id) = ancestor else { break };
                let data = builder
                    .storage
                    .get_instance(id)
                    .ok_or_else(|| invalid("stale parent instance"))?;
                if data.variables.keys().any(|name| {
                    fields
                        .iter()
                        .any(|(field, _)| field.eq_ignore_ascii_case(name))
                }) {
                    return Err(invalid("ambiguous shadowed seed member"));
                }
                ancestor = data.parent;
            }
            if ancestor.is_some() {
                return Err(invalid("cyclic parent seed graph"));
            }
            self.index_instance(parent_id, &parent, path, depth + 1)?;
        }
        for (name, value) in &instance.variables {
            let (_, ty) = fields
                .iter()
                .find(|(field, _)| field.eq_ignore_ascii_case(name))
                .ok_or_else(|| invalid(format!("undeclared seed member '{name}'")))?;
            let mut field = path.to_vec();
            field.push(AccessPart::Name(name.clone()));
            self.index_value(value, *ty, &field, depth + 1)?;
        }
        Ok(())
    }

    fn index_value(
        &mut self,
        value: &Value,
        ty: TypeId,
        path: &[AccessPart],
        depth: usize,
    ) -> Result<(), CompileError> {
        if depth >= 256 {
            return Err(invalid("seed graph depth exceeded"));
        }
        match (value, self.resolved_type(ty)?.clone()) {
            (Value::Instance(id), Type::FunctionBlock { name } | Type::Class { name }) => {
                self.index_instance(*id, &name, path, depth + 1)
            }
            (
                Value::Array(array),
                Type::Array {
                    element,
                    dimensions,
                },
            ) => {
                if array.dimensions() != dimensions.as_slice() {
                    return Err(invalid("array seed dimensions differ"));
                }
                for (offset, value) in array.elements().iter().enumerate() {
                    let mut path = path.to_vec();
                    path.push(AccessPart::Index(indices(&dimensions, offset)?));
                    self.index_value(value, element, &path, depth + 1)?;
                }
                Ok(())
            }
            (Value::Struct(value), Type::Struct { fields, .. }) => {
                for (name, value) in value.fields() {
                    let field = fields
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("unknown struct field"))?;
                    let mut path = path.to_vec();
                    path.push(AccessPart::Name(name.clone()));
                    self.index_value(value, field.type_id, &path, depth + 1)?;
                }
                Ok(())
            }
            (Value::Struct(value), Type::Union { variants, .. }) => {
                for (name, value) in value.fields() {
                    let field = variants
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("unknown union variant"))?;
                    let mut path = path.to_vec();
                    path.push(AccessPart::Name(name.clone()));
                    self.index_value(value, field.type_id, &path, depth + 1)?;
                }
                Ok(())
            }
            (Value::Instance(_), Type::Interface { .. }) => Ok(()),
            (Value::Instance(_), _) => Err(invalid("instance in a value declaration")),
            _ => Ok(()),
        }
    }

    fn reference(&self, reference: &ValueRef) -> Result<Vec<AccessPart>, CompileError> {
        let mut path = match reference.location {
            MemoryLocation::Global => vec![AccessPart::Name(
                self.builder
                    .storage
                    .globals()
                    .get_index(reference.offset)
                    .ok_or_else(|| invalid("stale global reference"))?
                    .0
                    .clone(),
            )],
            MemoryLocation::Instance(id) => {
                let mut path = self
                    .instances
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| invalid("reference to unowned instance"))?;
                let name = self
                    .builder
                    .storage
                    .get_instance(id)
                    .and_then(|instance| instance.variables.get_index(reference.offset))
                    .ok_or_else(|| invalid("stale member reference"))?
                    .0;
                path.push(AccessPart::Name(name.clone()));
                path
            }
            _ => {
                return Err(invalid(
                    "frame, raw I/O and raw retain references cannot seed persistent state",
                ))
            }
        };
        for part in &reference.path {
            path.push(match part {
                RefSegment::Field(name) => AccessPart::Name(name.clone()),
                RefSegment::Index(indices) => AccessPart::Index(indices.to_vec()),
            });
        }
        Ok(path)
    }

    fn expression(&self, value: &Value, ty: TypeId, depth: usize) -> Result<Expr, CompileError> {
        if depth >= 256 {
            return Err(invalid("seed value depth exceeded"));
        }
        match (value, self.resolved_type(ty)?) {
            (Value::Reference(Some(reference)), Type::Reference { .. } | Type::Pointer { .. }) => {
                Ok(Expr::Ref(lvalue(&self.reference(reference)?)?))
            }
            (Value::Reference(None), Type::Reference { .. } | Type::Pointer { .. }) => {
                Ok(Expr::Literal(Value::Null))
            }
            (Value::Instance(id), Type::Interface { .. }) => {
                let path = self
                    .instances
                    .get(id)
                    .ok_or_else(|| invalid("interface seed has no declared owner"))?;
                expression_path(path)
            }
            (Value::Instance(id), Type::FunctionBlock { .. } | Type::Class { .. }) => Ok(
                Expr::StructInitializer(self.instance_expression(*id, depth + 1)?),
            ),
            (Value::Array(array), Type::Array { element, .. }) => Ok(Expr::ArrayInitializer(
                array
                    .elements()
                    .iter()
                    .map(|value| self.expression(value, *element, depth + 1))
                    .collect::<Result<_, _>>()?,
            )),
            (Value::Struct(value), Type::Struct { fields, .. }) => Ok(Expr::StructInitializer(
                value
                    .fields()
                    .iter()
                    .map(|(name, value)| {
                        let field = fields
                            .iter()
                            .find(|field| field.name.eq_ignore_ascii_case(name))
                            .ok_or_else(|| invalid("unknown struct field"))?;
                        Ok((
                            field.name.clone(),
                            self.expression(value, field.type_id, depth + 1)?,
                        ))
                    })
                    .collect::<Result<_, CompileError>>()?,
            )),
            (Value::Struct(value), Type::Union { variants, .. }) => Ok(Expr::StructInitializer(
                value
                    .fields()
                    .iter()
                    .map(|(name, value)| {
                        let field = variants
                            .iter()
                            .find(|field| field.name.eq_ignore_ascii_case(name))
                            .ok_or_else(|| invalid("unknown union variant"))?;
                        Ok((
                            field.name.clone(),
                            self.expression(value, field.type_id, depth + 1)?,
                        ))
                    })
                    .collect::<Result<_, CompileError>>()?,
            )),
            _ => Ok(Expr::Literal(
                crate::harness::initializer::coerce_evaluated_initializer_value(
                    value.clone(),
                    ty,
                    &self.builder.registry,
                    &self.builder.profile,
                )?,
            )),
        }
    }

    fn instance_expression(
        &self,
        id: InstanceId,
        depth: usize,
    ) -> Result<Vec<(SmolStr, Expr)>, CompileError> {
        if depth >= 256 {
            return Err(invalid("instance seed depth exceeded"));
        }
        let instance = self
            .builder
            .storage
            .get_instance(id)
            .ok_or_else(|| invalid("stale instance"))?;
        let (_, fields) = self.fields(&instance.type_name)?;
        let mut values = instance
            .parent
            .map(|parent| self.instance_expression(parent, depth + 1))
            .transpose()?
            .unwrap_or_default();
        for (name, value) in &instance.variables {
            if values
                .iter()
                .any(|(parent, _)| parent.eq_ignore_ascii_case(name))
            {
                return Err(invalid("ambiguous shadowed seed member"));
            }
            let (_, ty) = fields
                .iter()
                .find(|(field, _)| field.eq_ignore_ascii_case(name))
                .ok_or_else(|| invalid("unknown seed member"))?;
            values.push((name.clone(), self.expression(value, *ty, depth + 1)?));
        }
        Ok(values)
    }

    fn path_type(
        &self,
        path: &[AccessPart],
        globals: &[VarDef],
        programs: &IndexMap<SmolStr, ProgramDef>,
    ) -> Result<TypeId, CompileError> {
        let Some(AccessPart::Name(root)) = path.first() else {
            return Err(invalid("binding root missing"));
        };
        let (mut ty, mut at) = if let Some(global) = globals
            .iter()
            .find(|global| global.name.eq_ignore_ascii_case(root))
        {
            (global.type_id, 1)
        } else if let Some(local) = self
            .builder
            .functions
            .values()
            .flat_map(|function| {
                function
                    .static_locals
                    .iter()
                    .map(move |local| (function, local))
            })
            .find(|(function, local)| {
                crate::program_model::static_storage_name(&function.name, &local.name)
                    .eq_ignore_ascii_case(root)
            })
            .map(|(_, local)| local)
        {
            (local.type_id, 1)
        } else {
            let program = programs
                .values()
                .find(|program| program.name.eq_ignore_ascii_case(root))
                .ok_or_else(|| invalid("unknown binding root"))?;
            let Some(AccessPart::Name(member)) = path.get(1) else {
                return Err(invalid("program root is not a typed value"));
            };
            (
                program
                    .vars
                    .iter()
                    .find(|var| var.name.eq_ignore_ascii_case(member))
                    .ok_or_else(|| invalid("unknown program member"))?
                    .type_id,
                2,
            )
        };
        while let Some(part) = path.get(at) {
            ty = match (part, self.resolved_type(ty)?) {
                (AccessPart::Index(_), Type::Array { element, .. }) => *element,
                (AccessPart::Name(name), Type::Struct { fields, .. }) => {
                    fields
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("unknown field"))?
                        .type_id
                }
                (AccessPart::Name(name), Type::Union { variants, .. }) => {
                    variants
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| invalid("unknown variant"))?
                        .type_id
                }
                (AccessPart::Name(member), Type::Class { name } | Type::FunctionBlock { name }) => {
                    self.member_type(name, member)?
                }
                _ => return Err(invalid("binding path differs from declaration")),
            };
            at += 1;
        }
        Ok(ty)
    }

    fn member_type(&self, owner: &str, name: &str) -> Result<TypeId, CompileError> {
        let mut owner: SmolStr = owner.into();
        for _ in 0..256 {
            let (parent, fields) = self.fields(&owner)?;
            if let Some((_, ty)) = fields
                .iter()
                .find(|(field, _)| field.eq_ignore_ascii_case(name))
            {
                return Ok(*ty);
            }
            owner = parent.ok_or_else(|| invalid("unknown instance member"))?;
        }
        Err(invalid("inheritance depth exceeded"))
    }
}

fn lvalue(path: &[AccessPart]) -> Result<LValue, CompileError> {
    let Some(AccessPart::Name(name)) = path.first() else {
        return Err(invalid("reference has no root"));
    };
    let mut result = LValue::Name(name.clone());
    for part in &path[1..] {
        result = match part {
            AccessPart::Name(field) => LValue::Field {
                target: Box::new(result),
                field: field.clone(),
            },
            AccessPart::Index(indices) => LValue::Index {
                target: Box::new(result),
                indices: indices
                    .iter()
                    .map(|index| Expr::Literal(Value::LInt(*index)))
                    .collect(),
            },
            AccessPart::Partial(_) => {
                return Err(invalid("partial access cannot be a persistent reference"))
            }
        };
    }
    Ok(result)
}

fn indices(dimensions: &[(i64, i64)], mut offset: usize) -> Result<Vec<i64>, CompileError> {
    let mut result = vec![0; dimensions.len()];
    for (index, (lower, upper)) in dimensions.iter().enumerate().rev() {
        let length = upper
            .checked_sub(*lower)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .filter(|length| *length != 0)
            .ok_or_else(|| invalid("invalid array dimension"))?;
        result[index] = lower
            .checked_add(
                i64::try_from(offset % length).map_err(|_| invalid("array offset overflow"))?,
            )
            .ok_or_else(|| invalid("array index overflow"))?;
        offset /= length;
    }
    if offset != 0 {
        return Err(invalid("array seed has excess elements"));
    }
    Ok(result)
}

fn expression_path(path: &[AccessPart]) -> Result<Expr, CompileError> {
    let Some(AccessPart::Name(name)) = path.first() else {
        return Err(invalid("value has no declared root"));
    };
    let mut result = Expr::Name(name.clone());
    for part in &path[1..] {
        result = match part {
            AccessPart::Name(field) => Expr::Field {
                target: Box::new(result),
                field: field.clone(),
            },
            AccessPart::Index(indices) => Expr::Index {
                target: Box::new(result),
                indices: indices
                    .iter()
                    .map(|index| Expr::Literal(Value::LInt(*index)))
                    .collect(),
            },
            AccessPart::Partial(_) => return Err(invalid("partial instance seed")),
        };
    }
    Ok(result)
}

fn method_static_fields(
    owner: &SmolStr,
    methods: &[crate::program_model::MethodDef],
) -> Vec<(SmolStr, TypeId)> {
    methods
        .iter()
        .flat_map(|method| {
            let owner = crate::program_model::method_static_storage_owner(owner, &method.name);
            method.static_locals.iter().map(move |local| {
                (
                    crate::program_model::static_storage_name(&owner, &local.name),
                    local.type_id,
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
