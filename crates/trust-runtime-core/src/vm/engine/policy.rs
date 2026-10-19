//! Runtime checks for mutable references and initialization visibility.
use super::store_access::Routed;
use super::*;
use crate::bytecode::{InitializerBodyKind, StorageOwner, StorageRole};
use crate::memory::view::{StoreAccess, ValueView};
use crate::memory::MemoryLocation;
use crate::value::ValueRefView;
use crate::vm::context::ExecutionContext;

/// The visited-instance set of one lifetime check. Its storage is reserved for the live instance
/// count and cleared, not reallocated, per check (spec 34 MEM-09, census I10). Growth is charged
/// against the capacity a set growing on demand would have, so reusing it changes no charge
/// (spec 34 TIME-08).
#[derive(Debug, Default)]
pub(in crate::vm::engine) struct LifetimeVisited {
    ids: Vec<u32>,
    charged_capacity: usize,
}

impl LifetimeVisited {
    /// Empty the set for the next check without releasing its reserved storage.
    pub(in crate::vm::engine) fn reset(&mut self) {
        self.ids.clear();
        self.charged_capacity = 0;
    }

    /// Reserve room for `instances` visited identities. Called once when the live instance count
    /// grows, never during a check of an unchanged program (spec 34 MEM-09).
    pub(in crate::vm::engine) fn reserve(&mut self, instances: usize) -> Result<(), RuntimeError> {
        if instances > self.ids.capacity() {
            self.ids
                .try_reserve_exact(instances - self.ids.len())
                .map_err(|_| RuntimeError::PreparationLimit)?;
        }
        Ok(())
    }
}

/// The two visited-instance sets a lifetime check may use: an `InstanceSet` that grows on demand,
/// and the reserved [`LifetimeVisited`]. Both charge identical growth (spec 34 TIME-08).
pub(in crate::vm::engine) trait VisitedInstances {
    fn visited_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError>;
    fn visited_insert(&mut self, id: InstanceId) -> Result<bool, RuntimeError>;
}

impl VisitedInstances for InstanceSet {
    fn visited_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError> {
        self.insertion_demand(id)
    }
    fn visited_insert(&mut self, id: InstanceId) -> Result<bool, RuntimeError> {
        self.insert(id)
    }
}

impl VisitedInstances for LifetimeVisited {
    fn visited_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError> {
        // Mirror `InstanceSet`'s sorted growth against the capacity a set growing on demand would
        // have; the work component is bookkeeping and is never charged, so only bytes matter.
        let count = self.ids.len();
        let comparisons = if count <= 1 {
            1
        } else {
            (count - 1).ilog2() as usize + 2
        };
        let search = 4 * comparisons;
        let index = match self.ids.binary_search(&id.0) {
            Ok(_) => return Ok((0, search)),
            Err(index) => index,
        };
        let bytes = if count < self.charged_capacity {
            0
        } else {
            self.charged_capacity
                .checked_mul(2)
                .ok_or(RuntimeError::Overflow)?
                .max(4)
                .checked_mul(core::mem::size_of::<u32>())
                .ok_or(RuntimeError::Overflow)?
        };
        let work = search
            .checked_add(count - index)
            .ok_or(RuntimeError::Overflow)?;
        Ok((bytes, work))
    }

    fn visited_insert(&mut self, id: InstanceId) -> Result<bool, RuntimeError> {
        match self.ids.binary_search(&id.0) {
            Ok(_) => Ok(false),
            Err(index) => {
                if self.ids.len() == self.charged_capacity {
                    let capacity = self
                        .charged_capacity
                        .checked_mul(2)
                        .ok_or(RuntimeError::Overflow)?
                        .max(4);
                    self.ids
                        .try_reserve_exact(capacity - self.ids.len())
                        .map_err(|_| RuntimeError::Overflow)?;
                    self.charged_capacity = capacity;
                }
                self.ids.insert(index, id.0);
                Ok(true)
            }
        }
    }
}

