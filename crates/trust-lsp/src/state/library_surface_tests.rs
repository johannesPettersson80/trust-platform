//! The library-surface rule (specification 14 §7.6, specification 28 §1): for each file, the
//! deepest folder that holds it among the project's own source folders and its library folders
//! decides, and an own folder wins over an equally deep library folder.

use super::*;

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

/// Folders given as already resolved, for the rule alone.
fn folders(own: &[&str], libraries: &[&str]) -> ProjectFolders {
    ProjectFolders {
        own: own.iter().map(PathBuf::from).collect(),
        libraries: libraries.iter().map(PathBuf::from).collect(),
    }
}

/// Whether `file`, named as the editor names it, is library surface under `folders`.
fn surface(folders: &ProjectFolders, file: &Path) -> bool {
    folders.is_library_surface(&canonicalize_through_existing_ancestor(file))
}

/// A fresh folder under the system's temporary folder, removed on drop.
struct TempFolder(PathBuf);

impl TempFolder {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trustlsp-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the temporary folder");
        Self(path)
    }

    /// Writes `text` to `relative`, with its folders.
    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("a folder")).expect("create folders");
        fs::write(&path, text).expect("write the file");
        path
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str = "FUNCTION F : INT\nF := 0;\nEND_FUNCTION\n";

/// A library folder inside the project folder is two components deeper than the root, so its
/// files are library surface; the root's other files are the project's own, and a sibling folder
/// that only shares the library's name as a prefix is not the library. Catches a rule that treats
/// every file as the project's own, and a textual prefix match (`libs/vendor-other`).
#[test]
fn the_deepest_holding_folder_decides() {
    let project = folders(&["/p"], &["/p/libs/vendor", "/q/shared"]);

    assert!(project.is_library_surface(Path::new("/p/libs/vendor/string.st")));
    assert!(project.is_library_surface(Path::new("/q/shared/util.st")));
    assert!(!project.is_library_surface(Path::new("/p/src/main.st")));
    assert!(!project.is_library_surface(Path::new("/p/libs/vendor-other/x.st")));
    assert!(!project.is_library_surface(Path::new("/q/other/x.st")));
}

/// A library entry equal to an own source folder holds the file exactly as deep as the own
/// folder, and the own folder wins: `[[libraries]] path = "."` without `include_paths`, or a
/// library at the include path `src`, exempts nothing. Catches a rule where any library folder
/// that holds a file makes it library surface, which would silence the whole application.
#[test]
fn an_own_folder_wins_over_an_equally_deep_library_folder() {
    let whole = folders(&["/p"], &["/p"]);
    assert!(!whole.is_library_surface(Path::new("/p/src/main.st")));
    assert!(!whole.is_library_surface(Path::new("/p/libs/vendor/x.st")));

    let at_include_path = folders(&["/p/src"], &["/p/src"]);
    assert!(!at_include_path.is_library_surface(Path::new("/p/src/main.st")));
    assert!(!at_include_path.is_library_surface(Path::new("/p/src/vendor/x.st")));
}

/// The Plant examples' layout, `Plant = { path = "../.." }` with `include_paths = ["src"]`: the
/// library folder `/w/plant` (three components) holds the example, whose own folder
/// `/w/plant/examples/x/src` (six components) is deeper. The library's sources and its other
/// examples are library surface, the example's own files are not. Catches the refused entry of
/// the former `L008`, which exempted nothing, and a rule where any library folder that holds a
/// file wins, which silenced the example.
#[test]
fn a_library_folder_that_holds_the_project_exempts_only_the_library_files() {
    let example = folders(&["/w/plant/examples/x/src"], &["/w/plant"]);

    assert!(example.is_library_surface(Path::new("/w/plant/src/tank.st")));
    assert!(example.is_library_surface(Path::new("/w/plant/examples/y/src/other.st")));
    assert!(!example.is_library_surface(Path::new("/w/plant/examples/x/src/main.st")));
}

/// `include_paths = ["src"]` with a library at `src/vendor`: the library folder is one component
/// deeper than the include path, so only `src/vendor` is library surface. Catches a rule where
/// any own folder that holds a file makes it the project's own.
#[test]
fn a_library_inside_an_include_path_is_library_surface() {
    let project = folders(&["/p/src"], &["/p/src/vendor"]);

    assert!(project.is_library_surface(Path::new("/p/src/vendor/v.st")));
    assert!(!project.is_library_surface(Path::new("/p/src/main.st")));
}

/// Without library folders nothing is library surface, in the project or outside it. Catches an
/// empty library list read as "everything is a library".
#[test]
fn a_project_without_library_folders_has_no_library_surface() {
    let project = folders(&["/p"], &[]);

    assert!(!project.is_library_surface(Path::new("/p/src/main.st")));
    assert!(!project.is_library_surface(Path::new("/elsewhere/x.st")));
}

