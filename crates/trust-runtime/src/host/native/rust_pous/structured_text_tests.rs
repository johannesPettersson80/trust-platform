//! What a crate calls of Structured Text (specification 48 RU-38, RU-39): the names it writes under
//! `st::`, and the line that names the bindings. Every expected name and place is worked out by
//! hand.

use std::path::PathBuf;

use super::{dependency_line, scan, StructuredTextUse, Uses};
use crate::native::rust_pous::read;

/// What `text` writes under `st::`, as `name line:column`, with its globs and its own `st`.
fn scanned(text: &str) -> (Vec<String>, Vec<String>, Option<String>) {
    let mut uses = Uses::default();
    scan(
        text.parse().expect("the text tokenizes"),
        "src/lib.rs",
        &mut uses,
    );
    let place = |used: &StructuredTextUse| format!("{} {}:{}", used.name, used.line, used.column);
    (
        uses.names.iter().map(place).collect(),
        uses.globs.iter().map(place).collect(),
        uses.own.as_ref().map(place),
    )
}

#[test]
fn the_crate_names_the_structured_text_it_calls() {
    // RU-38, RU-39: `use st::{..}` names the first name of each of its trees (HYSTERESIS of
    // `HYSTERESIS as Hysteresis`, `filters` of `filters::LOW`; `self` none), and a path that starts
    // with `st::` names its next segment: a call inside a body, a field's type. A path through
    // `st` (`crate::st::OTHER`), a field named `st` and another crate (`my_st::NOT`) name none.
    // `use st::*` names nothing and is kept with its place. Catches a call the build would leave
    // out of layer 1, a name taken from another path, and an alias taken for a POU.
    let text = "use st::{HYSTERESIS as Hysteresis, filters::LOW, self, LIMIT};\n\
                struct Guard { st: st::GUARD }\n\
                fn f(cx: &mut Cx, x: f64) -> f64 {\n    \
                let y = st::SCALE(cx, x);\n    \
                crate::st::OTHER;\n    \
                self.st.field;\n    \
                my_st::NOT;\n    \
                y\n\
                }\n\
                use st::*;\n";
    assert_eq!(
        scanned(text),
        (
            vec![
                "HYSTERESIS 1:10".to_string(),
                "filters 1:36".to_string(),
                "LIMIT 1:56".to_string(),
                "GUARD 2:24".to_string(),
                "SCALE 4:17".to_string(),
            ],
            vec!["* 10:9".to_string()],
            None
        )
    );
}

#[test]
fn a_crates_own_st_is_kept_apart_from_the_bindings() {
    // RU-39: a `mod st` or an `… as st` of the crate's own is what its `st::` names, as Rust
    // resolves it; the scan keeps the first with its place. Catches a crate's own module read as
    // Structured Text.
    assert_eq!(
        scanned("mod st {\n    pub fn helper() {}\n}\nfn f() { st::helper(); }\n").2,
        Some("st 1:5".to_string())
    );
    assert_eq!(
        scanned("use crate::state as st;\nmod st;\n").2,
        Some("st 1:21".to_string())
    );
}

#[test]
fn the_bindings_line_reaches_the_projects_build_folder() {
    // RU-39: the line a crate names its bindings with leads from its folder to the project's
    // `build/trust_st`: from `rust/` one folder up, from a Rust-first project's own folder none, from
    // a crate two folders deep two up. Catches a path from the wrong folder.
    assert_eq!(
        dependency_line("rust"),
        "trust_st = { path = \"../build/trust_st\" }"
    );
    assert_eq!(
        dependency_line("."),
        "trust_st = { path = \"build/trust_st\" }"
    );
    assert_eq!(
        dependency_line("plc/rust"),
        "trust_st = { path = \"../../build/trust_st\" }"
    );
}

/// A crate's manifest without the bindings.
const MANIFEST: &str = "[package]\nname = \"line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                        [dependencies]\ntrust-plc = \"*\"\n";

/// The manifest's line that names the bindings.
const BINDINGS: &str = "trust_st = { path = \"../build/trust_st\" }\n";

/// A crate's root whose function calls the Structured Text function SCALE at line 9, column 9,
/// after the lines `extra`.
fn lib(extra: &str) -> String {
    format!(
        "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"TWICE\")]
#[budget(\"1us\")]
pub fn twice(cx: &mut Cx, x: f64) -> Result<f64, Fault> {{
    st::SCALE(cx, x, 2.0)
}}
{extra}"
    )
}

/// A project whose crate in `rust/` has the manifest `manifest` and the root `lib`.
fn project(manifest: &str, lib: &str) -> PathBuf {
    let root = trust_test_support::unique_temp_path("rust-pou-structured-text");
    for (path, text) in [("rust/Cargo.toml", manifest), ("rust/src/lib.rs", lib)] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    root
}

/// The reader's errors for the project `root`.
fn refusals(root: &std::path::Path) -> Vec<String> {
    read(root)
        .expect_err("the crate is refused")
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_crate_that_calls_structured_text_names_its_bindings() {
    // RU-39: the reader gives the Structured Text the crate calls, and refuses a crate that writes
    // `st::` without the bindings in [dependencies], naming the line to add, as RU-08's refusals
    // name theirs; `use st::*` is refused at its place. Catches a crate whose own cargo would not
    // find `st`, with the compiler's error in place of the line, and a glob that hides a call.
    assert_eq!(
        refusals(&project(MANIFEST, &lib(""))),
        ["rust/Cargo.toml: the crate calls Structured Text (st::SCALE at rust/src/lib.rs:9): name \
          its bindings in [dependencies], `trust_st = { path = \"../build/trust_st\" }` \
          (specification 48 RU-39)"]
    );
    let named = format!("{MANIFEST}{BINDINGS}");
    assert_eq!(
        refusals(&project(&named, &lib("use st::*;\n"))),
        [
            "rust/src/lib.rs:11:9: `use st::*` hides which Structured Text the crate calls: name \
          each, `use st::{SCALE, HYSTERESIS};` (specification 48 RU-39)"
        ]
    );
    let krate = read(&project(&named, &lib("")))
        .expect("the crate reads")
        .expect("a crate");
    assert_eq!(
        krate.structured_text,
        [StructuredTextUse {
            name: "SCALE".to_string(),
            file: "rust/src/lib.rs".to_string(),
            line: 9,
            column: 9,
        }]
    );
}

#[test]
fn a_crates_own_st_calls_no_structured_text() {
    // RU-39: in a crate with a module `st` of its own, `st::` names that module, as Rust resolves
    // it: the crate calls no Structured Text and needs no bindings; with the bindings named too,
    // `st` would name both, and the reader refuses the crate's own at its place. Catches a crate
    // that builds today refused for a call it does not make.
    let own = "mod st {\n    pub fn SCALE(_: &mut Cx, x: f64, k: f64) -> Result<f64, Fault> { Ok(x * k) }\n}\n";
    let krate = read(&project(MANIFEST, &lib(own)))
        .expect("the crate reads")
        .expect("a crate");
    assert!(
        krate.structured_text.is_empty(),
        "{:?}",
        krate.structured_text
    );
    assert_eq!(
        refusals(&project(&format!("{MANIFEST}{BINDINGS}"), &lib(own))),
        [
            "rust/src/lib.rs:11:5: `st` names the Structured Text bindings that the crate's \
          [dependencies] name: give the crate's own `st` another name (specification 48 RU-39)"
        ]
    );
}
