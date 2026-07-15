//! Small adapter utilities.
//! - is_configuration_request: gate config-time requests
//! - env_flag: parse boolean env vars

pub(super) fn is_configuration_request(command: &str) -> bool {
    matches!(
        command,
        "initialize"
            | "launch"
            | "attach"
            | "setBreakpoints"
            | "setExceptionBreakpoints"
            | "setFunctionBreakpoints"
            | "setInstructionBreakpoints"
            | "setDataBreakpoints"
            | "configurationDone"
    )
}

pub(super) fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(raw) => {
            let value = raw.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}
