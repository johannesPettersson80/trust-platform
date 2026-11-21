//! The measured times of a project's Rust POUs (specification 48 RU-48, Appendix B): what makes a
//! measurement current, the hash of what the image is built from, and the record of each in
//! `trust.toml` `[timing.measured]`.

use std::path::Path;

use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

/// One POU's measurement on one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    /// The POU's name in the image's Rust POU table: `FILTER_EMA`, `AXIS.JOG`.
    pub pou: String,
    /// Its longest call in nanoseconds.
    pub longest_ns: u64,
    /// The hash of what the image is built from ([`code`]).
    pub code: String,
    /// Where it ran: `computer`, or a device's runtime ID.
    pub device: String,
}

/// The SHA-256, in lower-case hexadecimal, of what the program image of the project in `project`
/// is built from for `triple` (RU-48): the project's Structured Text sources, the files of its
/// crate that cargo reads (its `Cargo.toml`, its `Cargo.lock` and the files below `src/`), this
/// bundle's version, which fixes the SDK, the vendored crates and `rustc`, and the target. A
/// bench's test programs and the choice of a debug or a release image are no part of it.
///
/// # Errors
///
/// The text of a file that cannot be read.
pub fn code(project: &Path, triple: &str) -> Result<String, String> {
    let mut hash = Sha256::new();
    let mut part = |label: &str, bytes: &[u8]| {
        hash.update(label.as_bytes());
        hash.update(b"\n");
        hash.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        hash.update(bytes);
    };
    part("trust-rust-code/1", b"");
    part("bundle", env!("CARGO_PKG_VERSION").as_bytes());
    part("target", triple.as_bytes());
    let mut sources = crate::bundle_builder::collect_project_sources_labelled(project, &[])
        .map_err(|error| format!("the project's sources: {error}"))?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    for source in &sources {
        part(
            &format!("st {}", source.path.as_deref().unwrap_or_default()),
            source.text.as_bytes(),
        );
    }
    let layout = super::layout(project).map_err(|message| format!("trust.toml: {message}"))?;
    let folder = project.join(&layout.crate_folder);
    let mut files = Vec::new();
    for name in ["Cargo.toml", "Cargo.lock"] {
        let path = folder.join(name);
        if path.is_file() {
            files.push((name.to_string(), path));
        }
    }
    collect(&folder.join("src"), "src", &mut files)?;
    files.sort();
    for (label, path) in files {
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        part(&format!("crate {label}"), &bytes);
    }
    Ok(hex::encode(hash.finalize()))
}

/// Every file below `folder`, by its path from the crate's folder with `/`.
fn collect(
    folder: &Path,
    label: &str,
    files: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<(), String> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", folder.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect(&path, &format!("{label}/{name}"), files)?;
        } else if file_type.is_file() {
            files.push((format!("{label}/{name}"), path));
        }
    }
    Ok(())
}

/// Writes `measured`, measured on `triple`, into `[timing.measured]` of the project's `trust.toml`,
/// each POU's entry of that target replaced, every other entry and every comment kept, through a
/// file of its own renamed over it.
///
/// # Errors
///
/// The text of a `trust.toml` that does not read or parse, or cannot be written.
pub fn write(project: &Path, triple: &str, measured: &[Measured]) -> Result<(), String> {
    let file = project.join("trust.toml");
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("trust.toml: {error}")),
    };
    let mut document: DocumentMut = text
        .parse()
        .map_err(|error| format!("trust.toml: {error}"))?;
    let timing = document
        .entry("timing")
        .or_insert_with(|| Item::Table(implicit()))
        .as_table_mut()
        .ok_or("trust.toml: [timing] is no table")?;
    let table = timing
        .entry("measured")
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or("trust.toml: [timing.measured] is no table")?;
    for entry in measured {
        let pou = table
            .entry(&entry.pou)
            .or_insert_with(|| Item::Value(Value::InlineTable(InlineTable::new())));
        let targets = pou.as_inline_table_mut().ok_or_else(|| {
            format!(
                "trust.toml: [timing.measured] {} is no inline table",
                entry.pou
            )
        })?;
        let mut record = InlineTable::new();
        record.insert(
            "longest",
            i64::try_from(entry.longest_ns).unwrap_or(i64::MAX).into(),
        );
        record.insert("code", entry.code.as_str().into());
        record.insert("device", entry.device.as_str().into());
        targets.insert(triple, Value::InlineTable(record));
    }
    let written = project.join(format!("trust.toml.{}.tmp", std::process::id()));
    std::fs::write(&written, document.to_string())
        .map_err(|error| format!("trust.toml: {error}"))?;
    std::fs::rename(&written, &file).map_err(|error| format!("trust.toml: {error}"))
}

/// A table written only through its sub-tables, as `[timing.measured]` writes `[timing]`.
fn implicit() -> Table {
    let mut table = Table::new();
    table.set_implicit(true);
    table
}
