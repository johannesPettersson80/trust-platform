//! Deprecated compatibility wrapper for `trust-dev commit`.

use std::ffi::OsString;
use std::path::PathBuf;

pub fn run_commit(
    project: Option<PathBuf>,
    message: Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let mut args = vec![OsString::from("commit")];
    if let Some(project) = project {
        args.push(OsString::from("--project"));
        args.push(project.into_os_string());
    }
    if let Some(message) = message {
        args.push(OsString::from("--message"));
        args.push(OsString::from(message));
    }
    if dry_run {
        args.push(OsString::from("--dry-run"));
    }
    crate::dev_forward::run_trust_dev_with_warning("commit", args)
}
