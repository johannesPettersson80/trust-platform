//! The Libraries panel's edits of a project (specification 25 §7, §11.6). Each test writes its
//! project, bundled libraries and folders; the expected manifest follows from them.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-libraries-edits-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, text).expect("file");
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.path(relative)).expect("file")
    }

    /// A project with the new project's manifest and the extension's bundled OSCAT 0.1.0.
    fn project_and_bundled_oscat() -> Self {
        let scratch = Self::new();
        scratch.write("project/trust-lsp.toml", "include_paths = [\"src\"]\n");
        scratch.write(
            "bundled/oscat/trust-lsp.toml",
            "[package]\nversion = \"0.1.0\"\n",
        );
        scratch.write(
            "bundled/oscat/src/basic.st",
            "FUNCTION INC : INT\nEND_FUNCTION\n",
        );
        scratch
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn adding_oscat_vendors_the_bundled_copy_and_names_it_with_its_version() {
    let scratch = Scratch::project_and_bundled_oscat();
    let added =
        add_curated(&scratch.path("project"), &scratch.path("bundled"), "oscat").expect("added");
    assert_eq!(
        added,
        Added::Done("OSCAT added to this project.".to_owned())
    );
    assert_eq!(
        scratch.read("project/libraries/oscat/src/basic.st"),
        "FUNCTION INC : INT\nEND_FUNCTION\n"
    );
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "include_paths = [\"src\"]\n\n[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.1.0\" }\n"
    );
}

#[test]
fn adding_again_replaces_the_vendored_copy() {
    // An update re-adds the library. Catches: files of the old copy left beside the new one.
    let scratch = Scratch::project_and_bundled_oscat();
    scratch.write("project/libraries/oscat/src/removed_in_new_version.st", "");
    add_curated(&scratch.path("project"), &scratch.path("bundled"), "oscat").expect("added");
    assert!(!scratch
        .path("project/libraries/oscat/src/removed_in_new_version.st")
        .exists());
    assert!(scratch
        .path("project/libraries/oscat/src/basic.st")
        .is_file());
}

#[test]
fn a_missing_bundled_copy_an_unknown_id_and_plant_are_refused() {
    let scratch = Scratch::project_and_bundled_oscat();
    let project = scratch.path("project");
    assert_eq!(
        add_curated(&project, &scratch.path("bundled"), "plcopen_motion").expect_err("missing"),
        "Bundled PLCopen Motion files are missing from the extension."
    );
    assert_eq!(
        add_curated(&project, &scratch.path("bundled"), "nope").expect_err("unknown"),
        "Unknown curated library: nope"
    );
    assert!(add_curated(&project, &scratch.path("bundled"), "plant").is_err());
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "include_paths = [\"src\"]\n",
        "nothing written"
    );
}

#[test]
fn a_local_folder_without_a_manifest_asks_first_and_writes_the_template_only_when_agreed() {
    // Catches: a manifest created behind the user's back, or a dependency written for a folder that is
    // no library.
    let scratch = Scratch::project_and_bundled_oscat();
    let project = scratch.path("project");
    let folder = scratch.path("My Lib");
    std::fs::create_dir_all(&folder).expect("folder");
    assert_eq!(
        add_local(&project, &folder, None).expect("asks"),
        Added::NeedsManifest
    );
    assert!(!folder.join(MANIFEST_FILE).exists());
    assert_eq!(
        add_local(&project, &folder, Some(false)).expect_err("refused"),
        "No library added — the folder needs a truST library manifest."
    );
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "include_paths = [\"src\"]\n"
    );
    assert_eq!(
        add_local(&project, &folder, Some(true)).expect("added"),
        Added::Done("My_Lib added to this project.".to_owned())
    );
    assert_eq!(
        scratch.read("My Lib/trust-lsp.toml"),
        LIBRARY_MANIFEST_TEMPLATE
    );
    assert!(folder.join("src").is_dir());
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "include_paths = [\"src\"]\n\n[dependencies]\nMy_Lib = { path = \"../My Lib\", version = \"0.1.0\" }\n"
    );
}

#[test]
fn a_missing_local_folder_is_refused() {
    let scratch = Scratch::project_and_bundled_oscat();
    assert_eq!(
        add_local(
            &scratch.path("project"),
            &scratch.path("nowhere"),
            Some(true)
        )
        .expect_err("missing"),
        "No library added — the selected folder does not exist."
    );
}

