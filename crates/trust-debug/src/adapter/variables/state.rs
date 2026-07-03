//! Variable state snapshots and related helpers.
//! - handle_var_state: emit stVarState event
//! - build_var_state: collect snapshot variables

use serde_json::Value;

use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value as RuntimeValue;

use crate::protocol::{Request, VarStateEntry, VarStateEventBody, VarStateInstance};

use super::super::{DebugAdapter, DispatchOutcome, PausedStateView};
use super::format::format_value;

impl DebugAdapter {
    pub(in crate::adapter) fn handle_var_state(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if self.remote_session.is_some() {
            let body = VarStateEventBody {
                locals: Vec::new(),
                globals: Vec::new(),
                instances: Vec::new(),
                retain: Vec::new(),
                frame_id: None,
                paused: Some(false),
            };
            let event = self.event("stVarState", Some(body));
            return DispatchOutcome {
                responses: vec![self.ok_response::<Value>(&request, None)],
                events: vec![event],
                ..DispatchOutcome::default()
            };
        }
        let body = self.build_var_state();
        let event = self.event("stVarState", Some(body));
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events: vec![event],
            should_exit: false,
            stop_gate: None,
        }
    }
    fn build_var_state(&self) -> VarStateEventBody {
        let view =
            PausedStateView::new(self.session.debug_control(), self.session.runtime_handle());
        if let Some((locals, frame_id, globals, retain, instances)) = view.with_storage(|storage| {
            let (locals, frame_id) = storage
                .frames()
                .last()
                .map(|frame| {
                    let mut entries = Vec::new();
                    if let Some(instance_id) = frame.instance_id {
                        if let Some(instance) = storage.get_instance(instance_id) {
                            entries.extend(instance.variables.iter().map(|(name, value)| {
                                VarStateEntry {
                                    name: name.to_string(),
                                    value: format_var_state_value(value, storage),
                                }
                            }));
                        }
                    }
                    entries.extend(frame.variables.iter().map(|(name, value)| VarStateEntry {
                        name: name.to_string(),
                        value: format_var_state_value(value, storage),
                    }));
                    (entries, Some(frame.id.0))
                })
                .unwrap_or((Vec::new(), None));
            let globals = storage
                .globals()
                .iter()
                .map(|(name, value)| VarStateEntry {
                    name: name.to_string(),
                    value: format_var_state_value(value, storage),
                })
                .collect();
            let retain = storage
                .retain()
                .iter()
                .map(|(name, value)| VarStateEntry {
                    name: name.to_string(),
                    value: format_var_state_value(value, storage),
                })
                .collect();
            let instances = storage
                .instances()
                .iter()
                .map(|(id, data)| {
                    let mut vars = data
                        .variables
                        .iter()
                        .map(|(name, value)| VarStateEntry {
                            name: name.to_string(),
                            value: format_var_state_value(value, storage),
                        })
                        .collect::<Vec<_>>();
                    if let Some(parent) = data.parent {
                        vars.push(VarStateEntry {
                            name: "parent".to_string(),
                            value: format_var_state_value(&RuntimeValue::Instance(parent), storage),
                        });
                    }
                    VarStateInstance {
                        id: id.0,
                        name: format!("{}#{}", data.type_name, id.0),
                        vars,
                    }
                })
                .collect();
            (locals, frame_id, globals, retain, instances)
        }) {
            return VarStateEventBody {
                locals,
                globals,
                instances,
                retain,
                frame_id,
                paused: Some(view.is_paused()),
            };
        }
        VarStateEventBody {
            locals: Vec::new(),
            globals: Vec::new(),
            instances: Vec::new(),
            retain: Vec::new(),
            frame_id: None,
            paused: Some(false),
        }
    }
}

fn format_var_state_value(value: &RuntimeValue, storage: &VariableStorage) -> String {
    if let RuntimeValue::Instance(id) = value {
        if let Some(instance) = storage.get_instance(*id) {
            return instance.type_name.to_string();
        }
    }
    format_value(value)
}
