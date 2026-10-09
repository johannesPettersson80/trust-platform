//! `trust-runtime hmi check` (specification 39, §18 phase 5 item 6): the editor's checks over every
//! HMI file of a project, for its own CI: the files load, the assets the pages use are there, the
//! bindings resolve in the program with a compatible type and direction, and the controls are
//! within the write policy of `runtime.toml`. A problem fails the command.

use std::path::PathBuf;

use anyhow::Context;
use serde_json::json;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::harness::CompileSession;
use trust_runtime::hmi::access::config::hmi_access_from_runtime_toml;
use trust_runtime::hmi::bundle::checks::{check_bundle, Program};
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::hmi::catalogue::Catalogue;

pub fn run_hmi_check(project: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    let root = match project {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let bundle = HmiBundle::read(&root)
        .with_context(|| format!("read the HMI files of {}", root.display()))?;
    let sources = collect_project_source_files(&root, None)?;
    let runtime = CompileSession::from_sources(sources)
        .build_runtime()
        .map_err(|error| anyhow::anyhow!("the program does not compile: {error}"))?;
    let metadata = runtime.metadata_snapshot();
    let program = Program {
        metadata: &metadata,
        storage: runtime.storage(),
    };
    let policy = match std::fs::read_to_string(root.join("runtime.toml")) {
        Ok(text) => {
            hmi_access_from_runtime_toml(&text)
                .map_err(anyhow::Error::msg)?
                .write
        }
        Err(_) => Vec::new(),
    };
    let problems = check_bundle(&bundle, Catalogue::shipped(), &program, &policy);
    if json {
        let listed: Vec<_> = problems
            .iter()
            .map(|problem| json!({ "file": problem.file, "line": problem.line, "message": problem.message }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({ "ok": problems.is_empty(), "problems": listed })
            )?
        );
    } else if problems.is_empty() {
        println!("HMI check passed: {} files", bundle.files.len());
    } else {
        for problem in &problems {
            println!("{}:{}: {}", problem.file, problem.line, problem.message);
        }
        println!("{} problem(s)", problems.len());
    }
    if !problems.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
