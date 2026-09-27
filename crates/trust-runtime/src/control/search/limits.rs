//! OS-enforced worker memory budget; unsupported adapters fail closed.
pub(super) fn available() -> bool {
    cfg!(target_os = "linux")
}
#[cfg(target_os = "linux")]
pub(super) fn apply() -> Result<u64, String> {
    use nix::sys::resource::{getrlimit, setrlimit, Resource};
    const BUDGET: u64 = 1_073_741_824;
    let (_, hard) = getrlimit(Resource::RLIMIT_AS).map_err(|e| e.to_string())?;
    let limit = hard.min(BUDGET);
    setrlimit(Resource::RLIMIT_AS, limit, limit)
        .map_err(|e| format!("worker memory limit unavailable: {e}"))?;
    let (actual, _) = getrlimit(Resource::RLIMIT_AS).map_err(|e| e.to_string())?;
    Ok(actual)
}
#[cfg(not(target_os = "linux"))]
pub(super) fn apply() -> Result<u64, String> {
    Err(
        "bounded search requires a native process-memory-limit adapter; currently Linux only"
            .into(),
    )
}
