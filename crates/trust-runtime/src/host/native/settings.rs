//! The project's build settings in its settings file, `trust-lsp.toml` (specification 14): `[build]
//! debug`, which chooses the image it compiles into (specification 44 §13.2), `[signing] key`, the
//! key a deployed image is signed with (§5.11), and `[project] vendor_profile`, which chooses the
//! conversion rules the program is compiled with (specification 07 §2.10).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::NativeError;
use crate::bundle_builder::PROJECT_SETTINGS_FILES;
use crate::stdlib::conversions::{ConversionProfile, VENDOR_PROFILES};

/// The part of the settings file this reads; every other section and key is the language server's.
#[derive(Default, Deserialize)]
struct SettingsFile {
    #[serde(default)]
    build: BuildSection,
    #[serde(default)]
    signing: SigningSection,
    #[serde(default)]
    project: ProjectSection,
}

#[derive(Default, Deserialize)]
struct ProjectSection {
    vendor_profile: Option<String>,
}

#[derive(Default, Deserialize)]
struct BuildSection {
    debug: Option<bool>,
}

#[derive(Default, Deserialize)]
struct SigningSection {
    key: Option<PathBuf>,
}

/// The settings file of the project in folder `root`, the first of its names that is a file.
fn settings_path(root: &Path) -> Option<PathBuf> {
    PROJECT_SETTINGS_FILES
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

/// The settings file of the project in folder `root`, or the defaults without one.
fn settings_file(root: &Path) -> Result<SettingsFile, NativeError> {
    let Some(path) = settings_path(root) else {
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

/// The conversion rules the project in folder `root` is compiled with (specification 07 §2.10): the
/// CODESYS rules for its `[project] vendor_profile` `codesys`, `beckhoff` or `twincat`, the IEC rules
/// for `iec`, `siemens`, `mitsubishi` or `gxworks3`, and without a settings file, a
/// `vendor_profile` or with an empty one. Names compare without regard to case or the white space
/// around them.
///
/// # Errors
///
/// [`NativeError::Settings`] when the settings file cannot be read or is not TOML, and for a
/// `vendor_profile` that is no vendor profile, with the file and the names it may take, so that a
/// misspelled profile never builds the program with other conversion rules.
pub fn conversion_profile(root: &Path) -> Result<ConversionProfile, NativeError> {
    let Some(name) = settings_file(root)?.project.vendor_profile else {
        return Ok(ConversionProfile::Iec);
    };
    if name.trim().is_empty() {
        return Ok(ConversionProfile::Iec);
    }
    ConversionProfile::from_vendor_profile(&name).ok_or_else(|| {
        let path = settings_path(root).unwrap_or_else(|| root.join(PROJECT_SETTINGS_FILES[0]));
        NativeError::Settings(format!(
            "{}: unknown vendor_profile '{}' (supported: {})",
            path.display(),
            name.trim(),
            VENDOR_PROFILES.join(", ")
        ))
    })
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
    fn the_vendor_profile_chooses_the_conversion_rules() {
        // Specification 07 §2.10: `codesys`, `beckhoff` and `twincat` choose the CODESYS rules;
        // `iec`, `siemens`, `mitsubishi`, `gxworks3`, an empty value, no `vendor_profile` and no
        // settings file the IEC rules; names compare without case or surrounding white space; any
        // other name is refused with the file and the supported names. Catches a profile that
        // falls back to the IEC rules for a misspelling, and a case-sensitive comparison.
        let cases = [
            (
                "codesys",
                Some("[project]\nvendor_profile = \"codesys\"\n"),
                ConversionProfile::Codesys,
            ),
            (
                "beckhoff",
                Some("[project]\nvendor_profile = \" Beckhoff \"\n"),
                ConversionProfile::Codesys,
            ),
            (
                "twincat",
                Some("[project]\nvendor_profile = \"TWINCAT\"\n"),
                ConversionProfile::Codesys,
            ),
            (
                "siemens",
                Some("[project]\nvendor_profile = \"siemens\"\n"),
                ConversionProfile::Iec,
            ),
            (
                "gxworks3",
                Some("[project]\nvendor_profile = \"gxworks3\"\n"),
                ConversionProfile::Iec,
            ),
            (
                "empty",
                Some("[project]\nvendor_profile = \"\"\n"),
                ConversionProfile::Iec,
            ),
            (
                "blank",
                Some("[project]\nvendor_profile = \"  \"\n"),
                ConversionProfile::Iec,
            ),
            (
                "other-keys",
                Some("[project]\ninclude_paths = [\"src\"]\n"),
                ConversionProfile::Iec,
            ),
            (
                "unset",
                Some("[build]\ndebug = true\n"),
                ConversionProfile::Iec,
            ),
            ("nofile", None, ConversionProfile::Iec),
        ];
        for (name, text, expected) in cases {
            let root = project(&format!("profile-{name}"), text);
            assert_eq!(conversion_profile(&root).expect(name), expected, "{name}");
            let _ = std::fs::remove_dir_all(root);
        }
        let root = project(
            "profile-typo",
            Some("[project]\nvendor_profile = \"codesis\"\n"),
        );
        let error = conversion_profile(&root).expect_err("a misspelled profile");
        let text = error.to_string();
        assert!(text.contains("trust-lsp.toml"), "{text}");
        assert!(text.contains("unknown vendor_profile 'codesis'"), "{text}");
        assert!(
            text.contains("beckhoff, codesys, gxworks3, iec, mitsubishi, siemens, twincat"),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn each_vendor_profile_selects_matching_conversion_and_language_rules() {
        // Specification 14 and specification 07 §2.10: one `vendor_profile` selects both rule
        // sets, and the build derives the language rules from the conversion rules it reads.
        // Catches a profile name that converts as CODESYS and checks as IEC, or the reverse.
        for name in crate::stdlib::conversions::VENDOR_PROFILES {
            let conversions = ConversionProfile::from_vendor_profile(name).expect(name);
            assert_eq!(
                Some(crate::harness::dialect_of(conversions)),
                trust_hir::Dialect::from_vendor_profile(name),
                "{name}"
            );
        }
    }

    #[test]
    fn the_vendor_profile_is_read_under_each_settings_file_name() {
        // Specification 07 §2.10 and specification 14: the settings file is `trust-lsp.toml`,
        // `.trust-lsp.toml` or `trustlsp.toml`, as the language server finds it, and the
        // `vendor_profile` is read from whichever the project has. Catches a reader that knows one
        // name only, which would build a CODESYS project under the IEC rules.
        let root = project("profile-names", None);
        for name in PROJECT_SETTINGS_FILES {
            std::fs::write(
                root.join(name),
                "[project]\nvendor_profile = \" CoDeSys \"\n",
            )
            .expect("the settings file");
            assert_eq!(
                conversion_profile(&root).expect(name),
                ConversionProfile::Codesys,
                "{name}"
            );
            let _ = std::fs::remove_file(root.join(name));
        }
        let _ = std::fs::remove_dir_all(root);
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
