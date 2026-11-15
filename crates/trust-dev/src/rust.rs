//! `trust-dev rust`: the project's Rust POUs (specification 48).

use std::path::PathBuf;

use anyhow::{bail, Context};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::native::rust_pous::{write_declarations, Written, DECLARATIONS_FILE};

/// `trust-dev rust declarations`: writes the generated declarations of the project's crate
/// (specification 48 RU-24), and reports every error of the crate's sources where it is.
pub fn run_declarations(project: Option<PathBuf>) -> anyhow::Result<()> {
    let project = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    match write_declarations(&project) {
        Ok(Written::Wrote(count)) => println!("Wrote {DECLARATIONS_FILE}: {count} Rust POU(s)."),
        Ok(Written::UpToDate) => println!("{DECLARATIONS_FILE} is up to date."),
        Ok(Written::Removed) => {
            println!("Removed {DECLARATIONS_FILE}: the project declares no Rust POU.");
        }
        Ok(Written::Nothing) => println!("The project declares no Rust POU."),
        Err(errors) => {
            for error in &errors {
                eprintln!("{error}");
            }
            bail!(
                "the project's crate has {} error(s); {DECLARATIONS_FILE} is not written",
                errors.len()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "rust_tests.rs"]
mod tests;
