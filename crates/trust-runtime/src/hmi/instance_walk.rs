//! Finding the instances of a function block in the running program: every instance whose type
//! is the block or extends it, at any depth (specification 40, §5 items 1 and 5; specification
//! 42, §4 item 1).

use crate::hmi::instance_holdings::Holdings;
use crate::hmi::variable_tree::is_listed;
use crate::memory::{InstanceData, InstanceId, VariableStorage};
use crate::value::{array_offset_i64, ArrayValue, Value};

/// Deepest nesting walked; guards against a malformed program.
const MAX_DEPTH: usize = 32;

/// An instance the walk found.
pub(crate) struct Found {
    pub(crate) path: String,
    pub(crate) id: InstanceId,
    /// The function block instance nearest above it that holds it (specification 39, §13.6,
    /// item 9).
    pub(crate) equipment: Option<String>,
}

/// Every instance whose type is `base` or extends it, in declaration order, with its
/// equipment, at most `limit`; `is_program` tells program instances, which are not equipment,
/// from function block instances. A found instance's own members are not searched. The
/// program's instances, structures and arrays of instances are, through the members that hold
/// their instance (`holdings`): each instance is found once, where it is declared.
pub(crate) fn instances_of(
    storage: &VariableStorage,
    holdings: &Holdings<'_>,
    base: &str,
    limit: usize,
    is_program: &dyn Fn(&str) -> bool,
) -> Vec<Found> {
    instances_where(storage, holdings, limit, is_program, &|_, id| {
        base_part(storage, id, base).is_some()
    })
}

/// Every instance `takes` takes, given its path and id, in declaration order, as
/// [`instances_of`] walks them.
pub(crate) fn instances_where(
    storage: &VariableStorage,
    holdings: &Holdings<'_>,
    limit: usize,
    is_program: &dyn Fn(&str) -> bool,
    takes: &dyn Fn(&str, InstanceId) -> bool,
) -> Vec<Found> {
    let mut walk = Walk {
        storage,
        holdings,
        takes,
        limit,
        is_program,
        found: Vec::new(),
    };
    for (name, value) in storage.globals() {
        if is_listed(name, value) && is_container(value) {
            walk.value(name, value, None, 0);
        }
    }
    walk.found
}

/// Only instances, structures and arrays hold instances; paths are built for them alone.
fn is_container(value: &Value) -> bool {
    matches!(
        value,
        Value::Instance(_) | Value::Struct(_) | Value::Array(_)
    )
}

struct Walk<'a> {
    storage: &'a VariableStorage,
    holdings: &'a Holdings<'a>,
    takes: &'a dyn Fn(&str, InstanceId) -> bool,
    limit: usize,
    is_program: &'a dyn Fn(&str) -> bool,
    found: Vec<Found>,
}

impl Walk<'_> {
    /// One value at `path`; `holder` is the function block instance nearest above it.
    fn value(&mut self, path: &str, value: &Value, holder: Option<&str>, depth: usize) {
        if depth > MAX_DEPTH || self.found.len() >= self.limit {
            return;
        }
        match value {
            Value::Instance(id) => self.instance(path, *id, holder, depth),
            Value::Array(array) => self.array(path, array, holder, depth),
            Value::Struct(structure) => {
                for (field, field_value) in structure.fields() {
                    if is_container(field_value) {
                        self.value(&format!("{path}.{field}"), field_value, holder, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }

    fn instance(&mut self, path: &str, id: InstanceId, holder: Option<&str>, depth: usize) {
        if (self.takes)(path, id) {
            self.found.push(Found {
                path: path.to_string(),
                id,
                equipment: holder.map(str::to_string),
            });
            return;
        }
        let storage = self.storage;
        let Some(instance) = storage.get_instance(id) else {
            return;
        };
        // A function block instance holds the instances below it; a program instance does not.
        let holder = if (self.is_program)(instance.type_name.as_str()) {
            holder
        } else {
            Some(path)
        };
        // The instance's own members, then its base parts'.
        let mut part = Some(instance);
        for _ in 0..MAX_DEPTH {
            let Some(current) = part else {
                break;
            };
            for (member, member_value) in &current.variables {
                // An interface, reference or in-out member refers to an instance another holds.
                if is_container(member_value)
                    && is_listed(member, member_value)
                    && self
                        .holdings
                        .holds(current.type_name.as_str(), member.as_str())
                {
                    self.value(&format!("{path}.{member}"), member_value, holder, depth + 1);
                }
            }
            part = current
                .parent
                .and_then(|parent| storage.get_instance(parent));
        }
    }

    /// The elements of an array of instances, structures or arrays, labelled with their declared
    /// indices.
    fn array(&mut self, path: &str, array: &ArrayValue, holder: Option<&str>, depth: usize) {
        if !array.elements().first().is_some_and(is_container) {
            return;
        }
        let dimensions = array.dimensions();
        let mut indices: Vec<i64> = dimensions.iter().map(|(low, _)| *low).collect();
        loop {
            if let Some(element) = array_offset_i64(dimensions, &indices)
                .and_then(|offset| array.elements().get(offset))
            {
                let label = indices
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                self.value(&format!("{path}[{label}]"), element, holder, depth + 1);
            }
            // The next index, the last dimension fastest.
            let mut dimension = indices.len();
            loop {
                if dimension == 0 {
                    return;
                }
                dimension -= 1;
                if indices[dimension] < dimensions[dimension].1 {
                    indices[dimension] += 1;
                    break;
                }
                indices[dimension] = dimensions[dimension].0;
            }
        }
    }
}

/// The `base` part of an instance whose type is, or extends, `base`.
pub(crate) fn base_part<'a>(
    storage: &'a VariableStorage,
    id: InstanceId,
    base: &str,
) -> Option<&'a InstanceData> {
    let mut current = storage.get_instance(id);
    for _ in 0..MAX_DEPTH {
        let instance = current?;
        if instance.type_name.eq_ignore_ascii_case(base) {
            return Some(instance);
        }
        current = instance
            .parent
            .and_then(|parent| storage.get_instance(parent));
    }
    None
}
