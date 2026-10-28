//! Finding the function block instances of a project's declarations that a test takes, at any
//! depth, each where it is declared (specification 39, §8.9 item 2), as the editor offers them.
//! The running program's instances are found through its tables (`program_instances`).

use crate::hmi::instance_holdings::Holdings;
use crate::hmi::variable_tree::is_listed;
use crate::memory::{InstanceId, VariableStorage};
use crate::value::{array_offset_i64, ArrayValue, Value};

/// Deepest nesting walked; guards against a malformed program.
const MAX_DEPTH: usize = 32;

/// The path of every instance `takes` takes, given its path and id, in declaration order, at
/// most `limit`. A taken instance's own members are not searched. The program's instances,
/// structures and arrays of instances are, through the members that hold their instance
/// (`holdings`): each instance is found once, where it is declared.
pub(crate) fn instances_where(
    storage: &VariableStorage,
    holdings: &Holdings<'_>,
    limit: usize,
    takes: &dyn Fn(&str, InstanceId) -> bool,
) -> Vec<String> {
    let mut walk = Walk {
        storage,
        holdings,
        takes,
        limit,
        found: Vec::new(),
    };
    for (name, value) in storage.globals() {
        if is_listed(name, value) && is_container(value) {
            walk.value(name, value, 0);
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
    found: Vec<String>,
}

impl Walk<'_> {
    /// One value at `path`.
    fn value(&mut self, path: &str, value: &Value, depth: usize) {
        if depth > MAX_DEPTH || self.found.len() >= self.limit {
            return;
        }
        match value {
            Value::Instance(id) => self.instance(path, *id, depth),
            Value::Array(array) => self.array(path, array, depth),
            Value::Struct(structure) => {
                for (field, field_value) in structure.fields() {
                    if is_container(field_value) {
                        self.value(&format!("{path}.{field}"), field_value, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }

    fn instance(&mut self, path: &str, id: InstanceId, depth: usize) {
        if (self.takes)(path, id) {
            self.found.push(path.to_string());
            return;
        }
        let storage = self.storage;
        // The instance's own members, then its base parts'.
        let mut part = storage.get_instance(id);
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
                    self.value(&format!("{path}.{member}"), member_value, depth + 1);
                }
            }
            part = current
                .parent
                .and_then(|parent| storage.get_instance(parent));
        }
    }

    /// The elements of an array of instances, structures or arrays, labelled with their declared
    /// indices.
    fn array(&mut self, path: &str, array: &ArrayValue, depth: usize) {
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
                self.value(&format!("{path}[{label}]"), element, depth + 1);
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
