//! Retained members of function-block and class instances (IEC 61131-3 Ed.3 §6.5.6.1 and
//! §6.5.6.2, specification 10 §4.8). An instance declared `RETAIN` keeps all its variables, its
//! base part's included, except those its type declares `NON_RETAIN`. A variable that is itself
//! an instance (or an array of instances) is kept as a whole when it is declared `RETAIN`, or
//! when it is not qualified and its enclosing instance is kept as a whole; one declared
//! `NON_RETAIN` is not. Inside every instance, a variable its type declares `RETAIN` (or
//! `PERSISTENT`), whether a local, an input or an output, is kept, and one it declares
//! `NON_RETAIN` starts over.
//!
//! Checkpoint keys name the instance by the declaration that holds it and the member below it:
//! `@program-instance/<program>/<variable>/<member path>` and `@global-instance/<global>/<member
//! path>`. An element of an array of instances adds its subscript (`Pumps[2]`, `Grid[1,0]`). A
//! base part's members stand at the derived instance's level: IEC 61131-3 Ed.3 §6.6.5.3 forbids
//! redeclaring an inherited name, so member paths stay unique.
use super::*;
use crate::bytecode::{StorageDeclaration, StorageOwner, StorageRole, TypeData, TypeKind};
use crate::retain::{value_is_retainable, RetainSnapshot};
use alloc::format;
use alloc::string::String;
use core::fmt::Write as _;
use smol_str::SmolStr;

const PROGRAM_INSTANCE_PREFIX: &str = "@program-instance/";
const GLOBAL_INSTANCE_PREFIX: &str = "@global-instance/";
/// The `NON_RETAIN` code of a storage declaration's retention policy.
const NON_RETAIN: u8 = 2;
/// The `VAR_IN_OUT` flag of a storage declaration: it refers to a variable held elsewhere.
const IN_OUT: u8 = 8;

/// Where a checkpoint key's member lands in this installation: the instance part that declares
/// it, its stored name, and its declaration.
pub(super) struct MemberDestination {
    pub(super) part: InstanceId,
    pub(super) name: SmolStr,
    pub(super) declaration: u32,
}

