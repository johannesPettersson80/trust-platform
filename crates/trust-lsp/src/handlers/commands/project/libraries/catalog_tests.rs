//! The curated libraries' quick-fix symbols (specification 25 §11.6: library code actions are
//! offered only for known OSCAT or PLCopen Motion symbols when the dependency is absent). The first
//! three cases are the extension's former `library-code-actions.test.ts` assertions.

use super::*;

#[test]
fn an_oscat_symbol_offers_oscat_when_the_dependency_is_missing() {
    let library = quick_fix_library("INC", &[]).expect("OSCAT");
    assert_eq!(
        (library.id, library.label, library.dependency_name),
        ("oscat", "OSCAT", "OSCAT")
    );
}

#[test]
fn no_offer_when_the_project_already_depends_on_the_library() {
    // Catches: a quick fix that adds the library a second time.
    assert_eq!(quick_fix_library("INC", &["OSCAT"]), None);
}

#[test]
fn mc_symbols_offer_plcopen_motion_and_unknown_symbols_offer_nothing() {
    assert_eq!(
        quick_fix_library("MC_MoveAbsolute", &[]).map(|library| library.id),
        Some("plcopen_motion")
    );
    assert_eq!(quick_fix_library("NotALibrarySymbol", &[]), None);
}

#[test]
fn oscat_and_plant_names_match_in_any_letter_case_but_mc_is_upper_case() {
    // The rules: `Plant`, `INC`, `SCALE`, `BIT_COUNT` in any letter case; `MC_` exactly, followed by
    // at least one letter, digit or underscore. Catches: a case-sensitive OSCAT match, or `mc_x` and
    // a bare `MC_` taken for motion blocks.
    assert_eq!(quick_fix_library("scale", &[]).map(|l| l.id), Some("oscat"));
    assert_eq!(
        quick_fix_library("Bit_Count", &[]).map(|l| l.id),
        Some("oscat")
    );
    assert_eq!(quick_fix_library("PLANT", &[]).map(|l| l.id), Some("plant"));
    assert_eq!(quick_fix_library("mc_MoveAbsolute", &[]), None);
    assert_eq!(quick_fix_library("MC_", &[]), None);
    assert_eq!(quick_fix_library("INCR", &[]), None);
}

#[test]
fn a_dependency_key_in_another_letter_case_is_another_dependency() {
    // The manifest key is compared exactly, as the panel names libraries by their key.
    assert_eq!(
        quick_fix_library("INC", &["oscat"]).map(|library| library.id),
        Some("oscat")
    );
    assert_eq!(
        quick_fix_library("MC_Power", &["PLCopenMotionSingleAxis"]),
        None
    );
}

#[test]
fn the_curated_list_is_plant_oscat_and_plcopen_motion() {
    // Catches: a dependency name that differs from the one the bundled examples use.
    let names: Vec<_> = CURATED
        .iter()
        .map(|library| (library.id, library.dependency_name, library.folder))
        .collect();
    assert_eq!(
        names,
        [
            ("plant", "Plant", "plant"),
            ("oscat", "OSCAT", "oscat"),
            (
                "plcopen_motion",
                "PLCopenMotionSingleAxis",
                "plcopen_motion"
            ),
        ]
    );
    assert_eq!(by_dependency_name("OSCAT").map(|l| l.id), Some("oscat"));
    assert_eq!(by_dependency_name("Oscat"), None);
}
