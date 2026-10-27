//! Specification 39, §16.2.1–2: the checks of a bundle before it is deployed.

use super::checks::{check_bundle, BundleProblem, Program};
use super::HmiBundle;
use crate::harness::CompileSession;
use crate::hmi::access::config::hmi_access_from_runtime_toml;
use crate::hmi::catalogue::Catalogue;

const PROGRAM: &str = "PROGRAM Main\nVAR\n    Level : REAL;\n    Setpoint : REAL;\n    Spare : REAL;\nEND_VAR\nEND_PROGRAM\n";
const SCREEN: &str = "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n";
const PAGE: &str = r#"format = 1
title = "Overview"

[[object]]
id = "level"
type = "value"
at = [0, 0]
size = [160, 48]

[object.bind]
value = "Main.Level"

[[object]]
id = "sp"
type = "setpoint"
at = [0, 60]
size = [200, 48]

[object.bind]
value = { path = "Main.Setpoint", direction = "read-write" }

[[object]]
id = "logo"
type = "image"
at = [0, 120]
size = [120, 60]
image = "hmi/assets/logo.svg"
"#;

fn problems(files: &[(&str, &str)]) -> Vec<BundleProblem> {
    let declarations = CompileSession::from_source(PROGRAM)
        .declarations()
        .expect("the program compiles");
    let metadata = declarations.metadata().clone();
    let policy = hmi_access_from_runtime_toml(
        "[[runtime.web.hmi.write]]\npath = \"Main.Setpoint\"\nrole = \"operator\"\n",
    )
    .expect("policy")
    .write;
    let bundle = HmiBundle {
        files: files
            .iter()
            .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
            .collect(),
    };
    check_bundle(
        &bundle,
        Catalogue::shipped(),
        &Program {
            metadata: &metadata,
            storage: declarations.storage(),
        },
        &policy,
    )
}

fn valid() -> Vec<(&'static str, String)> {
    vec![
        ("hmi/screen.toml", SCREEN.to_string()),
        ("hmi/pages/overview.toml", PAGE.to_string()),
        ("hmi/assets/logo.svg", "<svg/>".to_string()),
    ]
}

fn with(changes: &[(&'static str, Option<String>)]) -> Vec<BundleProblem> {
    let mut files = valid();
    for (path, text) in changes {
        files.retain(|(existing, _)| existing != path);
        if let Some(text) = text {
            files.push((path, text.clone()));
        }
    }
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (*path, text.as_str()))
        .collect();
    problems(&refs)
}

fn one(found: Vec<BundleProblem>, file: &str, line: usize, words: &str) {
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        (found[0].file.as_str(), found[0].line),
        (file, line),
        "{found:#?}"
    );
    assert!(found[0].message.contains(words), "{words}: {found:#?}");
}

#[test]
fn a_correct_bundle_has_no_problems() {
    assert_eq!(with(&[]), Vec::<BundleProblem>::new());
}

#[test]
fn a_bundle_without_a_screen_layout_is_refused() {
    one(
        with(&[("hmi/screen.toml", None)]),
        "hmi/screen.toml",
        1,
        "screen layout",
    );
}

#[test]
fn a_page_that_does_not_load_is_reported_on_its_line() {
    one(
        with(&[(
            "hmi/pages/overview.toml",
            Some(PAGE.replace("title = \"Overview\"", "title = 3")),
        )]),
        "hmi/pages/overview.toml",
        2,
        "",
    );
}

#[test]
fn an_image_the_bundle_does_not_hold_is_reported() {
    one(
        with(&[("hmi/assets/logo.svg", None)]),
        "hmi/pages/overview.toml",
        27,
        "hmi/assets/logo.svg",
    );
}

#[test]
fn a_binding_the_program_does_not_have_is_reported_on_its_line() {
    one(
        with(&[(
            "hmi/pages/overview.toml",
            Some(PAGE.replace("Main.Level", "Main.Gone")),
        )]),
        "hmi/pages/overview.toml",
        11,
        "Main.Gone",
    );
}

