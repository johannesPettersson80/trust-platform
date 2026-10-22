//! Retained-name resolution from the admitted artifact alone (specification 11, "Live Values";
//! specification 34 §4.3.1).
//!
//! A retained label is a name the control surface may write or force: a retained global, or a
//! retained member of a program instance, spelled `@program/<program>/<member>`. The labels are
//! built once from the prepared program and resolved against live storage on use, so no cached
//! instance identity can survive a restart or a replacement. The engine's admitted declarations
//! are the only input, so a chip resolves the same labels as trust-runtime without the compiler.

use alloc::format;
use alloc::vec::Vec;

use indexmap::Equivalent;
use smol_str::SmolStr;
use trust_runtime_core::bytecode::{StorageOwner, StorageRole};
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{InstanceId, VariableStorage};
use trust_runtime_core::value::{Value, ValueRef};
use trust_runtime_core::vm::{CycleAccess, PreparedModule};

/// A retained management label: a retained global's name, or `@program/<program>/<member>`.
#[derive(Debug, Clone)]
pub struct RetainedLabel(
    /// The reported label text.
    pub SmolStr,
);

/// Where a retained label lives: the global it names, or the program root and member of one.
#[derive(Debug, Clone)]
pub struct RetainedBinding {
    /// The program root the member belongs to; `None` for a retained global.
    pub root: Option<SmolStr>,
    /// The retained global or member name.
    pub name: SmolStr,
}

/// The retained labels of one prepared program, in declaration order. The key is the reported
/// label and the value says where it resolves (specification 11, "Live Values").
pub type RetainedNames = OrderedMap<RetainedLabel, RetainedBinding>;

/// A borrowed lookup key, compared without regard to ASCII case and hashed like its label, so a
/// lookup allocates neither a label nor a name (specification 11, "Live Values").
struct LabelQuery<'a>(&'a str);

fn hash_label(name: &str, state: &mut impl core::hash::Hasher) {
    for byte in name.bytes() {
        state.write_u8(byte.to_ascii_uppercase());
    }
    state.write_u8(0xff);
}

impl core::hash::Hash for RetainedLabel {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        hash_label(&self.0, state);
    }
}

impl PartialEq for RetainedLabel {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for RetainedLabel {}

impl core::hash::Hash for LabelQuery<'_> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        hash_label(self.0, state);
    }
}

impl Equivalent<RetainedLabel> for LabelQuery<'_> {
    fn equivalent(&self, key: &RetainedLabel) -> bool {
        self.0.eq_ignore_ascii_case(&key.0)
    }
}

/// The store lookups a retained label resolves through. Inside a scan they are the owner's current
/// values ([`CycleAccess`]); outside one a [`VariableStorage`] serves them.
pub trait RetainedLookup {
    /// The instance a declared resource global holds, when it holds one.
    fn global_instance(&self, name: &str) -> Option<InstanceId>;
    /// A reference to a declared resource global.
    fn ref_for_global(&self, name: &str) -> Option<ValueRef>;
    /// A reference to a declared member of one instance.
    fn ref_for_instance(&self, id: InstanceId, name: &str) -> Option<ValueRef>;
}

impl RetainedLookup for VariableStorage {
    fn global_instance(&self, name: &str) -> Option<InstanceId> {
        match self.get_global(name)? {
            Value::Instance(id) => Some(*id),
            _ => None,
        }
    }

    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        VariableStorage::ref_for_global(self, name)
    }

    fn ref_for_instance(&self, id: InstanceId, name: &str) -> Option<ValueRef> {
        VariableStorage::ref_for_instance(self, id, name)
    }
}

impl RetainedLookup for CycleAccess<'_, '_> {
    fn global_instance(&self, name: &str) -> Option<InstanceId> {
        CycleAccess::global_instance(self, name)
    }

    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        CycleAccess::ref_for_global(self, name)
    }

    fn ref_for_instance(&self, id: InstanceId, name: &str) -> Option<ValueRef> {
        CycleAccess::ref_for_instance(self, id, name)
    }
}

