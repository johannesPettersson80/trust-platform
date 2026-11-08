//! `trust-runtime toolchain`: the toolchain bundle of specification 44 §4.6. `status` names the
//! toolchain the tools use, `install` installs this release's bundle (the one beside the tools,
//! downloaded, or from a file), and `bundle` writes one from a development toolchain folder; a
//! build that needs it and finds none installs this release's bundle only with the person's consent
//! ([`build_toolchain`]).

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Whether `--install-toolchain` gave the consent of [`build_toolchain`] for this process.
static INSTALL_CONSENT: AtomicBool = AtomicBool::new(false);

/// `--install-toolchain`: a build installs the toolchain it needs without asking.
pub fn consent_to_install() {
    INSTALL_CONSENT.store(true, Ordering::Relaxed);
}

/// The toolchain of a build of `trust-runtime` (specification 44 §4.6, NX-08): the one the tools
/// find; else this release's bundle, installed now with its progress on the standard error, with
/// `--install-toolchain` or when the person answers yes in a terminal to the question that names
/// where it comes from, its size and its folder. Nothing is installed silently: a build without
/// either fails with the advice to install it.
///
/// # Errors
///
/// The error of a toolchain that is not there and not installed, or of the install.
pub fn build_toolchain() -> Result<Toolchain, NativeError> {
    let not_found = match Toolchain::find() {
        Ok(toolchain) => return Ok(toolchain),
        Err(error) if std::env::var_os("TRUST_TOOLCHAIN").is_some() => return Err(error),
        Err(error) => error,
    };
    if !INSTALL_CONSENT.load(Ordering::Relaxed) && !agreed_in_terminal() {
        let advice = match not_found {
            NativeError::Toolchain(message) => message,
            other => other.to_string(),
        };
        return Err(NativeError::Toolchain(format!(
            "{advice}; or build again with `--install-toolchain`"
        )));
    }
    let mut progress = HumanProgress::default();
    let folder = bundle::install(&mut |event| progress.report(&event))?;
    Toolchain::at(&folder)
}

/// Whether the person, asked in a terminal, agrees to install this release's bundle: the question
/// names where it comes from, its size and its folder, and anything but yes declines. False
/// without a terminal on the standard input and error, where nobody can answer.
fn agreed_in_terminal() -> bool {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return false;
    }
    let folder = bundle::installed_folder().map_or_else(
        || "truST's data folder".to_string(),
        |folder| folder.display().to_string(),
    );
    eprint!(
        "The truST toolchain {TRUST_VERSION} is not installed. It comes from the bundle beside the \
         tools, or is downloaded once (about 180 to 210 MB), and takes about 630 MB in {folder}. \
         Install it now? [y/N] "
    );
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
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
        None => bundle::install(&mut report),
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
                        "Downloading the truST toolchain {TRUST_VERSION} (about 180 to 210 MB, once)..."
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
