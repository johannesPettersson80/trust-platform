//! `trust-dev rust audit`: the project's scan crate graph against the RustSec advisory database
//! (specification 48 RU-79).
//!
//! The graph is the build's own (`trust_runtime::native::scan_crate_graph`). The database is the
//! repository `rustsec/advisory-db`: its current state is downloaded and kept in `advisory-db/` of
//! truST's data folder, the kept copy serves when the download fails, and `--db` names a copy in a
//! folder instead. Each advisory is a Markdown file `crates/<crate>/<id>.md` whose TOML front matter
//! names the versions that are patched or unaffected.

use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context};
use semver::{Version, VersionReq};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::native::bundle::data_folder;
use trust_runtime::native::{scan_crate_graph, Toolchain};

/// The archive of the database's current state.
pub(crate) const DATABASE_ARCHIVE: &str =
    "https://github.com/rustsec/advisory-db/archive/refs/heads/main.tar.gz";

/// The folder of the kept copy in truST's data folder.
const KEPT_FOLDER: &str = "advisory-db";

/// The file of the kept copy that holds when it was downloaded, in seconds since 1970 (UTC).
const DOWNLOADED_FILE: &str = "downloaded";

/// The most a download may take, and the most it may hold: the archive is a few megabytes.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const DOWNLOAD_LIMIT: u64 = 64 * 1024 * 1024;

/// `trust-dev rust audit`: checks the project's scan crate graph against the advisory database and
/// fails when a vulnerability names a crate of it (specification 48 RU-79).
pub fn run_audit(project: Option<PathBuf>, db: Option<PathBuf>) -> anyhow::Result<()> {
    let project = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let toolchain = Toolchain::find()?;
    let work = project.join("build").join("audit");
    let Some(graph) = scan_crate_graph(&project, &toolchain, &work)? else {
        outln!("The project's build compiles no Rust crate: nothing to check.");
        return Ok(());
    };
    let (folder, source) = match db {
        Some(folder) => {
            let source = format!("in {}", folder.display());
            (folder, source)
        }
        None => {
            let data = data_folder().context("truST's data folder is unknown on this computer")?;
            kept_database(DATABASE_ARCHIVE, &data)?
        }
    };
    let findings = check(&graph, &folder)?;
    for finding in &findings {
        outln!("{}", finding.line());
    }
    let vulnerabilities = findings
        .iter()
        .filter(|finding| finding.kind.is_none())
        .count();
    let warnings = findings.len() - vulnerabilities;
    outln!(
        "Checked {} of the scan crate graph against the RustSec advisory database {source}: {}, {}.",
        counted(graph.len(), "crate", "crates"),
        counted(vulnerabilities, "vulnerability", "vulnerabilities"),
        counted(warnings, "warning", "warnings")
    );
    if vulnerabilities > 0 {
        bail!(
            "{} in the scan crate graph (specification 48 RU-79)",
            counted(vulnerabilities, "vulnerability", "vulnerabilities")
        );
    }
    Ok(())
}

/// `count` with the noun `one` or `many`: `1 crate`, `2 crates`, `0 crates`.
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The kept copy of the database in truST's data folder `data` after downloading its current state
/// from `url`, and how the report names it; the copy kept before when the download fails, with a
/// warning.
pub(crate) fn kept_database(url: &str, data: &Path) -> anyhow::Result<(PathBuf, String)> {
    let kept = data.join(KEPT_FOLDER);
    match download(url, data) {
        Ok(()) => Ok((kept, "downloaded now".to_string())),
        Err(error) => {
            let Some(when) = downloaded_at(&kept) else {
                return Err(error.context(
                    "the RustSec advisory database could not be downloaded and no copy is kept: \
                     name a copy with --db",
                ));
            };
            eprintln!(
                "warning: the RustSec advisory database could not be downloaded ({error:#}); \
                 checking against the copy downloaded {when}"
            );
            Ok((kept, format!("downloaded {when}")))
        }
    }
}

/// Downloads the database's archive from `url` and keeps its advisories in `advisory-db/` of
/// `data`, in place of the copy kept before, which stays when anything fails.
pub(crate) fn download(url: &str, data: &Path) -> anyhow::Result<()> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .build()
        .into();
    let archive = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .body_mut()
        .with_config()
        .limit(DOWNLOAD_LIMIT)
        .read_to_vec()
        .with_context(|| format!("GET {url}"))?;
    let staging = data.join(format!("{KEPT_FOLDER}.download"));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .with_context(|| format!("remove {}", staging.display()))?;
    }
    let count = unpack(&archive, &staging)?;
    if count == 0 {
        bail!("the download from {url} holds no advisory");
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    std::fs::write(staging.join(DOWNLOADED_FILE), now.to_string())
        .with_context(|| format!("write {}", staging.display()))?;
    let kept = data.join(KEPT_FOLDER);
    if kept.exists() {
        std::fs::remove_dir_all(&kept).with_context(|| format!("remove {}", kept.display()))?;
    }
    std::fs::rename(&staging, &kept).with_context(|| format!("write {}", kept.display()))?;
    Ok(())
}

