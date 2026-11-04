//! A project's diagnostics as `trust-lsp check` reports them, for other tools (specification 20
//! §7, specification 14 §7.7): the project's `trust-lsp.toml` decides the vendor profile, the
//! libraries and the severities, whichever route asks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::{project_diagnostics, Finding};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A project folder with `config` as its `trust-lsp.toml` and `files` under it, and a sibling
/// folder for libraries outside it, both removed on drop.
struct Project {
    root: PathBuf,
    outside: PathBuf,
}

impl Project {
    fn new(config: &str, files: &[(&str, &str)]) -> Self {
        let base = std::env::temp_dir().join(format!(
            "trust-lsp-project-diagnostics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("project");
        let outside = base.join("shared");
        std::fs::create_dir_all(&root).expect("project folder");
        std::fs::create_dir_all(&outside).expect("library folder");
        std::fs::write(root.join("trust-lsp.toml"), config).expect("config");
        for (path, text) in files {
            let path = base.join(path);
            std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
            std::fs::write(path, text).expect("source");
        }
        Self { root, outside }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.root.parent().expect("base folder"));
    }
}

const CODESYS: &str = "[project]\ninclude_paths = [\"src\"]\nvendor_profile = \"codesys\"\n";
const NARROWING: &str =
    "PROGRAM Main\nVAR\n    s : SINT;\n    b : BYTE;\nEND_VAR\nb := s;\nEND_PROGRAM\n";

fn with_code<'a>(findings: &'a [Finding], code: &str) -> Vec<&'a Finding> {
    findings
        .iter()
        .filter(|finding| finding.code.as_deref() == Some(code))
        .collect()
}

// Catches #236's route that measures the IEC rules on a CODESYS project: specification 14's vendor
// profile `codesys` (IEC_DEVIATIONS DEV-037) makes a narrowing integer store a warning, which stays
// an error under `iec`.
#[test]
fn the_vendor_profile_decides_a_narrowing_stores_severity() {
    for (profile, severity) in [("codesys", "warning"), ("iec", "error")] {
        let config =
            format!("[project]\ninclude_paths = [\"src\"]\nvendor_profile = \"{profile}\"\n");
        let project = Project::new(&config, &[("project/src/main.st", NARROWING)]);
        let findings = project_diagnostics(&project.root, None, None).expect("diagnostics");
        let narrowing = with_code(&findings, "E203");
        assert_eq!(narrowing.len(), 1, "{profile}: {findings:?}");
        assert_eq!(narrowing[0].severity, severity, "{profile}");
        assert_eq!(narrowing[0].message, "cannot assign 'SINT' to 'BYTE'");
        assert_eq!(
            (
                narrowing[0].path.as_str(),
                narrowing[0].line,
                narrowing[0].column
            ),
            ("src/main.st", 6, 1)
        );
    }
}

// Catches a route that loads only the project's own folder, so that every symbol of a library the
// configuration names elsewhere is undefined (#236: 12,379 errors on a vendor project), and one that
// drops the library file's own findings, which `trust-lsp check` reports with its full path.
#[test]
fn a_library_outside_the_project_resolves_and_is_reported() {
    let library = "FUNCTION_BLOCK fbLib\nVAR_INPUT\n    x : INT;\nEND_VAR\nVAR\n    s : SINT;\n    b : BYTE;\nEND_VAR\nb := s;\nEND_FUNCTION_BLOCK\n";
    let project = Project::new(
        &format!("{CODESYS}library_paths = [\"../shared\"]\n"),
        &[
            ("shared/fb_lib.st", library),
            (
                "project/src/main.st",
                "PROGRAM Main\nVAR\n    fb : fbLib;\nEND_VAR\nfb(x := 1);\nEND_PROGRAM\n",
            ),
        ],
    );
    let findings = project_diagnostics(&project.root, None, None).expect("diagnostics");
    assert!(
        !findings
            .iter()
            .any(|finding| finding.path == "src/main.st" && finding.message.contains("fbLib")),
        "the library's function block resolves: {findings:?}"
    );
    let library_file = project
        .outside
        .join("fb_lib.st")
        .canonicalize()
        .expect("library path");
    let in_library = with_code(&findings, "E203");
    assert_eq!(in_library.len(), 1, "{findings:?}");
    assert_eq!(in_library[0].file, library_file);
    assert_eq!(
        in_library[0].path,
        crate::check::native_path(&library_file).replace('\\', "/")
    );
    assert_eq!((in_library[0].line, in_library[0].severity), (9, "warning"));
}

