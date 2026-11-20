//! What `trust-dev service install` puts under the service manager (specification 48 RU-64): the
//! service's settings from the project's `trust.toml`, the release binary of its package, which
//! cargo's metadata names, and the user it runs as. Everything here only reads, so that `install`
//! refuses what it cannot install before it asks for root's rights.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};

/// One service of one project, as `install` puts it under the service manager.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    /// The service's name, `[services.<name>]` of the project's `trust.toml`.
    pub(crate) name: String,
    /// The project folder, absolute: the service's working folder and its `TRUST_PROJECT`.
    pub(crate) project: PathBuf,
    /// The binary the service runs, absolute.
    pub(crate) binary: PathBuf,
}

/// The user a service runs as on Linux and macOS: the project folder's owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Owner {
    /// The user's id.
    pub(crate) uid: u32,
    /// The user's name, which the definitions name.
    pub(crate) name: String,
}

/// The plan of the service `name` of the project in `project`, with the binary `binary` when the
/// command names it, else the release binary of the service's package.
///
/// # Errors
///
/// A name that is no identifier, a `trust.toml` that does not read or declares no such service, a
/// service without `package` or `endpoint`, a `credentials` file that does not exist, and a binary
/// that cannot be found or does not exist, each naming what to do.
pub(crate) fn resolve(project: &Path, name: &str, binary: Option<&Path>) -> anyhow::Result<Plan> {
    check_name(name)?;
    let project = absolute(project)?;
    check_path(&project, "the project folder")?;
    let file = project.join("trust.toml");
    let text =
        std::fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let document: toml::Table =
        toml::from_str(&text).with_context(|| format!("read {}", file.display()))?;
    let service = document
        .get("services")
        .and_then(|services| services.get(name))
        .and_then(toml::Value::as_table)
        .with_context(|| format!("trust.toml declares no [services.{name}]"))?;
    let setting = |key: &str| {
        service
            .get(key)
            .and_then(toml::Value::as_str)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("[services.{name}] has no {key}"))
    };
    let package = setting("package")?;
    setting("endpoint")?;
    let credentials = setting("credentials")?;
    let token = project.join(credentials);
    if !token.is_file() {
        bail!(
            "the service token {} does not exist, so the service could not connect: create it with \
             `trust-runtime ctl service-token {name} --out {credentials}`",
            token.display()
        );
    }
    let binary = match binary {
        Some(binary) => {
            let binary = absolute(binary)?;
            if !binary.is_file() {
                bail!("{} is no file", binary.display());
            }
            binary
        }
        None => release_binary(&project, package)?,
    };
    check_path(&binary, "the binary")?;
    Ok(Plan {
        name: name.to_string(),
        project,
        binary,
    })
}

/// Refuses a service name that is no identifier, which its definitions' names hold
/// (specification 48 RU-63: a service's name is an identifier).
pub(crate) fn check_name(name: &str) -> anyhow::Result<()> {
    let mut characters = name.chars();
    let first = characters.next();
    if first.is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|next| next.is_ascii_alphanumeric() || next == '_')
    {
        return Ok(());
    }
    bail!("`{name}` is no service name: a service's name is an identifier (specification 48 RU-63)")
}

