//! The curated libraries the extension bundles (specification 25 §7): Plant, OSCAT and PLCopen
//! Motion. Each has the dependency name a project's `trust-lsp.toml` gives it, its folder under the
//! extension's `media/libraries` and under the project's `libraries`, and the symbols whose quick fix
//! adds it to a project.

/// One curated library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CuratedLibrary {
    /// The id the extension's commands name.
    pub(super) id: &'static str,
    /// The name the panel shows.
    pub(super) label: &'static str,
    /// Its key in `[dependencies]`.
    pub(super) dependency_name: &'static str,
    /// Its folder under `media/libraries` and under the project's `libraries`.
    pub(super) folder: &'static str,
    /// What it is for, as the panel's card says.
    pub(super) purpose: &'static str,
}

/// The curated libraries, in the order the panel lists them.
pub(super) const CURATED: [CuratedLibrary; 3] = [
    CuratedLibrary {
        id: "plant",
        label: "Plant",
        dependency_name: "Plant",
        folder: "plant",
        purpose: "Reusable equipment, instruments, sequences and their HMI components",
    },
    CuratedLibrary {
        id: "oscat",
        label: "OSCAT",
        dependency_name: "OSCAT",
        folder: "oscat",
        purpose: "General-purpose utility blocks and functions",
    },
    CuratedLibrary {
        id: "plcopen_motion",
        label: "PLCopen Motion",
        dependency_name: "PLCopenMotionSingleAxis",
        folder: "plcopen_motion",
        purpose: "Single-axis motion control blocks",
    },
];

/// The curated library with the id `id`.
pub(super) fn by_id(id: &str) -> Option<&'static CuratedLibrary> {
    CURATED.iter().find(|library| library.id == id)
}

/// The curated library a project names `name` in `[dependencies]` (the exact key).
pub(super) fn by_dependency_name(name: &str) -> Option<&'static CuratedLibrary> {
    CURATED
        .iter()
        .find(|library| library.dependency_name == name)
}

/// The curated library whose quick fix `symbol` offers: `Plant` (any letter case) for Plant, `INC`,
/// `SCALE` and `BIT_COUNT` (any letter case) for OSCAT, and `MC_` followed by letters, digits or
/// underscores for PLCopen Motion.
pub(super) fn library_for_symbol(symbol: &str) -> Option<&'static CuratedLibrary> {
    let id = if symbol.eq_ignore_ascii_case("Plant") {
        "plant"
    } else if ["INC", "SCALE", "BIT_COUNT"]
        .iter()
        .any(|name| symbol.eq_ignore_ascii_case(name))
    {
        "oscat"
    } else if symbol.strip_prefix("MC_").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }) {
        "plcopen_motion"
    } else {
        return None;
    };
    by_id(id)
}

/// The quick fix's library for `symbol`, unless the project already names its dependency in
/// `dependency_names` (exact keys).
pub(super) fn quick_fix_library(
    symbol: &str,
    dependency_names: &[&str],
) -> Option<&'static CuratedLibrary> {
    library_for_symbol(symbol)
        .filter(|library| !dependency_names.contains(&library.dependency_name))
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
