//! The variable tree offered by the HMI Builder's variable picker (specification 39, §7.1).
//!
//! The tree is built from the compiled program: program instances and globals at the root,
//! function block members and structure fields as children, and each array once, with its bounds
//! and one element as a template. Every node's path resolves through
//! [`member_paths`](super::member_paths).

use serde::Serialize;
use trust_hir::symbols::ParamDirection;

use super::binding_checks::{global_write_block, member_write_block};
use super::catalogue::ValueKind;
use super::member_paths::find_ignoring_case as find;
use crate::debug::dap::value_type_name;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::FunctionBlockBase;
use crate::runtime::RuntimeMetadata;
use crate::value::Value;

mod published;
pub use published::published_variables;

#[cfg(test)]
mod tests;

/// One variable in the tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VariableNode {
    /// Name as declared.
    pub name: String,
    /// Runtime instance path, for example `FeedArea.Pumps[1].Running`.
    pub path: String,
    /// What the variable holds.
    pub kind: VariableKind,
    /// Declared type, for example `REAL`, `Pump` or `ARRAY[1..3] OF Pump`.
    #[serde(rename = "type")]
    pub type_name: String,
    /// Kinds of value the variable holds, as the catalogue names them (§5.6); empty for program
    /// and function block instances.
    pub kinds: Vec<ValueKind>,
    /// Whether a binding may write the variable (§7.6): not a constant, not a function block
    /// output, and not inside one.
    pub writable: bool,
    /// Why a binding cannot write it, in the binding checks' words, for example `is a constant`.
    #[serde(rename = "writeBlock", skip_serializing_if = "Option::is_none")]
    pub write_block: Option<String>,
    /// How a function block member is declared; absent where the program does not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<VariableAccess>,
    /// Function block members or structure fields, in declaration order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<VariableNode>,
    /// Bounds and element template of an array.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array: Option<ArrayShape>,
}

/// What a variable holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VariableKind {
    /// A program instance.
    Program,
    /// A function block instance.
    FunctionBlock,
    /// A structure.
    Structure,
    /// An array.
    Array,
    /// A single value.
    Value,
}

/// How a function block member is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VariableAccess {
    /// `VAR_INPUT`.
    Input,
    /// `VAR_OUTPUT`.
    Output,
    /// `VAR_IN_OUT`.
    InOut,
    /// `VAR`, internal to the block.
    Internal,
}

/// The bounds of an array and one element as a template.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArrayShape {
    /// Lower and upper bound of each dimension.
    pub dimensions: Vec<[i64; 2]>,
    /// The first element, standing for all elements.
    pub element: Box<VariableNode>,
}

/// Builds the tree from the compiled program and its storage.
///
/// The roots are the program instances and globals in declaration order. Runtime-internal
/// storage is not listed, and the walk is bounded so a malformed program cannot exhaust memory.
pub fn variable_tree(
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
) -> Result<Vec<VariableNode>, String> {
    let mut builder = TreeBuilder {
        metadata,
        storage,
        ancestors: Vec::new(),
        visited: 0,
    };
    storage
        .globals()
        .iter()
        .filter(|(name, value)| is_listed(name, value))
        .map(|(name, value)| {
            let block = global_write_block(metadata, name);
            builder.node(name, name.to_string(), value, None, block, 0)
        })
        .collect()
}

/// Deepest nesting walked.
const MAX_DEPTH: usize = 32;
/// Most nodes in one tree.
const MAX_NODES: usize = 65_536;

struct TreeBuilder<'a> {
    metadata: &'a RuntimeMetadata,
    storage: &'a VariableStorage,
    ancestors: Vec<InstanceId>,
    visited: usize,
}