/// The release binary of the Cargo package `package` in the project folder `project`: the
/// package's one binary in the release folder of the target folder cargo's metadata names.
fn release_binary(project: &Path, package: &str) -> anyhow::Result<PathBuf> {
    let manifests = package_manifests(project, package)?;
    let manifest = match manifests.as_slice() {
        [manifest] => manifest,
        [] => bail!(
            "the project folder holds no Cargo package {package}, the service's package: create it \
             there with `cargo new --bin {package}`"
        ),
        several => bail!(
            "the project folder holds the Cargo package {package} {} times: {}",
            several.len(),
            several
                .iter()
                .map(|manifest| relative(project, manifest))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let metadata = cargo_metadata(manifest)?;
    let (target, binaries) = package_binaries(&metadata, manifest)?;
    let binary = match binaries.as_slice() {
        [binary] => target
            .join("release")
            .join(format!("{binary}{}", std::env::consts::EXE_SUFFIX)),
        [] => bail!(
            "the Cargo package {package} has no binary: a service is a program (specification 48 \
             RU-61)"
        ),
        several => bail!(
            "the Cargo package {package} has {} binaries ({}): a service's package has one",
            several.len(),
            several.join(", ")
        ),
    };
    if !binary.is_file() {
        bail!(
            "{} does not exist: build the service first, `cargo build --release --manifest-path {}`",
            binary.display(),
            relative(project, manifest)
        );
    }
    Ok(binary)
}

/// The manifests in the project folder `project` that declare the package `package`, in order.
/// The search passes over `target` folders, `node_modules` and hidden folders, and does not follow
/// links.
fn package_manifests(project: &Path, package: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut folders = vec![project.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries =
            std::fs::read_dir(&folder).with_context(|| format!("read {}", folder.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("read {}", folder.display()))?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let kind = entry
                .file_type()
                .with_context(|| format!("read {}", entry.path().display()))?;
            if kind.is_dir() {
                if !(file_name.starts_with('.')
                    || file_name == "target"
                    || file_name == "node_modules")
                {
                    folders.push(entry.path());
                }
            } else if kind.is_file() && file_name == "Cargo.toml" {
                let declared = std::fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
                    .is_some_and(|manifest| {
                        manifest
                            .get("package")
                            .and_then(|table| table.get("name"))
                            .and_then(toml::Value::as_str)
                            == Some(package)
                    });
                if declared {
                    found.push(entry.path());
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Cargo's metadata of the package of `manifest`, its workspace's members only: read by the
/// user's cargo, so that it names the target folder the user's build wrote into.
fn cargo_metadata(manifest: &Path) -> anyhow::Result<serde_json::Value> {
    let cargo = cargo_program();
    let output = Command::new(&cargo)
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .with_context(|| {
            format!(
                "run {}, which names the service's binary: install Rust, or give the binary with \
                 --binary",
                cargo.display()
            )
        })?;
    if !output.status.success() {
        bail!(
            "cargo metadata of {}: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("read cargo's metadata")
}

/// The `cargo` on `PATH`, the user's own; else the truST toolchain's, with which a service can
/// be built too (specification 48 RU-61).
fn cargo_program() -> PathBuf {
    let name = format!("cargo{}", std::env::consts::EXE_SUFFIX);
    let on_path = std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|folder| folder.join(&name).is_file())
    });
    if on_path {
        return PathBuf::from(name);
    }
    trust_runtime::native::Toolchain::find()
        .ok()
        .and_then(|toolchain| toolchain.cargo)
        .unwrap_or_else(|| PathBuf::from(name))
}

/// The target folder of cargo's `metadata` and the names of the binaries of the package of
/// `manifest` in it.
fn package_binaries(
    metadata: &serde_json::Value,
    manifest: &Path,
) -> anyhow::Result<(PathBuf, Vec<String>)> {
    let target = metadata["target_directory"]
        .as_str()
        .context("cargo's metadata names no target folder")?;
    let manifest = absolute(manifest)?;
    let package = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|package| {
            package["manifest_path"]
                .as_str()
                .is_some_and(|path| absolute(Path::new(path)).ok().as_ref() == Some(&manifest))
        })
        .with_context(|| format!("cargo's metadata has no package of {}", manifest.display()))?;
    let binaries = package["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
        })
        .filter_map(|target| target["name"].as_str().map(str::to_string))
        .collect();
    Ok((PathBuf::from(target), binaries))
}

/// The user that owns the project folder `project`, whom its service runs as.
///
/// # Errors
///
/// A project folder that root owns, since a service never runs as root (specification 48 RU-64),
/// and an owner without a name.
#[cfg(unix)]
pub(crate) fn owner(project: &Path) -> anyhow::Result<Owner> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata(project)
        .with_context(|| format!("read {}", project.display()))?
        .uid();
    if uid == 0 {
        bail!(
            "{} belongs to root, and a service never runs as root (specification 48 RU-64): give \
             the project folder to the user the service runs as",
            project.display()
        );
    }
    let output = Command::new("id")
        .arg("-nu")
        .arg(uid.to_string())
        .output()
        .context("run id, which names the project folder's owner")?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || name.is_empty() {
        bail!(
            "no user has the id {uid} of the owner of {}",
            project.display()
        );
    }
    Ok(Owner { uid, name })
}

/// Windows names no owner: its service runs as a virtual account (specification 48 RU-64).
#[cfg(not(unix))]
pub(crate) fn owner(project: &Path) -> anyhow::Result<Owner> {
    bail!(
        "{}: a service runs as its project folder's owner only on Linux and macOS",
        project.display()
    )
}

/// `path` absolute, as the file system names it: without the device prefix of Windows' canonical
/// form (`\\?\C:\...`), which a definition and a command would carry otherwise.
fn absolute(path: &Path) -> anyhow::Result<PathBuf> {
    let canonical =
        std::fs::canonicalize(path).with_context(|| format!("find {}", path.display()))?;
    let text = canonical.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => Ok(PathBuf::from(rest)),
        _ => Ok(canonical),
    }
}

/// Refuses a path a definition cannot hold: not UTF-8, or with a control character, which would
/// end a line of a systemd unit.
fn check_path(path: &Path, what: &str) -> anyhow::Result<()> {
    match path.to_str() {
        Some(text) if !text.chars().any(char::is_control) => Ok(()),
        _ => bail!(
            "the path of {what}, {}, holds a character a service definition cannot hold",
            path.display()
        ),
    }
}

/// `path` relative to the project folder `project`, as a command run there names it.
fn relative(project: &Path, path: &Path) -> String {
    path.strip_prefix(project)
        .unwrap_or(path)
        .display()
        .to_string()
}
