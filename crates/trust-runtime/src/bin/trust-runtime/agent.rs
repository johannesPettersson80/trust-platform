//! Deprecated compatibility wrapper for the agent workbench command.

use std::ffi::OsString;
use std::path::PathBuf;

pub fn run_agent_serve(project: Option<PathBuf>) -> anyhow::Result<()> {
    let mut args = vec![OsString::from("agent"), OsString::from("serve")];
    if let Some(project) = project {
        args.push(OsString::from("--project"));
        args.push(project.into_os_string());
    }
    crate::dev_forward::run_trust_dev_with_warning("agent serve", args)
}
