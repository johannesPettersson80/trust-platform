//! Atomic retained-value migration using admitted types and initializer recipes.
use super::*;
use crate::bytecode::{InitializationStage, StorageOwner, StorageRole, TypeData};
use crate::collections::OrderedMap;
use crate::memory::layout::CELL_BYTES;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::retain::{value_is_retainable, RetainSnapshot};
use crate::value::{ArrayValue, StructValue};
use crate::vm::construction::values::{
    construct_value, ValueConstructionContext, ValueConstructionMode, ValueOperation,
};
use alloc::{boxed::Box, format, string::String, sync::Arc};
use smol_str::SmolStr;

/// Informational migration observations, emitted only after atomic application.
#[derive(Debug, Clone, PartialEq)]
pub enum RetainApplyEvent {
    /// A saved key no longer names a declared destination.
    Orphan {
        /// Saved symbolic checkpoint key.
        name: SmolStr,
    },
    /// A retained value was converted to the current admitted representation.
    Migrated {
        /// Saved symbolic checkpoint key.
        name: SmolStr,
        /// Original persisted representation.
        before: Value,
        /// Canonical admitted representation.
        after: Value,
    },
}

impl EngineState<'_> {
    pub(super) fn require_serializable_retained_values(&self) -> Result<(), RuntimeError> {
        let check = |value: &Value| -> Result<(), RuntimeError> {
            self.check_retention_walk(value, 0)?;
            if !value_is_retainable(value) {
                return Err(RuntimeError::RetainMigration(
                    "retained live-object graph requires congruent admitted schema".into(),
                ));
            }
            Ok(())
        };
        for &id in self
            .prepared
            .retained_declarations(StorageOwner::Global, None)
        {
            let declaration = &self.prepared.layout.entries[id as usize];
            let name = &self.prepared.vm.strings[declaration.name_idx as usize];
            if let Some(view) = self.read_global(name) {
                check(&view.materialize()?)?;
            }
        }
        for (index, root) in self.prepared.roots.entries.iter().enumerate() {
            if root.parent_root_idx.is_some()
                || self.prepared.layout.entries[root.declaration_idx as usize].role
                    != StorageRole::ProgramRoot
            {
                continue;
            }
            let instance =
                self.data.construction.roots[index].ok_or(RuntimeError::InvalidExecutionState)?;
            for &id in self
                .prepared
                .retained_declarations(StorageOwner::Instance, root.template_pou_id)
            {
                let declaration = &self.prepared.layout.entries[id as usize];
                let name = &self.prepared.vm.strings[declaration.name_idx as usize];
                if let Some(view) = self.read_instance_var(instance, name) {
                    check(&view.materialize()?)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn apply_retained_snapshot(
        &mut self,
        snapshot: &RetainSnapshot,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        let observed = self.data.construction_complete;
        self.data.construction_complete = false;
        let result = self.apply_retained_inner(snapshot);
        self.data.construction_complete = observed;
        result
    }

    /// Build charged lookup indexes once, without mutating retained storage.
    fn retained_migration_indexes(&self) -> Result<RetainedMigrationIndexes, RuntimeError> {
        // One index per management operation. Snapshot order remains observable to
        // later member-default recipes, as in the hosted migration transaction.
        let mut destinations = BTreeMap::new();
        let prepared = self.prepared;
        for (id, declaration) in prepared.layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Global
                || declaration.role == StorageRole::ProgramRoot
            {
                continue;
            }
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            // The destination index entry is bookkeeping (TIME-08) and is not charged. The label
            // comparison is bookkeeping too; its overflow guard stays.
            cost_sum(&[
                name.len(),
                12 * (destinations.len().max(1).ilog2() as usize + 1),
            ])?;
            if destinations
                .insert(name.to_ascii_uppercase(), (id as u32, None))
                .is_some()
            {
                return Err(RuntimeError::RetainMigration(
                    "ambiguous retained destination label".into(),
                ));
            }
        }
        for (root_index, root) in prepared.roots.entries.iter().enumerate() {
            let root_declaration = &prepared.layout.entries[root.declaration_idx as usize];
            if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let instance = self.data.construction.roots[root_index]
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let program = &prepared.vm.strings[root_declaration.name_idx as usize];
            for &id in prepared.declarations(StorageOwner::Instance, root.template_pou_id) {
                let declaration = &prepared.layout.entries[id as usize];
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                // The program-scoped index entry is bookkeeping (TIME-08) and is not charged.
                cost_sum(&[
                    program.len(),
                    name.len(),
                    12 * (destinations.len().max(1).ilog2() as usize + 1),
                ])?;
                if destinations
                    .insert(
                        format!("@program/{program}/{name}").to_ascii_uppercase(),
                        (id, Some(instance)),
                    )
                    .is_some()
                {
                    return Err(RuntimeError::RetainMigration(
                        "ambiguous retained destination label".into(),
                    ));
                }
            }
        }
        let mut actions = BTreeMap::new();
        for (index, entry) in prepared.initializers.entries.iter().enumerate() {
            if entry.stage == InitializationStage::Default
                && let Some(declaration) = entry.declaration_idx
            {
                // The initializer index entry is bookkeeping (TIME-08) and is not charged.
                actions
                    .entry(declaration)
                    .or_insert(entry.context_initializer_idx.unwrap_or(index as u32));
            }
        }
        Ok(RetainedMigrationIndexes {
            destinations,
            actions,
        })
    }

    fn apply_retained_inner(
        &mut self,
        snapshot: &RetainSnapshot,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        let indexes = self.retained_migration_indexes()?;
        let prepared = self.prepared;
        let mut journal: Vec<(Option<InstanceId>, SmolStr, Value)> = Vec::new();
        let mut events = Vec::new();
        // The journal and event vectors are bookkeeping (TIME-08): they are reserved, not charged.
        journal
            .try_reserve_exact(snapshot.values().len())
            .map_err(|_| RuntimeError::Overflow)?;
        events
            .try_reserve_exact(snapshot.values().len())
            .map_err(|_| RuntimeError::Overflow)?;
        let outcome = (|| {
            for (key, value) in snapshot.values() {
                cost_sum(&[
                    indexes.destinations.len().max(1).ilog2() as usize,
                    key.len(),
                    1,
                ])?;
                self.charge_allocation_bytes(key.len())?;
                let Some(&(id, instance)) = indexes.destinations.get(&key.to_ascii_uppercase())
                else {
                    events.push(RetainApplyEvent::Orphan { name: key.clone() });
                    continue;
                };
                let declaration = &prepared.layout.entries[id as usize];
                if !declaration.is_retained() {
                    continue;
                }
                self.check_retention_walk(value, 0)?;
                if !value_is_retainable(value) {
                    continue;
                }
                let ty = declaration
                    .type_id
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                let view = match instance {
                    Some(instance) => self.read_instance_var(instance, name),
                    None => self.read_global(name),
                }
                .ok_or(RuntimeError::InvalidExecutionState)?;
                self.charge_allocation_bytes(self.view_clone_charge(&view)?)?;
                journal.push((instance, name.clone(), view.materialize()?));
                let action = *indexes
                    .actions
                    .get(&id)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let mut context = MigrationContext {
                    engine: self,
                    context: action,
                    instance,
                };
                let migrated = context.migrate(ty, value, 0).map_err(|error| match error {
                    RuntimeError::SubrangeViolation {
                        value,
                        lower,
                        upper,
                    } => RuntimeError::RetainSubrangeViolation {
                        name: key.clone(),
                        value,
                        lower,
                        upper,
                    },
                    RuntimeError::ExecutionTimeout
                    | RuntimeError::ExecutionLimit { .. }
                    | RuntimeError::WatchdogTimeout
                    | RuntimeError::PreparationLimit
                    | RuntimeError::Overflow => error,
                    _ => RuntimeError::RetainMigration(
                        format!("invalid retained value for '{key}': {error}").into(),
                    ),
                })?;
                if &migrated != value {
                    context.engine.charge_allocation_bytes(cost_sum(&[
                        context.engine.value_clone_charge(value, 0)?,
                        context.engine.value_clone_charge(&migrated, 0)?,
                    ])?)?;
                    events.push(RetainApplyEvent::Migrated {
                        name: key.clone(),
                        before: value.clone(),
                        after: migrated.clone(),
                    });
                }
                context.engine.check_entry_deadline()?;
                match instance {
                    Some(instance) => {
                        context
                            .engine
                            .write_instance_var(instance, name.clone(), migrated);
                    }
                    None => {
                        context.engine.set_global(name.clone(), migrated);
                    }
                }
            }
            self.check_entry_deadline()?;
            if journal.is_empty() {
                // No retained value was applied, so no marker variable changed either.
                return Ok(());
            }
            // A retained variable located at %M keeps the restored value, so the marker image the
            // first scan loads must hold it. The publication refuses before it changes an image,
            // so a refusal here still rolls back through the journal (specification 11 6.7).
            self.publish_marker_image()
        })();
        if let Err(error) = outcome {
            for (instance, name, value) in journal.into_iter().rev() {
                match instance {
                    Some(instance) => {
                        self.write_instance_var(instance, name, value);
                    }
                    None => {
                        self.set_global(name, value);
                    }
                }
            }
            return Err(error);
        }
        Ok(events)
    }
}

struct RetainedMigrationIndexes {
    destinations: BTreeMap<String, (u32, Option<InstanceId>)>,
    actions: BTreeMap<u32, u32>,
}

struct MigrationContext<'a, 'p> {
    engine: &'a mut EngineState<'p>,
    context: u32,
    instance: Option<InstanceId>,
}
impl MigrationContext<'_, '_> {
    /// One cell for a copied leaf field or element of a migrated structure or array. A compound
    /// child allocates through its own leaves, so it adds none here (TIME-08 copy allocation).
    fn charge_copied_leaf(&mut self, value: &Value) -> Result<(), RuntimeError> {
        if !matches!(value, Value::Array(_) | Value::Struct(_)) {
            self.engine.charge_allocation_bytes(CELL_BYTES)?;
        }
        Ok(())
    }

    fn migrate(&mut self, ty: u32, value: &Value, depth: usize) -> Result<Value, RuntimeError> {
        self.charge_work(1)?;
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let prepared = self.engine.prepared;
        let entry = prepared
            .vm
            .types
            .entries
            .get(ty as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => self.migrate(*target_type_id, value, depth + 1),
            TypeData::Subrange {
                base_type_id,
                lower,
                upper,
            } => {
                let (base, lower, upper) = (*base_type_id, *lower, *upper);
                let value = self.migrate(base, value, depth + 1)?;
                let n = super::super::construction::values::scalar::integer(&value)
                    .ok_or(RuntimeError::TypeMismatch)?;
                if n < i128::from(lower) || n > i128::from(upper) {
                    return Err(RuntimeError::SubrangeViolation {
                        value: n,
                        lower: i128::from(lower),
                        upper: i128::from(upper),
                    });
                }
                Ok(value)
            }
            TypeData::Array { elem_type_id, dims } => {
                let Value::Array(array) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                if array.dimensions() != dims.as_slice() {
                    return Err(RuntimeError::TypeMismatch);
                }
                let element = *elem_type_id;
                let dims = dims.clone();
                let mut values = Vec::new();
                values
                    .try_reserve_exact(array.elements().len())
                    .map_err(|_| RuntimeError::Overflow)?;
                for value in array.elements() {
                    self.charge_copied_leaf(value)?;
                    values.push(self.migrate(element, value, depth + 1)?);
                }
                Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                    values, dims,
                ))))
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                let Value::Struct(value) = value else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let layout = self.engine.prepared.vm.physical_layout(ty);
                if value.physical_layout().is_some() && layout.is_none() {
                    return Err(RuntimeError::RetainMigration(
                        "retained physical backing requires a compatible physical destination"
                            .into(),
                    ));
                }
                let name = entry
                    .name_idx
                    .map(|id| self.engine.prepared.vm.strings[id as usize].clone())
                    .unwrap_or_default();
                let fields = fields.clone();
                let mut values = OrderedMap::default();
                values
                    .try_reserve(fields.len())
                    .map_err(|_| RuntimeError::Overflow)?;
                for (index, field) in fields.iter().enumerate() {
                    // Field matching is bookkeeping (TIME-08); its overflow guard stays.
                    value
                        .fields()
                        .len()
                        .checked_add(1)
                        .ok_or(RuntimeError::Overflow)?;
                    let name = self.engine.prepared.vm.strings[field.name_idx as usize].clone();
                    let next = if let Some((_, value)) = value
                        .fields()
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(&name))
                    {
                        self.charge_copied_leaf(value)?;
                        self.migrate(field.type_id, value, depth + 1)?
                    } else if let Some(recipe) = self.member_recipe(ty, index as u32) {
                        self.evaluate_recipe(recipe)?
                    } else {
                        let prepared = self.engine.prepared;
                        construct_value(
                            &prepared.vm.types,
                            self,
                            field.type_id,
                            ValueConstructionMode::Default,
                        )?
                    };
                    values.insert(name, next);
                }
                let rebuilt = StructValue::from_canonical_parts(name, values);
                if let Some(layout) = &layout {
                    self.charge_allocation(layout.byte_len()?)?;
                    self.charge_allocation(layout.initialization_staging_allocation(&rebuilt)?)?;
                    self.charge_work(layout.construction_work()?)?;
                }
                // A live snapshot keeps its backing and immutable descriptor.
                // Compare each destination layout, independently of table order;
                // strict attachment refuses incompatible bits before publication.
                let rebuilt = rebuilt.with_physical_layout_preserving(layout, value)?;
                Ok(Value::Struct(Arc::new(rebuilt)))
            }
            TypeData::Primitive { prim_id, .. } => {
                if !scalar_compatible(*prim_id, value) {
                    return Err(RuntimeError::TypeMismatch);
                }
                self.charge_allocation(self.engine.value_clone_charge(value, depth)?)?;
                let prepared = self.engine.prepared;
                construct_value(
                    &prepared.vm.types,
                    self,
                    ty,
                    ValueConstructionMode::Coerce(value.clone()),
                )
            }
            TypeData::Enum { .. } => {
                self.charge_allocation(self.engine.value_clone_charge(value, depth)?)?;
                let prepared = self.engine.prepared;
                construct_value(
                    &prepared.vm.types,
                    self,
                    ty,
                    ValueConstructionMode::Coerce(value.clone()),
                )
            }
            _ => Err(RuntimeError::TypeMismatch),
        }
    }
}

