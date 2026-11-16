//! `trust-dev rust`: the project's Rust POUs (specification 48).

use std::path::PathBuf;

use anyhow::{bail, Context};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::native::rust_pous::{layout, write_generated, Written};

/// `trust-dev rust declarations`: writes the generated declarations of the project's crate
/// (specification 48 RU-24), and a Rust-first project's configuration (RU-67), and reports every
/// error of the crate's sources and of `trust.toml` where it is.
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
    Ok(())
}

#[cfg(test)]
#[path = "rust_tests.rs"]
mod tests;
