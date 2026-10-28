//! A project's files as the client names them, and as the build resolves them (specification 13,
//! "Source Mapping"). A project opened through a symbolic link, as macOS's temporary folder `/var`
//! links to `/private/var`, or through a short name, as Windows names `C:\Users\RUNNER~1`, keeps
//! the client's paths: the open documents, the files the commands read and the edits they return
//! name the file the editor has open, and only the check that a file lies in the project compares
//! resolved paths.

use std::path::{Path, PathBuf};

use trust_runtime::runtime_sources::canonicalize_lossy;

/// The folder `root` as the build resolves it, through symbolic links and short names.
pub(super) fn resolved_root(root: &Path) -> PathBuf {
    canonicalize_lossy(root)
}

/// `path` named under `root` as the client names it when it lies under the root's resolved folder
/// `resolved_root`; any other path stays as it is. A path already under `root` is kept; another
/// spelling of a file in the project, as the build's resolved paths are on Windows (`\\?\C:\…`)
/// while the client's root has no such prefix, is named under `root` too.
pub(super) fn client_path(root: &Path, resolved_root: &Path, path: PathBuf) -> PathBuf {
    if path.starts_with(root) {
        return path;
    }
    match canonicalize_lossy(&path).strip_prefix(resolved_root) {
        Ok(rest) => root.join(rest),
        Err(_) => path,
    }
}

/// The project file `file`, a path relative to the project with `/` separators, under `root`, joined
/// part by part: a root in Windows' verbatim form (`\\?\C:\…`) takes no `/` as a separator.
pub(super) fn project_file(root: &Path, file: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    path.extend(file.split('/'));
    path
}

/// `path` resolved as the build resolves it, through its deepest folder that exists, with the
/// names that do not exist yet, as of a new file, kept after it.
pub(super) fn resolved(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if existing.exists() {
            return missing
                .into_iter()
                .rev()
                .fold(canonicalize_lossy(existing), |folder, name| {
                    folder.join(name)
                });
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_owned());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod path_form_tests {
    use super::{client_path, resolved_root};

    /// Specification 13, "Source Mapping": a file of the project that the build names in another
    /// form than the client's root, here through a sibling folder and `..` (on Windows, the
    /// resolved `\\?\C:\…` form of the same folder), is named under the client's root. A path the
    /// client already names under its root is kept. Catches a client path that keeps the build's
    /// form whenever the root needs no resolving, so that no project file lies under the root and
    /// Keep takes Create PLC code's program for a library's (Windows CI, 28 October 2026).
    #[test]
    fn another_spelling_of_a_project_file_is_named_under_the_clients_root() {
        let folder =
            std::env::temp_dir().join(format!("trustlsp-path-forms-{}", std::process::id()));
        let root = folder.join("project");
        std::fs::create_dir_all(root.join("src")).expect("project folder");
        std::fs::create_dir_all(folder.join("sibling")).expect("sibling folder");
        std::fs::write(root.join("src").join("Main.st"), "").expect("program file");
        let resolved = resolved_root(&root);
        let spelled = folder
            .join("sibling")
            .join("..")
            .join("project")
            .join("src")
            .join("Main.st");
        assert_eq!(
            client_path(&root, &resolved, spelled),
            root.join("src").join("Main.st")
        );
        let named = root.join("src").join("Main.st");
        assert_eq!(client_path(&root, &resolved, named.clone()), named);
        std::fs::remove_dir_all(&folder).expect("remove the test folder");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{client_path, resolved, resolved_root};

    /// Specification 13, "Source Mapping": a project behind a symbolic link keeps the client's
    /// paths for the files the build resolved, a new file resolves through the folder that holds
    /// it, and a file outside the project stays as the build named it. Catches commands that read
    /// or edit `/private/var/…` while the editor has `/var/…` open, and a new file that would count
    /// as outside the project.
    #[test]
    fn a_project_behind_a_link_keeps_the_clients_paths() {
        let folder =
            std::env::temp_dir().join(format!("trustlsp-client-paths-{}", std::process::id()));
        let target = folder.join("target");
        std::fs::create_dir_all(target.join("src")).expect("project folder");
        let link = folder.join("link");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).expect("symbolic link");
        let resolved_project = resolved_root(&link);
        assert_eq!(
            resolved_project,
            std::fs::canonicalize(&target).expect("resolved project")
        );

        assert_eq!(
            client_path(
                &link,
                &resolved_project,
                resolved_project.join("src").join("Main.st")
            ),
            link.join("src").join("Main.st")
        );
        let outside = std::fs::canonicalize(&folder)
            .expect("resolved folder")
            .join("Lib.st");
        assert_eq!(
            client_path(&link, &resolved_project, outside.clone()),
            outside
        );
        assert_eq!(
            resolved(&link.join("src").join("New").join("New.st")),
            resolved_project.join("src").join("New").join("New.st")
        );
        std::fs::remove_dir_all(&folder).expect("remove the project folder");
    }
}
