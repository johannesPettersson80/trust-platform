pub(super) fn handle_debug_evaluate(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugEvaluateParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let frame_id = params.frame_id.map(crate::memory::FrameId);
    if let Some(frame_id) = frame_id {
        if !snapshot
            .storage
            .frames()
            .iter()
            .any(|frame| frame.id == frame_id)
        {
            return ControlResponse::error(id, "unknown frame id".into());
        }
    }
    let (using, mut registry, profile) = {
        let metadata = match state.metadata.lock() {
            Ok(guard) => guard,
            Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
        };
        let using = if let Some(frame_id) = frame_id {
            match metadata.using_for_frame(&snapshot.storage, frame_id) {
                Some(using) => using,
                None => {
                    return ControlResponse::error(
                        id,
                        format!("debug scope unavailable for frame id {}", frame_id.0),
                    );
                }
            }
        } else {
            Vec::new()
        };
        (using, metadata.registry().clone(), metadata.profile())
    };
    let expr = match crate::harness::parse_debug_expression(
        &params.expression,
        &mut registry,
        profile,
        &using,
    ) {
        Ok(expr) => expr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    let value = match evaluate_with_snapshot(&expr, &registry, frame_id, &snapshot, &using, state) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    let result = crate::debug::dap::format_value(&value);
    let type_name = crate::debug::dap::value_type_name(&value);
    ControlResponse::ok(
        id,
        json!({
            "result": result,
            "type": type_name,
            "variables_reference": 0,
        }),
    )
}

pub(super) fn handle_debug_breakpoint_locations(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugBreakpointLocationsParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let path = PathBuf::from(params.source);
    let file_id = match state.sources.file_id_for_path(&path) {
        Some(id) => id,
        None => return ControlResponse::error(id, "unknown source path".into()),
    };
    let source_text = match state.sources.source_text(file_id) {
        Some(text) => text,
        None => return ControlResponse::error(id, "source text not loaded".into()),
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let mut breakpoints = Vec::new();
    if let Some(locations) = metadata.statement_locations(file_id) {
        let max_line = params.end_line.unwrap_or(params.line);
        for location in locations {
            let (loc_line, loc_col) = location_to_line_col(source_text, location);
            if loc_line < params.line || loc_line > max_line {
                continue;
            }
            if let Some(min_col) = params.column {
                if loc_line == params.line && loc_col < min_col {
                    continue;
                }
            }
            if let Some(max_col) = params.end_column {
                if loc_line == max_line && loc_col > max_col {
                    continue;
                }
            }
            breakpoints.push(json!({ "line": loc_line, "column": loc_col }));
        }
    }
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}
