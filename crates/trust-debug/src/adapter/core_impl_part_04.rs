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

    fn set_breakpoints_remote(
        &mut self,
        args: SetBreakpointsArguments,
    ) -> SetBreakpointsResponseBody {
        let source_path = match args.source.path.as_deref() {
            Some(path) => path,
            None => {
                return SetBreakpointsResponseBody {
                    breakpoints: Vec::new(),
                };
            }
        };
        let mut lines = Vec::new();
        if let Some(breakpoints) = args.breakpoints.as_ref() {
            for breakpoint in breakpoints {
                if let Some(line) = self.to_runtime_line(breakpoint.line) {
                    lines.push(line);
                }
            }
        } else if let Some(list) = args.lines.as_ref() {
            for line in list {
                if let Some(line) = self.to_runtime_line(*line) {
                    lines.push(line);
                }
            }
        }
        if lines.is_empty() {
            if let Some(remote) = self.remote_session.as_mut() {
                let _ = remote.clear_breakpoints(source_path);
            }
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        }
        let response = if let Some(remote) = self.remote_session.as_mut() {
            remote.set_breakpoints(source_path, lines)
        } else {
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        };
        match response {
            Ok((mut breakpoints, file_id, generation)) => {
                if let (Some(file_id), Some(generation)) = (file_id, generation)
                    && let Ok(mut guard) = self.remote_breakpoints.lock()
                {
                    guard.insert(file_id, generation);
                }
                for breakpoint in &mut breakpoints {
                    if let Some(line) = breakpoint.line.as_mut() {
                        *line = self.to_client_line(*line);
                    }
                    if let Some(column) = breakpoint.column.as_mut() {
                        *column = self.to_client_column(*column);
                    }
                }
                SetBreakpointsResponseBody { breakpoints }
            }
            Err(_) => SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            },
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
