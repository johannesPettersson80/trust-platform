//! Deprecated compatibility wrapper for `trust-dev test`.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::cli::TestOutput;

pub fn run_test(
    project: Option<PathBuf>,
    filter: Option<String>,
    list: bool,
    timeout: u64,
    output: TestOutput,
    ci: bool,
) -> anyhow::Result<()> {
    let mut args = vec![OsString::from("test")];
    if let Some(project) = project {
        args.push(OsString::from("--project"));
        args.push(project.into_os_string());
    }
    if let Some(filter) = filter {
        args.push(OsString::from("--filter"));
        args.push(OsString::from(filter));
    }
    if list {
        args.push(OsString::from("--list"));
    }
    args.push(OsString::from("--timeout"));
    args.push(OsString::from(timeout.to_string()));
    args.push(OsString::from("--output"));
    args.push(OsString::from(test_output_arg(output)));
    if ci {
        args.push(OsString::from("--ci"));
    }
    crate::dev_forward::run_trust_dev_with_warning("test", args)
}

fn test_output_arg(output: TestOutput) -> &'static str {
    match output {
        TestOutput::Human => "human",
        TestOutput::Junit => "junit",
        TestOutput::Tap => "tap",
        TestOutput::Json => "json",
    }
}
