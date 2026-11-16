//! Where a project's Rust and the Structured Text generated from it lie (specification 48 RU-06,
//! RU-23, RU-65, Appendix B): `trust.toml`'s `[rust] crate` names the crate's folder, `rust`
//! without it. A project whose crate is the project's own folder (`crate = "."`) is a Rust-first
//! project: its generated Structured Text lies in `src/generated/trust/` beside the crate's sources,
//! with the configuration that its `trust.toml` declares (RU-66, RU-67).

use std::path::{Component, Path};

/// The generated declarations of a Structured Text project (RU-23).
pub const DECLARATIONS_FILE: &str = "src/generated/rust_pous.st";

/// The folder of the project's crate in a Structured Text project (RU-06).
pub const CRATE_FOLDER: &str = "rust";

/// The folder of the bindings, the crate `trust_st` that the build generates for a crate that calls
/// Structured Text (RU-39).
pub const BINDINGS_FOLDER: &str = "build/trust_st";

/// The folder of a Rust-first project's generated Structured Text (RU-65).
const RUST_FIRST_GENERATED: &str = "src/generated/trust";

/// Where a project's crate and its generated Structured Text lie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The crate's folder, relative to the project: `rust`, or `.` in a Rust-first project.
    pub crate_folder: String,
    /// The generated declarations, relative to the project with `/`.
    pub declarations: String,
    /// The generated configuration, relative to the project with `/`, of a Rust-first project whose
    /// `trust.toml` declares a resource (RU-67).
    pub configuration: Option<String>,
}

impl Layout {
    /// The file `file` of the crate, relative to the project with `/`: `rust/Cargo.toml`, or
    /// `Cargo.toml` in a Rust-first project.
    #[must_use]
    pub fn crate_file(&self, file: &str) -> String {
        if self.crate_folder == "." {
            file.to_string()
        } else {
            format!("{}/{file}", self.crate_folder)
        }
    }
}

/// The layout of the project in `project`, from its `trust.toml`.
///
/// # Errors
///
/// A `trust.toml` that does not read, a `[rust] crate` that is no folder inside the project, and a
/// `[resource]` in a project whose crate is not the project's own folder.
pub fn layout(project: &Path) -> Result<Layout, String> {
    let file = project.join("trust.toml");
    let table: toml::Table = match std::fs::read_to_string(&file) {
        Ok(text) => toml::from_str(&text).map_err(|error| error.to_string())?,
        Err(_) => toml::Table::new(),
    };
    let crate_folder = match table.get("rust").and_then(|rust| rust.get("crate")) {
        None => CRATE_FOLDER.to_string(),
        Some(value) => value
            .as_str()
            .filter(|folder| inside(folder))
            .ok_or_else(|| {
                "[rust] crate names the crate's folder inside the project, \
                 \"rust\" or \".\" (specification 48 Appendix B)"
                    .to_string()
            })?
            .trim_end_matches('/')
            .to_string(),
    };
    let rust_first = crate_folder == ".";
    let resource = table.contains_key("resource");
    if resource && !rust_first {
        return Err(
            "[resource] declares the configuration of a Rust-first project, whose \
             crate is the project's own folder (`[rust] crate = \".\"`, specification 48 RU-65, \
             RU-66)"
                .to_string(),
        );
    }
    Ok(Layout {
        declarations: if rust_first {
            format!("{RUST_FIRST_GENERATED}/rust_pous.st")
        } else {
            DECLARATIONS_FILE.to_string()
        },
        configuration: resource.then(|| format!("{RUST_FIRST_GENERATED}/configuration.st")),
        crate_folder,
    })
}

/// Whether `folder` is a relative folder inside the project, `.` among them.
fn inside(folder: &str) -> bool {
    let path = Path::new(folder);
    !folder.is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}
