#[derive(Debug)]
struct BreakpointManager {
    requested: HashMap<SourceKey, Vec<SourceBreakpoint>>,
    last_report: Option<String>,
}

impl BreakpointManager {
    fn new() -> Self {
        Self {
            requested: HashMap::new(),
            last_report: None,
        }
    }

    fn take_report(&mut self) -> Option<String> {
        self.last_report.take()
    }

    #[cfg(test)]
    fn clear_requested(&mut self) {
        self.requested.clear();
    }

    fn set_breakpoints(
        &mut self,
        context: BreakpointContext<'_>,
        args: &SetBreakpointsArguments,
    ) -> SetBreakpointsResponseBody {
        let requested = requested_breakpoints(args);
        let raw_path = args.source.path.as_deref();
        let source_key = raw_path.map(source_key_for_path);

        if let Some(key) = source_key.as_ref() {
            if requested.is_empty() {
                self.requested.remove(key);
            } else {
                self.requested.insert(key.clone(), requested.clone());
            }
        }
        let mut report_lines = Vec::new();
        if let Some(path) = raw_path {
            report_lines.push(format!("[trust-debug] breakpoint resolve: {path}"));
        } else {
            report_lines.push("[trust-debug] breakpoint resolve: <unknown source>".to_string());
        }
        if requested.is_empty() {
            if let Some(key) = source_key.as_ref()
                && let Some(source_file) = context.sources.get(key)
            {
                context
                    .control
                    .set_breakpoints_for_file(source_file.file_id, Vec::new());
            }
            report_lines.push("  cleared".to_string());
            self.last_report = Some(report_lines.join("\n"));
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        }

        let Some(key) = source_key else {
            report_lines.push("  error: missing source path".to_string());
            self.last_report = Some(report_lines.join("\n"));
            return SetBreakpointsResponseBody {
                breakpoints: requested
                    .into_iter()
                    .map(|bp| {
                        Breakpoint::unverified(
                            bp.line,
                            bp.column,
                            None,
                            Some(MSG_MISSING_SOURCE.into()),
                        )
                    })
                    .collect(),
            };
        };

        let source_file = context.sources.get(&key);
        let Some(source_file) = source_file else {
            if context.sources.is_empty() {
                report_lines.push("  pending: program not loaded yet".to_string());
                self.last_report = Some(report_lines.join("\n"));
                return SetBreakpointsResponseBody {
                    breakpoints: requested
                        .into_iter()
                        .map(|bp| {
                            Breakpoint::unverified(
                                bp.line,
                                bp.column,
                                Some(args.source.clone()),
                                Some(MSG_PENDING_SOURCE.into()),
                            )
                        })
                        .collect(),
                };
            }
            report_lines.push("  error: unknown source file".to_string());
            self.last_report = Some(report_lines.join("\n"));
            return SetBreakpointsResponseBody {
                breakpoints: requested
                    .into_iter()
                    .map(|bp| {
                        Breakpoint::unverified(
                            bp.line,
                            bp.column,
                            Some(args.source.clone()),
                            Some(MSG_UNKNOWN_SOURCE.into()),
                        )
                    })
                    .collect(),
            };
        };

        let source_text = source_file.text.clone();
        let file_id = source_file.file_id;
        let profile = context.metadata.profile();
        let mut registry = context.metadata.registry().clone();
        let using = context
            .control
            .snapshot()
            .and_then(|snapshot| {
                snapshot
                    .storage
                    .current_frame()
                    .map(|frame| frame.id)
                    .and_then(|frame_id| {
                        context
                            .metadata
                            .using_for_frame(&snapshot.storage, frame_id)
                    })
            })
            .unwrap_or_default();

        let mut resolved_breakpoints = Vec::new();
        let mut breakpoints = Vec::with_capacity(requested.len());
        for requested_bp in requested {
            let requested_line = requested_bp.line;
            let requested_column = requested_bp.column.unwrap_or(1);
            let first_non_ws =
                first_non_whitespace_column(&source_text, requested_bp.line.saturating_sub(1))
                    .map(|col| col.saturating_add(1));
            let column_override = match requested_bp.column {
                None => first_non_ws,
                Some(col) => match first_non_ws {
                    Some(first) if col <= first => Some(first),
                    _ => None,
                },
            };
            let Some((line, column)) =
                to_zero_based(requested_bp.line, column_override.or(requested_bp.column))
            else {
                report_lines.push(format!(
                    "  req {requested_line}:{requested_column} -> invalid position"
                ));
                breakpoints.push(Breakpoint::unverified(
                    requested_bp.line,
                    requested_bp.column,
                    Some(args.source.clone()),
                    Some(MSG_INVALID_POSITION.into()),
                ));
                continue;
            };

            match context
                .metadata
                .resolve_breakpoint_position(&source_text, file_id, line, column)
            {
                Some((location, resolved_line, resolved_col)) => {
                    let line_text = source_text
                        .lines()
                        .nth(resolved_line as usize)
                        .unwrap_or("")
                        .trim();
                    let mut column_note = String::new();
                    if let Some(override_col) = column_override
                        && override_col != requested_column
                    {
                        column_note = format!(" (snapped col {override_col})");
                    }
                    report_lines.push(format!(
                        "  req {requested_line}:{requested_column} -> resolved {}:{} range {}..{}{} text='{}'",
                        resolved_line + 1,
                        resolved_col + 1,
                        location.start,
                        location.end,
                        column_note,
                        line_text
                    ));
                    let condition = match requested_bp.condition.as_deref() {
                        Some(condition) => {
                            match parse_debug_expression(condition, &mut registry, profile, &using)
                            {
                                Ok(expr) => Some(expr),
                                Err(err) => {
                                    breakpoints.push(Breakpoint::unverified(
                                        requested_bp.line,
                                        requested_bp.column,
                                        Some(args.source.clone()),
                                        Some(format!("{MSG_INVALID_CONDITION}: {err}")),
                                    ));
                                    continue;
                                }
                            }
                        }
                        None => None,
                    };

                    let hit_condition = match requested_bp.hit_condition.as_deref() {
                        Some(hit_condition) => match parse_hit_condition(hit_condition) {
                            Some(parsed) => Some(parsed),
                            None => {
                                breakpoints.push(Breakpoint::unverified(
                                    requested_bp.line,
                                    requested_bp.column,
                                    Some(args.source.clone()),
                                    Some(MSG_INVALID_HIT_CONDITION.into()),
                                ));
                                continue;
                            }
                        },
                        None => None,
                    };

                    let log_message = match requested_bp.log_message.as_deref() {
                        Some(template) => {
                            match parse_log_message(template, &mut registry, profile, &using) {
                                Ok(fragments) => Some(fragments),
                                Err(err) => {
                                    breakpoints.push(Breakpoint::unverified(
                                        requested_bp.line,
                                        requested_bp.column,
                                        Some(args.source.clone()),
                                        Some(format!("{MSG_INVALID_LOG_MESSAGE}: {err}")),
                                    ));
                                    continue;
                                }
                            }
                        }
                        None => None,
                    };

                    resolved_breakpoints.push(DebugBreakpoint {
                        location,
                        condition,
                        hit_condition,
                        log_message,
                        hits: 0,
                        generation: 0,
                    });
                    breakpoints.push(Breakpoint::verified(
                        resolved_line + 1,
                        resolved_col + 1,
                        Some(args.source.clone()),
                    ));
                }
                None => {
                    report_lines.push(format!(
                        "  req {requested_line}:{requested_column} -> unresolved (no statement)"
                    ));
                    breakpoints.push(Breakpoint::unverified(
                        requested_bp.line,
                        requested_bp.column,
                        Some(args.source.clone()),
                        Some(MSG_NO_STATEMENT.into()),
                    ));
                }
            }
        }

        context
            .control
            .set_breakpoints_for_file(file_id, resolved_breakpoints);
        self.last_report = Some(report_lines.join("\n"));

        SetBreakpointsResponseBody { breakpoints }
    }

    fn revalidate_breakpoints(&mut self, context: BreakpointContext<'_>) -> Vec<Breakpoint> {
        let mut updated = Vec::new();
        let entries = self
            .requested
            .iter()
            .map(|(key, breakpoints)| (key.clone(), breakpoints.clone()))
            .collect::<Vec<_>>();
        for (key, breakpoints) in entries {
            let path = key.display();
            let args = SetBreakpointsArguments {
                source: Source {
                    name: Some(path.clone()),
                    path: Some(path.clone()),
                    source_reference: None,
                },
                breakpoints: Some(breakpoints),
                lines: None,
                source_modified: None,
            };
            let result = self.set_breakpoints(context, &args);
            updated.extend(result.breakpoints);
        }
        updated
    }
}