/// Writes the advisories of the gzipped tar `archive`, the files `crates/<crate>/<id>.md` below
/// its one top folder, into `folder` at the same paths; anything else is left out. Gives how many
/// it wrote.
pub(crate) fn unpack(archive: &[u8], folder: &Path) -> anyhow::Result<usize> {
    let mut entries = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut count = 0;
    for entry in entries.entries().context("read the advisory archive")? {
        let mut entry = entry.context("read the advisory archive")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .context("read the advisory archive")?
            .into_owned();
        // `<top>/crates/<crate>/<id>.md`, every part a plain name.
        let parts: Vec<&str> = path
            .components()
            .map(|component| match component {
                Component::Normal(part) => part.to_str(),
                _ => None,
            })
            .collect::<Option<_>>()
            .unwrap_or_default();
        let [_, "crates", name, file] = parts.as_slice() else {
            continue;
        };
        if !file.ends_with(".md") {
            continue;
        }
        let target = folder.join("crates").join(name).join(file);
        let mut text = Vec::new();
        entry
            .read_to_end(&mut text)
            .context("read the advisory archive")?;
        std::fs::create_dir_all(folder.join("crates").join(name))
            .and_then(|()| std::fs::write(&target, text))
            .with_context(|| format!("write {}", target.display()))?;
        count += 1;
    }
    Ok(count)
}

/// When the copy in `kept` was downloaded, as `YYYY-MM-DD HH:MM UTC`; `None` without a copy.
fn downloaded_at(kept: &Path) -> Option<String> {
    let seconds: i64 = std::fs::read_to_string(kept.join(DOWNLOADED_FILE))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let at = time::OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute()
    ))
}

/// An advisory of the database, as far as the audit reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Advisory {
    pub(crate) id: String,
    pub(crate) title: String,
    /// `unmaintained`, `unsound` or `notice` for an informational advisory; `None` for a
    /// vulnerability.
    pub(crate) informational: Option<String>,
    pub(crate) withdrawn: bool,
    pub(crate) patched: Vec<String>,
    pub(crate) unaffected: Vec<String>,
}

impl Advisory {
    /// Reads the advisory file `text`: the TOML front matter between the lines ```` ```toml ````
    /// and ```` ``` ````, and the title, the first heading after it.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        lines
            .by_ref()
            .find(|line| line.trim() == "```toml")
            .ok_or("no TOML front matter")?;
        let front: Vec<&str> = lines
            .by_ref()
            .take_while(|line| line.trim() != "```")
            .collect();
        let title = lines
            .find_map(|line| line.strip_prefix("# "))
            .unwrap_or_default()
            .trim()
            .to_string();
        let table: toml::Table = front
            .join("\n")
            .parse()
            .map_err(|error| format!("front matter: {error}"))?;
        let advisory = table
            .get("advisory")
            .and_then(toml::Value::as_table)
            .ok_or("no [advisory]")?;
        let id = advisory
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or("no advisory.id")?
            .to_string();
        let requirements = |key: &str| -> Vec<String> {
            table
                .get("versions")
                .and_then(|versions| versions.get(key))
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|requirement| requirement.as_str().map(str::to_string))
                .collect()
        };
        Ok(Self {
            id,
            title,
            informational: advisory
                .get("informational")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            withdrawn: advisory.contains_key("withdrawn"),
            patched: requirements("patched"),
            unaffected: requirements("unaffected"),
        })
    }

    /// Whether the advisory names `version`: it matches none of the patched and none of the
    /// unaffected requirements, and the advisory is not withdrawn.
    pub(crate) fn names(&self, version: &Version) -> Result<bool, String> {
        if self.withdrawn {
            return Ok(false);
        }
        for requirement in self.patched.iter().chain(&self.unaffected) {
            let requirement = VersionReq::parse(requirement).map_err(|error| {
                format!("{}: version requirement {requirement}: {error}", self.id)
            })?;
            if requirement.matches(version) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// An advisory that names a crate of the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) title: String,
    pub(crate) patched: Vec<String>,
    /// The kind of an informational advisory, a warning; `None` for a vulnerability.
    pub(crate) kind: Option<String>,
}

impl Finding {
    /// The report's line of the finding.
    pub(crate) fn line(&self) -> String {
        let patched = if self.patched.is_empty() {
            "none".to_string()
        } else {
            self.patched.join(", ")
        };
        match &self.kind {
            None => format!(
                "error: {} {} {}: {} (patched: {patched})",
                self.id, self.name, self.version, self.title
            ),
            Some(kind) => format!(
                "warning: {} {} {}, {kind}: {} (patched: {patched})",
                self.id, self.name, self.version, self.title
            ),
        }
    }
}

/// The advisories of the database copy in `folder` that name a crate of `graph`, each crate's in
/// the order of the advisories' files.
pub(crate) fn check(graph: &[(String, String)], folder: &Path) -> anyhow::Result<Vec<Finding>> {
    if !folder.join("crates").is_dir() {
        bail!(
            "{} holds no RustSec advisory database: its advisories are in crates/<crate>/",
            folder.display()
        );
    }
    let mut findings = Vec::new();
    for (name, version) in graph {
        let crate_folder = folder.join("crates").join(name);
        let Ok(entries) = std::fs::read_dir(&crate_folder) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
            .collect();
        files.sort();
        let parsed = Version::parse(version)
            .with_context(|| format!("{name} {version} is no semantic version"))?;
        for file in files {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("read {}", file.display()))?;
            let advisory = Advisory::parse(&text)
                .map_err(|error| anyhow::anyhow!("{}: {error}", file.display()))?;
            if advisory
                .names(&parsed)
                .map_err(|error| anyhow::anyhow!("{}: {error}", file.display()))?
            {
                findings.push(Finding {
                    id: advisory.id,
                    name: name.clone(),
                    version: version.clone(),
                    title: advisory.title,
                    patched: advisory.patched,
                    kind: advisory.informational,
                });
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
#[path = "rust_audit_tests.rs"]
mod tests;
