//! Instance variables kept across a warm restart (IEC 61131-3 Ed.3 §6.5.6.1 and §6.5.6.2,
//! specification 10 §4.8). An instance declared `RETAIN` keeps all its variables, its base part's
//! included, except those its type declares `NON_RETAIN`; a variable that is itself an instance
//! follows its own type's declarations; a variable a type declares `RETAIN`, whether a local, an
//! input or an output, is kept in every instance reached.

use indexmap::IndexMap;
use smol_str::SmolStr;

use super::retain_snapshot::{retain_on_warm, value_is_retainable};
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::{ClassDef, FunctionBlockDef};
use crate::value::Value;
use crate::RetainPolicy;

/// Nesting deeper than this is not followed; it guards against a malformed cycle.
const MAX_DEPTH: usize = 32;

/// The kept values of one instance.
#[derive(Debug, Default)]
pub(super) struct InstanceImage {
    values: Vec<(SmolStr, Value)>,
    nested: Vec<(SmolStr, InstanceImage)>,
    base: Option<Box<InstanceImage>>,
}

impl InstanceImage {
    fn is_empty(&self) -> bool {
        self.values.is_empty()
            && self.nested.is_empty()
            && self.base.as_ref().is_none_or(|base| base.is_empty())
    }
}

/// The declared policies of the types' variables.
pub(super) struct Declarations<'a> {
    pub(super) function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: &'a IndexMap<SmolStr, ClassDef>,
}

impl Declarations<'_> {
    /// The policy a type declares for one of its own variables: a local, or a function block's
    /// input or output.
    pub(super) fn declared(&self, type_name: &str, member: &str) -> RetainPolicy {
        let key = SmolStr::new(type_name.to_ascii_uppercase());
        let block = self.function_blocks.get(&key);
        let vars = block
            .map(|block| block.vars.as_slice())
            .or_else(|| self.classes.get(&key).map(|class| class.vars.as_slice()))
            .unwrap_or_default();
        vars.iter()
            .find(|var| var.name.eq_ignore_ascii_case(member))
            .map(|var| var.retain)
            .or_else(|| {
                block?
                    .params
                    .iter()
                    .find(|param| param.name.eq_ignore_ascii_case(member))
                    .map(|param| param.retain)
            })
            .unwrap_or(RetainPolicy::Unspecified)
    }

    /// The declared type of one of a type's own variables, inputs or outputs.
    pub(super) fn declared_type(&self, type_name: &str, member: &str) -> Option<trust_hir::TypeId> {
        let key = SmolStr::new(type_name.to_ascii_uppercase());
        let block = self.function_blocks.get(&key);
        let vars = block
            .map(|block| block.vars.as_slice())
            .or_else(|| self.classes.get(&key).map(|class| class.vars.as_slice()))
            .unwrap_or_default();
        vars.iter()
            .find(|var| var.name.eq_ignore_ascii_case(member))
            .map(|var| var.type_id)
            .or_else(|| {
                block?
                    .params
                    .iter()
                    .find(|param| param.name.eq_ignore_ascii_case(member))
                    .map(|param| param.type_id)
            })
    }
}

/// What an instance keeps: everything for a `RETAIN` instance (`whole`), otherwise only what its
/// types declare `RETAIN`. `None` when it keeps nothing.
pub(super) fn capture(
    storage: &VariableStorage,
    declarations: &Declarations<'_>,
    id: InstanceId,
    whole: bool,
) -> Option<InstanceImage> {
    capture_at(storage, declarations, id, whole, 0).filter(|image| !image.is_empty())
}

fn capture_at(
    storage: &VariableStorage,
    declarations: &Declarations<'_>,
    id: InstanceId,
    whole: bool,
    depth: usize,
) -> Option<InstanceImage> {
    if depth > MAX_DEPTH {
        return None;
    }
    let instance = storage.get_instance(id)?;
    let mut image = InstanceImage::default();
    for (name, value) in &instance.variables {
        if let Value::Instance(nested) = value {
            // A nested instance follows its own type's declarations, not this RETAIN.
            if let Some(kept) = capture_at(storage, declarations, *nested, false, depth + 1) {
                if !kept.is_empty() {
                    image.nested.push((name.clone(), kept));
                }
            }
            continue;
        }
        let declared = declarations.declared(instance.type_name.as_str(), name.as_str());
        let kept = retain_on_warm(declared) || (whole && declared != RetainPolicy::NonRetain);
        if kept && value_is_retainable(value) {
            image.values.push((name.clone(), value.clone()));
        }
    }
    if let Some(parent) = instance.parent {
        image.base = capture_at(storage, declarations, parent, whole, depth + 1).map(Box::new);
    }
    Some(image)
}

