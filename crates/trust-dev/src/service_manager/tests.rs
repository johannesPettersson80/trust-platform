//! `trust-dev service install` and `uninstall` (specification 48 RU-64): what `install` refuses
//! before it changes anything, the steps of each service manager and the command it names without
//! root's rights. Every expectation is worked out from RU-64. The binary in the target folder
//! cargo's metadata names is proven by `tests/service_install.rs`, which runs trust-dev without the
//! target folder the tests' own build may set.

use std::path::{Path, PathBuf};

use super::definition::{launchd_plist, systemd_unit};
use super::plan::{resolve, Owner, Plan};
use super::steps::{install_steps, installed_for_another, uninstall_steps, Step, System, Unix};
use super::{install_command, uninstall_command};

/// Appendix B's service, with its package.
const RECIPE: &str = "[services.recipe]\npackage = \"recipe-service\"\nheartbeat = \"500ms\"\n\
                      endpoint = \"tcp://127.0.0.1:47800\"\ncredentials = \"secrets/recipe.token\"\n";

/// A project folder with `trust_toml` and the service token of `[services.recipe]`.
fn project(trust_toml: &str) -> PathBuf {
    let folder = trust_test_support::unique_temp_path("trust-dev-service");
    std::fs::create_dir_all(folder.join("secrets")).expect("the project folder");
    std::fs::write(folder.join("trust.toml"), trust_toml).expect("trust.toml");
    std::fs::write(folder.join("secrets").join("recipe.token"), "token\n").expect("the token");
    folder
}

/// A Cargo package `name` with one binary in the folder `folder` of `project`: a workspace of its
/// own when `own_workspace`, as `cargo new --bin` makes it, else a member of the project's.
fn package(project: &Path, folder: &str, name: &str, own_workspace: bool) -> PathBuf {
    let root = project.join(folder);
    std::fs::create_dir_all(root.join("src")).expect("the package");
    let workspace = if own_workspace { "\n[workspace]\n" } else { "" };
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{workspace}"
        ),
    )
    .expect("the manifest");
    std::fs::write(root.join("src").join("main.rs"), "fn main() {}\n").expect("the source");
    root
}

/// The release binary `name` in the target folder `target`, as cargo's build writes it.
fn built(target: &Path, name: &str) -> PathBuf {
    let release = target.join("release");
    std::fs::create_dir_all(&release).expect("the release folder");
    let binary = release.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&binary, "binary").expect("the binary");
    std::fs::canonicalize(&binary).expect("the binary's path")
}

/// The refusal of the service recipe of `project`.
fn refusal(project: &Path, binary: Option<&Path>) -> String {
    format!(
        "{:#}",
        resolve(project, "recipe", binary).expect_err("a refusal")
    )
}

/// The plan of the service recipe of `/home/ada/plant`.
fn plant() -> Plan {
    Plan {
        name: "recipe".into(),
        project: PathBuf::from("/home/ada/plant"),
        binary: PathBuf::from("/home/ada/plant/target/release/recipe-service"),
    }
}

/// The user ada, who owns `/home/ada/plant`.
fn ada() -> Owner {
    Owner {
        uid: 1000,
        name: "ada".into(),
    }
}

#[test]
fn a_service_is_refused_until_its_settings_let_it_connect() {
    // RU-64: `install` refuses a service trust.toml does not declare, one without its package or
    // endpoint, and one whose token file does not exist, naming the command that writes it (RU-81);
    // and a name that is no identifier (RU-63). Catches a service installed only to fail at every
    // start, restarted every 2 s for ever.
    let undeclared = project("");
    assert!(refusal(&undeclared, None).contains("trust.toml declares no [services.recipe]"));
    let no_package = project(&RECIPE.replace("package = \"recipe-service\"\n", ""));
    assert!(refusal(&no_package, None).contains("[services.recipe] has no package"));
    let no_endpoint = project(&RECIPE.replace("endpoint = \"tcp://127.0.0.1:47800\"\n", ""));
    assert!(refusal(&no_endpoint, None).contains("[services.recipe] has no endpoint"));
    let no_token = project(RECIPE);
    std::fs::remove_file(no_token.join("secrets").join("recipe.token")).expect("no token");
    let message = refusal(&no_token, None);
    assert!(
        message.contains("trust-runtime ctl service-token recipe --out secrets/recipe.token"),
        "{message}"
    );
    let named = project(RECIPE);
    let message = format!(
        "{:#}",
        resolve(&named, "re-cipe", None).expect_err("a refusal")
    );
    assert!(
        message.contains("`re-cipe` is no service name"),
        "{message}"
    );
}

