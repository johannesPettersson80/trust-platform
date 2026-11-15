//! A project's Rust POUs (specification 48 §4, §6.1): the project's crate in `rust/`, the Rust POUs
//! its sources declare with the paths the program's generated code names them by, and the
//! generated declarations, `src/generated/rust_pous.st`, which the rest of the project reads
//! (RU-23, RU-24).
//!
//! The crate's sources are read, not compiled: each attributed item is read with `trust-plc-decl`,
//! the reader the attributes check items with, so the declarations and the compiled code agree by
//! construction (RU-24).

mod tree;

use std::fmt;
use std::path::Path;

/// The generated declarations, relative to a Structured Text project (RU-23).
pub const DECLARATIONS_FILE: &str = "src/generated/rust_pous.st";

/// The folder of the project's crate in a Structured Text project (RU-06).
pub const CRATE_FOLDER: &str = "rust";

/// A Rust POU of the project's crate.
#[derive(Debug, Clone, PartialEq)]
pub struct RustPou {
    /// The POU as its attributes declare it.
    pub pou: trust_plc_decl::Pou,
    /// The file its item lies in, relative to the project with `/`: `rust/src/filter.rs`.
    pub file: String,
    /// The line of its item in that file, 1-based.
    pub line: usize,
    /// The path the program's generated code names the item by from the crate's root:
    /// `["filter", "FilterEma"]` for `crate::filter::FilterEma`.
    pub path: Vec<String>,
}

/// The project's crate and its Rust POUs.
#[derive(Debug, Clone, PartialEq)]
pub struct RustCrate {
    /// The crate's name as Rust code names it: `[lib] name`, else the package's name with `_` for
    /// `-`.
    pub name: String,
    /// The Rust POUs, in the order the crate's modules declare them.
    pub pous: Vec<RustPou>,
}

/// Something of the project's crate that keeps its declarations from being written, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustPouError {
    /// The file, relative to the project with `/`.
    pub file: String,
    /// The line, 1-based; 0 for the file as a whole.
    pub line: usize,
    /// The column, 1-based; 0 for the file as a whole.
    pub column: usize,
    /// What is wrong and what to do.
    pub message: String,
}

impl fmt::Display for RustPouError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.file, self.message)
        } else {
            write!(
                f,
                "{}:{}:{}: {}",
                self.file, self.line, self.column, self.message
            )
        }
    }
}

/// The project's crate of the project in `project`; `None` for a project without one, which has
/// no Rust (RU-06).
///
/// # Errors
///
/// Every [`RustPouError`] of the crate's manifest and sources: a file that does not read or parse,
/// an item an attribute refuses, a POU that the generated code cannot reach, a name two POUs have.
pub fn read(project: &Path) -> Result<Option<RustCrate>, Vec<RustPouError>> {
    let folder = project.join(CRATE_FOLDER);
    let manifest = folder.join("Cargo.toml");
    if !manifest.is_file() {
        return Ok(None);
    }
    let manifest_name = format!("{CRATE_FOLDER}/Cargo.toml");
    let whole = |message: String| {
        vec![RustPouError {
            file: manifest_name.clone(),
            line: 0,
            column: 0,
            message,
        }]
    };
    let text = std::fs::read_to_string(&manifest).map_err(|error| whole(error.to_string()))?;
    let table: toml::Table = toml::from_str(&text).map_err(|error| whole(error.to_string()))?;
    let lib = table.get("lib").and_then(toml::Value::as_table);
    let name = lib
        .and_then(|lib| lib.get("name"))
        .or_else(|| table.get("package").and_then(|package| package.get("name")))
        .and_then(toml::Value::as_str)
        .map(|name| name.replace('-', "_"))
        .ok_or_else(|| whole("the crate's manifest names no package".into()))?;
    let root = lib
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .unwrap_or("src/lib.rs");
    let found = tree::walk(project, &folder.join(root));
    if found.errors.is_empty() {
        Ok(Some(RustCrate {
            name,
            pous: found.pous,
        }))
    } else {
        Err(found.errors)
    }
}

/// The generated declarations of `krate` (RU-23): each POU's declaration, in the order the crate
/// declares them, with a blank line between two.
#[must_use]
pub fn declarations_text(krate: &RustCrate) -> String {
    krate
        .pous
        .iter()
        .map(|pou| pou.pou.declaration(&pou.file))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What [`write_declarations`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// The file held the crate's declarations already.
    UpToDate,
    /// The file was written with this many POUs.
    Wrote(usize),
    /// The project has no crate or its crate no POU, and the file that held declarations is
    /// removed.
    Removed,
    /// The project has no crate or its crate no POU, and there is no file.
    Nothing,
}

/// Writes the generated declarations of the project in `project` (RU-24, `trust-dev rust
/// declarations`): the file holds its crate's POUs, and a project without POUs has no file.
///
/// # Errors
///
/// The crate's [`RustPouError`]s, and a file that cannot be written as one without a line.
pub fn write_declarations(project: &Path) -> Result<Written, Vec<RustPouError>> {
    let file = project.join(DECLARATIONS_FILE);
    let failed = |error: std::io::Error| {
        vec![RustPouError {
            file: DECLARATIONS_FILE.into(),
            line: 0,
            column: 0,
            message: error.to_string(),
        }]
    };
    let krate = read(project)?.filter(|krate| !krate.pous.is_empty());
    let Some(krate) = krate else {
        if file.is_file() {
            std::fs::remove_file(&file).map_err(failed)?;
            return Ok(Written::Removed);
        }
        return Ok(Written::Nothing);
    };
    let text = declarations_text(&krate);
    if std::fs::read_to_string(&file).is_ok_and(|held| held == text) {
        return Ok(Written::UpToDate);
    }
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    std::fs::write(&file, text).map_err(failed)?;
    Ok(Written::Wrote(krate.pous.len()))
}

#[cfg(test)]
mod tests;