impl RetainedBinding {
    fn resolve(&self, lookup: &impl RetainedLookup) -> Option<ValueRef> {
        match &self.root {
            Some(root) => lookup.ref_for_instance(lookup.global_instance(root)?, &self.name),
            None => lookup.ref_for_global(&self.name),
        }
    }
}

/// Build the retained labels of `prepared`, in declaration order: the retained globals first,
/// then each program root's retained members.
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] when an admitted index is out of range, and
/// [`RuntimeError::InvalidConfig`] when two declarations project to the same label.
pub fn bind_retained_declarations(
    prepared: &PreparedModule,
) -> Result<RetainedNames, RuntimeError> {
    let mut bindings = RetainedNames::default();
    let name = |id: u32| {
        prepared
            .strings()
            .get(id as usize)
            .map(SmolStr::new)
            .ok_or(RuntimeError::InvalidExecutionState)
    };
    for declaration in prepared.storage_declarations() {
        if declaration.owner == StorageOwner::Global
            && declaration.is_retained()
            && declaration.role != StorageRole::ProgramRoot
            && declaration.owns_storage()
        {
            let name = name(declaration.name_idx)?;
            insert_retained_binding(
                &mut bindings,
                RetainedLabel(name.clone()),
                RetainedBinding { root: None, name },
            )?;
        }
    }
    for root in prepared.construction_roots() {
        let declaration = prepared
            .storage_declaration(root.declaration_idx as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if root.parent_root_idx.is_some() || declaration.role != StorageRole::ProgramRoot {
            continue;
        }
        let program = name(declaration.name_idx)?;
        let template = root
            .template_pou_id
            .ok_or(RuntimeError::InvalidExecutionState)?;
        for member in prepared.instance_declarations(template) {
            if !member.is_retained() || !member.owns_storage() {
                continue;
            }
            let member = name(member.name_idx)?;
            insert_retained_binding(
                &mut bindings,
                RetainedLabel(format!("@program/{program}/{member}").into()),
                RetainedBinding {
                    root: Some(program.clone()),
                    name: member,
                },
            )?;
        }
    }
    Ok(bindings)
}

/// Resolve a declared retained label without allocating or scanning the other declarations. No
/// cached live instance identity survives a restart or a replacement.
#[must_use]
pub fn retained_reference(
    names: &RetainedNames,
    lookup: &impl RetainedLookup,
    name: &str,
) -> Option<ValueRef> {
    names.get(&LabelQuery(name))?.resolve(lookup)
}

/// Preserve the control protocol's exact-name snapshot lookup: the label must match `name` in
/// case as well as value.
#[must_use]
pub fn retained_reference_exact(
    names: &RetainedNames,
    storage: &VariableStorage,
    name: &str,
) -> Option<ValueRef> {
    let (label, binding) = names.get_key_value(&LabelQuery(name))?;
    (label.0.as_str() == name)
        .then(|| binding.resolve(storage))
        .flatten()
}

/// Project retained global and program destinations in stable declaration order. This allocating
/// list is for presentation only, never a single cycle write.
#[must_use]
pub fn retained_references(
    names: &RetainedNames,
    storage: &VariableStorage,
) -> Vec<(SmolStr, ValueRef)> {
    names
        .iter()
        .filter_map(|(label, binding)| {
            binding
                .resolve(storage)
                .map(|reference| (label.0.clone(), reference))
        })
        .collect()
}

fn insert_retained_binding(
    bindings: &mut RetainedNames,
    label: RetainedLabel,
    binding: RetainedBinding,
) -> Result<(), RuntimeError> {
    match bindings.entry(label) {
        indexmap::map::Entry::Vacant(entry) => {
            entry.insert(binding);
            Ok(())
        }
        indexmap::map::Entry::Occupied(entry) => Err(RuntimeError::InvalidConfig(
            format!("ambiguous retained management label '{}'", entry.key().0).into(),
        )),
    }
}