#[test]
fn a_package_the_project_does_not_hold_exactly_once_with_one_binary_is_refused() {
    // RU-64: `install` refuses a package the project folder does not hold, which it does not look
    // for in `target` or hidden folders; a package it holds twice; and a package without exactly
    // one binary. Catches an install that runs some other program of the same name.
    let missing = project(RECIPE);
    package(&missing, "target/recipe-service", "recipe-service", true);
    package(&missing, ".cache/recipe-service", "recipe-service", true);
    let message = refusal(&missing, None);
    assert!(
        message.contains("holds no Cargo package recipe-service")
            && message.contains("`cargo new --bin recipe-service`"),
        "{message}"
    );
    let twice = project(RECIPE);
    package(&twice, "a", "recipe-service", true);
    package(&twice, "b", "recipe-service", true);
    let message = refusal(&twice, None);
    assert!(
        message.contains("holds the Cargo package recipe-service 2 times"),
        "{message}"
    );
    let two_binaries = project(RECIPE);
    let root = package(&two_binaries, "recipe-service", "recipe-service", true);
    std::fs::create_dir_all(root.join("src").join("bin")).expect("a second binary's folder");
    std::fs::write(
        root.join("src").join("bin").join("other.rs"),
        "fn main() {}\n",
    )
    .expect("a second binary");
    let message = refusal(&two_binaries, None);
    assert!(
        message.contains("has 2 binaries")
            && message.contains("other")
            && message.contains("a service's package has one"),
        "{message}"
    );
}

#[test]
fn a_binary_the_command_names_is_taken_as_it_is() {
    // RU-64: the command named for root carries the binary, so that under sudo `install` needs no
    // cargo; it still refuses a binary that does not exist. Catches a root install that runs cargo.
    let project = project(RECIPE);
    let binary = built(&project.join("bin"), "recipe");
    let plan = resolve(&project, "recipe", Some(binary.as_path())).expect("a plan");
    assert_eq!(
        std::fs::canonicalize(&plan.binary).expect("the plan's binary"),
        binary
    );
    let missing = project.join("bin").join("missing");
    let message = refusal(&project, Some(missing.as_path()));
    assert!(message.contains("missing"), "{message}");
}

#[test]
fn each_service_manager_gets_the_steps_of_the_specification() {
    // RU-64: on Linux the unit, then daemon-reload, enable and restart, so that an installed
    // service is replaced and restarted; on macOS boot out an earlier daemon (which may not be
    // loaded), the plist, the log given to the owner, enable and bootstrap. Uninstall: disable
    // --now, remove, reload; boot out and remove. Catches a service that is installed but not
    // started, not started at boot, or left running after uninstall.
    let plan = plant();
    let owner = ada();
    let run = |program: &'static str, args: &[&str], may_fail: bool| Step::Run {
        program,
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        may_fail,
    };
    assert_eq!(
        install_steps(Unix::Systemd, &plan, &owner),
        vec![
            Step::Write {
                path: "/etc/systemd/system/trust-service-recipe.service".into(),
                text: systemd_unit(&plan, "ada"),
            },
            run("systemctl", &["daemon-reload"], false),
            run(
                "systemctl",
                &["enable", "trust-service-recipe.service"],
                false
            ),
            run(
                "systemctl",
                &["restart", "trust-service-recipe.service"],
                false
            ),
        ]
    );
    assert_eq!(
        install_steps(Unix::Launchd, &plan, &owner),
        vec![
            run(
                "launchctl",
                &["bootout", "system/trust-service-recipe"],
                true
            ),
            Step::Write {
                path: "/Library/LaunchDaemons/trust-service-recipe.plist".into(),
                text: launchd_plist(&plan, "ada"),
            },
            Step::Log {
                path: "/Library/Logs/trust-service-recipe.log".into(),
                uid: 1000,
            },
            run(
                "launchctl",
                &["enable", "system/trust-service-recipe"],
                false
            ),
            run(
                "launchctl",
                &[
                    "bootstrap",
                    "system",
                    "/Library/LaunchDaemons/trust-service-recipe.plist"
                ],
                false
            ),
        ]
    );
    assert_eq!(
        uninstall_steps(Unix::Systemd, "recipe"),
        vec![
            run(
                "systemctl",
                &["disable", "--now", "trust-service-recipe.service"],
                false
            ),
            Step::Remove {
                path: "/etc/systemd/system/trust-service-recipe.service".into(),
            },
            run("systemctl", &["daemon-reload"], false),
        ]
    );
    assert_eq!(
        uninstall_steps(Unix::Launchd, "recipe"),
        vec![
            run(
                "launchctl",
                &["bootout", "system/trust-service-recipe"],
                true
            ),
            Step::Remove {
                path: "/Library/LaunchDaemons/trust-service-recipe.plist".into(),
            },
        ]
    );
}

