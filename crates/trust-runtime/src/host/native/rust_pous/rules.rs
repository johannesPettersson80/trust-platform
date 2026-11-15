//! The rules the build holds a project's crate to (specification 48 RU-08): a library of edition
//! 2024 without a binary, which depends on the SDK at exactly this release's version, and whose
//! root keeps the standard library out of the scan and forbids `unsafe` code. Each refusal names
//! the line to write.

use std::path::Path;

use syn::{Attribute, Meta};

use super::{RustPouError, CRATE_FOLDER};

/// The version of the SDK a project's crate depends on: this release's (RU-08).
const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Refuses the project's crate in `project` where it breaks RU-08; `root` is its root file.
///
/// # Errors
///
/// Every rule the manifest and the root file break, each with the line to write.
pub(in crate::native) fn check_crate(project: &Path, root: &Path) -> Result<(), Vec<RustPouError>> {
    let manifest_label = format!("{CRATE_FOLDER}/Cargo.toml");
    let mut errors = Vec::new();
    let mut refuse = |file: &str, message: String| {
        errors.push(RustPouError {
            file: file.to_string(),
            line: 0,
            column: 0,
            message,
        });
    };
    let manifest = std::fs::read_to_string(project.join(CRATE_FOLDER).join("Cargo.toml"))
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .unwrap_or_default();
    let package = manifest.get("package").and_then(toml::Value::as_table);
    if package
        .and_then(|package| package.get("edition"))
        .and_then(toml::Value::as_str)
        != Some("2024")
    {
        refuse(
            &manifest_label,
            "the crate's edition is 2024 (specification 48 RU-08): `edition = \"2024\"` in \
             [package]"
                .into(),
        );
    }
    if manifest.contains_key("bin") {
        refuse(
            &manifest_label,
            "the crate is a library without a [[bin]] (specification 48 RU-08): the program's \
             module is built from it"
                .into(),
        );
    }
    let crate_types = manifest
        .get("lib")
        .and_then(|lib| lib.get("crate-type"))
        .and_then(toml::Value::as_array);
    if crate_types.is_some_and(|types| !types.iter().any(|kind| kind.as_str() == Some("rlib"))) {
        refuse(
            &manifest_label,
            "the crate's library is an rlib (specification 48 RU-08): remove `crate-type` from \
             [lib], or name \"rlib\" in it"
                .into(),
        );
    }
    let sdk = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("trust-plc"));
    let requirement = sdk.and_then(|sdk| {
        sdk.as_str()
            .or_else(|| sdk.get("version").and_then(toml::Value::as_str))
    });
    if requirement != Some(&format!("={SDK_VERSION}")) {
        refuse(
            &manifest_label,
            format!(
                "the crate depends on the SDK at exactly this release's version (specification 48 \
                 RU-08): `trust-plc = \"={SDK_VERSION}\"` in [dependencies]"
            ),
        );
    }
    let root_label = root
        .strip_prefix(project)
        .unwrap_or(root)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let attributes = std::fs::read_to_string(root)
        .ok()
        .and_then(|text| syn::parse_file(&text).ok())
        .map(|file| file.attrs)
        .unwrap_or_default();
    if !attributes.iter().any(is_no_std) {
        refuse(
            &root_label,
            "the crate's root keeps the standard library out of the scan (specification 48 RU-08, \
             RU-19): `#![cfg_attr(not(test), no_std)]` as its first line"
                .into(),
        );
    }
    if !attributes.iter().any(forbids_unsafe) {
        refuse(
            &root_label,
            "the crate's root forbids unsafe code (specification 48 RU-08, RU-20): \
             `#![forbid(unsafe_code)]` as its first line"
                .into(),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// `#![no_std]`, or `#![cfg_attr(not(test), no_std)]`, which lets the crate's tests use the
/// standard library (RU-19).
fn is_no_std(attribute: &Attribute) -> bool {
    if attribute.path().is_ident("no_std") {
        return true;
    }
    let Meta::List(list) = &attribute.meta else {
        return false;
    };
    list.path.is_ident("cfg_attr")
        && list
            .tokens
            .to_string()
            .replace(' ', "")
            .eq_ignore_ascii_case("not(test),no_std")
}

/// `#![forbid(unsafe_code)]`, also among other lints.
fn forbids_unsafe(attribute: &Attribute) -> bool {
    let Meta::List(list) = &attribute.meta else {
        return false;
    };
    list.path.is_ident("forbid")
        && list
            .tokens
            .to_string()
            .split(',')
            .any(|lint| lint.trim() == "unsafe_code")
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
