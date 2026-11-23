//! Finding the installed trust-runtime and checking its version (specification 48 RU-58, "Finding
//! the runtime").

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

/// The runtime's program name on this computer.
const PROGRAM: &str = if cfg!(windows) {
    "trust-runtime.exe"
} else {
    "trust-runtime"
};

/// The extension folders of VS Code and its editions, under the home folder.
const EXTENSION_FOLDERS: &[&str] = &[
    ".vscode/extensions",
    ".vscode-insiders/extensions",
    ".vscode-oss/extensions",
    ".vscode-server/extensions",
    ".cursor/extensions",
];

/// The start of a truST extension's folder name: its publisher and name.
const EXTENSION_PREFIX: &str = "trust-platform.trust-lsp-";

/// The installed trust-runtime: the one `TRUST_RUNTIME` names, else `trust-runtime` on the `PATH`,
/// else the runtime of the newest truST extension in the extension folders. Its version is checked
/// by [`check_version`] where the project opens.
///
/// # Errors
///
/// [`Error::RuntimeNotFound`] with the places looked in.
pub(crate) fn installed() -> Result<PathBuf> {
    let mut looked = Vec::new();
    let found = from_environment(&mut looked)
        .or_else(|| on_path(&mut looked))
        .or_else(|| in_extensions(&mut looked));
    let Some(runtime) = found else {
        return Err(Error::RuntimeNotFound {
            looked: looked.join("; "),
        });
    };
    Ok(runtime)
}

fn from_environment(looked: &mut Vec<String>) -> Option<PathBuf> {
    let named = std::env::var_os("TRUST_RUNTIME")?;
    let path = PathBuf::from(named);
    looked.push(format!("TRUST_RUNTIME ({})", path.display()));
    path.is_file().then_some(path)
}

fn on_path(looked: &mut Vec<String>) -> Option<PathBuf> {
    looked.push("the PATH".to_string());
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|folder| folder.join(PROGRAM))
        .find(|candidate| candidate.is_file())
}

fn in_extensions(looked: &mut Vec<String>) -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    let home = PathBuf::from(home);
    let mut newest: Option<(Vec<u64>, PathBuf)> = None;
    for folder in EXTENSION_FOLDERS {
        let folder = home.join(folder);
        looked.push(folder.display().to_string());
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(version) = name.strip_prefix(EXTENSION_PREFIX).map(version_numbers) else {
                continue;
            };
            let program = entry.path().join("bin").join(PROGRAM);
            if program.is_file() && newest.as_ref().is_none_or(|(best, _)| version > *best) {
                newest = Some((version, program));
            }
        }
    }
    newest.map(|(_, program)| program)
}

/// The numbers of a version such as `0.44.1` or `0.44.1-linux-x64`, which compare as versions do.
fn version_numbers(text: &str) -> Vec<u64> {
    text.split(['.', '-'])
        .map_while(|part| part.parse().ok())
        .collect()
}

/// Refuses a runtime whose version is not trust-sim's own: the SDK's crates and the runtime are
/// released together (specification 48 RU-12).
pub(crate) fn check_version(runtime: &Path) -> Result<()> {
    let output = Command::new(runtime)
        .arg("--version")
        .output()
        .map_err(|error| Error::Simulator(format!("{} --version: {error}", runtime.display())))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.split_whitespace().nth(1).unwrap_or_default();
    if version == env!("CARGO_PKG_VERSION") {
        Ok(())
    } else {
        Err(Error::RuntimeVersion {
            path: runtime.to_path_buf(),
            runtime: version.to_string(),
            sim: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::version_numbers;

    #[test]
    fn extension_versions_compare_as_versions_do() {
        // A newer extension wins whatever the platform suffix of its folder: 0.44.10 is newer than
        // 0.44.9, which text would order the other way. Catches a comparison of the folder names as
        // text, which would take the older runtime.
        assert!(version_numbers("0.44.10-linux-x64") > version_numbers("0.44.9-linux-x64"));
        assert_eq!(version_numbers("0.44.1"), [0, 44, 1]);
        assert_eq!(version_numbers("0.44.1-darwin-arm64"), [0, 44, 1]);
    }
}
