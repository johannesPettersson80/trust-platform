impl DebugAdapter {
    /// `setBreakpoints` while attached: the breakpoints go to the runtime with their options, and
    /// each requested breakpoint is answered in the request's order (specification 13), a
    /// breakpoint whose options cannot go to this runtime unverified with the reason, so that no
    /// breakpoint stops without the condition the client asked for (specification 11 §6.13.2).
    fn set_breakpoints_remote(
        &mut self,
        args: SetBreakpointsArguments,
    ) -> SetBreakpointsResponseBody {
        let Some(source_path) = args.source.path.clone() else {
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        };
        let requested = super::handlers::breakpoints::requested_breakpoints(&args);
        let unverified = |breakpoint: &SourceBreakpoint, message: &str| Breakpoint {
            id: None,
            verified: false,
            message: Some(message.to_string()),
            source: Some(args.source.clone()),
            line: Some(breakpoint.line),
            column: breakpoint.column,
            end_line: None,
            end_column: None,
        };
        let mut answers: Vec<Option<Breakpoint>> = vec![None; requested.len()];
        let mut sent = Vec::new();
        for (index, breakpoint) in requested.iter().enumerate() {
            let Some(line) = self.to_runtime_line(breakpoint.line) else {
                answers[index] = Some(unverified(breakpoint, "invalid line"));
                continue;
            };
            match self.remote_options(breakpoint) {
                Ok(options) => sent.push((index, line, options)),
                Err(message) => answers[index] = Some(unverified(breakpoint, &message)),
            }
        }
        // A runtime resolves a line to the first statement at or after it, so a line it cannot
        // resolve lies after the file's last statement: sent in ascending order, the lines it
        // resolves come first and its answer lists them in that order.
        sent.sort_by_key(|(_, line, _)| *line);
        let Some(remote) = self.remote_session.as_mut() else {
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        };
        if sent.is_empty() {
            let _ = remote.clear_breakpoints(&source_path);
        } else {
            let lines = sent.iter().map(|(_, line, _)| *line).collect();
            let options = sent
                .iter()
                .any(|(_, _, options)| options.is_some())
                .then(|| {
                    sent.iter()
                        .map(|(_, _, options)| options.clone().unwrap_or(Value::Null))
                        .collect()
                });
            match remote.set_breakpoints(&source_path, lines, options) {
                Ok((breakpoints, file_id, generation)) => {
                    if let (Some(file_id), Some(generation)) = (file_id, generation)
                        && let Ok(mut guard) = self.remote_breakpoints.lock()
                    {
                        guard.insert(file_id, generation);
                    }
                    let mut resolved = breakpoints.into_iter();
                    for (index, _, _) in &sent {
                        answers[*index] = Some(match resolved.next() {
                            Some(mut breakpoint) => {
                                if let Some(line) = breakpoint.line.as_mut() {
                                    *line = self.to_client_line(*line);
                                }
                                if let Some(column) = breakpoint.column.as_mut() {
                                    *column = self.to_client_column(*column);
                                }
                                breakpoint
                            }
                            None => unverified(
                                &requested[*index],
                                super::handlers::breakpoints::NO_STATEMENT,
                            ),
                        });
                    }
                }
                Err(error) => {
                    for (index, _, _) in &sent {
                        answers[*index] = Some(unverified(&requested[*index], &error));
                    }
                }
            }
        }
        SetBreakpointsResponseBody {
            breakpoints: answers.into_iter().flatten().collect(),
        }
    }

    pub(super) fn to_client_line(&self, line: u32) -> u32 {
        self.coordinate.to_client_line(line)
    }

    pub(super) fn to_client_column(&self, column: u32) -> u32 {
        self.coordinate.to_client_column(column)
    }

    pub(super) fn to_runtime_line(&self, line: u32) -> Option<u32> {
        self.coordinate.to_runtime_line(line)
    }

    pub(super) fn to_runtime_column(&self, column: u32) -> Option<u32> {
        self.coordinate.to_runtime_column(column)
    }
}
