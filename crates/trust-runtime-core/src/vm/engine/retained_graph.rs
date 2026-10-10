//! Transactional transfer of live retained globals into replacement storage.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole, TypeData};
use crate::memory::MemoryLocation;
use alloc::sync::Arc;

struct RetainedGraph<'s, 'p> {
    source: &'s EngineState<'p>,
    identities: BTreeMap<InstanceId, InstanceId>,
    copied: BTreeSet<InstanceId>,
    initialized: BTreeMap<InstanceId, Vec<u32>>,
    once: BTreeMap<InstanceId, Vec<u32>>,
    root_indices: BTreeMap<InstanceId, Vec<usize>>,
}

// A B-tree node scans a bounded number of keys at each logarithmic level.
fn tree_work(len: usize) -> usize {
    12 * (len.max(1).ilog2() as usize + 1)
}

fn index_marks(
    target: &EngineState<'_>,
    marks: &BTreeSet<(u32, Option<InstanceId>)>,
) -> Result<BTreeMap<InstanceId, Vec<u32>>, RuntimeError> {
    let mut index: BTreeMap<InstanceId, Vec<u32>> = BTreeMap::new();
    for &(declaration, owner) in marks {
        target.charge_work_units(1)?;
        let Some(owner) = owner else {
            continue;
        };
        target.charge_work_units(tree_work(index.len()))?;
        target.charge_allocation_bytes(
            core::mem::size_of::<(InstanceId, Vec<u32>)>() * 4 + 2 * core::mem::size_of::<u32>(),
        )?;
        let declarations = index.entry(owner).or_default();
        declarations
            .try_reserve(1)
            .map_err(|_| RuntimeError::Overflow)?;
        declarations.push(declaration);
    }
    Ok(index)
}

impl EngineState<'_> {
    pub(in crate::vm::engine) fn import_retained_globals(
        &mut self,
        source: &EngineState<'_>,
    ) -> Result<(), RuntimeError> {
        let mut graph = RetainedGraph {
            source,
            identities: BTreeMap::new(),
            copied: BTreeSet::new(),
            initialized: index_marks(self, &source.construction.initialized_declarations)?,
            once: index_marks(self, &source.construction.once)?,
            root_indices: BTreeMap::new(),
        };
        for (index, (old, new)) in source
            .construction
            .roots
            .iter()
            .zip(&self.construction.roots)
            .enumerate()
        {
            self.charge_work_units(1)?;
            if let (Some(old), Some(new)) = (old, new) {
                self.charge_allocation_bytes(core::mem::size_of::<(InstanceId, InstanceId)>() * 4)?;
                self.charge_work_units(tree_work(graph.identities.len()))?;
                graph.identities.insert(*old, *new);
                self.charge_work_units(tree_work(graph.root_indices.len()))?;
                self.charge_allocation_bytes(
                    core::mem::size_of::<(InstanceId, Vec<usize>)>() * 4
                        + 2 * core::mem::size_of::<usize>(),
                )?;
                let indices = graph.root_indices.entry(*new).or_default();
                indices.try_reserve(1).map_err(|_| RuntimeError::Overflow)?;
                indices.push(index);
            }
        }
        for (index, declaration) in source.prepared.layout.entries.iter().enumerate() {
            self.charge_work_units(1)?;
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::Variable
                || !declaration.is_retained()
            {
                continue;
            }
            let name = &source.prepared.vm.strings[declaration.name_idx as usize];
            let value = source
                .storage
                .get_global(name)
                .ok_or(RuntimeError::NullReference)?;
            let value = graph.value(self, declaration.type_id, value, 0)?;
            self.charge_allocation_bytes(self.value_clone_charge(&value, 0)?)?;
            self.storage.set_global(name.clone(), value.clone());
            self.charge_work_units(
                tree_work(self.construction.retained_globals.len())
                    + tree_work(self.construction.initialized_declarations.len()),
            )?;
            self.charge_allocation_bytes(core::mem::size_of::<(u32, Value)>() * 8)?;
            self.construction
                .retained_globals
                .insert(index as u32, value);
            self.construction
                .initialized_declarations
                .insert((index as u32, None));
        }
        Ok(())
    }
}

