//! The retained variables of the function block and class instances a resource holds (specification
//! 11 §6.7; IEC 61131-3 §6.5.6.2), listed from the construction roots: each instance that a global or
//! a program holds, followed depth first through the variables that hold instances and through its
//! base, and each variable of it under its checkpoint key, retained or not.
use super::*;
use crate::bytecode::{ConstructionRoot, StorageOwner, StorageRole};
use alloc::format;
use alloc::string::{String, ToString};

/// A retained variable of an instance: its checkpoint key, the construction root of the instance
/// that holds it and its declaration.
#[derive(Debug, Clone)]
pub(super) struct RetainedMember {
    pub(super) key: SmolStr,
    pub(super) root: u32,
    pub(super) declaration: u32,
}

/// Whether a variable declared with the retention code `own` (0 none, 1 `RETAIN`, 2 `NON_RETAIN`, 3
/// `PERSISTENT`) is retained in an instance whose holding declaration has the code `holder`: its own
/// qualifier decides, and without one the holder's does (IEC 61131-3 §6.5.6.2).
fn retained(own: u8, holder: u8) -> bool {
    match own {
        1 | 3 => true,
        2 => false,
        _ => matches!(holder, 1 | 3),
    }
}

/// Calls `visit` with the key, the instance's root, the declaration and whether it is retained, for
/// every variable of an instance that `prepared`'s globals and programs hold, in the order of
/// specification 11 §6.7: the globals in declaration order, then the programs in registration order,
/// each one's variables in declaration order; an instance's own variables in declaration order, an
/// instance a variable holds where that variable lies, then its base's variables. A variable that
/// holds a reference, an interface or an instance is never retained.
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] for roots that do not form the tree construction admits,
/// and the error of `visit`.
pub(super) fn instance_members(
    prepared: &PreparedModule,
    mut visit: impl FnMut(&str, u32, u32, bool) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    let roots = &prepared.roots.entries;
    // The instances each variable of an instance holds, in index order, and each instance's base.
    let mut held: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
    let mut bases: BTreeMap<u32, u32> = BTreeMap::new();
    for (index, root) in roots.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
        let Some(parent) = root.parent_root_idx else {
            continue;
        };
        if root.is_inheritance_parent() {
            bases.insert(parent, index);
        } else {
            held.entry((parent, root.declaration_idx))
                .or_default()
                .push(index);
        }
    }
    let walk = Walk {
        prepared,
        held: &held,
        bases: &bases,
    };
    // The globals, in declaration order: a global that is an instance, or that holds instances in
    // its elements.
    for (index, root) in roots.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
        let declaration = prepared
            .storage_declaration(root.declaration_idx as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if root.parent_root_idx.is_some() || declaration.role == StorageRole::ProgramRoot {
            continue;
        }
        if root.template_pou_id.is_some() {
            walk.instance(index, None, declaration.retain, &mut visit)?;
        }
        for &element in held
            .get(&(index, root.declaration_idx))
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            walk.instance(element, None, declaration.retain, &mut visit)?;
        }
    }
    // The programs, in registration order: the instances their variables hold.
    for (index, root) in roots.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
        let declaration = prepared
            .storage_declaration(root.declaration_idx as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if root.parent_root_idx.is_some() || declaration.role != StorageRole::ProgramRoot {
            continue;
        }
        let program = prepared
            .vm
            .strings
            .get(declaration.name_idx as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        for &id in prepared.declarations(StorageOwner::Instance, root.template_pou_id) {
            let member = prepared.storage_declaration_at(id as usize);
            for &instance in held.get(&(index, id)).map(Vec::as_slice).unwrap_or(&[]) {
                walk.instance(instance, Some(program), member.retain, &mut visit)?;
            }
        }
    }
    Ok(())
}

/// The roots of one resource, with the instances each variable holds and each instance's base.
struct Walk<'p> {
    prepared: &'p PreparedModule,
    held: &'p BTreeMap<(u32, u32), Vec<u32>>,
    bases: &'p BTreeMap<u32, u32>,
}

impl Walk<'_> {
    /// The variables of the instance at `root`, which a program named `program` holds (or a
    /// global, without one), under a holding declaration with the retention code `holder`.
    fn instance(
        &self,
        root: u32,
        program: Option<&str>,
        holder: u8,
        visit: &mut impl FnMut(&str, u32, u32, bool) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let record = prepared
            .roots
            .entries
            .get(root as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let key = self.key(record, program)?;
        for &id in prepared.declarations(StorageOwner::Instance, record.template_pou_id) {
            let declaration = prepared.storage_declaration_at(id as usize);
            if !matches!(
                declaration.role,
                StorageRole::Variable | StorageRole::Parameter
            ) {
                continue;
            }
            if let Some(instances) = self.held.get(&(root, id)) {
                // A variable that is an instance follows its own type's declarations; its own
                // qualifier is its instances' holder (IEC 61131-3 §6.5.6.2).
                for &instance in instances {
                    self.instance(instance, program, declaration.retain, visit)?;
                }
                continue;
            }
            let name = prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let holds_leaves = declaration.type_id.is_none_or(|ty| {
                prepared
                    .type_layout(ty)
                    .is_none_or(|layout| layout.holds_reference_or_instance)
            });
            let member_key = format!("{key}.{name}");
            visit(
                &member_key,
                root,
                id,
                !holds_leaves && retained(declaration.retain, holder),
            )?;
        }
        if let Some(&base) = self.bases.get(&root) {
            self.instance(base, program, holder, visit)?;
        }
        Ok(())
    }

    /// The checkpoint key of the instance at `record`: its declared path, under
    /// `@program/<program>/` for an instance a program holds.
    fn key(
        &self,
        record: &ConstructionRoot,
        program: Option<&str>,
    ) -> Result<String, RuntimeError> {
        let prepared = self.prepared;
        // An inheritance parent has no binding of its own: it is its instance's base part.
        let mut named = record;
        for _ in 0..=prepared.roots.entries.len() {
            if named.binding_ref_idx.is_some() {
                break;
            }
            named = named
                .parent_root_idx
                .and_then(|parent| prepared.roots.entries.get(parent as usize))
                .ok_or(RuntimeError::InvalidExecutionState)?;
        }
        let binding = named
            .binding_ref_idx
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let path = prepared.persistent_reference_name(binding)?;
        Ok(match program {
            Some(program) => {
                let within = path
                    .strip_prefix(program)
                    .and_then(|rest| rest.strip_prefix('.'))
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                format!("@program/{program}/{within}")
            }
            None => path.to_string(),
        })
    }
}

impl EngineState<'_> {
    /// Lists the retained variables of the resource's instances once, after construction, so that
    /// a capture walks the list and allocates nothing; the list is charged with the construction.
    pub(super) fn list_retained_members(&mut self) -> Result<(), RuntimeError> {
        let mut members = Vec::new();
        instance_members(self.prepared, |key, root, declaration, retained| {
            if retained {
                members.try_reserve(1).map_err(|_| RuntimeError::Overflow)?;
                members.push(RetainedMember {
                    key: SmolStr::new(key),
                    root,
                    declaration,
                });
            }
            Ok(())
        })?;
        let bytes = members.iter().try_fold(
            members.len() * core::mem::size_of::<RetainedMember>(),
            |total, member| total.checked_add(member.key.len()),
        );
        self.charge_allocation_bytes(bytes.ok_or(RuntimeError::Overflow)?)?;
        self.data.construction.retained_members = members;
        Ok(())
    }
}