/// The three kinds of library folder all count: a `[[libraries]]` entry, a `[project]
/// library_paths` entry and a resolved `[dependencies]` package, inside the project folder or
/// outside it. Catches a kind of library folder the rule leaves out.
#[test]
fn every_kind_of_library_folder_is_library_surface() {
    let temp = TempFolder::new("library-kinds");
    let outside = TempFolder::new("library-kinds-outside");
    let root = temp.0.clone();
    let files = [
        temp.write("libs/vendor/string.st", SOURCE),
        temp.write("shared/util.st", SOURCE),
        temp.write("packages/pkg/pkg.st", SOURCE),
        outside.write("ext.st", SOURCE),
    ];
    let main = temp.write("src/main.st", SOURCE);
    let sibling = temp.write("libs/vendor-other/x.st", SOURCE);
    temp.write(
        "trust-lsp.toml",
        &format!(
            "[project]\nlibrary_paths = [\"shared\"]\n\n\
             [[libraries]]\nname = \"Vendor\"\npath = \"libs/vendor\"\n\n\
             [[libraries]]\nname = \"Outside\"\npath = '{}'\n\n\
             [dependencies]\nPkg = {{ path = \"packages/pkg\" }}\n",
            outside.0.display()
        ),
    );

    let config = ProjectConfig::load(&root);
    let project = ProjectFolders::of(&config);
    for file in &files {
        assert!(
            surface(&project, file),
            "{} must be library surface",
            file.display()
        );
    }
    assert!(!surface(&project, &main));
    assert!(!surface(&project, &sibling));
    assert!(!surface(&project, &root.join("trust-lsp.toml")));
}

/// A `[dependencies]` folder is stored canonicalized, which on Windows adds the `\\?\` prefix and
/// everywhere resolves links and letter case, while the editor names the file as the project was
/// opened (macOS `/var` for `/private/var`, Windows `RUNNER~1`). Both sides are resolved, so the
/// file is library surface. Catches a comparison of the spelling as written against the resolved
/// folder, which failed on the macOS and Windows runners.
#[test]
fn library_surface_matches_canonicalized_dependency_paths() {
    let temp = TempFolder::new("library-canonical");
    let root = temp.0.clone();
    let document = temp.write("libs/vendor/string.st", SOURCE);
    let main = temp.write("src/main.st", SOURCE);
    let canonical = root
        .join("libs/vendor")
        .canonicalize()
        .expect("canonicalize the library folder");
    let config = ProjectConfig::from_contents(
        &root,
        Some(root.join("trust-lsp.toml")),
        &format!(
            "[[libraries]]\nname = 'Vendor'\npath = '{}'\n",
            canonical.display()
        ),
    );

    let project = ProjectFolders::of(&config);
    assert!(surface(&project, &document));
    assert!(!surface(&project, &main));
}

/// `[[libraries]] path = "."` is the project folder and `".."` holds it; neither exempts any
/// file of the project, which is deeper in its own folder, while a file beside the project in
/// the parent folder is the parent library's. Catches the former refusal (`L008`), which
/// exempted nothing at all, and a rule where any holding library folder wins.
#[test]
fn a_library_at_or_above_the_project_folder_exempts_none_of_its_files() {
    let temp = TempFolder::new("library-above");
    let project_main = temp.write("project/src/main.st", SOURCE);
    let project_vendor = temp.write("project/libs/vendor/x.st", SOURCE);
    let beside = temp.write("beside/x.st", SOURCE);
    let root = temp.0.join("project");
    temp.write(
        "project/trust-lsp.toml",
        "[[libraries]]\nname = \"Whole\"\npath = \".\"\n\n\
         [[libraries]]\nname = \"Parent\"\npath = '..'\n",
    );

    let project = ProjectFolders::of(&ProjectConfig::load(&root));
    assert!(!surface(&project, &project_main));
    assert!(!surface(&project, &project_vendor));
    assert!(surface(&project, &beside));
}

/// A document that does not exist (deleted, or not saved yet) under a library folder reached
/// through a link resolves through the folder that exists, so it stays library surface. Catches
/// a missing file compared as written against the resolved library folder.
#[cfg(unix)]
#[test]
fn a_missing_file_under_a_linked_library_folder_is_library_surface() {
    let temp = TempFolder::new("library-linked");
    temp.write("real/vendor/x.st", SOURCE);
    let root = temp.0.join("project");
    fs::create_dir_all(root.join("libs")).expect("create the project folder");
    std::os::unix::fs::symlink(temp.0.join("real/vendor"), root.join("libs/vendor"))
        .expect("link the library folder");
    temp.write(
        "project/trust-lsp.toml",
        "[[libraries]]\nname = \"Vendor\"\npath = \"libs/vendor\"\n",
    );

    let project = ProjectFolders::of(&ProjectConfig::load(&root));
    assert!(surface(&project, &root.join("libs/vendor/x.st")));
    assert!(surface(&project, &root.join("libs/vendor/new.st")));
    assert!(!surface(&project, &root.join("src/new.st")));
}
