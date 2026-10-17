/// Publishes the project's endpoint file once the control endpoint is bound (spec 11, section
/// 6.8.0a). A runtime that cannot write the file keeps running, because a tooling file never stops
/// a PLC program (spec 34, ARCH-07). It reports a warning in its log and its events instead, and
/// its status says that the endpoint file is unavailable and why.
fn publish_endpoint_file(
    project_root: &Path,
    endpoint: &ControlEndpoint,
    generated_token: Option<&str>,
    debug: &trust_runtime::debug::DebugControl,
    settings: &Mutex<RuntimeSettings>,
) -> Option<PublishedEndpoint> {
    let (published, state) =
        match PublishedEndpoint::publish(project_root, endpoint, generated_token) {
            Ok(published) => (Some(published), EndpointFileState::Published),
            Err(error) => {
                let reason = runtime_error_text(&error);
                debug.push_runtime_event(
                    trust_runtime::debug::RuntimeEvent::ControlEndpointFileUnavailable {
                        endpoint: SmolStr::new(format_endpoint(endpoint)),
                        error: reason.clone(),
                        time: wall_clock_event_time(),
                    },
                );
                (None, EndpointFileState::Unavailable(SmolStr::new(reason)))
            }
        };
    if let Ok(mut guard) = settings.lock() {
        guard.control_endpoint_file = state;
    }
    published
}

/// The message of a runtime error without its category prefix.
fn runtime_error_text(error: &trust_runtime::error::RuntimeError) -> String {
    match error {
        trust_runtime::error::RuntimeError::ControlError(message)
        | trust_runtime::error::RuntimeError::InvalidBundle(message) => message.to_string(),
        other => other.to_string(),
    }
}

/// The wall-clock time of a launcher event, in the millisecond resolution that control events
/// use.
fn wall_clock_event_time() -> Duration {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    Duration::from_millis(i64::try_from(millis).unwrap_or(i64::MAX))
}
