//! Installing a toolchain bundle (specification 44 §4.6): from a file, or downloaded from the
//! release with its SHA-256 checked against the release's `SHA256SUMS`; unpacked into a new folder
//! beside its destination, checked against its manifest, and moved into place only then. Installs
//! take turns on a lock file.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};

use super::{
    archive_name, platform, relative, toolchains_folder, verify, Manifest, RELEASES, TRUST_VERSION,
};
use crate::native::NativeError;

/// What an install has reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// The download has `bytes` of `total`, when the server said.
    Download {
        /// The bytes downloaded.
        bytes: u64,
        /// The archive's length.
        total: Option<u64>,
    },
    /// The bundle is being checked against its manifest.
    Verify,
    /// The bundle is in place.
    Installed {
        /// Where it lies.
        folder: PathBuf,
    },
}

/// Where and what an install installs: the bundle of `version` for `platform` into
/// `<toolchains>/<version>/`.
#[derive(Clone, Debug)]
pub struct Installer {
    /// The folder of the installed bundles.
    pub toolchains: PathBuf,
    /// The truST version of the bundle.
    pub version: String,
    /// The platform of the bundle.
    pub platform: String,
}

impl Installer {
    /// The installer of this release's bundle for this computer, into truST's data folder.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] on a computer the release builds no bundle for, or a user
    /// without a data folder.
    pub fn for_this_computer() -> Result<Self, NativeError> {
        let platform = platform().ok_or_else(|| {
            NativeError::Toolchain(String::from(
                "the release builds no toolchain for this computer's platform",
            ))
        })?;
        let toolchains = toolchains_folder().ok_or_else(|| {
            NativeError::Toolchain(String::from(
                "the user has no data folder for the toolchain",
            ))
        })?;
        Ok(Self {
            toolchains,
            version: TRUST_VERSION.to_string(),
            platform: platform.to_string(),
        })
    }

    /// The installed bundle's folder.
    #[must_use]
    pub fn folder(&self) -> PathBuf {
        self.toolchains.join(&self.version)
    }