fn scalar_compatible(primitive: u16, value: &Value) -> bool {
    matches!(
        (primitive, value),
        (1, Value::Bool(_))
            | (2..=5, Value::Byte(_))
            | (3..=5, Value::Word(_))
            | (4..=5, Value::DWord(_))
            | (5, Value::LWord(_))
            | (6..=9, Value::SInt(_))
            | (7..=9, Value::Int(_))
            | (8..=9, Value::DInt(_))
            | (9, Value::LInt(_))
            | (10..=13, Value::USInt(_))
            | (11..=13, Value::UInt(_))
            | (12..=13, Value::UDInt(_))
            | (13, Value::ULInt(_))
            | (14..=15, Value::Real(_))
            | (15, Value::LReal(_))
            | (16, Value::Time(_))
            | (17, Value::LTime(_))
            | (18, Value::Date(_))
            | (19, Value::LDate(_))
            | (20, Value::Tod(_))
            | (21, Value::LTod(_))
            | (22, Value::Dt(_))
            | (23, Value::Ldt(_))
            | (24, Value::String(_))
            | (25, Value::WString(_))
            | (26, Value::Char(_))
            | (27, Value::WChar(_))
    )
}

impl ValueConstructionContext for MigrationContext<'_, '_> {
    fn bind_structure_layout_preserving(
        &mut self,
        ty: u32,
        value: StructValue,
        source: &StructValue,
    ) -> Result<StructValue, RuntimeError> {
        self.engine
            .bind_structure_layout_preserving(ty, value, source)
    }
    fn bind_structure_layout(
        &mut self,
        ty: u32,
        value: StructValue,
    ) -> Result<StructValue, RuntimeError> {
        self.engine.bind_structure_layout(ty, value)
    }
    fn initialize_structure_layout(
        &mut self,
        ty: u32,
        value: StructValue,
    ) -> Result<StructValue, RuntimeError> {
        self.engine.initialize_structure_layout(ty, value)
    }
    fn strings(&self) -> &[SmolStr] {
        &self.engine.prepared.vm.strings
    }
    fn profile(&self) -> DateTimeProfile {
        self.engine.data.profile
    }
    fn type_recipe(&self, ty: u32) -> Option<u32> {
        self.engine.prepared.recipe(self.context, ty, None)
    }
    fn member_recipe(&self, ty: u32, member: u32) -> Option<u32> {
        self.engine.prepared.recipe(self.context, ty, Some(member))
    }
    fn evaluate_recipe(&mut self, id: u32) -> Result<Value, RuntimeError> {
        self.engine.evaluate_initializer(id, None, self.instance, 0)
    }
    fn construct_instance(&mut self, _: u32, _: bool) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
    fn charge_value(&mut self) -> Result<(), RuntimeError> {
        self.engine.charge_constructed_value()
    }
    fn charge_work(&mut self, units: usize) -> Result<(), RuntimeError> {
        self.engine.charge_work_units(units)
    }
    fn charge_allocation(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        self.engine.charge_allocation_bytes(bytes)
    }
    fn check_value(&mut self, ty: u32, value: &Value) -> Result<(), RuntimeError> {
        self.engine.check_typed_value(ty, value)
    }
    fn enter_type(&mut self, ty: u32, operation: ValueOperation) -> Result<(), RuntimeError> {
        ValueConstructionContext::enter_type(self.engine, ty, operation)
    }
    fn leave_type(&mut self) {
        ValueConstructionContext::leave_type(self.engine);
    }
}

fn cost_sum(values: &[usize]) -> Result<usize, RuntimeError> {
    values.iter().try_fold(0usize, |total, value| {
        total.checked_add(*value).ok_or(RuntimeError::Overflow)
    })
}
