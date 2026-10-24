//! The toolchain bundle (specification 44 §4.6): the archive in which the tools carry the
//! compiler, its manifest, where an installed bundle lies, and the check of a bundle's files
//! against its manifest. Installing a bundle is `bundle/install.rs`, writing one `bundle/pack.rs`.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::NativeError;
use trust_runtime_core::native::image::Target;

mod install;
mod pack;

pub use install::{download_and_install, install_from, Installer, Progress};
pub use pack::write_bundle;

/// The format of `manifest.json` this release writes and reads.
pub const MANIFEST_FORMAT: u32 = 1;

/// The manifest's file at the bundle's root.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Where the releases' assets are downloaded from: `<this><v<version>>/<asset>`.
pub const RELEASES: &str =
    "https://github.com/johannesPettersson80/trust-platform/releases/download/";

/// The module targets of §4.4, for each of which a bundle carries the compiler's `core` and the
/// program library.
pub const MODULE_TARGETS: [Target; 4] = [
    Target::Rv32imac,
    Target::ThumbV8mMainHf,
    Target::X86_64,
    Target::Aarch64,
];

/// The truST version whose bundle these tools use, the kernel's.
pub const TRUST_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A bundle's manifest (§4.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// [`MANIFEST_FORMAT`].
    pub format: u32,
    /// The truST version, the kernel's.
    pub trust: String,
    /// The computer platform the bundle's compiler runs on.
    pub platform: String,
    /// The pinned Rust version.
    pub rustc: String,
    /// The program library, `trust-plc-lib <version>`.
    pub program_library: String,
    /// The module targets the bundle builds for.
    pub targets: Vec<String>,
    /// Every other file of the bundle.
    pub files: Vec<ManifestFile>,
}

/// One file of a bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFile {
    /// The path below the bundle's root, its parts joined with `/`.
    pub path: String,
    /// The file's length.
    pub bytes: u64,
    /// The SHA-256 of its bytes, in lowercase hexadecimal.
    pub sha256: String,
    /// Whether it is a program the tools run.
    pub executable: bool,
}

/// The platform of this computer as the release names it (§4.6), or `None` for one the release
/// builds no tools for.
#[must_use]
pub fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        ("macos", "x86_64") => Some("darwin-x64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("windows", "x86_64") => Some("win32-x64"),
        _ => None,
    }
}

/// The archive's name of the bundle of `platform`: `trust-toolchain-<platform>.tar.gz`.
#[must_use]
pub fn archive_name(platform: &str) -> String {
    format!("trust-toolchain-{platform}.tar.gz")
}

/// truST's data folder of the user: `$XDG_DATA_HOME/trust`, else `~/.local/share/trust`, on Linux;
/// `~/Library/Application Support/trust` on macOS; `%LOCALAPPDATA%\trust` on Windows.
#[must_use]
pub fn data_folder() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|folder| PathBuf::from(folder).join("trust"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/trust"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|folder| !folder.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
            .map(|folder| folder.join("trust"))
    }
}

/// The folder of the installed bundles, `toolchains/` of truST's data folder.
#[must_use]
pub fn toolchains_folder() -> Option<PathBuf> {
    data_folder().map(|folder| folder.join("toolchains"))
}

/// The folder of this release's installed bundle, `toolchains/<truST version>/`, whether or not
/// it is there.
#[must_use]
pub fn installed_folder() -> Option<PathBuf> {
    toolchains_folder().map(|folder| folder.join(TRUST_VERSION))
}

/// The SHA-256 of the file at `path`, in lowercase hexadecimal, and its length.
///
/// # Errors
///
/// [`NativeError::Toolchain`] when the file cannot be read.
pub fn file_sha256(path: &Path) -> Result<(String, u64), NativeError> {
    let failed =
        |error: std::io::Error| NativeError::Toolchain(format!("{}: {error}", path.display()));
    let mut file = std::fs::File::open(path).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut bytes = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(failed)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes += read as u64;
    }
    Ok((hex::encode(hasher.finalize()), bytes))
}

/// The path of `path` below `root`, its parts joined with `/`, or `None` when it is not a plain
/// path below it (an absolute path, a `..`, a drive prefix).
#[must_use]
pub fn relative(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Checks the bundle in `folder` against its manifest (§4.6): every file it lists has its length
/// and SHA-256, and the folder holds no file it does not list.
///
/// # Errors
///
/// [`NativeError::Toolchain`] naming the first file that is missing, changed or not listed, or a
/// manifest that cannot be read.
pub fn verify(folder: &Path) -> Result<Manifest, NativeError> {
    let manifest = read_manifest(folder)?;
    let mut listed = std::collections::BTreeSet::new();
    for file in &manifest.files {
        let Some(path) = relative(Path::new(&file.path)) else {
            return Err(NativeError::Toolchain(format!(
                "the manifest names a file outside the bundle: {}",
                file.path
            )));
        };
        let (sha256, bytes) = file_sha256(&folder.join(&path))?;
        if bytes != file.bytes || sha256 != file.sha256 {
            return Err(NativeError::Toolchain(format!(
                "{} is not the file the manifest names",
                folder.join(&path).display()
            )));
        }
        listed.insert(path);
    }
    for path in files_below(folder)? {
        if path != MANIFEST_FILE && !listed.contains(&path) {
            return Err(NativeError::Toolchain(format!(
                "{} is not in the manifest",
                folder.join(&path).display()
            )));
        }
    }
    Ok(manifest)
}

/// The manifest of the bundle in `folder`.
///
/// # Errors
///
/// [`NativeError::Toolchain`] when it cannot be read, is not a manifest, or has another format.
pub fn read_manifest(folder: &Path) -> Result<Manifest, NativeError> {
    let path = folder.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
    let manifest: Manifest = serde_json::from_str(&text)
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
    if manifest.format != MANIFEST_FORMAT {
        return Err(NativeError::Toolchain(format!(
            "{} has format {}, this release reads {MANIFEST_FORMAT}",
            path.display(),
            manifest.format
        )));
    }
    Ok(manifest)
}

/// Every file below `folder`, by its path joined with `/`, in order.
fn files_below(folder: &Path) -> Result<Vec<String>, NativeError> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", current.display())))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                NativeError::Toolchain(format!("{}: {error}", current.display()))
            })?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
            if kind.is_dir() {
                pending.push(path);
            } else {
                let below = path
                    .strip_prefix(folder)
                    .ok()
                    .and_then(relative)
                    .ok_or_else(|| {
                        NativeError::Toolchain(format!("{} cannot be named", path.display()))
                    })?;
                found.push(below);
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
#[path = "bundle/tests.rs"]
mod tests;