impl EngineState<'_> {
    /// Adds the retained members of every instance reached from a global or a program
    /// variable of `source` to `snapshot`, charging this state's allocation allowance.
    pub(super) fn capture_retained_instance_members(
        &self,
        source: &EngineState<'_>,
        snapshot: &mut RetainSnapshot,
    ) -> Result<(), RuntimeError> {
        let prepared = source.prepared;
        for declaration in &prepared.layout.entries {
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::Variable
                || !source.holds_instances(declaration)
            {
                continue;
            }
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            let Some(value) = source.data.storage.get_global(name) else {
                continue;
            };
            for (subscript, id) in held_instances(value)? {
                let prefix = format!("{GLOBAL_INSTANCE_PREFIX}{name}{subscript}/");
                self.capture_instance(source, id, declaration.is_retained(), &prefix, snapshot, 0)?;
            }
        }
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let root_declaration = &prepared.layout.entries[root.declaration_idx as usize];
            if root.parent_root_idx.is_some() || root_declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let Some(program_instance) = source.data.construction.roots[index] else {
                continue;
            };
            let program = &prepared.vm.strings[root_declaration.name_idx as usize];
            for &id in prepared.declarations(StorageOwner::Instance, root.template_pou_id) {
                let declaration = &prepared.layout.entries[id as usize];
                if declaration.role != StorageRole::Variable || !source.holds_instances(declaration)
                {
                    continue;
                }
                let variable = &prepared.vm.strings[declaration.name_idx as usize];
                let Some(value) = source
                    .data
                    .storage
                    .get_instance_var(program_instance, variable)
                else {
                    continue;
                };
                for (subscript, member) in held_instances(value)? {
                    let prefix =
                        format!("{PROGRAM_INSTANCE_PREFIX}{program}/{variable}{subscript}/");
                    self.capture_instance(
                        source,
                        member,
                        declaration.is_retained(),
                        &prefix,
                        snapshot,
                        0,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn capture_instance(
        &self,
        source: &EngineState<'_>,
        id: InstanceId,
        whole: bool,
        prefix: &str,
        snapshot: &mut RetainSnapshot,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let Some(instance) = source.data.storage.get_instance(id) else {
            return Ok(());
        };
        let pou = source.instance_pou(id, &instance.type_name);
        for (name, value) in &instance.variables {
            let declaration = pou.and_then(|pou| source.member_declaration(pou, name));
            // A member that holds instances leads on with its own qualifier; an interface or a
            // reference only refers to an instance held elsewhere.
            if let Some((_, declared)) =
                declaration.filter(|(_, declared)| source.holds_instances(declared))
            {
                for (subscript, nested) in held_instances(value)? {
                    let nested_prefix = format!("{prefix}{name}{subscript}.");
                    self.capture_instance(
                        source,
                        nested,
                        inherited(declared, whole),
                        &nested_prefix,
                        snapshot,
                        depth + 1,
                    )?;
                }
                continue;
            }
            if !kept(declaration.map(|(_, declaration)| declaration), whole)
                || !value_is_retainable(value)
            {
                continue;
            }
            source.check_retention_walk(value, 0)?;
            let key_len = prefix
                .len()
                .checked_add(name.len())
                .ok_or(RuntimeError::Overflow)?;
            let bytes = self
                .value_clone_charge(value, 0)?
                .checked_add(key_len)
                .and_then(|bytes| bytes.checked_add(4 * core::mem::size_of::<(SmolStr, Value)>()))
                .ok_or(RuntimeError::Overflow)?;
            self.charge_allocation_bytes(bytes)?;
            let key = format!("{prefix}{name}");
            if snapshot.values().contains_key(key.as_str()) {
                return Err(RuntimeError::RetainMigration(
                    "ambiguous retained checkpoint label".into(),
                ));
            }
            snapshot.insert(key, value.clone());
        }
        if let Some(parent) = instance.parent {
            self.capture_instance(source, parent, whole, prefix, snapshot, depth + 1)?;
        }
        Ok(())
    }

    /// Carries the retained instance members of `source` into this candidate, as a warm
    /// restart or an online change of the same identity graph does.
    pub(super) fn restore_retained_instance_members(
        &mut self,
        source: &EngineState<'_>,
    ) -> Result<(), RuntimeError> {
        let mut members = RetainSnapshot::default();
        self.capture_retained_instance_members(source, &mut members)?;
        if members.values().is_empty() {
            return Ok(());
        }
        self.apply_retained_snapshot(&members).map(|_| ())
    }

    /// A warm restart of the running runtime carries the instances of each `RETAIN` global over
    /// whole, so references and aliases to them stay valid. This starts over every variable in
    /// them that the rules above do not keep, from its declaration in its type.
    pub(in crate::vm::engine) fn reinitialize_unkept_retained_global_members(
        &mut self,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let mut unkept = Vec::new();
        for declaration in &prepared.layout.entries {
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::Variable
                || !declaration.is_retained()
                || !self.holds_instances(declaration)
            {
                continue;
            }
            let name = &prepared.vm.strings[declaration.name_idx as usize];
            let Some(value) = self.data.storage.get_global(name) else {
                continue;
            };
            for (_, id) in held_instances(value)? {
                self.collect_unkept(id, true, &mut unkept, 0)?;
            }
        }
        for (part, declaration) in unkept {
            self.reinitialize_member(part, declaration)?;
        }
        Ok(())
    }

    /// Lists the members of an instance tree that are not kept: each as the part that holds it
    /// and its declaration.
    fn collect_unkept(
        &self,
        id: InstanceId,
        whole: bool,
        unkept: &mut Vec<(InstanceId, u32)>,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let Some(instance) = self.data.storage.get_instance(id) else {
            return Ok(());
        };
        let Some(pou) = self.instance_pou(id, &instance.type_name) else {
            return Ok(());
        };
        let prepared = self.prepared;
        for &declaration_id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let declaration = &prepared.layout.entries[declaration_id as usize];
            if declaration.owner_pou_id != Some(pou) || declaration.flags & IN_OUT != 0 {
                continue;
            }
            match declaration.role {
                StorageRole::Variable | StorageRole::Parameter => {}
                // An edge's memory follows its input's qualifier; native state follows the
                // instance.
                StorageRole::EdgePhase | StorageRole::NativeState => {
                    if !kept(Some(declaration), whole) {
                        unkept.push((id, declaration_id));
                    }
                    continue;
                }
                _ => continue,
            }
            if self.holds_instances(declaration) {
                let name = &prepared.vm.strings[declaration.name_idx as usize];
                if let Some(value) = self.data.storage.get_instance_var(id, name) {
                    for (_, nested) in held_instances(value)? {
                        self.collect_unkept(
                            nested,
                            inherited(declaration, whole),
                            unkept,
                            depth + 1,
                        )?;
                    }
                }
            } else if !kept(Some(declaration), whole) {
                unkept.push((id, declaration_id));
            }
        }
        if let Some(parent) = instance.parent {
            self.collect_unkept(parent, whole, unkept, depth + 1)?;
        }
        Ok(())
    }

    /// Runs a member's declaration actions in its part again; a member without one takes its
    /// declared default, or its type's initial value.
    fn reinitialize_member(
        &mut self,
        part: InstanceId,
        declaration_id: u32,
    ) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let declaration = &prepared.layout.entries[declaration_id as usize];
        let pou = declaration
            .owner_pou_id
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let mut ran = false;
        for &action in prepared.instance_actions(pou) {
            if prepared.initializers.entries[action as usize].declaration_idx
                == Some(declaration_id)
            {
                self.run_declaration_action(action, Some(part), None, 0)?;
                ran = true;
            }
        }
        if ran {
            return Ok(());
        }
        let value = match declaration
            .default_const_idx
            .and_then(|index| prepared.vm.consts.get(index as usize))
        {
            Some(value) => {
                self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
                value.clone()
            }
            None if declaration.role == StorageRole::NativeState => Value::Null,
            None => {
                let ty = declaration
                    .type_id
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                super::super::construction::values::construct_intrinsic_value(
                    &prepared.vm.types,
                    self,
                    ty,
                )?
            }
        };
        let name = prepared.vm.strings[declaration.name_idx as usize].clone();
        if !self.data.storage.set_instance_var(part, name, value) {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(())
    }

    /// Finds where a retained instance-member key lands in this installation, and whether the
    /// member is retained here; `None` for any other key, or when the program no longer has the
    /// member or no longer retains it.
    pub(super) fn instance_member_destination(&self, key: &str) -> Option<MemberDestination> {
        let prepared = self.prepared;
        let (root, whole, path) = if let Some(rest) = key.strip_prefix(PROGRAM_INSTANCE_PREFIX) {
            let (program, rest) = rest.split_once('/')?;
            let (variable, path) = rest.split_once('/')?;
            let (variable, subscript) = split_subscript(variable)?;
            let (index, root) = prepared
                .roots
                .entries
                .iter()
                .enumerate()
                .find(|(_, root)| {
                    root.parent_root_idx.is_none() && {
                        let declaration = &prepared.layout.entries[root.declaration_idx as usize];
                        declaration.role == StorageRole::ProgramRoot
                            && prepared.vm.strings[declaration.name_idx as usize]
                                .eq_ignore_ascii_case(program)
                    }
                })?;
            let program_instance = self.data.construction.roots[index]?;
            let declaration = prepared
                .declarations(StorageOwner::Instance, root.template_pou_id)
                .iter()
                .map(|&id| &prepared.layout.entries[id as usize])
                .find(|declaration| {
                    declaration.role == StorageRole::Variable
                        && prepared.vm.strings[declaration.name_idx as usize]
                            .eq_ignore_ascii_case(variable)
                })
                .filter(|declaration| self.holds_instances(declaration))?;
            let stored = &prepared.vm.strings[declaration.name_idx as usize];
            let value = self
                .data
                .storage
                .get_instance_var(program_instance, stored)?;
            (
                held_instance(value, subscript.as_deref())?,
                declaration.is_retained(),
                path,
            )
        } else {
            let rest = key.strip_prefix(GLOBAL_INSTANCE_PREFIX)?;
            let (global, path) = rest.split_once('/')?;
            let (global, subscript) = split_subscript(global)?;
            let declaration = prepared
                .layout
                .entries
                .iter()
                .find(|declaration| {
                    declaration.owner == StorageOwner::Global
                        && declaration.role == StorageRole::Variable
                        && prepared.vm.strings[declaration.name_idx as usize]
                            .eq_ignore_ascii_case(global)
                })
                .filter(|declaration| self.holds_instances(declaration))?;
            let stored = &prepared.vm.strings[declaration.name_idx as usize];
            let value = self.data.storage.get_global(stored)?;
            (
                held_instance(value, subscript.as_deref())?,
                declaration.is_retained(),
                path,
            )
        };
        let mut segments = path.split('.').peekable();
        let mut current = root;
        let mut whole = whole;
        for depth in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            let (segment, subscript) = split_subscript(segments.next()?)?;
            if segment.is_empty() {
                return None;
            }
            let (part, name) = self.declaring_part(current, segment, depth)?;
            let instance = self.data.storage.get_instance(part)?;
            let pou = self.instance_pou(part, &instance.type_name)?;
            let (id, declaration) = self.member_declaration(pou, &name)?;
            let last = segments.peek().is_none();
            if self.holds_instances(declaration) {
                // A path ends at a variable; only a member that holds its instances leads on.
                if last {
                    return None;
                }
                let value = self.data.storage.get_instance_var(part, &name)?;
                current = held_instance(value, subscript.as_deref())?;
                whole = inherited(declaration, whole);
                continue;
            }
            if !last || subscript.is_some() {
                return None;
            }
            return kept(Some(declaration), whole).then_some(MemberDestination {
                part,
                name,
                declaration: id,
            });
        }
        None
    }

    /// The part of an instance that holds `name` (the instance itself or one of its base
    /// parts) and the member's stored name.
    fn declaring_part(
        &self,
        id: InstanceId,
        name: &str,
        depth: usize,
    ) -> Option<(InstanceId, SmolStr)> {
        let mut current = Some(id);
        for _ in depth..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            let part = current?;
            let instance = self.data.storage.get_instance(part)?;
            if let Some(stored) = instance
                .variables
                .keys()
                .find(|stored| stored.eq_ignore_ascii_case(name))
            {
                return Some((part, stored.clone()));
            }
            current = instance.parent;
        }
        None
    }

    /// Whether a declaration's type holds function-block or class instances: one, or an array
    /// of them, as opposed to an interface or reference that refers to one held elsewhere.
    fn holds_instances(&self, declaration: &StorageDeclaration) -> bool {
        let entries = &self.prepared.vm.types.entries;
        let mut ty = declaration.type_id;
        let mut element = false;
        for _ in 0..=crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            let Some(entry) = ty.and_then(|ty| entries.get(ty as usize)) else {
                return false;
            };
            match entry.data {
                TypeData::Alias { target_type_id } => ty = Some(target_type_id),
                TypeData::Array { elem_type_id, .. } if !element => {
                    element = true;
                    ty = Some(elem_type_id);
                }
                TypeData::Pou { .. } => {
                    return matches!(entry.kind, TypeKind::FunctionBlock | TypeKind::Class)
                }
                _ => return false,
            }
        }
        false
    }

    /// The POU that templates a live instance part.
    fn instance_pou(&self, id: InstanceId, type_name: &str) -> Option<u32> {
        if let Some(pou) = self.data.construction.instance_templates.get(&id) {
            return Some(*pou);
        }
        let key = SmolStr::new(type_name.to_ascii_uppercase());
        let module = &self.prepared.vm;
        module
            .function_block_ids
            .get(&key)
            .or_else(|| module.class_ids.get(&key))
            .copied()
    }

    /// A POU's own declaration of an instance member.
    fn member_declaration(&self, pou: u32, name: &str) -> Option<(u32, &StorageDeclaration)> {
        let prepared = self.prepared;
        prepared
            .declarations(StorageOwner::Instance, Some(pou))
            .iter()
            .map(|&id| (id, &prepared.layout.entries[id as usize]))
            .find(|(_, declaration)| {
                prepared.vm.strings[declaration.name_idx as usize].eq_ignore_ascii_case(name)
            })
    }
}

