fn debug_stop_to_json(
    stop: crate::debug::DebugStop,
    state: &ControlState,
) -> Option<serde_json::Value> {
    // The stop names its place only when its file's text is loaded and the file is registered.
    let place = stop.location.and_then(|location| {
        let text = state.sources.source_text(location.file_id)?;
        let source = state
            .sources
            .files()
            .iter()
            .find(|file| file.id == location.file_id)?;
        let (line, column) = location_to_line_col(text, &location);
        Some((
            location.file_id,
            line,
            column,
            source.path.to_string_lossy().to_string(),
        ))
    });
    Some(replies::stop(
        stop.reason,
        stop.thread_id,
        stop.breakpoint_generation,
        place
            .as_ref()
            .map(|(file_id, line, column, path)| StopPlace {
                file_id: *file_id,
                line: *line,
                column: *column,
                path: path.as_str(),
            }),
    ))
}

fn location_to_source(
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<(DebugSource, u32, u32)> {
    let source_text = state.sources.source_text(location.file_id)?;
    let (line, column) = location_to_line_col(source_text, location);
    let source = state
        .sources
        .files()
        .iter()
        .find(|file| file.id == location.file_id)?;
    let name = source
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string());
    let path = Some(source.path.to_string_lossy().to_string());
    Some((DebugSource { name, path }, line, column))
}

fn location_to_stack_frame(
    frame_id: u32,
    frame_name: &str,
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<serde_json::Value> {
    let (source, line, column) = location_to_source(location, state)?;
    Some(replies::stack_frame(
        frame_id, frame_name, &source, line, column,
    ))
}

fn evaluate_with_snapshot(
    expr: &crate::program_model::Expr,
    registry: &trust_hir::types::TypeRegistry,
    frame_id: Option<crate::memory::FrameId>,
    snapshot: &crate::debug::DebugSnapshot,
    _using: &[smol_str::SmolStr],
    state: &ControlState,
) -> Result<Value, RuntimeError> {
    let metadata = state
        .metadata
        .lock()
        .map_err(|_| RuntimeError::ControlError("metadata unavailable".into()))?;
    let profile = metadata.profile();
    let mut storage = snapshot.storage.clone();
    let eval = |storage: &mut crate::memory::VariableStorage,
                instance_id: Option<crate::memory::InstanceId>|
     -> Result<Value, RuntimeError> {
        crate::helper_eval::eval_storage_expr_with_abi(
            storage,
            registry,
            snapshot.storage_abi,
            &profile,
            instance_id,
            Some(metadata.stdlib()),
            expr,
        )
    };

    if let Some(frame_id) = frame_id {
        storage
            .with_frame(frame_id, |storage| {
                let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                eval(storage, instance_id)
            })
            .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
    } else {
        eval(&mut storage, None)
    }
}
