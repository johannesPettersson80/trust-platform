//! Unit tests of the platform default endpoint, the per-user folders and the generated control
//! token (spec 11, section 6.8.0a).

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
use super::defaults::{
    client_unix_default_in, ensure_user_private_directory, fallback_socket, fits_socket_path,
    project_socket, runtime_directory_in, unix_default_endpoint_in,
};
use super::defaults::{
    generate_control_token, per_user_token_directory_in, project_path_hash, token_from_rng,
};
#[cfg(unix)]
use super::test_support::Scratch;
#[cfg(unix)]
use crate::control::ControlEndpoint;

/// A random source that fills every byte with one value, so that the encoding is known.
struct FillRng(u8);

impl rand::TryRng for FillRng {
    type Error = std::io::Error;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(u32::from(self.0))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(u64::from(self.0))
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        destination.fill(self.0);
        Ok(())
    }
}

/// A random source with no entropy: every request fails.
struct FailingRng;

impl rand::TryRng for FailingRng {
    type Error = std::io::Error;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Err(std::io::Error::other("entropy unavailable"))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Err(std::io::Error::other("entropy unavailable"))
    }

    fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
        Err(std::io::Error::other("entropy unavailable"))
    }
}

// The project-path hash (spec 11, section 6.8.0a).

#[test]
fn the_project_hash_is_the_first_16_hex_digits_of_the_sha256_of_the_path() {
    // sha256("/srv/plc/line-a") = 5b4fcd7f11291ecc...
    assert_eq!(
        project_path_hash(Path::new("/srv/plc/line-a")),
        "5b4fcd7f11291ecc"
    );
}

#[test]
fn the_per_user_token_folder_is_trust_runtimes_in_the_local_application_data_folder() {
    let local = Path::new("local-app-data");

    assert_eq!(
        per_user_token_directory_in(local),
        local.join("trust").join("runtimes")
    );
}

// The Unix default socket path (spec 11, section 6.8.0a).

/// The default with the project-local socket usable and the given runtime directory.
#[cfg(unix)]
fn unix_default(project: &Path, runtime_dir: &Path) -> ControlEndpoint {
    let runtime_dir = runtime_dir.to_path_buf();
    unix_default_endpoint_in(project, true, move || Ok(runtime_dir)).expect("default endpoint")
}

/// The directory and the file name of a Unix socket endpoint.
#[cfg(unix)]
fn socket_parts(endpoint: ControlEndpoint) -> (PathBuf, String) {
    match endpoint {
        ControlEndpoint::Unix(path) => {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("socket file name")
                .to_string();
            let parent = path.parent().expect("socket directory").to_path_buf();
            (parent, name)
        }
        other => panic!("expected a Unix socket, got {other}"),
    }
}

#[cfg(unix)]
#[test]
fn a_short_project_path_uses_the_project_local_socket() {
    let endpoint = unix_default(Path::new("/tmp/trust-project"), Path::new("/run/user/1000"));

    match endpoint {
        ControlEndpoint::Unix(path) => assert_eq!(
            path,
            PathBuf::from("/tmp/trust-project/.trust-runtime/control.sock")
        ),
        other => panic!("expected a Unix socket, got {other}"),
    }
}

#[cfg(unix)]
#[test]
fn a_project_path_too_long_for_a_socket_falls_back_to_its_hash_in_the_runtime_directory() {
    let runtime_dir = Path::new("/run/user/1000");
    let long_project = PathBuf::from(format!("/{}", "p".repeat(120)));

    let (parent, name) = socket_parts(unix_default(&long_project, runtime_dir));

    assert_eq!(parent.as_path(), runtime_dir);
    // sha256("/" followed by 120 "p") = a67b38a5b5de7872...
    assert_eq!(name, "trust-a67b38a5b5de7872.sock");
}

#[cfg(unix)]
#[test]
fn the_fallback_name_is_stable_for_a_project_and_differs_between_projects() {
    let runtime_dir = Path::new("/run/user/1000");
    let project = PathBuf::from(format!("/{}", "p".repeat(120)));
    let other_project = PathBuf::from(format!("/{}", "q".repeat(120)));

    let (_, first) = socket_parts(unix_default(&project, runtime_dir));
    let (_, again) = socket_parts(unix_default(&project, runtime_dir));
    let (_, other) = socket_parts(unix_default(&other_project, runtime_dir));

    assert_eq!(first, again);
    assert_ne!(first, other);
}

#[cfg(target_os = "linux")]
#[test]
fn the_socket_path_limit_is_strict_on_linux() {
    // The socket path is the project path plus "/.trust-runtime/control.sock", which adds 28
    // bytes. A path of 107 bytes leaves room for its NUL terminator; 108 bytes does not fit.
    let runtime_dir = Path::new("/run/user/1000");
    let fits = PathBuf::from(format!("/{}", "a".repeat(78)));
    let too_long = PathBuf::from(format!("/{}", "a".repeat(79)));

    match unix_default(&fits, runtime_dir) {
        ControlEndpoint::Unix(path) => {
            assert_eq!(path, fits.join(".trust-runtime").join("control.sock"));
        }
        other => panic!("expected a Unix socket, got {other}"),
    }
    let (parent, _) = socket_parts(unix_default(&too_long, runtime_dir));
    assert_eq!(parent.as_path(), runtime_dir);
}

