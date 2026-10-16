//! Scope enumeration handler.
//! - handle_scopes: build locals/globals/retain/io scopes

use serde_json::Value;

use trust_runtime::memory::FrameId;

use crate::protocol::{Request, Scope, ScopesArguments, ScopesResponseBody};

use super::super::{DebugAdapter, DispatchOutcome, PausedStateView, VariableHandle};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_scopes(&mut self, request: Request<Value>) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<ScopesArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid scopes args")],
                ..DispatchOutcome::default()
            };
        };

        if let Some(remote) = self.remote_session.as_mut() {
            let mut scopes = remote.scopes(args.frame_id).unwrap_or_default();
            for scope in &mut scopes {
                if let Some(line) = scope.line.as_mut() {
                    *line = self.to_client_line(*line);
                }
                if let Some(column) = scope.column.as_mut() {
                    *column = self.to_client_column(*column);
                }
                if let Some(end_line) = scope.end_line.as_mut() {
                    *end_line = self.to_client_line(*end_line);
                }
                if let Some(end_column) = scope.end_column.as_mut() {
                    *end_column = self.to_client_column(*end_column);
                }
            }
            let body = ScopesResponseBody { scopes };
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            };
        }

        self.variable_handles.clear();
        self.next_variable_ref = 1;

        let frame_id = FrameId(args.frame_id);
        let location = self
            .session
            .debug_control()
            .frame_location(frame_id)
            .and_then(|loc| self.location_to_client(&loc))
            .or_else(|| self.current_location());
        let view =
            PausedStateView::new(self.session.debug_control(), self.session.runtime_handle());
        let paused = view.is_paused();
        let (has_frame, has_globals, has_retain, has_instances) = view
            .with_storage(|storage| {
                (
                    storage.frames().iter().any(|frame| frame.id == frame_id),
                    !storage.globals().is_empty(),
                    !super::super::variables::retained::references(
                        self.session.metadata(),
                        storage,
                    )
                    .is_empty(),
                    !storage.instances().is_empty(),
                )
            })
            .unwrap_or((false, false, false, false));
        let has_io = if paused {
            let state = self.build_io_state();
            !(state.inputs.is_empty() && state.outputs.is_empty() && state.memory.is_empty())
        } else {
            false
        };

        let mut scopes = Vec::new();
        if has_frame {
            let locals_ref = self.alloc_variable_handle(VariableHandle::Locals(frame_id));
            scopes.push(Scope {
                name: "Locals".to_string(),
                variables_reference: locals_ref,
                expensive: false,
                source: location.as_ref().and_then(|(source, _, _)| source.clone()),
                line: location.as_ref().map(|(_, line, _)| *line),
                column: location.as_ref().map(|(_, _, column)| *column),
                end_line: None,
                end_column: None,
            });
        }

        if has_globals {
            let globals_ref = self.alloc_variable_handle(VariableHandle::Globals);
            scopes.push(Scope {
                name: "Globals".to_string(),
                variables_reference: globals_ref,
                expensive: false,
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        }

        if has_retain {
            let retain_ref = self.alloc_variable_handle(VariableHandle::Retain);
            scopes.push(Scope {
                name: "Retain".to_string(),
                variables_reference: retain_ref,
                expensive: false,
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        }

        if has_io {
            let io_ref = self.alloc_variable_handle(VariableHandle::IoRoot);
            scopes.push(Scope {
                name: "I/O".to_string(),
                variables_reference: io_ref,
                expensive: false,
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        }

        if has_instances {
            let instances_ref = self.alloc_variable_handle(VariableHandle::Instances);
            scopes.push(Scope {
                name: "Instances".to_string(),
                variables_reference: instances_ref,
                expensive: false,
                source: None,
                line: None,
                column: None,
                end_line: None,
                end_column: None,
            });
        }

        let body = ScopesResponseBody { scopes };

        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
