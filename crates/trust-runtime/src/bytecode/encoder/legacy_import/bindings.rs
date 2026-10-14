//! Bind supplied wire identities through declarations, including unnamed types.
use super::*;
use crate::bytecode::PouEntry;
use crate::program_model::{Param, VarDef};
use trust_hir::symbols::ParamDirection;

struct SourceSignature {
    result: Option<TypeId>,
    params: Vec<Param>,
    locals: Vec<(SmolStr, TypeId)>,
}

fn local_types(
    result: Option<(&SmolStr, TypeId)>,
    params: &[Param],
    vars: &[VarDef],
) -> Vec<(SmolStr, TypeId)> {
    result
        .into_iter()
        .map(|(name, ty)| (name.clone(), ty))
        .chain(
            params
                .iter()
                .map(|param| (param.name.clone(), param.type_id)),
        )
        .chain(
            vars.iter()
                .filter(|var| !var.external)
                .map(|var| (var.name.clone(), var.type_id)),
        )
        .collect()
}

impl BytecodeEncoder<'_> {
    pub(super) fn seed_legacy_declaration_types(
        &mut self,
        supplied: &BytecodeModule,
        index: &PouIndex,
    ) -> Result<(), BytecodeError> {
        let mut locals = HashMap::new();
        for pou in &index.entries {
            let signature = self.legacy_source_signature(pou, index)?;
            match (signature.result, pou.return_type_id) {
                (Some(source), Some(wire)) => self.bind_legacy_type(source, wire)?,
                (None, None) => {}
                _ => return Err(invalid("return type presence differs")),
            }
            let name = &self.strings.entries[pou.name_idx as usize];
            let empty_builtin = pou.kind == PouKind::FunctionBlock
                && pou.params.is_empty()
                && self.is_stdlib_fb(name);
            if !empty_builtin {
                if signature.params.len() != pou.params.len() {
                    return Err(invalid("parameter count differs"));
                }
                for (source, wire) in signature.params.iter().zip(&pou.params) {
                    let direction = match source.direction {
                        ParamDirection::In => 0,
                        ParamDirection::Out => 1,
                        ParamDirection::InOut => 2,
                    };
                    if !source
                        .name
                        .eq_ignore_ascii_case(&self.strings.entries[wire.name_idx as usize])
                        || wire.direction != direction
                    {
                        return Err(invalid("parameter name or direction differs"));
                    }
                    self.bind_legacy_type(source.type_id, wire.type_id)?;
                }
            }
            locals.insert(pou.id, signature.locals);
        }
        let Some(SectionData::VarMeta(meta)) = supplied.section(SectionId::VarMeta) else {
            return Ok(());
        };
        let input = self.authoring.ok_or_else(|| invalid("context missing"))?;
        for old in &meta.entries {
            let reference = &self.ref_entries[old.ref_idx as usize];
            let name = &self.strings.entries[old.name_idx as usize];
            let ty = match reference.location {
                RefLocation::Local => {
                    let pou = index
                        .entries
                        .iter()
                        .find(|pou| {
                            old.ref_idx >= pou.local_ref_start
                                && pou
                                    .local_ref_start
                                    .checked_add(pou.local_ref_count)
                                    .is_some_and(|end| old.ref_idx < end)
                        })
                        .ok_or_else(|| invalid("local declaration owner missing"))?;
                    if !reference.segments.is_empty()
                        || old.ref_idx - pou.local_ref_start != reference.offset
                    {
                        return Err(invalid("local declaration slot differs"));
                    }
                    let (local_name, ty) = locals
                        .get(&pou.id)
                        .and_then(|slots| slots.get(reference.offset as usize))
                        .ok_or_else(|| invalid("local declaration type missing"))?;
                    if name.as_str()
                        != format!("@local/{}/{}/{local_name}", pou.id, reference.offset)
                    {
                        return Err(invalid("local declaration name differs"));
                    }
                    *ty
                }
                RefLocation::Global => input
                    .globals
                    .iter()
                    .find(|global| global.name.eq_ignore_ascii_case(name))
                    .map(|global| global.type_id)
                    .or_else(|| self.legacy_static_type(name))
                    .ok_or_else(|| invalid("global declaration type missing"))?,
                RefLocation::Instance => {
                    let config = input
                        .configuration
                        .as_ref()
                        .ok_or_else(|| invalid("configuration missing"))?;
                    let (program, member) = config
                        .programs
                        .iter()
                        .find_map(|program| {
                            let prefix = name.get(..program.name.len())?;
                            if !prefix.eq_ignore_ascii_case(&program.name) {
                                return None;
                            }
                            Some((program, name.get(program.name.len()..)?.strip_prefix('.')?))
                        })
                        .ok_or_else(|| invalid("instance declaration owner missing"))?;
                    input
                        .program_defs
                        .get(&normalize_name(&program.type_name))
                        .and_then(|definition| {
                            definition
                                .vars
                                .iter()
                                .find(|var| !var.external && var.name.eq_ignore_ascii_case(member))
                        })
                        .map(|var| var.type_id)
                        .ok_or_else(|| invalid("instance declaration type missing"))?
                }
                _ => return Err(invalid("unexpected declaration reference domain")),
            };
            self.bind_legacy_type(ty, old.type_id)?;
        }
        Ok(())
    }

    fn legacy_source_signature(
        &self,
        pou: &PouEntry,
        index: &PouIndex,
    ) -> Result<SourceSignature, BytecodeError> {
        let name = &self.strings.entries[pou.name_idx as usize];
        let key = normalize_name(name);
        let input = self.authoring.ok_or_else(|| invalid("context missing"))?;
        let mut signature = SourceSignature {
            result: None,
            params: Vec::new(),
            locals: Vec::new(),
        };
        match pou.kind {
            PouKind::Program => {
                let configured = input
                    .configuration
                    .as_ref()
                    .and_then(|config| {
                        config
                            .programs
                            .iter()
                            .find(|program| program.name.eq_ignore_ascii_case(name))
                    })
                    .ok_or_else(|| invalid("program context missing"))?;
                let definition = input
                    .program_defs
                    .get(&normalize_name(&configured.type_name))
                    .ok_or_else(|| invalid("program definition missing"))?;
                signature.locals = local_types(None, &[], &definition.temps);
            }
            PouKind::Function => {
                let definition = self
                    .runtime
                    .functions()
                    .get(&key)
                    .ok_or_else(|| invalid("function definition missing"))?;
                signature.result = Some(definition.return_type);
                signature.params = definition.params.clone();
                signature.locals = local_types(
                    Some((&definition.name, definition.return_type)),
                    &definition.params,
                    &definition.locals,
                );
            }
            PouKind::FunctionBlock => {
                let definition = self
                    .runtime
                    .function_blocks()
                    .get(&key)
                    .ok_or_else(|| invalid("block definition missing"))?;
                signature.params = definition.params.clone();
                signature.locals = local_types(None, &[], &definition.temps);
            }
            PouKind::Class => {
                if !self.runtime.classes().contains_key(&key) {
                    return Err(invalid("class definition missing"));
                }
            }
            PouKind::Method => {
                let owner = index
                    .entries
                    .iter()
                    .find(|owner| Some(owner.id) == pou.owner_pou_id)
                    .ok_or_else(|| invalid("method owner missing"))?;
                let owner_name = normalize_name(&self.strings.entries[owner.name_idx as usize]);
                let methods = match owner.kind {
                    PouKind::FunctionBlock => self
                        .runtime
                        .function_blocks()
                        .get(&owner_name)
                        .map(|owner| owner.methods.as_slice()),
                    PouKind::Class => self
                        .runtime
                        .classes()
                        .get(&owner_name)
                        .map(|owner| owner.methods.as_slice()),
                    _ => None,
                }
                .ok_or_else(|| invalid("method owner definition missing"))?;
                let definition = methods
                    .iter()
                    .find(|method| method.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| invalid("method definition missing"))?;
                signature.result = definition.return_type;
                signature.params = definition.params.clone();
                signature.locals = local_types(
                    definition.return_type.map(|ty| (&definition.name, ty)),
                    &definition.params,
                    &definition.locals,
                );
            }
        }
        Ok(signature)
    }

    fn legacy_static_type(&self, name: &str) -> Option<TypeId> {
        for function in self.runtime.functions().values() {
            for local in &function.static_locals {
                if crate::program_model::static_storage_name(&function.name, &local.name)
                    .eq_ignore_ascii_case(name)
                {
                    return Some(local.type_id);
                }
            }
        }
        for (owner, methods) in self
            .runtime
            .function_blocks()
            .values()
            .map(|owner| (&owner.name, &owner.methods))
            .chain(
                self.runtime
                    .classes()
                    .values()
                    .map(|owner| (&owner.name, &owner.methods)),
            )
        {
            for method in methods {
                let storage_owner =
                    crate::program_model::method_static_storage_owner(owner, &method.name);
                for local in &method.static_locals {
                    if crate::program_model::static_storage_name(&storage_owner, &local.name)
                        .eq_ignore_ascii_case(name)
                    {
                        return Some(local.type_id);
                    }
                }
            }
        }
        None
    }
}