/// Arrays are homogeneous: an empty array, or one whose first element is a primitive
/// scalar, cannot hold a reference or instance, so its lifetime check visits nothing.
fn array_holds_only_primitives(array: &crate::value::ArrayValue) -> bool {
    array.elements().first().is_none_or(|first| {
        !matches!(
            first,
            Value::Reference(_)
                | Value::Instance(_)
                | Value::Array(_)
                | Value::Struct(_)
                | Value::Null
        )
    })
}

impl EngineState<'_> {
    pub(super) fn check_entry_deadline(&self) -> Result<(), RuntimeError> {
        self.check_execution_deadline()
    }
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(in crate::vm::engine) fn charge_work_units(
        &self,
        units: usize,
    ) -> Result<(), RuntimeError> {
        // Empty paths do not consume fuel or advance the deadline stride.
        if units == 0 {
            return Ok(());
        }
        self.charge_execution_work(units)
    }

    /// Whether `check_value_lifetime` admits `value` without visiting anything or
    /// charging. Exhaustive, so a new value kind must be classified here too.
    /// Arrays are excluded even when they hold only primitives.
    pub(in crate::vm::engine) fn lifetime_check_is_trivial(value: &Value) -> bool {
        match value {
            Value::Array(_) | Value::Struct(_) | Value::Reference(Some(_)) | Value::Instance(_) => {
                false
            }
            Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Real(_)
            | Value::LReal(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
            | Value::String(_)
            | Value::WString(_)
            | Value::Char(_)
            | Value::WChar(_)
            | Value::Enum(_)
            | Value::Reference(None)
            | Value::Null => true,
        }
    }

    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(in crate::vm::engine) fn check_value_lifetime(
        &self,
        value: &Value,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        // TIME-08 LIFETIME: scalars, and arrays of primitive scalars, visit nothing
        // that can hold a reference, so they are admitted without a charge.
        match value {
            Value::Array(array) if array_holds_only_primitives(array) => Ok(()),
            Value::Reference(Some(_)) | Value::Instance(_) | Value::Array(_) | Value::Struct(_) => {
                let visited = self
                    .data
                    .resources
                    .bookkeeping
                    .lifetime_visited
                    .borrow_mut();
                let mut visited = visited;
                visited.reset();
                self.check_value_lifetime_inner(value, destination, 0, &mut *visited)
            }
            Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Real(_)
            | Value::LReal(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
            | Value::String(_)
            | Value::WString(_)
            | Value::Char(_)
            | Value::WChar(_)
            | Value::Enum(_)
            | Value::Reference(None)
            | Value::Null => Ok(()),
        }
    }

    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(in crate::vm::engine) fn check_value_lifetime_inner<V: VisitedInstances>(
        &self,
        value: &Value,
        destination: Option<FrameId>,
        depth: usize,
        visited: &mut V,
    ) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match value {
            Value::Reference(Some(reference)) => {
                // A reference of another generation is never stored, so every reference
                // the PLC holds belongs to the installed generation (spec 34 §11.3).
                self.check_generation(reference.generation)?;
                if let MemoryLocation::Local(source) = reference.location {
                    self.check_local_lifetime(source, destination)?;
                }
                if let MemoryLocation::Instance(instance) = reference.location {
                    self.check_instance_lifetime(instance, destination)?;
                }
            }
            Value::Instance(instance) => {
                self.check_instance_lifetime(*instance, destination)?;
                self.charge_sorted_growth(visited.visited_demand(*instance))?;
                if visited.visited_insert(*instance)? {
                    self.check_instance_members_lifetime(*instance, destination, depth, visited)?;
                }
            }
            Value::Array(array) if !array_holds_only_primitives(array) => {
                for value in array.elements() {
                    self.check_value_lifetime_inner(value, destination, depth + 1, visited)?;
                }
            }
            Value::Struct(value) => {
                for value in value.fields().values() {
                    self.check_value_lifetime_inner(value, destination, depth + 1, visited)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The lifetime check of the parent and the members of the instance `instance`, which the check of
    /// an instance visits once (TIME-08: one unit per node visited). The `Value` store walks the record it
    /// holds; the cell store walks the members as views, in declaration order (C1-9d2b-1).
    fn check_instance_members_lifetime<V: VisitedInstances>(
        &self,
        instance: InstanceId,
        destination: Option<FrameId>,
        depth: usize,
        visited: &mut V,
    ) -> Result<(), RuntimeError> {
        let Routed::Values(store) = self.data.routed(self.prepared) else {
            return self.check_cell_instance_lifetime(instance, destination, depth, visited);
        };
        let data = store
            .get_instance(instance)
            .ok_or_else(|| RuntimeError::NullReference)?;
        if let Some(parent) = data.parent {
            self.check_value_lifetime_inner(
                &Value::Instance(parent),
                destination,
                depth + 1,
                visited,
            )?;
        }
        for value in data.variables.values() {
            self.check_value_lifetime_inner(value, destination, depth + 1, visited)?;
        }
        Ok(())
    }

    /// The parent and the members of `instance` on the cell store, each checked as the `Value` store
    /// checks the value it holds.
    fn check_cell_instance_lifetime<V: VisitedInstances>(
        &self,
        instance: InstanceId,
        destination: Option<FrameId>,
        depth: usize,
        visited: &mut V,
    ) -> Result<(), RuntimeError> {
        let members = self
            .instance_member_count(instance)
            .ok_or_else(|| RuntimeError::NullReference)?;
        if let Some(parent) = self
            .instance_parent(instance)
            .map_err(|_| RuntimeError::NullReference)?
        {
            self.check_value_lifetime_inner(
                &Value::Instance(parent),
                destination,
                depth + 1,
                visited,
            )?;
        }
        for ordinal in 0..members {
            let view = self
                .instance_member_view(instance, ordinal)
                .ok_or_else(|| RuntimeError::NullReference)?;
            self.check_view_lifetime(view, destination, depth + 1, visited)?;
        }
        Ok(())
    }

    /// The lifetime check of a stored view, with the nodes `check_value_lifetime_inner` visits for the
    /// `Value` it names: one unit for the node, then a reference, an instance, the members of a
    /// structure and the elements of an array that is not of primitives (C1-9d2b-1).
    fn check_view_lifetime<V: VisitedInstances>(
        &self,
        view: ValueView<'_>,
        destination: Option<FrameId>,
        depth: usize,
        visited: &mut V,
    ) -> Result<(), RuntimeError> {
        if let ValueView::Value(value) = view {
            return self.check_value_lifetime_inner(value, destination, depth, visited);
        }
        self.charge_work_units(1)?;
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        if let Some(reference) = view.cell_reference()? {
            self.check_generation(reference.generation)?;
            if let MemoryLocation::Local(source) = reference.location {
                self.check_local_lifetime(source, destination)?;
            }
            if let MemoryLocation::Instance(instance) = reference.location {
                self.check_instance_lifetime(instance, destination)?;
            }
            return Ok(());
        }
        if let Some(instance) = view.instance()? {
            self.check_instance_lifetime(instance, destination)?;
            self.charge_sorted_growth(visited.visited_demand(instance))?;
            if visited.visited_insert(instance)? {
                self.check_instance_members_lifetime(instance, destination, depth, visited)?;
            }
            return Ok(());
        }
        let Some(count) = view.cell_child_count()? else {
            return Ok(());
        };
        if view.cell_array_of_primitives() {
            return Ok(());
        }
        for ordinal in 0..count {
            let child = view.cell_child(ordinal)?;
            self.check_view_lifetime(child, destination, depth + 1, visited)?;
        }
        Ok(())
    }

    /// Refuse a reference taken in another installation generation: a restart or
    /// replacement since then reconstructed the state it named (spec 34 §11.3).
    pub(in crate::vm::engine) fn check_generation(
        &self,
        generation: crate::memory::Generation,
    ) -> Result<(), RuntimeError> {
        if generation != StoreAccess::generation(self) {
            return Err(RuntimeError::ReferenceLifetime);
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_local_lifetime(
        &self,
        source: FrameId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if !self.data.storage.execution_activation_is_live(source) {
            return Err(RuntimeError::ReferenceLifetime);
        }
        let Some(destination) =
            destination.filter(|id| self.data.storage.execution_activation_is_live(*id))
        else {
            return Err(RuntimeError::ReferenceLifetime);
        };
        // Identities increase within a top-level entry and no activation outlives its
        // entry, so among live activations identity order is ancestry order.
        if source.0 > destination.0 {
            return Err(RuntimeError::ReferenceLifetime);
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_instance_lifetime(
        &self,
        instance: InstanceId,
        destination: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if self.instance_type_name(instance).is_none() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        if let Some(Some(owner)) = self.data.lifetimes.instance_lifetimes.get(&instance) {
            self.check_local_lifetime(owner, destination)?;
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn check_write(
        &self,
        reference: ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        let destination = self.check_write_target(reference)?;
        self.check_value_lifetime(value, destination)
    }

    /// The checks of a write that do not look at the value written: the reference's generation, the
    /// declaration's mutability and an initializer's staging. Answers the frame whose lifetime the
    /// destination has, against which the value's references are checked.
    pub(in crate::vm::engine) fn check_write_target(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<Option<FrameId>, RuntimeError> {
        self.check_generation(reference.generation)?;
        self.check_writable_declaration(reference)?;
        let destination = match reference.location {
            MemoryLocation::Local(id) => Some(id),
            MemoryLocation::Instance(id) => {
                self.data.lifetimes.instance_lifetimes.get(&id).flatten()
            }
            _ => None,
        };
        if let Some(initializer) = self.data.construction.initializers.last() {
            let staging = reference.location == MemoryLocation::Local(initializer.result)
                || matches!(reference.location, MemoryLocation::Instance(id) if self.data.lifetimes.instance_lifetimes.get(&id) == Some(Some(initializer.result)) || initializer.writable_instances.contains(&id));
            if !staging {
                return Err(RuntimeError::StagingViolation);
            }
        }
        Ok(destination)
    }

    pub(in crate::vm::engine) fn check_read(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        self.check_generation(reference.generation)?;
        if let MemoryLocation::Local(id) = reference.location
            && !self.data.storage.execution_frame_is_live(id)
        {
            return Err(RuntimeError::ReferenceLifetime);
        }
        let Some(active) = self.data.construction.initializers.last() else {
            return Ok(());
        };
        if reference.location == MemoryLocation::Local(active.result) {
            return Ok(());
        }
        // A flat reference is checked as the variable and path that hold its target (C1-11d).
        if let MemoryLocation::Flat { .. } = reference.location {
            let owned = ValueRef {
                location: reference.location,
                offset: u32::try_from(reference.offset).map_err(|_| RuntimeError::Overflow)?,
                generation: reference.generation,
                path: reference.path.into(),
            };
            let named = self.path_form(&owned);
            if matches!(named.location, MemoryLocation::Flat { .. }) {
                return Err(RuntimeError::VisibilityViolation);
            }
            return self.check_read(named.as_view());
        }
        match reference.location {
            MemoryLocation::Local(id) => {
                if active.entry.body_kind != InitializerBodyKind::Action
                    || Some(id) != active.lexical_frame
                    || reference.offset >= active.entry.visible_local_count as usize
                {
                    return Err(RuntimeError::VisibilityViolation);
                }
            }
            MemoryLocation::Global => {
                if let Some((index, declaration)) =
                    self.prepared
                        .declaration(StorageOwner::Global, None, reference.offset)
                    && declaration.role == StorageRole::Static
                {
                    if active.entry.body_kind != InitializerBodyKind::Action
                        || declaration.owner_pou_id != active.entry.owner_pou_id
                    {
                        return Err(RuntimeError::VisibilityViolation);
                    }
                    let ordinal = self
                        .prepared
                        .static_ordinal(index)
                        .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                    if ordinal >= active.entry.visible_static_count as usize {
                        return Err(RuntimeError::VisibilityViolation);
                    }
                }
            }
            MemoryLocation::Instance(instance) => {
                let template = self
                    .data
                    .construction
                    .instance_templates
                    .get(&instance)
                    .copied()
                    .ok_or_else(|| RuntimeError::VisibilityViolation)?;
                if let Some((index, declaration)) = self.prepared.declaration(
                    StorageOwner::Instance,
                    Some(template),
                    reference.offset,
                ) {
                    if declaration.role == StorageRole::Static {
                        if active.entry.body_kind != InitializerBodyKind::Action
                            || declaration.owner_pou_id != active.entry.owner_pou_id
                        {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                        let ordinal = self
                            .prepared
                            .static_ordinal(index)
                            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                        if ordinal >= active.entry.visible_static_count as usize {
                            return Err(RuntimeError::VisibilityViolation);
                        }
                    }
                    let marker = (index, Some(instance));
                    if !self
                        .data
                        .construction
                        .initialized_declarations
                        .contains(&marker)?
                    {
                        return Err(RuntimeError::VisibilityViolation);
                    }
                }
            }
            MemoryLocation::Io(_) | MemoryLocation::Retain | MemoryLocation::Flat { .. } => {
                return Err(RuntimeError::VisibilityViolation)
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "policy/lifetime_tests.rs"]
mod lifetime_tests;
#[cfg(test)]
#[path = "policy/work_allowance_lifetime_tests.rs"]
mod work_allowance_lifetime_tests;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn normalize_reference_value(
        &self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let Some(type_id) = self.reference_type(reference)? else {
            return Err(RuntimeError::TypeMismatch);
        };
        self.normalize_assignment_value(type_id, value)
    }

    pub(in crate::vm::engine) fn check_typed_value(
        &self,
        type_id: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        if self.assignment_is_canonical(type_id, value, 0)? {
            return Ok(());
        }
        self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
        self.normalize_assignment_value(type_id, value.clone())
            .map(|_| ())
    }

    pub(in crate::vm::engine) fn check_identity_value(
        &self,
        type_id: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        use crate::bytecode::TypeData;
        let type_id =
            super::super::type_policy::resolved_alias_type(&self.prepared.vm.types, type_id, 0)
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
        let entry = self
            .prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or_else(|| RuntimeError::TypeMismatch)?;
        match (&entry.data, value) {
            (TypeData::Pou { pou_id }, Value::Instance(instance)) => {
                let mut actual = *self
                    .data
                    .construction
                    .instance_templates
                    .get(instance)
                    .ok_or_else(|| RuntimeError::NullReference)?;
                for _ in 0..=self.prepared.vm.pou_by_id.len() {
                    if actual == *pou_id {
                        return Ok(());
                    }
                    let Some(parent) = self.prepared.vm.parent_pou_ids.get(&actual) else {
                        break;
                    };
                    actual = *parent;
                }
                Err(RuntimeError::TypeMismatch)
            }
            (TypeData::Interface { .. }, Value::Instance(instance)) => {
                let mut actual = *self
                    .data
                    .construction
                    .instance_templates
                    .get(instance)
                    .ok_or_else(|| RuntimeError::NullReference)?;
                for _ in 0..=self.prepared.vm.pou_by_id.len() {
                    if self
                        .prepared
                        .vm
                        .interface_type_ids_by_pou
                        .get(&actual)
                        .is_some_and(|interfaces| interfaces.contains(&type_id))
                    {
                        return Ok(());
                    }
                    let Some(parent) = self.prepared.vm.parent_pou_ids.get(&actual) else {
                        break;
                    };
                    actual = *parent;
                }
                Err(RuntimeError::TypeMismatch)
            }
            (TypeData::Interface { .. }, Value::Null)
            | (TypeData::Reference { .. }, Value::Null | Value::Reference(None)) => Ok(()),
            (TypeData::Reference { target_type_id }, Value::Reference(Some(reference))) => {
                let actual = self
                    .reference_type(reference.as_view())?
                    .and_then(|id| {
                        super::super::type_policy::resolved_alias_type(
                            &self.prepared.vm.types,
                            id,
                            0,
                        )
                    })
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
                let target = super::super::type_policy::resolved_alias_type(
                    &self.prepared.vm.types,
                    *target_type_id,
                    0,
                )
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
                if super::super::reference_attempt::runtime_type_is_compatible(
                    &self.prepared.vm,
                    actual,
                    target,
                )? {
                    Ok(())
                } else {
                    Err(RuntimeError::TypeMismatch)
                }
            }
            (TypeData::Pou { .. } | TypeData::Interface { .. } | TypeData::Reference { .. }, _) => {
                Err(RuntimeError::TypeMismatch)
            }
            _ => Ok(()),
        }
    }
}