impl TreeBuilder<'_> {
    /// One node; `block` is why the binding checks refuse writes to it or to a container.
    fn node(
        &mut self,
        name: &str,
        path: String,
        value: &Value,
        access: Option<VariableAccess>,
        block: Option<String>,
        depth: usize,
    ) -> Result<VariableNode, String> {
        self.visited += 1;
        if depth > MAX_DEPTH || self.visited > MAX_NODES {
            return Err("the program's variable tree exceeds its size limit".to_string());
        }
        let mut node = VariableNode {
            name: name.to_string(),
            path,
            kind: VariableKind::Value,
            type_name: value_type_name(value).unwrap_or_default(),
            kinds: ValueKind::kinds_of(value),
            writable: block.is_none(),
            write_block: block.clone(),
            access,
            children: Vec::new(),
            array: None,
        };
        match value {
            Value::Instance(id) => self.instance(&mut node, *id, block, depth)?,
            Value::Struct(structure) => {
                node.kind = VariableKind::Structure;
                for (field, field_value) in structure.fields() {
                    if is_listed(field, field_value) {
                        let path = format!("{}.{field}", node.path);
                        let child =
                            self.node(field, path, field_value, None, block.clone(), depth + 1)?;
                        node.children.push(child);
                    }
                }
            }
            Value::Array(array) => {
                let dimensions: Vec<[i64; 2]> = array
                    .dimensions()
                    .iter()
                    .map(|&(lower, upper)| [lower, upper])
                    .collect();
                let first = array
                    .elements()
                    .first()
                    .ok_or_else(|| format!("the array {} has no elements", node.path))?;
                let index = dimensions
                    .iter()
                    .map(|bounds| bounds[0].to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let path = format!("{}[{index}]", node.path);
                let element =
                    self.node(&format!("[{index}]"), path, first, None, block, depth + 1)?;
                let bounds = dimensions
                    .iter()
                    .map(|bounds| format!("{}..{}", bounds[0], bounds[1]))
                    .collect::<Vec<_>>()
                    .join(", ");
                node.kind = VariableKind::Array;
                node.type_name = format!("ARRAY[{bounds}] OF {}", element.type_name);
                node.array = Some(ArrayShape {
                    dimensions,
                    element: Box::new(element),
                });
            }
            _ => {}
        }
        Ok(node)
    }

    fn instance(
        &mut self,
        node: &mut VariableNode,
        id: InstanceId,
        block: Option<String>,
        depth: usize,
    ) -> Result<(), String> {
        if self.ancestors.contains(&id) {
            return Err(format!("the instance {} contains itself", node.path));
        }
        let instance = self
            .storage
            .get_instance(id)
            .ok_or_else(|| format!("the instance {} is not in storage", node.path))?;
        let type_name = instance.type_name.as_str();
        // A compiled program's storage names a configured PROGRAM instance after itself, so a root
        // is a program when the configuration declares an instance of its name.
        let configured = self.ancestors.is_empty()
            && find(self.metadata.programs(), node.name.as_str()).is_some();
        node.kind = if configured || find(self.metadata.program_templates(), type_name).is_some() {
            VariableKind::Program
        } else {
            VariableKind::FunctionBlock
        };
        node.type_name = type_name.to_string();
        self.ancestors.push(id);
        // The instance's own members, then those it inherits from its base parts (IEC 61131-3
        // Edition 3, 6.6.5.3), each with the declaration of the type that declares it.
        let mut part = Some(instance);
        let mut parts = 0;
        while let Some(current) = part {
            let declaring = current.type_name.as_str();
            for (member, member_value) in &current.variables {
                if is_listed(member, member_value)
                    && !node
                        .children
                        .iter()
                        .any(|child| child.name.eq_ignore_ascii_case(member))
                {
                    let access = self.member_access(declaring, member);
                    let path = format!("{}.{member}", node.path);
                    let member_block = block
                        .clone()
                        .or_else(|| member_write_block(self.metadata, declaring, member));
                    let child =
                        self.node(member, path, member_value, access, member_block, depth + 1)?;
                    node.children.push(child);
                }
            }
            parts += 1;
            part = current
                .parent
                .filter(|_| parts < MAX_DEPTH)
                .and_then(|parent| self.storage.get_instance(parent));
        }
        self.ancestors.pop();
        Ok(())
    }

    /// How a member of a function block (or a block it extends) is declared.
    fn member_access(&self, type_name: &str, member: &str) -> Option<VariableAccess> {
        let mut current = find(self.metadata.function_blocks(), type_name)?;
        for _ in 0..MAX_DEPTH {
            if let Some(param) = current
                .params
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(member))
            {
                return Some(match param.direction {
                    ParamDirection::In => VariableAccess::Input,
                    ParamDirection::Out => VariableAccess::Output,
                    ParamDirection::InOut => VariableAccess::InOut,
                });
            }
            if current
                .vars
                .iter()
                .any(|var| var.name.eq_ignore_ascii_case(member))
            {
                return Some(VariableAccess::Internal);
            }
            match &current.base {
                Some(FunctionBlockBase::FunctionBlock(base)) => {
                    current = find(self.metadata.function_blocks(), base)?;
                }
                _ => return None,
            }
        }
        None
    }
}

/// Listed are variables with an identifier name that can hold a value an HMI shows; runtime
/// bookkeeping (names that are not identifiers) and references are not.
pub(crate) fn is_listed(name: &str, value: &Value) -> bool {
    let mut chars = name.chars();
    let identifier = matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    identifier && !matches!(value, Value::Reference(_) | Value::Null)
}
