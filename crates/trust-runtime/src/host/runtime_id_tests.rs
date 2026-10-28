//! Where the stable runtime ID is kept (specification 11, runtime discovery): in the project, or for
//! the user when the project folder cannot hold it, or nowhere, and the runtime still starts.

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch folder, removed when the test ends; write access is given back first, so a test may
/// make it read-only.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-id-{label}-{}-{}",
            std::process::id(),
            SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the scratch folder");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Makes `folder` read-only. Returns false when this process can still write into it, as a
/// privileged user can, so the test does not apply.
#[cfg(unix)]
fn make_read_only(folder: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(folder, fs::Permissions::from_mode(0o555)).expect("make read-only");
    fs::write(folder.join("probe"), "x").is_err()
}

/// The file kept for the user for `project`: `<state>/runtimes/<first 16 hex digits of the SHA-256
/// of the canonical project path>.id`.
fn expected_user_file(project: &Path, state: &Path) -> PathBuf {
    let canonical = fs::canonicalize(project).expect("canonical project path");
    state
        .join("runtimes")
        .join(format!("{}.id", project_path_hash(&canonical)))
}

/// Wrong behaviour caught: a writable project's ID kept somewhere else than in the project, or a
/// file written for the user although the project holds the ID.
#[test]
fn a_writable_project_keeps_its_id_in_the_project_as_before() {
    let project = Scratch::new("writable");
    let state = Scratch::new("writable-state");

    let first = runtime_identity_in(project.path(), Some(state.path())).expect("an ID");
    let second = runtime_identity_in(project.path(), Some(state.path())).expect("the ID again");

    assert_eq!(
        first.place,
        RuntimeIdPlace::Project(runtime_id_path(project.path()))
    );
    assert_eq!(
        fs::read_to_string(runtime_id_path(project.path())).expect("the project's file"),
        first.id.as_str()
    );
    assert_eq!(second, first, "the ID stays the same across starts");
    assert!(
        !state.path().join("runtimes").exists(),
        "nothing is kept for the user"
    );
}

/// Wrong behaviour caught: a runtime that refuses to start because its project folder cannot hold
/// the ID file, or one that has a new ID at every start.
#[cfg(unix)]
#[test]
fn a_read_only_project_keeps_the_same_id_for_the_user_on_two_starts() {
    let project = Scratch::new("read-only");
    let state = Scratch::new("read-only-state");
    if !make_read_only(project.path()) {
        eprintln!("skipped: this process writes into read-only folders");
        return;
    }

    let first =
        runtime_identity_in(project.path(), Some(state.path())).expect("the runtime starts");
    let second =
        runtime_identity_in(project.path(), Some(state.path())).expect("the runtime starts again");

    let user_file = expected_user_file(project.path(), state.path());
    assert_eq!(first.place, RuntimeIdPlace::User(user_file.clone()));
    assert_eq!(
        fs::read_to_string(&user_file).expect("the file kept for the user"),
        first.id.as_str()
    );
    assert!(is_valid_runtime_id(first.id.as_str()));
    assert_eq!(second, first, "the ID stays the same across starts");
    assert!(!project.path().join(".trust-runtime").exists());
}

/// Wrong behaviour caught: an ID kept for the user replaced by a new one once the project folder can
/// be written, so the runtime's identity changes.
#[test]
fn an_id_kept_for_the_user_is_used_while_the_project_holds_none() {
    let project = Scratch::new("kept");
    let state = Scratch::new("kept-state");
    let user_file = expected_user_file(project.path(), state.path());
    fs::create_dir_all(user_file.parent().expect("the folder")).expect("the user folder");
    fs::write(&user_file, "kept-id-0123").expect("the kept ID");

    let identity = runtime_identity_in(project.path(), Some(state.path())).expect("an ID");

    assert_eq!(identity.id, "kept-id-0123");
    assert_eq!(identity.place, RuntimeIdPlace::User(user_file));
    assert!(!runtime_id_path(project.path()).exists());
}

/// Wrong behaviour caught: a file kept for the user that holds no valid ID silently replaced.
#[test]
fn an_invalid_id_kept_for_the_user_is_an_error() {
    let project = Scratch::new("invalid-kept");
    let state = Scratch::new("invalid-kept-state");
    let user_file = expected_user_file(project.path(), state.path());
    fs::create_dir_all(user_file.parent().expect("the folder")).expect("the user folder");
    fs::write(&user_file, "not a valid id!").expect("the invalid ID");

    let error = runtime_identity_in(project.path(), Some(state.path()))
        .expect_err("an invalid ID must fail");

    assert!(
        error
            .to_string()
            .contains("does not hold a valid runtime ID"),
        "{error}"
    );
}

/// Wrong behaviour caught: a runtime that refuses to start when neither its project nor the user's
/// state folder can hold the ID (specification 34, ARCH-07), or one that keeps quiet about it.
#[cfg(unix)]
#[test]
fn without_a_place_for_the_id_the_runtime_starts_with_one_for_its_process() {
    let project = Scratch::new("nowhere");
    let state = Scratch::new("nowhere-state");
    if !make_read_only(project.path()) || !make_read_only(state.path()) {
        eprintln!("skipped: this process writes into read-only folders");
        return;
    }

    let identity =
        runtime_identity_in(project.path(), Some(state.path())).expect("the runtime starts");

    assert!(is_valid_runtime_id(identity.id.as_str()));
    let RuntimeIdPlace::NotKept(reason) = &identity.place else {
        panic!("the ID is kept nowhere: {:?}", identity.place);
    };
    assert!(reason.contains(".trust-runtime"), "{reason}");
    assert!(reason.contains("runtimes"), "{reason}");

    let unknown = runtime_identity_in(project.path(), None).expect("the runtime starts");
    let RuntimeIdPlace::NotKept(reason) = &unknown.place else {
        panic!("the ID is kept nowhere: {:?}", unknown.place);
    };
    assert!(
        reason.contains("the user's state folder is not known"),
        "{reason}"
    );
}

/// Wrong behaviour caught: the user's folder taken from the wrong variable on a platform, or a
/// relative `XDG_STATE_HOME` used. Unix only: the absolute paths below are Unix paths.
#[cfg(unix)]
#[test]
fn the_user_state_folder_follows_the_platform_convention() {
    let variables = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    };
    let unix = variables(&[("XDG_STATE_HOME", "/state"), ("HOME", "/home/u")]);
    assert_eq!(
        user_state_folder("linux", unix),
        Some(Path::new("/state").join("trust"))
    );
    let relative = variables(&[("XDG_STATE_HOME", "state"), ("HOME", "/home/u")]);
    assert_eq!(
        user_state_folder("linux", relative),
        Some(
            Path::new("/home/u")
                .join(".local")
                .join("state")
                .join("trust")
        )
    );
    let mac = variables(&[("HOME", "/Users/u"), ("XDG_STATE_HOME", "/state")]);
    assert_eq!(
        user_state_folder("macos", mac),
        Some(
            Path::new("/Users/u")
                .join("Library")
                .join("Application Support")
                .join("trust")
        )
    );
    let windows = variables(&[("LOCALAPPDATA", "C:/Users/u/AppData/Local")]);
    assert_eq!(
        user_state_folder("windows", windows),
        Some(Path::new("C:/Users/u/AppData/Local").join("trust"))
    );
    assert_eq!(user_state_folder("linux", variables(&[])), None);
}
