// The scopes and variables lists are the shared ones (trust-runtime-services `debug::reads`): the
// same code lists the published state on a chip, so a scope reads the same on every runtime
// (specification 34, ARCH-11).

pub(super) fn handle_debug_scopes(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugScopesParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    debug!("control debug.scopes frame_id={}", params.frame_id);
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let requested_frame = crate::memory::FrameId(params.frame_id);
    if !snapshot
        .storage
        .frames()
        .iter()
        .any(|frame| frame.id == requested_frame)
    {
        return ControlResponse::error(id, replies::unknown_frame(params.frame_id));
    }
    let place = state
        .debug
        .frame_location(requested_frame)
        .or_else(|| state.debug.last_stop().and_then(|stop| stop.location))
        .and_then(|loc| location_to_source(&loc, state))
        .map(|(source, line, column)| Located {
            source,
            line,
            column,
        });
    let has_io = match state.io_snapshot() {
        IoSnapshotRead::Published(io) => {
            !(io.inputs.is_empty() && io.outputs.is_empty() && io.memory.is_empty())
        }
        IoSnapshotRead::Missing => false,
        IoSnapshotRead::Unavailable => {
            return ControlResponse::error(id, "I/O snapshot unavailable".to_string());
        }
    };
    let mut handles = match state.debug_variables.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "debug variables unavailable".into()),
    };
    let scopes = reads::scopes(
        &mut handles,
        &snapshot.storage,
        requested_frame,
        place.as_ref(),
        has_io,
    );
    debug!(
        "control debug.scopes result={:?}",
        scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>()
    );
    replies::scopes(id, &scopes)
}

pub(super) fn handle_debug_variables(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugVariablesParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    debug!(
        "control debug.variables reference={}",
        params.variables_reference
    );
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let io = match state.io_snapshot() {
        IoSnapshotRead::Published(io) => Some(io),
        IoSnapshotRead::Missing => None,
        IoSnapshotRead::Unavailable => {
            return ControlResponse::error(id, "I/O snapshot unavailable".to_string());
        }
    };
    let mut handles = match state.debug_variables.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "debug variables unavailable".into()),
    };
    let Some(handle) = handles.get(params.variables_reference).cloned() else {
        return ControlResponse::error(
            id,
            format!("unknown variables reference {}", params.variables_reference),
        );
    };
    debug!("control debug.variables handle={:?}", handle);
    let read = ReadState {
        storage: &snapshot.storage,
        frames: named_frames(&snapshot.storage),
    };
    let variables = match reads::variables(&read, &handle, io.as_ref(), &mut handles, params.page())
    {
        Ok(variables) => variables,
        Err(error) => return ControlResponse::error(id, error),
    };
    debug!("control debug.variables result_count={}", variables.len());
    replies::variables(id, &variables)
}

/// The call stack of the snapshot's storage, outermost frame first, with each frame's variables
/// named. The compiler's scratch rows are not listed (specification 13, "Variables / Evaluate").
fn named_frames(storage: &crate::memory::VariableStorage) -> Vec<NamedFrame<'_>> {
    storage
        .frames()
        .iter()
        .map(|frame| NamedFrame {
            id: frame.id,
            owner: frame.owner.as_str(),
            instance: frame.instance_id,
            variables: frame
                .variables
                .iter()
                .filter(|(name, _)| !name.starts_with("@scratch/"))
                .map(|(name, value)| (name.as_str(), value))
                .collect(),
        })
        .collect()
}