/// Writes the kept values into a rebuilt instance of the same type.
pub(super) fn restore(storage: &mut VariableStorage, id: InstanceId, image: &InstanceImage) {
    restore_at(storage, id, image, 0);
}

fn restore_at(storage: &mut VariableStorage, id: InstanceId, image: &InstanceImage, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for (name, value) in &image.values {
        storage.set_instance_var(id, name.clone(), value.clone());
    }
    for (name, kept) in &image.nested {
        if let Some(Value::Instance(nested)) = storage.get_instance_var(id, name.as_str()).cloned()
        {
            restore_at(storage, nested, kept, depth + 1);
        }
    }
    if let (Some(base), Some(parent)) = (
        &image.base,
        storage
            .get_instance(id)
            .and_then(|instance| instance.parent),
    ) {
        restore_at(storage, parent, base, depth + 1);
    }
}

/// The kept values of an image as member paths below its instance (`Settings.Limit`,
/// `Loop.Gain`). A base part's values stand at the derived instance's level: IEC 61131-3 Ed.3
/// §6.6.5.3 forbids redeclaring an inherited name, so the names stay unique.
pub(super) fn flatten(image: &InstanceImage) -> Vec<(String, Value)> {
    let mut paths = Vec::new();
    flatten_into(image, "", &mut paths, 0);
    paths
}

fn flatten_into(
    image: &InstanceImage,
    prefix: &str,
    paths: &mut Vec<(String, Value)>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for (name, value) in &image.values {
        paths.push((format!("{prefix}{name}"), value.clone()));
    }
    for (name, nested) in &image.nested {
        flatten_into(nested, &format!("{prefix}{name}."), paths, depth + 1);
    }
    if let Some(base) = &image.base {
        flatten_into(base, prefix, paths, depth + 1);
    }
}

/// Where a kept member path lands in an instance: the part that declares the variable, the
/// variable's stored name and that part's type.
pub(super) struct Landing {
    pub(super) part: InstanceId,
    pub(super) name: SmolStr,
    pub(super) type_name: SmolStr,
}

/// Finds a member path of [`flatten`] in an instance, walking nested instances and base parts;
/// `None` when the program no longer has that variable.
pub(super) fn locate(storage: &VariableStorage, id: InstanceId, path: &str) -> Option<Landing> {
    let mut segments = path.split('.').peekable();
    let mut current = id;
    let mut depth = 0;
    while let Some(segment) = segments.next() {
        if depth > MAX_DEPTH || segment.is_empty() {
            return None;
        }
        let (part, name, nested) = declaring_part(storage, current, segment)?;
        if segments.peek().is_none() {
            if nested.is_some() {
                return None;
            }
            let type_name = storage.get_instance(part)?.type_name.clone();
            return Some(Landing {
                part,
                name,
                type_name,
            });
        }
        current = nested?;
        depth += 1;
    }
    None
}

/// The part of an instance that holds `name` (the instance itself or one of its base parts), the
/// variable's stored name, and the instance it holds when it is one.
fn declaring_part(
    storage: &VariableStorage,
    id: InstanceId,
    name: &str,
) -> Option<(InstanceId, SmolStr, Option<InstanceId>)> {
    let mut current = Some(id);
    let mut depth = 0;
    while let Some(part) = current {
        if depth > MAX_DEPTH {
            return None;
        }
        let instance = storage.get_instance(part)?;
        if let Some((stored, value)) = instance
            .variables
            .iter()
            .find(|(stored, _)| stored.eq_ignore_ascii_case(name))
        {
            let nested = match value {
                Value::Instance(nested) => Some(*nested),
                _ => None,
            };
            return Some((part, stored.clone(), nested));
        }
        current = instance.parent;
        depth += 1;
    }
    None
}
