pub(super) fn handle_debug_state(id: u64, state: &ControlState) -> ControlResponse {
    let paused = state.debug.is_paused();
    let last_stop = state
        .debug
        .last_stop()
        .and_then(|stop| debug_stop_to_json(stop, state));
    ControlResponse::ok(
        id,
        json!({
            "paused": paused,
            "last_stop": last_stop,
        }),
    )
}

pub(super) fn handle_debug_stops(id: u64, state: &ControlState) -> ControlResponse {
    let stops = state
        .debug
        .drain_stops()
        .into_iter()
        .filter_map(|stop| debug_stop_to_json(stop, state))
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "stops": stops }))
}

pub(super) fn handle_debug_stack(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let frames = snapshot.storage.frames();
    let frame_locations = state.debug.frame_locations();
    let fallback = state.debug.last_stop().and_then(|stop| stop.location);
    let mut stack_frames = Vec::new();
    if frames.is_empty() {
        if let Some(location) = fallback
            && let Some(frame) = location_to_stack_frame(0, "Main", &location, state)
        {
            stack_frames.push(frame);
        }
    } else {
        for frame in frames.iter().rev() {
            let resolved = frame_locations.get(&frame.id).copied().or(fallback);
            let frame_name = frame.owner.as_str();
            if let Some(location) = resolved
                && let Some(frame_json) =
                    location_to_stack_frame(frame.id.0, frame_name, &location, state)
            {
                stack_frames.push(frame_json);
            }
        }
    }
    ControlResponse::ok(
        id,
        json!({
            "stack_frames": stack_frames,
            "total_frames": stack_frames.len(),
        }),
    )
}
