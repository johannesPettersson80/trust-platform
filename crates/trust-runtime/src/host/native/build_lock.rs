//! The project's build lock (specification 11 §6.8.1d): two builds of one project, from any client,
//! run one after the other, and a project check that writes the build's work folder waits for a
//! build there.

use std::fs::{self, File};
use std::io;
use std::path::Path;

/// The lock file, in the project's build folder.
pub const BUILD_LOCK_FILE: &str = "build/.lock";

/// The build lock of the project in `project`, taken, once no other build or check holds it. It
/// ends when the returned file is dropped.
///
/// # Errors
///
/// The error of a build folder or a lock file that cannot be created, or of the lock.
pub fn lock_project_build(project: &Path) -> io::Result<File> {
    let path = project.join(BUILD_LOCK_FILE);
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    file.lock()?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use std::fs::{OpenOptions, TryLockError};

    use super::{lock_project_build, BUILD_LOCK_FILE};

    #[test]
    fn a_second_build_waits_while_the_first_holds_the_projects_lock() {
        // Specification 11 §6.8.1d: two builds of one project run one after the other. While one
        // holds the project's build lock, another cannot take it, and can once the first ends.
        // Catches a build that writes the work folder and the image while another build of the
        // project writes them.
        let project = trust_test_support::unique_temp_path("build-lock");
        std::fs::create_dir_all(&project).expect("create the project");
        let held = lock_project_build(&project).expect("the first build takes the lock");
        let second = OpenOptions::new()
            .write(true)
            .open(project.join(BUILD_LOCK_FILE))
            .expect("open the lock file");
        assert!(matches!(second.try_lock(), Err(TryLockError::WouldBlock)));
        drop(held);
        assert!(
            second.try_lock().is_ok(),
            "the lock is free once the first build ends"
        );
        drop(second);
        let _ = std::fs::remove_dir_all(project);
    }
}
