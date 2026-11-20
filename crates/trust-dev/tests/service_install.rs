//! `trust-dev service install` as a user runs it (specification 48 RU-64): the binary it takes is
//! the release build of the service's package in the target folder cargo's metadata names, and
//! before that build it refuses, naming the build. The command runs without the target folder the
//! tests' own build may set (`CARGO_TARGET_DIR`), as the user's build of the service runs without
//! it. Run as a user, `install` changes nothing and names the command for root, which carries the
//! binary's path.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Appendix B's service, with its package.
const RECIPE: &str = "[services.recipe]\npackage = \"recipe-service\"\nheartbeat = \"500ms\"\n\
                      endpoint = \"tcp://127.0.0.1:47800\"\ncredentials = \"secrets/recipe.token\"\n";

/// trust-dev, with the environment of a user's shell: no target folder of the tests' build.
fn trust_dev() -> Command {
    let mut command = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-dev")
            .expect("Cargo must provide trust-dev binary while executing integration tests"),
    );
    command
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR");
    command
}

/// A project folder with Appendix B's service and its token, as the file system names it.
fn project() -> PathBuf {
    let folder = trust_test_support::unique_temp_path("trust-dev-service-install");
    std::fs::create_dir_all(folder.join("secrets")).expect("the project folder");
    std::fs::write(folder.join("trust.toml"), RECIPE).expect("trust.toml");
    std::fs::write(folder.join("secrets").join("recipe.token"), "token\n").expect("the token");
    canonical(&folder)
}

/// The Cargo package recipe-service with one binary in the folder `folder` of `project`: a
/// workspace of its own when `own_workspace`, as `cargo new --bin` makes it, else a member of the
/// project's.
fn package(project: &Path, folder: &str, own_workspace: bool) {
    let root = project.join(folder);
    std::fs::create_dir_all(root.join("src")).expect("the package");
    let workspace = if own_workspace { "\n[workspace]\n" } else { "" };
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"recipe-service\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{workspace}"
        ),
    )
    .expect("the manifest");
    std::fs::write(root.join("src").join("main.rs"), "fn main() {}\n").expect("the source");
}

/// `path` as the file system names it, without the device prefix of Windows' canonical form,
/// which the command's messages do not have.
fn canonical(path: &Path) -> PathBuf {
    let canonical = std::fs::canonicalize(path).expect("the path");
    let text = canonical.display().to_string();
    text.strip_prefix(r"\\?\").map_or(canonical, PathBuf::from)
}

/// The release binary recipe-service in the target folder `target`, as cargo's build writes it.
#[cfg(unix)]
fn build(target: &Path) -> PathBuf {
    let release = target.join("release");
    std::fs::create_dir_all(&release).expect("the release folder");
    let binary = release.join(format!("recipe-service{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&binary, "binary").expect("the binary");
    binary
}

/// What `trust-dev service install recipe` says for `project`: its error output.
fn install(project: &Path) -> String {
    let output = trust_dev()
        .args(["service", "install", "recipe", "--project"])
        .arg(project)
        .output()
        .expect("run trust-dev");
    assert!(
        !output.status.success(),
        "trust-dev service install ran as a user and succeeded:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Whether the tests run as root, when `install` would change the computer.
#[cfg(unix)]
fn root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "0")
}

#[test]
fn before_its_build_a_service_is_refused_naming_the_build() {
    // RU-64: `install` builds nothing; it names the release binary it needs, in the package's own
    // target folder for a package `cargo new --bin` made, and in the workspace's for a member of
    // the project's workspace, with the build that writes it. Catches an install that builds (as
    // root, under sudo), one that takes a debug build, and one that guesses the target folder from
    // the package's folder.
    let own = project();
    package(&own, "recipe-service", true);
    let message = install(&own);
    let expected = own
        .join("recipe-service")
        .join("target")
        .join("release")
        .join(format!("recipe-service{}", std::env::consts::EXE_SUFFIX));
    let manifest = Path::new("recipe-service").join("Cargo.toml");
    assert!(
        message.contains(&format!("{} does not exist", expected.display()))
            && message.contains(&format!(
                "build the service first, `cargo build --release --manifest-path {}`",
                manifest.display()
            )),
        "{message}"
    );
    let member = project();
    std::fs::write(
        member.join("Cargo.toml"),
        "[workspace]\nmembers = [\"services/recipe\"]\nresolver = \"2\"\n",
    )
    .expect("the workspace");
    package(&member, "services/recipe", false);
    let message = install(&member);
    let expected = member
        .join("target")
        .join("release")
        .join(format!("recipe-service{}", std::env::consts::EXE_SUFFIX));
    assert!(
        message.contains(&format!("{} does not exist", expected.display())),
        "{message}"
    );
}

#[cfg(unix)]
#[test]
fn a_built_service_is_named_for_root_with_its_release_binary() {
    // RU-64: with the binary built, a user's `install` changes nothing and names the command for
    // root with the binary's path, for a package of its own and for a member of the project's
    // workspace. Catches an install that asks for root before it finds the binary, and a command
    // for root that would look for the binary again without the user's cargo.
    if root() {
        eprintln!("the tests run as root: install would change the computer, so it is not run");
        return;
    }
    let own = project();
    package(&own, "recipe-service", true);
    let binary = build(&own.join("recipe-service").join("target"));
    let message = install(&own);
    assert!(
        message.contains("takes root; nothing was changed")
            && message.contains(&format!(
                " service install recipe --project {} --binary {}",
                own.display(),
                binary.display()
            )),
        "{message}"
    );
    let member = project();
    std::fs::write(
        member.join("Cargo.toml"),
        "[workspace]\nmembers = [\"services/recipe\"]\nresolver = \"2\"\n",
    )
    .expect("the workspace");
    package(&member, "services/recipe", false);
    let binary = build(&member.join("target"));
    let message = install(&member);
    assert!(
        message.contains(&format!("--binary {}", binary.display())),
        "{message}"
    );
}