#[cfg(unix)]
#[test]
fn a_project_whose_endpoint_folder_cannot_be_created_binds_the_fallback_socket() {
    let runtime_dir = Path::new("/run/user/1000");
    let project = Path::new("/tmp/trust-project");

    let endpoint = unix_default_endpoint_in(project, false, || Ok(runtime_dir.to_path_buf()))
        .expect("fallback endpoint");

    let (parent, name) = socket_parts(endpoint);
    assert_eq!(parent.as_path(), runtime_dir);
    assert_eq!(name, format!("trust-{}.sock", project_path_hash(project)));
}

#[cfg(unix)]
#[test]
fn a_project_local_socket_never_asks_for_the_runtime_directory() {
    let endpoint = unix_default_endpoint_in(Path::new("/tmp/trust-project"), true, || {
        panic!("the runtime directory is needed only for the fallback")
    })
    .expect("project-local endpoint");

    assert!(matches!(endpoint, ControlEndpoint::Unix(_)));
}

#[cfg(unix)]
#[test]
fn a_fallback_socket_path_that_does_not_fit_is_an_error() {
    let long_runtime_dir = PathBuf::from(format!("/{}", "r".repeat(120)));

    let error = unix_default_endpoint_in(Path::new("/tmp/trust-project"), false, || {
        Ok(long_runtime_dir.clone())
    })
    .expect_err("the fallback socket does not fit");

    assert!(error.to_string().contains("does not fit"), "{error}");
    assert!(
        error.to_string().contains("[runtime.control] endpoint"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn a_client_takes_the_fallback_socket_only_when_the_fallback_socket_alone_exists() {
    // Real sockets are bound, so both folders get short paths under /tmp.
    let project = Scratch::under(Path::new("/tmp"), "tep");
    let runtime_dir = Scratch::under(Path::new("/tmp"), "ter");
    let canonical = std::fs::canonicalize(project.path()).expect("canonical project");
    let runtime_path = std::fs::canonicalize(runtime_dir.path()).expect("canonical directory");
    let local = project_socket(&canonical);
    let fallback = fallback_socket(&canonical, &runtime_path);
    if !fits_socket_path(&local) || !fits_socket_path(&fallback) {
        eprintln!("skipped: /tmp is too long a folder for these sockets");
        return;
    }
    let resolve =
        || client_unix_default_in(&canonical, || Ok(runtime_path.clone())).expect("client default");

    // No socket at all: the project-local socket, where a running runtime would be.
    assert!(matches!(resolve(), ControlEndpoint::Unix(path) if path == local));

    // Only the fallback socket: a runtime that could not create the endpoint folder.
    let fallback_listener =
        std::os::unix::net::UnixListener::bind(&fallback).expect("bind fallback socket");
    assert!(matches!(resolve(), ControlEndpoint::Unix(path) if path == fallback));

    // Both: the project-local socket wins.
    std::fs::create_dir_all(local.parent().expect("endpoint folder")).expect("endpoint folder");
    let local_listener =
        std::os::unix::net::UnixListener::bind(&local).expect("bind project socket");
    assert!(matches!(resolve(), ControlEndpoint::Unix(path) if path == local));

    drop(local_listener);
    drop(fallback_listener);
}

// The per-user runtime directory (spec 11, section 6.8.0a).

// Catches a runtime directory other than a private `$XDG_RUNTIME_DIR`, or other than
// `trust-runtime-<uid>` in the temporary folder when that variable is unset or empty.
#[cfg(unix)]
#[test]
fn the_runtime_directory_is_a_private_xdg_runtime_dir_else_a_per_user_folder_in_the_temporary_folder(
) {
    let scratch = Scratch::new("runtime-dir-choice");
    let uid = rustix::process::geteuid().as_raw();
    let xdg = scratch.path().join("xdg");
    std::fs::create_dir(&xdg).expect("create directory");
    super::test_support::set_mode(&xdg, 0o700);
    let per_user = scratch.path().join(format!("trust-runtime-{uid}"));

    assert_eq!(
        runtime_directory_in(Some(xdg.clone().into_os_string()), scratch.path(), uid)
            .expect("a private XDG_RUNTIME_DIR"),
        xdg
    );
    assert_eq!(
        runtime_directory_in(Some("".into()), scratch.path(), uid).expect("an empty variable"),
        per_user
    );
    assert_eq!(
        runtime_directory_in(None, scratch.path(), uid).expect("no variable"),
        per_user
    );
}

// F12 (29 October 2026): on the Raspberry Pi, `/run/user/1000` has mode 0770, and a project too
// long for its own socket could not start. Catches both a refusal and a socket in a folder that
// the user's group can enter.
#[cfg(unix)]
#[test]
fn an_xdg_runtime_dir_open_to_the_group_is_passed_over_for_a_private_one_in_the_temporary_folder() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let scratch = Scratch::new("runtime-dir-group");
    let uid = rustix::process::geteuid().as_raw();
    let xdg = scratch.path().join("xdg");
    std::fs::create_dir(&xdg).expect("create directory");
    super::test_support::set_mode(&xdg, 0o770);

    let dir = runtime_directory_in(Some(xdg.into_os_string()), scratch.path(), uid)
        .expect("a private folder in the temporary folder");

    assert_eq!(dir, scratch.path().join(format!("trust-runtime-{uid}")));
    let metadata = std::fs::symlink_metadata(&dir).expect("runtime directory");
    assert!(metadata.is_dir());
    assert_eq!(metadata.uid(), uid);
    assert_eq!(metadata.permissions().mode() & 0o077, 0);
}

// Catches a fallback socket placed in a `trust-runtime-<uid>` folder that other users can enter:
// with no private folder left, the default cannot be used.
#[cfg(unix)]
#[test]
fn without_a_private_runtime_directory_the_default_cannot_be_used() {
    let scratch = Scratch::new("runtime-dir-none");
    let uid = rustix::process::geteuid().as_raw();
    let xdg = scratch.path().join("xdg");
    std::fs::create_dir(&xdg).expect("create directory");
    super::test_support::set_mode(&xdg, 0o770);
    let per_user = scratch.path().join(format!("trust-runtime-{uid}"));
    std::fs::create_dir(&per_user).expect("create directory");
    super::test_support::set_mode(&per_user, 0o755);

    let error = runtime_directory_in(Some(xdg.into_os_string()), scratch.path(), uid)
        .expect_err("no private runtime directory");

    assert!(error.to_string().contains("other users"), "{error}");
    assert!(
        error.to_string().contains(&format!("trust-runtime-{uid}")),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn a_missing_runtime_directory_is_created_for_this_user_only() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let parent = Scratch::new("runtime-dir-create");
    let dir = parent.path().join("trust-runtime-test");
    let uid = rustix::process::geteuid().as_raw();

    ensure_user_private_directory(&dir, uid).expect("create the runtime directory");

    let metadata = std::fs::symlink_metadata(&dir).expect("runtime directory");
    assert!(metadata.is_dir());
    assert_eq!(metadata.uid(), uid);
    assert_eq!(metadata.permissions().mode() & 0o077, 0);
    // A second call accepts the directory it created.
    ensure_user_private_directory(&dir, uid).expect("reuse the runtime directory");
}

#[cfg(unix)]
#[test]
fn a_runtime_directory_open_to_others_or_of_another_user_is_refused() {
    let parent = Scratch::new("runtime-dir-refuse");
    let uid = rustix::process::geteuid().as_raw();

    let open = parent.path().join("open");
    std::fs::create_dir(&open).expect("create directory");
    super::test_support::set_mode(&open, 0o755);
    let error = ensure_user_private_directory(&open, uid).expect_err("mode 0755 is refused");
    assert!(error.to_string().contains("other users"), "{error}");

    let mine = parent.path().join("mine");
    std::fs::create_dir(&mine).expect("create directory");
    super::test_support::set_mode(&mine, 0o700);
    let error = ensure_user_private_directory(&mine, uid.wrapping_add(1))
        .expect_err("a directory of another user is refused");
    assert!(error.to_string().contains("belongs to user"), "{error}");

    let file = parent.path().join("file");
    std::fs::write(&file, "x").expect("write file");
    let error = ensure_user_private_directory(&file, uid).expect_err("a file is refused");
    assert!(error.to_string().contains("not a directory"), "{error}");

    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&mine, &link).expect("create symbolic link");
    let error = ensure_user_private_directory(&link, uid).expect_err("a symbolic link is refused");
    assert!(error.to_string().contains("not a directory"), "{error}");
}

// The generated control token (spec 11, section 6.8.0a).

#[test]
fn a_generated_token_is_43_unpadded_base64url_characters() {
    let token = generate_control_token().expect("token from the operating system");

    assert_eq!(token.len(), 43, "{token}");
    assert!(
        token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "{token}"
    );
}

#[test]
fn two_generated_tokens_differ() {
    let first = generate_control_token().expect("first token");
    let second = generate_control_token().expect("second token");

    assert_ne!(first, second);
}

#[test]
fn the_token_is_the_unpadded_base64url_of_the_random_bytes() {
    // 32 bytes of 0xFF encode as ten groups of "____" and a last partial group, "__8".
    let token = token_from_rng(&mut FillRng(0xFF)).expect("token from a working source");

    assert_eq!(token, format!("{}8", "_".repeat(42)));
}

#[test]
fn a_failing_random_source_is_an_error_and_never_a_fallback_token() {
    let error =
        token_from_rng(&mut FailingRng).expect_err("a failing source must not produce a token");

    assert!(
        error.to_string().contains("secure random source"),
        "{error}"
    );
}
