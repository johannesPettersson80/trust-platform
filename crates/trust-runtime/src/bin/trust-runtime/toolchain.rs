//! `trust-runtime toolchain`: the toolchain bundle of specification 44 §4.6. `status` names the
//! toolchain the tools use, `install` installs this release's bundle, downloaded or from a file,
//! and `bundle` writes one from a development toolchain folder; the builds install the bundle on
//! the first build that needs it through [`build_toolchain`].

use std::path::PathBuf;

use serde_json::json;
use trust_runtime::native::bundle::{self, Progress, TRUST_VERSION};
use trust_runtime::native::{NativeError, Toolchain, ToolchainSource};

use crate::cli::ToolchainAction;

pub fn run_toolchain(action: ToolchainAction) -> anyhow::Result<()> {
    match action {
        ToolchainAction::Status { json } => status(json),
        ToolchainAction::Install { from, json } => install(from, json),
        ToolchainAction::Bundle { folder, archive } => {
            let manifest = bundle::write_bundle(&folder, &archive)?;
            println!(
                "Wrote {}: truST {} for {}, rustc {}, {}, {} files",
                archive.display(),
                manifest.trust,
                manifest.platform,
                manifest.rustc,
                manifest.program_library,
                manifest.files.len()
            );
            Ok(())
        }
    }
}

/// The toolchain of a build of `trust-runtime` (specification 44 §4.6): the one the tools find, or
/// this release's bundle, installed now with its progress on the standard error.
pub fn build_toolchain() -> Result<Toolchain, NativeError> {
    let mut progress = HumanProgress::default();
    Toolchain::find_or_install(&mut |event| progress.report(&event))
}

fn status(json: bool) -> anyhow::Result<()> {
    let located = Toolchain::locate();
    let (folder, source) = match &located {
        Ok((toolchain, source)) => (
            toolchain.libraries.parent().map(PathBuf::from),
            Some(*source),
        ),
        Err(_) => (None, None),
    };
    let trust = match (&folder, source) {
        (Some(folder), Some(ToolchainSource::Installed)) => bundle::read_manifest(folder)
            .ok()
            .map(|manifest| manifest.trust),
        _ => None,
    };
    if json {
        println!(
            "{}",
            json!({
                "installed": located.is_ok(),
                "folder": folder.as_ref().map(|folder| folder.display().to_string()),
                "source": source.map(|source| match source {
                    ToolchainSource::Environment => "TRUST_TOOLCHAIN",
                    ToolchainSource::Installed => "installed",
                }),
                "trust": trust,
            })
        );
        return Ok(());
    }
    match (&located, folder) {
        (Ok((toolchain, ToolchainSource::Environment)), Some(folder)) => println!(
            "TRUST_TOOLCHAIN names the toolchain in {} (rustc {})",
            folder.display(),
            toolchain.version
        ),
        (Ok((toolchain, ToolchainSource::Installed)), Some(folder)) => println!(
            "The truST toolchain {} is installed in {} (rustc {})",
            trust.as_deref().unwrap_or(TRUST_VERSION),
            folder.display(),
            toolchain.version
        ),
        (Err(error), _) => println!("{error}"),
        (Ok(_), None) => println!("The toolchain's folder cannot be named"),
    }
    Ok(())
}

fn install(from: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    if std::env::var_os("TRUST_TOOLCHAIN").is_some() && !json {
        eprintln!("note: TRUST_TOOLCHAIN is set, so the tools use its folder and not this bundle");
    }
    let mut human = HumanProgress::default();
    let mut report = |event: Progress| {
        if json {
            let line = match &event {
                Progress::Download { bytes, total } => {
                    json!({"event": "download", "bytes": bytes, "total": total})
                }
                Progress::Verify => json!({"event": "verify"}),
                Progress::Installed { folder } => {
                    json!({"event": "installed", "folder": folder.display().to_string()})
                }
            };
            println!("{line}");
        } else {
            human.report(&event);
        }
    };
    let installed = match &from {
        Some(file) => bundle::install_from(file, &mut report),
        None => bundle::download_and_install(&mut report),
    };
    match installed {
        Ok(_) => Ok(()),
        Err(error) => {
            if json {
                println!("{}", json!({"event": "failed", "error": error.to_string()}));
            }
            Err(error.into())
        }
    }
}

/// The progress of an install for a person, on the standard error: the download every 10 %, or
/// every 25 MB when its length is unknown.
#[derive(Default)]
struct HumanProgress {
    started: bool,
    shown: u64,
}

impl HumanProgress {
    fn report(&mut self, event: &Progress) {
        match event {
            Progress::Download { bytes, total } => {
                if !self.started {
                    self.started = true;
                    eprintln!(
                        "Downloading the truST toolchain {TRUST_VERSION} (about 220 MB, once)..."
                    );
                }
                let step = total.map_or(25 << 20, |total| (total / 10).max(1));
                if *bytes >= self.shown + step || total.is_some_and(|total| *bytes == total) {
                    self.shown = *bytes;
                    match total {
                        Some(total) => eprintln!("  {} of {} MB", bytes >> 20, total >> 20),
                        None => eprintln!("  {} MB", bytes >> 20),
                    }
                }
            }
            Progress::Verify => eprintln!("Checking the toolchain against its manifest..."),
            Progress::Installed { folder } => {
                eprintln!("The truST toolchain is installed in {}", folder.display());
            }
        }
    }
}