/// Whether a member with this declaration is kept: its type declares it `RETAIN` or
/// `PERSISTENT`, or its instance is kept as a whole and the type does not declare it
/// `NON_RETAIN`. A member without a declaration has no declared type to restore it through, so
/// it is not kept.
fn kept(declaration: Option<&StorageDeclaration>, whole: bool) -> bool {
    declaration.is_some_and(|declaration| {
        declaration.is_retained() || (whole && declaration.retain != NON_RETAIN)
    })
}

/// Whether the instances a member holds are kept as a whole: exactly when a member with that
/// declaration would be kept (IEC 61131-3 Ed.3 §6.5.6.2).
fn inherited(declaration: &StorageDeclaration, whole: bool) -> bool {
    kept(Some(declaration), whole)
}

/// The instances a value holds, each with the subscript that names it: empty for one instance,
/// `[1,2]` for an element of an array of instances.
fn held_instances(value: &Value) -> Result<Vec<(String, InstanceId)>, RuntimeError> {
    let mut held = Vec::new();
    match value {
        Value::Instance(id) => held.push((String::new(), *id)),
        Value::Array(array) => {
            for (offset, element) in array.elements().iter().enumerate() {
                if let Value::Instance(id) = element {
                    let subscript =
                        subscript(array.dimensions(), offset).ok_or(RuntimeError::Overflow)?;
                    held.push((subscript, *id));
                }
            }
        }
        _ => {}
    }
    Ok(held)
}

