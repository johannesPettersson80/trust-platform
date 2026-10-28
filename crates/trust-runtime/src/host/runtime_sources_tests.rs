//! The simulator's source selection (specification 13 `stReload`, specification 25 §11.6,
//! specification 38 §3). Each expected list is worked out from the files each test writes.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_SCRATCH: AtomicUsize = AtomicUsize::new(0);

/// A folder under the system temporary folder, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let id = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-sources-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch folder");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(&path, text).expect("file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The file names of `sources`, in their order.
fn names(sources: &[(String, String)]) -> Vec<String> {
    sources
        .iter()
        .map(|(path, _)| {
            Path::new(path)
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn options(root: &Path, include: &[&str]) -> RuntimeSourceOptions {
    RuntimeSourceOptions {
        root: Some(root.to_path_buf()),
        include_globs: include.iter().map(|glob| (*glob).to_string()).collect(),
        ..RuntimeSourceOptions::default()
    }
}

#[test]
fn expands_brace_globs() {
    let patterns = expand_braces("**/*.{st,ST,pou,POU}");
    assert_eq!(patterns, ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"]);
}

#[test]
fn expands_nested_braces() {
    let mut patterns = expand_braces("a{b,c}d{e,f}");
    patterns.sort();
    assert_eq!(patterns, ["abde", "abdf", "acde", "acdf"]);
}

#[test]
fn the_default_runtime_ignore_marker_keeps_a_test_source_out_of_a_launch() {
    // Specification 38 §3: a source marked `@trustlsp:runtime-ignore` in its first lines stays out
    // of the simulator's sources with the default pragmas. Catches: test programs compiled into the
    // running PLC program.
    let project = Scratch::new("marker");
    let entry = project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write("src/Lib.st", "FUNCTION Twice : INT\nEND_FUNCTION\n");
    project.write(
        "src/tests.st",
        "// @trustlsp:runtime-ignore\nTEST_PROGRAM T\nEND_TEST_PROGRAM\n",
    );
    let sources = launch_sources(
        &entry,
        &options(&project.0, &[DEFAULT_INCLUDE_GLOB]),
        &read_from_disk,
    )
    .expect("sources");
    assert_eq!(names(&sources), ["Lib.st", "Main.st"]);
}

#[test]
fn an_empty_pragma_list_takes_marked_sources_and_the_entry_is_always_taken() {
    // `ignore_pragmas: Some([])` names no pragma, so the marked file is taken; the entry is taken even
    // when it carries the marker. Catches: an empty list read as "use the default".
    let project = Scratch::new("no-pragmas");
    let entry = project.write(
        "src/Main.st",
        "// @trustlsp:runtime-ignore\nPROGRAM Main\nEND_PROGRAM\n",
    );
    project.write("src/tests.st", "// @trustlsp:runtime-ignore\n");
    let mut chosen = options(&project.0, &["src/*.st"]);
    let marked_out = launch_sources(&entry, &chosen, &read_from_disk).expect("sources");
    assert_eq!(
        names(&marked_out),
        ["Main.st"],
        "the entry stays, the marked file goes"
    );
    chosen.ignore_pragmas = Some(Vec::new());
    let all = launch_sources(&entry, &chosen, &read_from_disk).expect("sources");
    assert_eq!(names(&all), ["Main.st", "tests.st"]);
}

#[test]
fn a_pragma_below_the_twentieth_line_does_not_count() {
    // The marker is searched in the first 20 lines only (PRAGMA_SCAN_LINES). Catches: a scan of the
    // whole file, which would drop a source that names the marker in a comment further down.
    let project = Scratch::new("pragma-depth");
    let entry = project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let late = format!("{}// @trustlsp:runtime-ignore\n", "\n".repeat(20));
    project.write("src/Late.st", &late);
    let early = format!("{}// @trustlsp:runtime-ignore\n", "\n".repeat(19));
    project.write("src/Early.st", &early);
    let sources = launch_sources(&entry, &options(&project.0, &["src/*.st"]), &read_from_disk)
        .expect("sources");
    assert_eq!(names(&sources), ["Late.st", "Main.st"]);
}

#[test]
fn exclude_globs_drop_files_and_folders_but_never_the_entry() {
    // Catches: an exclude glob that removes the entry, or one that misses a folder's files.
    let project = Scratch::new("exclude");
    let entry = project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write("src/Keep.st", "");
    project.write("src/vendor/Old.st", "");
    project.write("src/Skip.st", "");
    let mut chosen = options(&project.0, &[DEFAULT_INCLUDE_GLOB]);
    chosen.exclude_globs = vec![
        "src/vendor".to_owned(),
        " src/Skip.st ".to_owned(),
        "src/Main.st".to_owned(),
    ];
    let sources = launch_sources(&entry, &chosen, &read_from_disk).expect("sources");
    assert_eq!(names(&sources), ["Keep.st", "Main.st"]);
}

#[test]
fn without_include_globs_a_launch_takes_the_entry_folder_only() {
    // A session that names no include glob takes the files in the entry's folder, not its
    // subfolders. Catches: the default glob of `project_sources` applied to a launch.
    let project = Scratch::new("folder");
    let entry = project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write("src/Beside.pou", "");
    project.write("src/deep/Below.st", "");
    project.write("src/notes.txt", "");
    let sources =
        launch_sources(&entry, &options(&project.0, &[]), &read_from_disk).expect("sources");
    assert_eq!(names(&sources), ["Beside.pou", "Main.st"]);
}

#[test]
fn a_launch_takes_the_sources_of_local_package_dependencies() {
    // As `trust-runtime build` resolves them (`[dependencies]` in trust-lsp.toml). Catches: a check
    // that finds a library's PROGRAM or FUNCTION_BLOCK unknown although the simulator compiles it.
    let project = Scratch::new("dependency");
    let entry = project.write("app/src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write(
        "app/trust-lsp.toml",
        "[project]\ninclude_paths = [\"src\"]\n\n[dependencies]\nLib = { path = \"../lib\" }\n",
    );
    project.write("lib/trust-lsp.toml", "[package]\nversion = \"0.1.0\"\n");
    project.write(
        "lib/src/Block.st",
        "FUNCTION_BLOCK Block\nEND_FUNCTION_BLOCK\n",
    );
    let sources = launch_sources(
        &entry,
        &options(&project.0.join("app"), &[DEFAULT_INCLUDE_GLOB]),
        &read_from_disk,
    )
    .expect("sources");
    // Ordered by path: `<scratch>/app/src/Main.st` before `<scratch>/lib/src/Block.st`.
    assert_eq!(names(&sources), ["Main.st", "Block.st"]);
    let block = sources
        .iter()
        .find(|(path, _)| path.ends_with("Block.st"))
        .expect("the dependency's source");
    assert!(block.1.contains("FUNCTION_BLOCK Block"));
}

#[test]
fn an_open_editor_text_is_read_instead_of_the_file() {
    // The language server passes an editor's unsaved text; the pragma check reads that text. Catches:
    // a selection that reads the disk behind the reader's back.
    let project = Scratch::new("overlay");
    let entry = project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let marked = project.write("src/Helper.st", "FUNCTION Helper : INT\nEND_FUNCTION\n");
    let marked_canonical = canonicalize_lossy(&marked);
    let overlay = |path: &Path| {
        if path == marked_canonical {
            Ok("// @trustlsp:runtime-ignore\n".to_owned())
        } else {
            read_from_disk(path)
        }
    };
    let sources =
        launch_sources(&entry, &options(&project.0, &["src/*.st"]), &overlay).expect("sources");
    assert_eq!(names(&sources), ["Main.st"]);
}

#[test]
fn project_sources_take_every_structured_text_file_by_default() {
    // Without include globs the project selection uses DEFAULT_INCLUDE_GLOB below the root, and
    // keeps the runtime-ignore marker out. Catches: a project selection limited to one folder.
    let project = Scratch::new("project");
    project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write("src/deep/Below.POU", "");
    project.write("src/tests.st", "(* @trustlsp:runtime-ignore *)\n");
    project.write("README.md", "");
    let sources = project_sources(&options(&project.0, &[]), &read_from_disk).expect("sources");
    // Ordered by path: `src/Main.st` before `src/deep/Below.POU` ('M' sorts before 'd').
    assert_eq!(names(&sources), ["Main.st", "Below.POU"]);
}

#[test]
fn project_sources_need_a_root() {
    let error =
        project_sources(&RuntimeSourceOptions::default(), &read_from_disk).expect_err("no root");
    assert!(error.to_string().contains("no project folder"), "{error}");
}

#[test]
fn settings_are_trimmed_and_a_blank_entry_is_no_entry() {
    // The globs and pragmas as the user's settings give them (specification 25 §11.6): spaces
    // around an entry do not count and a blank entry is no entry, so `["", "   "]` is no include
    // glob and the default selection applies. Catches a blank glob that matches nothing and so
    // empties the project, and a pragma that keeps its spaces and never matches (the rule the
    // extension applied before; it now holds only here).
    let project = Scratch::new("trimmed");
    project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write("src/Other.st", "PROGRAM Other\nEND_PROGRAM\n");
    let spaced = project_sources(&options(&project.0, &["  src/Main.st  "]), &read_from_disk)
        .expect("sources");
    assert_eq!(names(&spaced), ["Main.st"]);
    let blank =
        project_sources(&options(&project.0, &["", "   "]), &read_from_disk).expect("sources");
    assert_eq!(names(&blank), ["Main.st", "Other.st"]);
    let mut pragmas = options(&project.0, &[]);
    pragmas.ignore_pragmas = Some(vec![
        "  @trustlsp:runtime-ignore  ".to_string(),
        " ".to_string(),
    ]);
    assert_eq!(
        resolve_ignore_pragmas(&pragmas),
        ["@trustlsp:runtime-ignore"]
    );
}

#[cfg(windows)]
#[test]
fn a_windows_verbatim_path_compares_with_the_path_a_client_gives() {
    // `canonicalize` gives `\\?\C:\proj\Main.st` and `\\?\UNC\server\share\Main.st` on Windows,
    // where a client gives `C:\proj\Main.st` and `\\server\share\Main.st`. Catches the prefix
    // kept, so that no path a client gives ever matches (the pattern had one leading backslash
    // until 26 October 2026).
    for (canonical, given) in [
        (r"\\?\C:\proj\Main.st", r"C:\proj\Main.st"),
        (r"\\?\UNC\server\share\Main.st", r"\\server\share\Main.st"),
        (r"C:\proj\Main.st", r"C:\proj\Main.st"),
    ] {
        assert_eq!(
            strip_windows_device_prefix(PathBuf::from(canonical)),
            PathBuf::from(given)
        );
    }
}