// Catches a route that ignores `[diagnostics.severity_overrides]`, which the editor applies.
#[test]
fn the_projects_severity_overrides_apply() {
    let project = Project::new(
        &format!("{CODESYS}\n[diagnostics.severity_overrides]\nE203 = \"hint\"\n"),
        &[("project/src/main.st", NARROWING)],
    );
    let findings = project_diagnostics(&project.root, None, None).expect("diagnostics");
    let narrowing = with_code(&findings, "E203");
    assert_eq!(narrowing.len(), 1, "{findings:?}");
    assert_eq!(narrowing[0].severity, "hint");
}

// Catches an override that is written to disk, or ignored in favour of the saved text: the agent
// diagnoses changes it has not saved (specification 20 §7).
#[test]
fn content_replaces_the_saved_text_of_its_file_only_in_the_analysis() {
    let saved =
        "PROGRAM Main\nVAR\n    s : SINT;\n    b : BYTE;\nEND_VAR\nb := BYTE#1;\nEND_PROGRAM\n";
    let project = Project::new(CODESYS, &[("project/src/main.st", saved)]);
    let path = Path::new("src/main.st");
    let clean = project_diagnostics(&project.root, Some(path), None).expect("diagnostics");
    assert!(with_code(&clean, "E203").is_empty(), "{clean:?}");
    let changed =
        project_diagnostics(&project.root, Some(path), Some(NARROWING)).expect("diagnostics");
    assert_eq!(with_code(&changed, "E203").len(), 1, "{changed:?}");
    assert_eq!(
        std::fs::read_to_string(project.root.join(path)).expect("saved file"),
        saved
    );
}

// Catches columns counted in bytes and byte offsets counted in UTF-16 code units: the line before the
// finding holds "Ä", one UTF-16 code unit and two UTF-8 bytes.
#[test]
fn columns_count_utf16_units_and_offsets_count_bytes() {
    let source = "PROGRAM Main\nVAR\n    s : SINT;\n    b : BYTE;\n    w : WSTRING;\nEND_VAR\nw := \"Ä\"; b := s;\nEND_PROGRAM\n";
    let project = Project::new(CODESYS, &[("project/src/main.st", source)]);
    let findings = project_diagnostics(&project.root, None, None).expect("diagnostics");
    let narrowing = with_code(&findings, "E203");
    assert_eq!(narrowing.len(), 1, "{findings:?}");
    // Lines 1 to 6 hold 13 + 4 + 14 + 14 + 17 + 8 = 70 bytes. On line 7, `w := "Ä"; ` before `b` is
    // ten UTF-16 code units and eleven bytes.
    assert_eq!((narrowing[0].line, narrowing[0].column), (7, 11));
    assert_eq!(narrowing[0].start, 70 + 11);
    assert!(narrowing[0].end > narrowing[0].start);
}

// Catches a named file outside the project's include paths that is diagnosed anyway, and an
// override without the file it replaces.
#[test]
fn a_file_the_project_does_not_hold_and_content_without_a_path_are_refused() {
    let project = Project::new(
        CODESYS,
        &[
            ("project/src/main.st", NARROWING),
            ("project/notes/draft.st", NARROWING),
        ],
    );
    let error = project_diagnostics(&project.root, Some(Path::new("notes/draft.st")), None)
        .expect_err("not a project file");
    assert!(
        error.contains("not a source file of the project"),
        "{error}"
    );
    let error = project_diagnostics(&project.root, None, Some(NARROWING)).expect_err("no path");
    assert!(error.contains("content needs the path"), "{error}");
}