    /// Installs the bundle file `archive` (§4.6, `--from`), unless the bundle is in place.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when the file is no bundle of this version and platform, a file
    /// of it is not the one its manifest names, or the folder cannot be written; nothing is then
    /// installed.
    pub fn install_file(
        &self,
        archive: &Path,
        report: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, NativeError> {
        let _lock = self.lock()?;
        if let Some(folder) = self.in_place() {
            report(Progress::Installed {
                folder: folder.clone(),
            });
            return Ok(folder);
        }
        self.unpack_and_place(archive, report)
    }

    /// Downloads the bundle of the release from `base` (`<base>v<version>/<asset>`), checks its
    /// SHA-256 against the release's `SHA256SUMS` and installs it, unless it is in place.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when a download fails, the release lists no such bundle, its
    /// SHA-256 differs, or the install fails.
    pub fn download(
        &self,
        base: &str,
        report: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, NativeError> {
        let _lock = self.lock()?;
        if let Some(folder) = self.in_place() {
            report(Progress::Installed {
                folder: folder.clone(),
            });
            return Ok(folder);
        }
        let release = format!("{base}v{}/", self.version);
        let name = archive_name(&self.platform);
        let sums = fetch_text(&format!("{release}SHA256SUMS"))?;
        // `sha256sum` lines, `<hash>  <path>`, the path with the folder the release job kept it in.
        let expected = sums
            .lines()
            .filter_map(|line| line.split_once(char::is_whitespace))
            .find(|(_, file)| {
                file.trim().trim_start_matches('*').rsplit('/').next() == Some(name.as_str())
            })
            .map(|(hash, _)| hash.to_ascii_lowercase())
            .ok_or_else(|| {
                NativeError::Toolchain(format!("the release v{} lists no {name}", self.version))
            })?;
        let partial = self
            .toolchains
            .join(format!(".{name}.{}.part", std::process::id()));
        let downloaded = fetch_file(&format!("{release}{name}"), &partial, report);
        let installed = downloaded.and_then(|sha256| {
            if sha256 == expected {
                self.unpack_and_place(&partial, report)
            } else {
                Err(NativeError::Toolchain(format!(
                    "{name} has the SHA-256 {sha256}, the release lists {expected}"
                )))
            }
        });
        let _ = std::fs::remove_file(&partial);
        installed
    }

    /// The bundle's folder when it is in place with a manifest of this version and platform.
    fn in_place(&self) -> Option<PathBuf> {
        let folder = self.folder();
        super::read_manifest(&folder)
            .ok()
            .filter(|manifest| self.matches(manifest))
            .map(|_| folder)
    }

    fn matches(&self, manifest: &Manifest) -> bool {
        manifest.trust == self.version && manifest.platform == self.platform
    }

    /// The lock that installs take turns on, held until dropped.
    fn lock(&self) -> Result<File, NativeError> {
        std::fs::create_dir_all(&self.toolchains).map_err(|error| {
            NativeError::Toolchain(format!("{}: {error}", self.toolchains.display()))
        })?;
        let path = self.toolchains.join(".lock");
        let file = File::create(&path)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
        file.lock()
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
        Ok(file)
    }

    /// Unpacks `archive` into a new folder beside the destination, checks it and moves it into
    /// place; a failure removes the new folder.
    fn unpack_and_place(
        &self,
        archive: &Path,
        report: &mut dyn FnMut(Progress),
    ) -> Result<PathBuf, NativeError> {
        let staging = self
            .toolchains
            .join(format!(".{}.{}.new", self.version, std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        let placed = unpack(archive, &staging).and_then(|()| {
            report(Progress::Verify);
            let manifest = verify(&staging)?;
            if !self.matches(&manifest) {
                return Err(NativeError::Toolchain(format!(
                    "the bundle is truST {} for {}, this is truST {} on {}",
                    manifest.trust, manifest.platform, self.version, self.platform
                )));
            }
            set_executables(&staging, &manifest)?;
            let folder = self.folder();
            // A folder left by an install of this version that was stopped, or of another platform.
            if folder.exists() {
                std::fs::remove_dir_all(&folder).map_err(|error| {
                    NativeError::Toolchain(format!("{}: {error}", folder.display()))
                })?;
            }
            std::fs::rename(&staging, &folder).map_err(|error| {
                NativeError::Toolchain(format!("{}: {error}", folder.display()))
            })?;
            Ok(folder)
        });
        match placed {
            Ok(folder) => {
                report(Progress::Installed {
                    folder: folder.clone(),
                });
                Ok(folder)
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                Err(error)
            }
        }
    }
}

/// Installs this release's bundle for this computer from the file `archive`.
///
/// # Errors
///
/// As [`Installer::install_file`].
pub fn install_from(
    archive: &Path,
    report: &mut dyn FnMut(Progress),
) -> Result<PathBuf, NativeError> {
    Installer::for_this_computer()?.install_file(archive, report)
}

/// Downloads and installs this release's bundle for this computer from the release.
///
/// # Errors
///
/// As [`Installer::download`].
pub fn download_and_install(report: &mut dyn FnMut(Progress)) -> Result<PathBuf, NativeError> {
    Installer::for_this_computer()?.download(RELEASES, report)
}

/// Unpacks the gzip-compressed tar archive `archive` into the new folder `folder`: only folders and
/// plain files, each below the folder.
fn unpack(archive: &Path, folder: &Path) -> Result<(), NativeError> {
    let failed =
        |error: std::io::Error| NativeError::Toolchain(format!("{}: {error}", archive.display()));
    std::fs::create_dir_all(folder)
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", folder.display())))?;
    let file = File::open(archive).map_err(failed)?;
    let mut tar = tar::Archive::new(GzDecoder::new(file));
    for entry in tar.entries().map_err(failed)? {
        let mut entry = entry.map_err(failed)?;
        let path = entry.path().map_err(failed)?.into_owned();
        let Some(below) = relative(&path) else {
            return Err(NativeError::Toolchain(format!(
                "{} holds a path outside the bundle: {}",
                archive.display(),
                path.display()
            )));
        };
        let target = folder.join(&below);
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|error| {
                NativeError::Toolchain(format!("{}: {error}", target.display()))
            })?;
        } else if kind.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    NativeError::Toolchain(format!("{}: {error}", parent.display()))
                })?;
            }
            let mut out = File::create(&target).map_err(|error| {
                NativeError::Toolchain(format!("{}: {error}", target.display()))
            })?;
            std::io::copy(&mut entry, &mut out).map_err(failed)?;
        } else {
            return Err(NativeError::Toolchain(format!(
                "{} holds {below}, which is not a plain file",
                archive.display()
            )));
        }
    }
    Ok(())
}

/// Marks the files the manifest names as programs executable, on systems with such a mark.
fn set_executables(folder: &Path, manifest: &Manifest) -> Result<(), NativeError> {
    #[cfg(unix)]
    for file in manifest.files.iter().filter(|file| file.executable) {
        use std::os::unix::fs::PermissionsExt;
        let path = folder.join(&file.path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
    }
    #[cfg(not(unix))]
    let _ = (folder, manifest);
    Ok(())
}

/// The text at `url`.
fn fetch_text(url: &str) -> Result<String, NativeError> {
    let mut text = String::new();
    ureq::get(url)
        .call()
        .map_err(|error| NativeError::Toolchain(format!("{url}: {error}")))?
        .into_body()
        .into_reader()
        .read_to_string(&mut text)
        .map_err(|error| NativeError::Toolchain(format!("{url}: {error}")))?;
    Ok(text)
}

/// Downloads `url` into the file `path`, reporting its progress, and returns its SHA-256.
fn fetch_file(
    url: &str,
    path: &Path,
    report: &mut dyn FnMut(Progress),
) -> Result<String, NativeError> {
    let failed = |error: std::io::Error| NativeError::Toolchain(format!("{url}: {error}"));
    let response = ureq::get(url)
        .call()
        .map_err(|error| NativeError::Toolchain(format!("{url}: {error}")))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let mut body = response.into_body();
    let mut reader = body.with_config().limit(u64::MAX).reader();
    let mut out = File::create(path)
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut bytes = 0u64;
    let mut reported = 0u64;
    report(Progress::Download { bytes, total });
    loop {
        let read = reader.read(&mut buffer).map_err(failed)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read]).map_err(failed)?;
        bytes += read as u64;
        // About one report for every megabyte, and the last.
        if bytes - reported >= 1 << 20 {
            reported = bytes;
            report(Progress::Download { bytes, total });
        }
    }
    report(Progress::Download { bytes, total });
    Ok(hex::encode(hasher.finalize()))
}
