//! The isolated worker's memory limit, 1 GiB (protocol annex E8.1): its address space on Linux,
//! which the operating system limits, and its allocations elsewhere, which the program's allocator
//! holds to the budget. A worker without either refuses to run.
const LIMIT: usize = 1_073_741_824;
pub(super) fn available() -> bool {
    cfg!(target_os = "linux") || crate::memory_budget::reporting()
}
#[cfg(target_os = "linux")]
pub(super) fn apply() -> Result<u64, String> {
    use nix::sys::resource::{getrlimit, setrlimit, Resource};
    let (_, hard) = getrlimit(Resource::RLIMIT_AS).map_err(|e| e.to_string())?;
    let limit = hard.min(LIMIT as u64);
    setrlimit(Resource::RLIMIT_AS, limit, limit)
        .map_err(|e| format!("worker memory limit unavailable: {e}"))?;
    let (actual, _) = getrlimit(Resource::RLIMIT_AS).map_err(|e| e.to_string())?;
    Ok(actual)
}
#[cfg(not(target_os = "linux"))]
pub(super) fn apply() -> Result<u64, String> {
    let limit = crate::memory_budget::limit(LIMIT)?;
    u64::try_from(limit).map_err(|e| e.to_string())
}