#[test]
fn a_service_installed_for_another_project_folder_is_not_replaced() {
    // RU-64: a computer has one trust-service-<name>; `install` replaces it only when it was
    // written for the same project folder. Catches an install that silently takes another
    // project's service, also one whose folder's name only starts alike.
    let plan = plant();
    for (system, existing) in [
        (System::Unix(Unix::Systemd), systemd_unit(&plan, "ada")),
        (System::Unix(Unix::Launchd), launchd_plist(&plan, "ada")),
    ] {
        assert!(!installed_for_another(
            system,
            &existing,
            Path::new("/home/ada/plant")
        ));
        assert!(installed_for_another(
            system,
            &existing,
            Path::new("/home/ada/plant2")
        ));
        assert!(installed_for_another(
            system,
            &existing,
            Path::new("/home/ada/plan")
        ));
    }
}

#[test]
fn without_rights_the_command_that_has_them_is_named() {
    // RU-64: `sudo` with trust-dev's own path, the project folder and the binary, so that the
    // command needs neither root's PATH nor cargo; a word with a space or a quote is quoted for the
    // shell. Catches a command that does not run as printed.
    let trust_dev = Path::new("/home/ada/.cargo/bin/trust-dev");
    assert_eq!(
        install_command(System::Unix(Unix::Systemd), trust_dev, &plant()),
        "sudo /home/ada/.cargo/bin/trust-dev service install recipe --project /home/ada/plant \
         --binary /home/ada/plant/target/release/recipe-service"
    );
    let spaced = Plan {
        name: "recipe".into(),
        project: PathBuf::from("/home/ada/my plant"),
        binary: PathBuf::from("/home/ada/my plant/target/release/recipe-service"),
    };
    assert_eq!(
        install_command(
            System::Unix(Unix::Launchd),
            Path::new("/opt/it's/trust-dev"),
            &spaced
        ),
        "sudo '/opt/it'\\''s/trust-dev' service install recipe --project '/home/ada/my plant' \
         --binary '/home/ada/my plant/target/release/recipe-service'"
    );
    assert_eq!(
        uninstall_command(System::Unix(Unix::Systemd), trust_dev, "recipe"),
        "sudo /home/ada/.cargo/bin/trust-dev service uninstall recipe"
    );
}

#[cfg(unix)]
#[test]
fn the_service_runs_as_the_project_folder_s_owner_and_never_as_root() {
    // RU-64: the service runs as the user who owns the project folder; a folder of root is refused.
    // Catches a service that runs as root, as `sudo` would make it.
    use std::os::unix::fs::MetadataExt;
    let message = format!(
        "{:#}",
        super::plan::owner(Path::new("/")).expect_err("root's folder")
    );
    assert!(message.contains("belongs to root"), "{message}");
    let project = project(RECIPE);
    let owner = super::plan::owner(&project).expect("the owner");
    assert_eq!(
        owner.uid,
        std::fs::metadata(&project).expect("the folder").uid()
    );
    let output = std::process::Command::new("id")
        .arg("-un")
        .output()
        .expect("id");
    assert_eq!(owner.name, String::from_utf8_lossy(&output.stdout).trim());
}

#[cfg(unix)]
#[test]
fn without_root_install_changes_nothing_and_names_the_command() {
    // RU-64: run as a user, `install` checks everything, changes nothing and names the command
    // with root's rights. A test run as root would install a service, so it does not run then.
    // Catches an install that asks for root before it checks, or that changes the computer
    // without it.
    if super::is_root().expect("id") {
        eprintln!("the tests run as root: install would change the computer, so it is not run");
        return;
    }
    let project = project(RECIPE);
    let binary = built(&project.join("bin"), "recipe");
    let message = format!(
        "{:#}",
        super::run_install(Some(project.clone()), "recipe", Some(binary.clone()))
            .expect_err("no rights")
    );
    assert!(
        message.contains("takes root; nothing was changed")
            && message.contains(" service install recipe --project ")
            && message.contains(&format!("--binary {}", binary.display())),
        "{message}"
    );
}
