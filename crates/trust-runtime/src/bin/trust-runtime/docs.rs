//! Deprecated compatibility wrapper for `trust-dev docs`.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::cli::DocsFormat;

pub fn run_docs(
    project: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    format: DocsFormat,
) -> anyhow::Result<()> {
    let mut args = vec![OsString::from("docs")];
    if let Some(project) = project {
        args.push(OsString::from("--project"));
        args.push(project.into_os_string());
    }
    if let Some(out_dir) = out_dir {
        args.push(OsString::from("--out-dir"));
        args.push(out_dir.into_os_string());
    }
    args.push(OsString::from("--format"));
    args.push(OsString::from(docs_format_arg(format)));
    crate::dev_forward::run_trust_dev_with_warning("docs", args)
}

fn docs_format_arg(format: DocsFormat) -> &'static str {
    match format {
        DocsFormat::Markdown => "markdown",
        DocsFormat::Html => "html",
        DocsFormat::Both => "both",
    }
}