/// The instance a value holds at `subscript`: the value itself without one, else the element.
fn held_instance(value: &Value, subscript: Option<&[i64]>) -> Option<InstanceId> {
    let element = match (value, subscript) {
        (Value::Instance(id), None) => return Some(*id),
        (Value::Array(array), Some(indices)) => {
            array.elements().get(offset(array.dimensions(), indices)?)?
        }
        _ => return None,
    };
    match element {
        Value::Instance(id) => Some(*id),
        _ => None,
    }
}

/// The subscript of the element at row-major `offset` of an array with these dimensions.
fn subscript(dimensions: &[(i64, i64)], offset: usize) -> Option<String> {
    let mut indices = alloc::vec![0i64; dimensions.len()];
    let mut rest = offset;
    for (at, (lower, upper)) in dimensions.iter().enumerate().rev() {
        let length = usize::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
        if length == 0 {
            return None;
        }
        indices[at] = lower.checked_add(i64::try_from(rest % length).ok()?)?;
        rest /= length;
    }
    let mut text = String::from("[");
    for (at, index) in indices.iter().enumerate() {
        if at > 0 {
            text.push(',');
        }
        write!(text, "{index}").ok()?;
    }
    text.push(']');
    Some(text)
}

/// The row-major offset of the element at `indices`, when every index is within its bounds.
fn offset(dimensions: &[(i64, i64)], indices: &[i64]) -> Option<usize> {
    if dimensions.len() != indices.len() {
        return None;
    }
    let mut offset = 0usize;
    for ((lower, upper), index) in dimensions.iter().zip(indices) {
        if index < lower || index > upper {
            return None;
        }
        let length = usize::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
        let position = usize::try_from(index.checked_sub(*lower)?).ok()?;
        offset = offset.checked_mul(length)?.checked_add(position)?;
    }
    Some(offset)
}