#[test]
fn a_git_library_is_pinned_by_what_the_user_typed() {
    let scratch = Scratch::project_and_bundled_oscat();
    let project = scratch.path("project");
    assert_eq!(
        add_git(
            &project,
            "VendorLib",
            "file:///tmp/vendor",
            Some("v1.0.0"),
            Some("1.0.0".to_owned())
        )
        .expect("added"),
        Added::Done("VendorLib added from Git.".to_owned())
    );
    add_git(
        &project,
        "Other",
        "file:///tmp/other",
        Some("  "),
        Some(String::new()),
    )
    .expect("added");
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "include_paths = [\"src\"]\n\n[dependencies]\nVendorLib = { git = \"file:///tmp/vendor\", tag = \"v1.0.0\", version = \"1.0.0\" }\nOther = { git = \"file:///tmp/other\" }\n"
    );
    assert!(add_git(&project, " ", "file:///tmp/x", None, None).is_err());
}

#[test]
fn removing_a_library_keeps_the_others() {
    let scratch = Scratch::new();
    scratch.write(
        "project/trust-lsp.toml",
        "[dependencies]\nA = \"libs/a\"\nB = \"libs/b\"\n",
    );
    assert_eq!(
        remove(&scratch.path("project"), "A").expect("removed"),
        "A removed from this project."
    );
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "[dependencies]\nB = \"libs/b\"\n"
    );
}

#[test]
fn an_update_names_the_vendored_and_the_bundled_version() {
    let scratch = Scratch::project_and_bundled_oscat();
    let project = scratch.path("project");
    assert_eq!(
        update_plan(&project, &scratch.path("bundled"), "oscat").expect("plan"),
        ("OSCAT", "unknown".to_owned(), "0.1.0".to_owned()),
        "nothing vendored yet"
    );
    scratch.write(
        "project/libraries/oscat/trust-lsp.toml",
        "[package]\nversion = \"0.0.9\"\n",
    );
    assert_eq!(
        update_plan(&project, &scratch.path("bundled"), "oscat").expect("plan"),
        ("OSCAT", "0.0.9".to_owned(), "0.1.0".to_owned())
    );
}

#[test]
fn fixing_a_path_needs_a_library_manifest_in_the_new_folder() {
    // Catches: a dependency pointed at a folder that is no library.
    let scratch = Scratch::new();
    scratch.write(
        "project/trust-lsp.toml",
        "[dependencies]\nShared = { path = \"../gone\" }\n",
    );
    std::fs::create_dir_all(scratch.path("plain")).expect("folder");
    assert_eq!(
        fix_path(&scratch.path("project"), "Shared", &scratch.path("plain")).expect_err("refused"),
        "No library path changed — the selected folder needs a truST library manifest."
    );
    scratch.write("shared/trust-lsp.toml", "[package]\nversion = \"2.1.0\"\n");
    assert_eq!(
        fix_path(&scratch.path("project"), "Shared", &scratch.path("shared")).expect("fixed"),
        "Shared path updated."
    );
    assert_eq!(
        scratch.read("project/trust-lsp.toml"),
        "[dependencies]\nShared = { path = \"../shared\", version = \"2.1.0\" }\n"
    );
}

#[test]
fn a_declaration_goes_below_the_first_var_line_with_its_indentation() {
    // Catches: an insertion into VAR_INPUT, or one without the block's indentation.
    let text = "PROGRAM Main\nVAR_INPUT\n  start : BOOL;\nEND_VAR\nVAR\n\tcount : INT;\nEND_VAR\nEND_PROGRAM\n";
    assert_eq!(
        insertion(text, " averager : Averager; "),
        Some((5, "\taverager : Averager;\n".to_owned()))
    );
    assert_eq!(
        insertion("program p\nvar\nend_var\n", "value : AxisState;"),
        Some((2, "    value : AxisState;\n".to_owned())),
        "an unindented next line gets four blanks; VAR in any letter case"
    );
    assert_eq!(
        insertion("PROGRAM P\r\nVAR\r\n  x : INT;\r\nEND_VAR\r\n", "y : INT;"),
        Some((2, "  y : INT;\n".to_owned()))
    );
    assert_eq!(
        insertion(text, "SCALE(...)"),
        None,
        "a call is no declaration"
    );
    assert_eq!(
        insertion("PROGRAM P\nVAR_INPUT\nEND_VAR\n", "y : INT;"),
        None
    );
}
