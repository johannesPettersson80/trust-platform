//! Variables request + handle allocation helpers.
//! - handle_variables: resolve variables for a handle
//! - variable_from_value: format a value into a DAP Variable
//! - alloc_variable_handle: track variable handles

use serde_json::Value;

use trust_runtime::memory::InstanceId;
use trust_runtime::value::{ArrayValue, StructValue, Value as RuntimeValue};

use crate::protocol::{IoStateEntry, Request, Variable, VariablesArguments, VariablesResponseBody};

use super::super::{DebugAdapter, DispatchOutcome, PausedStateView, VariableHandle};
use super::format::{format_value, value_type_name};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_variables(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<VariablesArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid variables args")],
                ..DispatchOutcome::default()
            };
        };

        if let Some(remote) = self.remote_session.as_mut() {
            let variables = remote
                .variables(args.variables_reference)
                .unwrap_or_default();
            let body = VariablesResponseBody { variables };
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            };
        }

        let Some(handle) = self
            .variable_handles
            .get(&args.variables_reference)
            .cloned()
        else {
            return DispatchOutcome {
                responses: vec![self.ok_response(
                    &request,
                    Some(VariablesResponseBody {
                        variables: Vec::new(),
                    }),
                )],
                ..DispatchOutcome::default()
            };
        };

        let view =
            PausedStateView::new(self.session.debug_control(), self.session.runtime_handle());

        let variables = match handle {
            VariableHandle::Locals(frame_id) => {
                let entries = view
                    .with_storage(|storage| {
                        storage
                            .frames()
                            .iter()
                            .find(|frame| frame.id == frame_id)
                            .map(|frame| {
                                let mut entries = Vec::new();
                                if let Some(instance_id) = frame.instance_id {
                                    if let Some(instance) = storage.get_instance(instance_id) {
                                        entries.extend(collect_entries(&instance.variables));
                                    }
                                }
                                entries.extend(collect_entries(&frame.variables));
                                entries
                            })
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                self.variables_from_entries(entries)
            }
            VariableHandle::Globals => {
                let entries = view
                    .with_storage(|storage| collect_entries(storage.globals()))
                    .unwrap_or_default();
                self.variables_from_entries(entries)
            }
            VariableHandle::Retain => {
                let entries = view
                    .with_storage(|storage| collect_entries(storage.retain()))
                    .unwrap_or_default();
                self.variables_from_entries(entries)
            }
            VariableHandle::Instances => {
                let instances = view
                    .with_storage(|storage| {
                        storage
                            .instances()
                            .iter()
                            .map(|(id, data)| (*id, data.type_name.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                self.variables_from_instances(instances)
            }
            VariableHandle::Instance(instance_id) => {
                let entries = view
                    .with_storage(|storage| {
                        storage.get_instance(instance_id).map(|instance| {
                            let mut entries = collect_entries(&instance.variables);
                            if let Some(parent_id) = instance.parent {
                                entries.push((
                                    "parent".to_string(),
                                    RuntimeValue::Instance(parent_id),
                                ));
                            }
                            entries
                        })
                    })
                    .flatten()
                    .unwrap_or_default();
                self.variables_from_entries(entries)
            }
            VariableHandle::Struct(struct_value) => self.variables_from_struct(struct_value),
            VariableHandle::Array(array_value) => self.variables_from_array(array_value),
            VariableHandle::Reference(value_ref) => {
                let value = view
                    .with_storage(|storage| storage.read_by_ref(value_ref).cloned())
                    .flatten();
                value
                    .map(|value| vec![self.variable_from_value("*".to_string(), value, None)])
                    .unwrap_or_default()
            }
            VariableHandle::IoRoot => {
                let state = self.build_io_state();
                let inputs_ref = self.alloc_variable_handle(VariableHandle::IoInputs);
                let outputs_ref = self.alloc_variable_handle(VariableHandle::IoOutputs);
                let memory_ref = self.alloc_variable_handle(VariableHandle::IoMemory);
                vec![
                    Variable {
                        name: "Inputs".to_string(),
                        value: format!("{} items", state.inputs.len()),
                        r#type: None,
                        variables_reference: inputs_ref,
                        evaluate_name: None,
                    },
                    Variable {
                        name: "Outputs".to_string(),
                        value: format!("{} items", state.outputs.len()),
                        r#type: None,
                        variables_reference: outputs_ref,
                        evaluate_name: None,
                    },
                    Variable {
                        name: "Memory".to_string(),
                        value: format!("{} items", state.memory.len()),
                        r#type: None,
                        variables_reference: memory_ref,
                        evaluate_name: None,
                    },
                ]
            }
            VariableHandle::IoInputs => {
                let state = self.build_io_state();
                self.variables_from_io_entries(&state.inputs)
            }
            VariableHandle::IoOutputs => {
                let state = self.build_io_state();
                self.variables_from_io_entries(&state.outputs)
            }
            VariableHandle::IoMemory => {
                let state = self.build_io_state();
                self.variables_from_io_entries(&state.memory)
            }
        };

        let body = VariablesResponseBody { variables };

        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
    fn variables_from_entries(&mut self, entries: Vec<(String, RuntimeValue)>) -> Vec<Variable> {
        entries
            .into_iter()
            .map(|(name, value)| self.variable_from_value(name.clone(), value, Some(name)))
            .collect()
    }

    fn variables_from_io_entries(&self, entries: &[IoStateEntry]) -> Vec<Variable> {
        entries
            .iter()
            .map(|entry| {
                let name = entry.name.clone().unwrap_or_else(|| entry.address.clone());
                Variable {
                    name: name.clone(),
                    value: entry.value.clone(),
                    r#type: None,
                    variables_reference: 0,
                    evaluate_name: Some(name),
                }
            })
            .collect()
    }

    fn variables_from_struct(&mut self, value: StructValue) -> Vec<Variable> {
        let entries = value
            .fields()
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect::<Vec<_>>();
        entries
            .into_iter()
            .map(|(name, value)| self.variable_from_value(name, value, None))
            .collect()
    }

    fn variables_from_array(&mut self, value: ArrayValue) -> Vec<Variable> {
        let dimensions = value.dimensions().to_vec();
        value
            .elements()
            .iter()
            .cloned()
            .enumerate()
            .map(|(offset, element)| {
                let indices = array_indices_for_offset(&dimensions, offset);
                let name = if indices.len() == 1 {
                    format!("[{}]", indices[0])
                } else {
                    let joined = indices
                        .iter()
                        .map(|idx| idx.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("[{joined}]")
                };
                self.variable_from_value(name, element, None)
            })
            .collect()
    }

    fn variables_from_instances(&mut self, instances: Vec<(InstanceId, String)>) -> Vec<Variable> {
        instances
            .into_iter()
            .map(|(id, type_name)| {
                let variables_reference = self.alloc_variable_handle(VariableHandle::Instance(id));
                Variable {
                    name: format!("{type_name}#{}", id.0),
                    value: "Instance".to_string(),
                    r#type: Some(type_name),
                    variables_reference,
                    evaluate_name: None,
                }
            })
            .collect()
    }

    pub(super) fn variable_from_value(
        &mut self,
        name: String,
        value: RuntimeValue,
        evaluate_name: Option<String>,
    ) -> Variable {
        let display = format_value(&value);
        let r#type = value_type_name(&value);
        let variables_reference = match value {
            RuntimeValue::Struct(value) => {
                self.alloc_variable_handle(VariableHandle::Struct((*value).clone()))
            }
            RuntimeValue::Array(value) => self.alloc_variable_handle(VariableHandle::Array(*value)),
            RuntimeValue::Instance(id) => self.alloc_variable_handle(VariableHandle::Instance(id)),
            RuntimeValue::Reference(Some(value_ref)) => {
                self.alloc_variable_handle(VariableHandle::Reference(value_ref))
            }
            _ => 0,
        };
        Variable {
            name,
            value: display,
            r#type,
            variables_reference,
            evaluate_name,
        }
    }

    pub(in crate::adapter) fn alloc_variable_handle(&mut self, handle: VariableHandle) -> u32 {
        let id = self.next_variable_ref;
        self.next_variable_ref = self.next_variable_ref.saturating_add(1);
        self.variable_handles.insert(id, handle);
        id
    }
}

fn collect_entries(
    vars: &indexmap::IndexMap<smol_str::SmolStr, RuntimeValue>,
) -> Vec<(String, RuntimeValue)> {
    vars.iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect()
}

fn array_indices_for_offset(dimensions: &[(i64, i64)], mut offset: usize) -> Vec<i64> {
    if dimensions.is_empty() {
        return Vec::new();
    }
    let mut indices_rev = Vec::with_capacity(dimensions.len());
    for (lower, upper) in dimensions.iter().rev() {
        let len = (*upper - *lower + 1) as usize;
        if len == 0 {
            indices_rev.push(*lower);
            continue;
        }
        let idx = (offset % len) as i64 + *lower;
        indices_rev.push(idx);
        offset /= len;
    }
    indices_rev.into_iter().rev().collect()
}