#[test]
fn a_control_outside_the_write_policy_is_reported_on_its_line() {
    one(
        with(&[(
            "hmi/pages/overview.toml",
            Some(PAGE.replace("Main.Setpoint", "Main.Spare")),
        )]),
        "hmi/pages/overview.toml",
        20,
        "write policy",
    );
}

const POPUP: &str = r#"format = 1
title = "Level"
size = [320, 200]

[[object]]
id = "level"
type = "value"
at = [0, 0]
size = [160, 48]

[object.bind]
value = "Main.Gone"
"#;

#[test]
fn a_popups_bindings_are_checked_like_a_pages() {
    let found = with(&[("hmi/popups/level.toml", Some(POPUP.to_string()))]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        (found[0].file.as_str(), found[0].line),
        ("hmi/popups/level.toml", 12)
    );
    assert!(found[0].message.contains("Main.Gone"), "{found:#?}");
}

#[test]
fn a_click_to_a_page_or_popup_the_bundle_does_not_have_is_reported_on_its_line() {
    let page = format!("{PAGE}\n[[object]]\nid = \"more\"\ntype = \"page-button\"\nat = [0, 200]\nsize = [120, 40]\n\n[object.click]\npopup = \"details\"\n");
    let found = with(&[("hmi/pages/overview.toml", Some(page.clone()))]);
    let line = page
        .lines()
        .position(|line| line == "popup = \"details\"")
        .expect("the click line")
        + 1;
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        (found[0].file.as_str(), found[0].line),
        ("hmi/pages/overview.toml", line)
    );
    assert!(found[0].message.contains("details"), "{found:#?}");
    let present = with(&[
        ("hmi/pages/overview.toml", Some(page)),
        (
            "hmi/popups/details.toml",
            Some(POPUP.replace("Main.Gone", "Main.Level")),
        ),
    ]);
    assert_eq!(present, vec![], "the popup is in the bundle");
}

const SYMBOL: &str = "format = 1\ntitle = \"Agitator\"\nsize = [128, 108]\n\n[properties]\nlevel = \"number\"\n\n[[part]]\nid = \"liquid\"\nlevel = \"level\"\n";
const SYMBOL_DRAWING: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><rect id=\"liquid\" width=\"64\" height=\"64\"/></svg>";

fn page_with_symbol(kind: &str) -> String {
    format!("{PAGE}\n[[object]]\nid = \"mixer\"\ntype = \"{kind}\"\nat = [300, 0]\nsize = [128, 108]\n\n[object.bind]\nlevel = \"Main.Level\"\n")
}

/// Specification 39, §14.4 items 1 and 6: a page places the project's own symbols, whose files
/// travel in the bundle; their problems are reported before deploying.
#[test]
fn a_page_places_the_projects_own_symbols() {
    let found = with(&[
        ("hmi/symbols/agitator.toml", Some(SYMBOL.to_string())),
        ("hmi/symbols/agitator.svg", Some(SYMBOL_DRAWING.to_string())),
        (
            "hmi/pages/overview.toml",
            Some(page_with_symbol("my:agitator")),
        ),
    ]);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn a_symbols_problems_and_a_symbol_the_project_lacks_are_reported() {
    let broken = SYMBOL.replace("id = \"liquid\"", "id = \"water\"");
    let found = with(&[
        ("hmi/symbols/agitator.toml", Some(broken)),
        ("hmi/symbols/agitator.svg", Some(SYMBOL_DRAWING.to_string())),
    ]);
    one(
        found,
        "hmi/symbols/agitator.toml",
        9,
        "no element \"water\"",
    );
    let found = with(&[(
        "hmi/pages/overview.toml",
        Some(page_with_symbol("my:missing")),
    )]);
    one(
        found,
        "hmi/pages/overview.toml",
        31,
        "the component type \"my:missing\" is not defined",
    );
}

#[test]
fn an_explicitly_incomplete_page_cannot_pass_deployment_checks() {
    let incomplete = PAGE.replacen("format = 1", "format = 1\nincomplete = true", 1);
    let result = with(&[("hmi/pages/overview.toml", Some(incomplete))]);
    assert!(result
        .iter()
        .any(|problem| problem.message.contains("marked incomplete")));
    assert!(
        with(&[]).is_empty(),
        "the ordinary fixture remains deployable"
    );
}
