//! `trust-dev rust`: the project's Rust POUs (specification 48).

use std::path::PathBuf;

use anyhow::{bail, Context};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::collect_project_sources_labelled;
use trust_runtime::native::rust_pous::{layout, read, write_generated, Written, BINDINGS_FOLDER};
use trust_runtime::native::{conversion_profile, write_bindings};

/// `trust-dev rust declarations`: writes the generated declarations of the project's crate
/// (specification 48 RU-24), a Rust-first project's configuration (RU-67) and, for a crate that
/// calls Structured Text, its bindings (RU-39), and reports every error of the crate's sources,
/// of `trust.toml` and of the Structured Text where it is.
pub fn run_declarations(project: Option<PathBuf>) -> anyhow::Result<()> {
    let project = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let layout = layout(&project).map_err(|message| anyhow::anyhow!("trust.toml: {message}"))?;
    let declarations = &layout.declarations;
    match write_generated(&project) {
        Ok((written, configuration)) => {
            match written {
                Written::Wrote(count) => println!("Wrote {declarations}: {count} Rust POU(s)."),
                Written::UpToDate => println!("{declarations} is up to date."),
                Written::Removed => {
                    println!("Removed {declarations}: the project declares no Rust POU.");
                }
                Written::Nothing => println!("The project declares no Rust POU."),
            }
            if let (Some(configuration), Some(file)) = (configuration, &layout.configuration) {
                match configuration {
                    Written::UpToDate => println!("{file} is up to date."),
                    _ => println!("Wrote {file} from trust.toml."),
                }
            }
        }
        Err(errors) => {
            for error in &errors {
                eprintln!("{error}");
            }
            bail!(
                "the project has {} error(s); {declarations} is not written",
                errors.len()
            );
        }
    }
    // The bindings, generated from the Structured Text with the declarations just written, which
    // rust-analyzer and the crate's own cargo find before the first build (RU-39, RU-77).
    if matches!(read(&project), Ok(Some(krate)) if !krate.structured_text.is_empty()) {
        let sources = collect_project_sources_labelled(&project, &[])?;
        let conversions = conversion_profile(&project)?;
        if let Err(error) = write_bindings(&project, &sources, conversions) {
            bail!("{BINDINGS_FOLDER} is not written: {error}");
        }
        println!("Wrote {BINDINGS_FOLDER}: the bindings of the Structured Text the crate calls.");
    }
    Ok(())
}

#[cfg(test)]
#[path = "rust_tests.rs"]
mod tests;