/// Splits a path segment into its name and its subscript, if it has one: `Pumps[2]` gives
/// `("Pumps", Some([2]))`.
fn split_subscript(segment: &str) -> Option<(&str, Option<Vec<i64>>)> {
    let Some((name, rest)) = segment.split_once('[') else {
        return Some((segment, None));
    };
    let inside = rest.strip_suffix(']')?;
    let indices = inside
        .split(',')
        .map(|index| index.trim().parse::<i64>().ok())
        .collect::<Option<Vec<_>>>()?;
    Some((name, Some(indices)))
}

/// Whether a checkpoint key names a retained instance member.
pub(super) fn is_instance_member_key(key: &str) -> bool {
    key.starts_with(PROGRAM_INSTANCE_PREFIX) || key.starts_with(GLOBAL_INSTANCE_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscripts_name_elements_in_row_major_order_and_round_trip() {
        // ARRAY[1..2, 0..2]: the last dimension varies fastest, so offset 4 is [2,1].
        let dimensions = [(1, 2), (0, 2)];
        assert_eq!(subscript(&dimensions, 0).as_deref(), Some("[1,0]"));
        assert_eq!(subscript(&dimensions, 4).as_deref(), Some("[2,1]"));
        assert_eq!(offset(&dimensions, &[2, 1]), Some(4));
        assert_eq!(offset(&dimensions, &[3, 0]), None, "outside the bounds");
        assert_eq!(offset(&dimensions, &[1]), None, "too few indices");
        assert_eq!(
            split_subscript("Pumps[-1]"),
            Some(("Pumps", Some(alloc::vec![-1])))
        );
        assert_eq!(split_subscript("Total"), Some(("Total", None)));
        assert_eq!(split_subscript("Pumps[x]"), None);
    }
}