impl RetainedGraph<'_, '_> {
    fn step(target: &EngineState<'_>, depth: usize) -> Result<(), RuntimeError> {
        target.charge_work_units(1)?;
        if depth >= target.prepared.limits.max_call_depth.min(128) {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        Ok(())
    }

    fn value(
        &mut self,
        target: &mut EngineState<'_>,
        type_id: Option<u32>,
        value: &Value,
        depth: usize,
    ) -> Result<Value, RuntimeError> {
        Self::step(target, depth)?;
        target.charge_allocation_bytes(target.value_clone_charge(value, depth)?)?;
        let source = self.source;
        let resolved_type = type_id.and_then(|id| {
            super::super::type_policy::resolved_alias_type(&source.prepared.vm.types, id, 0)
        });
        let ty = resolved_type.and_then(|id| source.prepared.vm.types.entries.get(id as usize));
        let mut value = value.clone();
        match &mut value {
            Value::Instance(id)
                if ty.is_some_and(|entry| matches!(entry.data, TypeData::Interface { .. })) =>
            {
                *id = self.binding(target, *id, depth + 1)?;
            }
            Value::Instance(id) => *id = self.instance(target, *id, depth + 1)?,
            Value::Reference(Some(reference)) => self.reference(target, reference, depth + 1)?,
            Value::Array(array) => {
                let element_type = match ty.map(|entry| &entry.data) {
                    Some(TypeData::Array { elem_type_id, .. }) => Some(*elem_type_id),
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                for element in array.elements_mut() {
                    *element = self.value(target, element_type, element, depth + 1)?;
                }
            }
            Value::Struct(structure) => {
                // Arc clone alone is cheap, but remapping must never mutate source state.
                target.charge_allocation_bytes(
                    structure
                        .fields()
                        .len()
                        .checked_mul(core::mem::size_of::<Value>() * 4)
                        .ok_or(RuntimeError::Overflow)?,
                )?;
                for field in structure.fields().values() {
                    target.charge_allocation_bytes(target.value_clone_charge(field, depth + 1)?)?;
                }
                let names = structure.fields().keys().cloned().collect::<Vec<_>>();
                for name in names {
                    let old = structure.field(&name).ok_or(RuntimeError::NullReference)?;
                    let type_id = resolved_type.ok_or(RuntimeError::TypeMismatch)?;
                    target.charge_work_units(source.prepared.name_lookup_work(&name))?;
                    let field_type = source
                        .prepared
                        .type_member(type_id, &name)
                        .ok_or(RuntimeError::TypeMismatch)?;
                    let mapped = self.value(target, Some(field_type), old, depth + 1)?;
                    Arc::make_mut(structure).set_existing_field(name, mapped);
                }
            }
            _ => {}
        }
        Ok(value)
    }

    fn instance(
        &mut self,
        target: &mut EngineState<'_>,
        old: InstanceId,
        depth: usize,
    ) -> Result<InstanceId, RuntimeError> {
        Self::step(target, depth)?;
        target.charge_work_units(
            tree_work(self.source.lifetimes.instance_lifetimes.len())
                + tree_work(self.source.construction.instance_templates.len()),
        )?;
        if self
            .source
            .lifetimes
            .instance_lifetimes
            .get(&old)
            .is_some_and(Option::is_some)
        {
            return Err(RuntimeError::ReferenceLifetime);
        }
        let source = self.source;
        let data = source
            .storage
            .get_instance(old)
            .ok_or(RuntimeError::NullReference)?;
        target
            .charge_work_units(tree_work(self.identities.len()) + tree_work(self.copied.len()))?;
        let new = if let Some(new) = self.identities.get(&old) {
            *new
        } else {
            let pou = *self
                .source
                .construction
                .instance_templates
                .get(&old)
                .ok_or(RuntimeError::NullReference)?;
            let new = target.reserve_instance(pou, None)?;
            target.charge_allocation_bytes(core::mem::size_of::<(InstanceId, InstanceId)>() * 4)?;
            target.charge_work_units(tree_work(self.identities.len()))?;
            self.identities.insert(old, new);
            new
        };
        if self.copied.contains(&old) {
            return Ok(new);
        }
        target.charge_allocation_bytes(core::mem::size_of::<InstanceId>() * 4)?;
        target.charge_work_units(tree_work(self.copied.len()))?;
        self.copied.insert(old);
        target.charge_allocation_bytes(
            data.variables
                .len()
                .checked_mul(core::mem::size_of::<Value>() * 4)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        // Reserve every field before recursion so cycles can bind native/private slots too.
        for name in data.variables.keys() {
            if target.storage.get_instance_var(new, name).is_none() {
                target
                    .storage
                    .set_instance_var(new, name.clone(), Value::Null);
            }
        }
        let parent = data.parent;
        let names = data.variables.keys().cloned().collect::<Vec<_>>();
        for name in names {
            let old_value = source
                .storage
                .get_instance_var(old, &name)
                .ok_or(RuntimeError::NullReference)?;
            // Type metadata is identical in the mapped candidate. Charge its
            // fresh restart budget, never the source state's previous scan fuel.
            let ty = match target.storage.ref_for_instance(new, &name) {
                Some(reference) => target.reference_type(reference.as_view())?,
                None => None,
            };
            let value = self.value(target, ty, old_value, depth + 1)?;
            if !target.storage.set_instance_var(new, name, value) {
                return Err(RuntimeError::NullReference);
            }
        }
        let parent = parent
            .map(|parent| self.instance(target, parent, depth + 1))
            .transpose()?;
        target
            .storage
            .get_instance_mut(new)
            .ok_or(RuntimeError::NullReference)?
            .parent = parent;
        target.charge_work_units(tree_work(target.construction.initialized_instances.len()))?;
        target.charge_allocation_bytes(core::mem::size_of::<InstanceId>() * 4)?;
        target.construction.initialized_instances.insert(new);
        target.charge_work_units(tree_work(self.initialized.len()))?;
        if let Some(declarations) = self.initialized.get(&old) {
            for &declaration in declarations {
                target.charge_work_units(
                    1 + tree_work(target.construction.initialized_declarations.len()),
                )?;
                target.charge_allocation_bytes(
                    core::mem::size_of::<(u32, Option<InstanceId>)>() * 4,
                )?;
                target
                    .construction
                    .initialized_declarations
                    .insert((declaration, Some(new)));
            }
        }
        target.charge_work_units(tree_work(self.once.len()))?;
        if let Some(declarations) = self.once.get(&old) {
            for &declaration in declarations {
                target.charge_work_units(1 + tree_work(target.construction.once.len()))?;
                target.charge_allocation_bytes(
                    core::mem::size_of::<(u32, Option<InstanceId>)>() * 4,
                )?;
                target.construction.once.insert((declaration, Some(new)));
            }
        }
        target.charge_work_units(tree_work(self.root_indices.len()))?;
        if let Some(indices) = self.root_indices.get(&new) {
            for &index in indices {
                target.charge_work_units(1 + tree_work(target.construction.claimed_roots.len()))?;
                target.charge_allocation_bytes(core::mem::size_of::<usize>() * 4)?;
                target.construction.claimed_roots.insert(index);
            }
        }
        Ok(new)
    }

    fn binding(
        &mut self,
        target: &mut EngineState<'_>,
        old: InstanceId,
        depth: usize,
    ) -> Result<InstanceId, RuntimeError> {
        Self::step(target, depth)?;
        target.charge_work_units(tree_work(self.identities.len()))?;
        match self.identities.get(&old) {
            Some(new) => Ok(*new),
            // A persistent dynamic object has no artifact root to reconstruct.
            // Keep it alive if its remaining retained link is a reference/interface.
            None => self.instance(target, old, depth + 1),
        }
    }

    fn reference(
        &mut self,
        target: &mut EngineState<'_>,
        reference: &mut ValueRef,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        Self::step(target, depth)?;
        match reference.location {
            MemoryLocation::Global => {
                let (name, _) = self
                    .source
                    .storage
                    .globals()
                    .get_index(reference.offset)
                    .ok_or(RuntimeError::NullReference)?;
                reference.offset = target
                    .storage
                    .ref_for_global(name)
                    .ok_or(RuntimeError::NullReference)?
                    .offset;
            }
            MemoryLocation::Instance(old) => {
                let data = self
                    .source
                    .storage
                    .get_instance(old)
                    .ok_or(RuntimeError::NullReference)?;
                let name = data
                    .variables
                    .get_index(reference.offset)
                    .ok_or(RuntimeError::NullReference)?
                    .0
                    .clone();
                // A reference to an artifact root preserves binding, not ownership.
                let new = self.binding(target, old, depth + 1)?;
                let mapped = target
                    .storage
                    .ref_for_instance(new, &name)
                    .ok_or(RuntimeError::NullReference)?;
                reference.location = mapped.location;
                reference.offset = mapped.offset;
            }
            MemoryLocation::Io(_) | MemoryLocation::Local(_) | MemoryLocation::Retain => {
                return Err(RuntimeError::ReferenceLifetime)
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_object_type_queries_consume_candidate_fuel_not_old_scan_fuel() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let source = EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let mut target =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let Some(Value::Instance(program)) = source.storage.get_global("Plant") else {
            panic!("program")
        };
        let Some(Value::Instance(counter)) = source.storage.get_instance_var(*program, "counter")
        else {
            panic!("counter")
        };
        source.resources.work_budget.reset(0);
        let before = target.resources.work_budget.remaining();
        let mut graph = RetainedGraph {
            source: &source,
            identities: BTreeMap::new(),
            copied: BTreeSet::new(),
            initialized: BTreeMap::new(),
            once: BTreeMap::new(),
            root_indices: BTreeMap::new(),
        };
        let copied = graph.instance(&mut target, *counter, 0).unwrap();
        assert_eq!(
            target.storage.get_instance_var(copied, "value"),
            Some(&Value::Int(3))
        );
        assert!(target.resources.work_budget.remaining() < before);
        assert_eq!(source.resources.work_budget.remaining(), 0);
    }
}
