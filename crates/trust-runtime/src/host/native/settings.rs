//! The project's build settings in its settings file, `trust-lsp.toml` (specification 14): `[build]
//! debug`, which chooses the image it compiles into (specification 44 §13.2), and `[signing] key`,
//! the key a deployed image is signed with (§5.11).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::NativeError;
use crate::bundle_builder::PROJECT_SETTINGS_FILES;

/// The part of the settings file this reads; every other section and key is the language server's.
#[derive(Default, Deserialize)]
struct SettingsFile {
    #[serde(default)]
    build: BuildSection,
    #[serde(default)]
    signing: SigningSection,
}

#[derive(Default, Deserialize)]
struct BuildSection {
    debug: Option<bool>,
}

#[derive(Default, Deserialize)]
struct SigningSection {
    key: Option<PathBuf>,
}

/// The settings file of the project in folder `root`, or the defaults without one.
fn settings_file(root: &Path) -> Result<SettingsFile, NativeError> {
    let Some(path) = PROJECT_SETTINGS_FILES
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
    else {
        return Ok(SettingsFile::default());
    };
    let settings = |error: &dyn std::fmt::Display| {
        NativeError::Settings(format!("{}: {error}", path.display()))
    };
    let text = std::fs::read_to_string(&path).map_err(|error| settings(&error))?;
    toml::from_str(&text).map_err(|error| settings(&error))
}

/// Whether the project in folder `root` is compiled with statement hooks, a debug image: its
/// `[build] debug`, `true` when the setting or the settings file is absent. `false` builds a release
/// image (specification 44 §13.2).
///
/// # Errors
///
/// [`NativeError::Settings`] when the settings file cannot be read, is not TOML, or has a `debug`
/// that is not a Boolean.
pub fn debug_hooks(root: &Path) -> Result<bool, NativeError> {
    Ok(settings_file(root)?.build.debug.unwrap_or(true))
}

/// The key file `[signing] key` names for the project in folder `root`, relative to it; `None`
/// when the project names none, and its images are signed with the user's own key (specification
/// 44 §5.11).
///
/// # Errors
///
/// [`NativeError::Settings`] when the settings file cannot be read or is not TOML.
pub fn signing_key_setting(root: &Path) -> Result<Option<PathBuf>, NativeError> {
    Ok(settings_file(root)?.signing.key.map(|key| {
        if key.is_relative() {
            root.join(key)
        } else {
            key
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own with `text` as its `trust-lsp.toml`, or none for `None`.
    fn project(name: &str, text: Option<&str>) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "trust-native-settings-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a test folder");
        if let Some(text) = text {
            std::fs::write(root.join("trust-lsp.toml"), text).expect("the settings file");
        }
        root
    }

    #[test]
    fn the_build_setting_chooses_a_debug_or_a_release_image() {
        // Specification 44 §13.2 and specification 14: `[build] debug`, `true` when absent, in the
        // project's `trust-lsp.toml`; the other keys of `[build]` and the other sections are the
        // language server's and do not disturb it. Catches a default of `false`, which would make
        // every project's image undebuggable, and a reader that refuses the language server's keys.
        let none = project("none", None);
        assert!(debug_hooks(&none).expect("no settings file"));
        let absent = project(
            "absent",
            Some("[build]\nflags = [\"-O\"]\n\n[workspace]\npriority = 1\n"),
        );
        assert!(debug_hooks(&absent).expect("no debug key"));
        let release = project(
            "release",
            Some("[build]\ndebug = false\ndefines = [\"SIM=1\"]\n"),
        );
        assert!(!debug_hooks(&release).expect("a release build"));
        let debug = project("debug", Some("[build]\ndebug = true\n"));
        assert!(debug_hooks(&debug).expect("a debug build"));
        for root in [none, absent, release, debug] {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn a_debug_setting_that_is_no_boolean_is_refused_with_the_file() {
        // Specification 44 §13.2: `debug` is a Boolean. Catches a text "false", which a lenient
        // reader would take as a debug build without a word.
        let root = project("text", Some("[build]\ndebug = \"false\"\n"));
        let error = debug_hooks(&root).expect_err("a text is no Boolean");
        let text = error.to_string();
        assert!(text.starts_with("the project's settings: "), "{text}");
        assert!(text.contains("trust-lsp.toml"), "{text}");
        let _ = std::fs::remove_dir_all(root);
    }
}
