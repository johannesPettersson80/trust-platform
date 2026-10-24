impl DebugAdapter {
    fn to_session_breakpoints(&self, mut args: SetBreakpointsArguments) -> SetBreakpointsArguments {
        let adjust_line = |line: u32| -> u32 {
            if self.coordinate.lines_start_at1() {
                line
            } else {
                line.saturating_add(1)
            }
        };
        let adjust_column = |column: Option<u32>| -> Option<u32> {
            column.map(|column| {
                if self.coordinate.columns_start_at1() {
                    column
                } else {
                    column.saturating_add(1)
                }
            })
        };

        if let Some(breakpoints) = args.breakpoints.as_mut() {
            for breakpoint in breakpoints {
                breakpoint.line = adjust_line(breakpoint.line);
                breakpoint.column = adjust_column(breakpoint.column);
            }
        }

        if let Some(lines) = args.lines.as_mut() {
            for line in lines.iter_mut() {
                *line = adjust_line(*line);
            }
        }

        args
    }

    fn to_client_breakpoints(
        &self,
        mut response: SetBreakpointsResponseBody,
    ) -> SetBreakpointsResponseBody {
        let adjust_line = |line: u32| -> u32 {
            if self.coordinate.lines_start_at1() {
                line
            } else {
                line.saturating_sub(1)
            }
        };
        let adjust_column = |column: u32| -> u32 {
            if self.coordinate.columns_start_at1() {
                column
            } else {
                column.saturating_sub(1)
            }
        };

        for breakpoint in &mut response.breakpoints {
            if let Some(line) = breakpoint.line.as_mut() {
                *line = adjust_line(*line);
            }
            if let Some(column) = breakpoint.column.as_mut() {
                *column = adjust_column(*column);
            }
            if let Some(line) = breakpoint.end_line.as_mut() {
                *line = adjust_line(*line);
            }
            if let Some(column) = breakpoint.end_column.as_mut() {
                *column = adjust_column(*column);
            }
        }

        response
    }

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

    pub(super) fn current_location(&self) -> Option<(Option<Source>, u32, u32)> {
        let location = self.session.debug_control().last_location()?;
        self.location_to_client(&location)
    }

    pub(super) fn location_to_client(
        &self,
        location: &trust_runtime::debug::SourceLocation,
    ) -> Option<(Option<Source>, u32, u32)> {
        let source = self.session.source_for_file_id(location.file_id);
        let text = self.session.source_text_for_file_id(location.file_id)?;
        let (line, column) = location_to_line_col(text, location);
        Some((
            source,
            self.to_client_line(line),
            self.to_client_column(column),
        ))
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

    pub(super) fn default_line(&self) -> u32 {
        self.coordinate.default_line()
    }

    pub(super) fn default_column(&self) -> u32 {
        self.coordinate.default_column()
    }
}
