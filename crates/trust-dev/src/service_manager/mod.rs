//! `trust-dev service install`, `uninstall` and `run`: a service of the project under the
//! computer's service manager (specification 48 RU-64). `install` reads the service's settings and
//! finds its release binary as the user who builds it, works out every step, and runs them only
//! with root's or an administrator's rights; without them it changes nothing and names the command
//! that has them, with the binary's path, so that the command needs neither cargo nor the user's
//! `PATH`. `run` is the program of a Windows service.

mod definition;
mod plan;
mod steps;
#[cfg(windows)]
mod windows;
mod windows_steps;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};
use trust_runtime::bundle::detect_bundle_path;

use self::definition::service_id;
use self::plan::Plan;
use self::steps::{Step, System};

/// `trust-dev service install <name>`: puts the service `name` of the project under the computer's
/// service manager and starts it (specification 48 RU-64).
pub fn run_install(
    project: Option<PathBuf>,
    name: &str,
    binary: Option<PathBuf>,
) -> anyhow::Result<()> {
    let project = project_folder(project)?;
    let plan = plan::resolve(&project, name, binary.as_deref())?;
    let system = current_system()?;
    let owner = match system {
        System::Unix(_) => Some(plan::owner(&plan.project)?),
        System::Windows => None,
    };
    let id = service_id(name);
    let definition = system.definition_path(name);
    let existing = std::fs::read_to_string(&definition).ok();
    if let Some(existing) = &existing
        && steps::installed_for_another(system, existing, &plan.project)
    {
        bail!(
            "{id} is installed for another project folder ({}): a computer has one service of a \
             name, so uninstall it first with `trust-dev service uninstall {name}`",
            definition.display()
        );
    }
    if !has_rights(system)? {
        bail!(
            "{}",
            rights_refusal(
                system,
                &format!("installing {id}"),
                &install_command(system, &current_exe()?, &plan)
            )
        );
    }
    let steps = match (system, &owner) {
        (System::Unix(unix), Some(owner)) => steps::install_steps(unix, &plan, owner),
        _ => windows_steps::install_steps(
            &plan,
            &current_exe()?,
            &windows_steps::services_folder(),
            existing.is_some(),
        ),
    };
    execute(&steps)?;
    match &owner {
        Some(owner) => outln!(
            "Installed and started {id}: {} runs as {} in {}.",
            plan.binary.display(),
            owner.name,
            plan.project.display()
        ),
        None => outln!(
            "Installed and started {id}: {} runs as {} in {}.",
            plan.binary.display(),
            windows_steps::account(name),
            plan.project.display()
        ),
    }
    outln!("Its output: {}", system.output(name));
    Ok(())
}

/// `trust-dev service uninstall <name>`: stops the service `name` and removes it from the
/// computer's service manager (specification 48 RU-64).
pub fn run_uninstall(name: &str) -> anyhow::Result<()> {
    plan::check_name(name)?;
    let system = current_system()?;
    let id = service_id(name);
    let definition = system.definition_path(name);
    let Ok(existing) = std::fs::read_to_string(&definition) else {
        bail!(
            "{id} is not installed: there is no {}",
            definition.display()
        );
    };
    if !has_rights(system)? {
        bail!(
            "{}",
            rights_refusal(
                system,
                &format!("removing {id}"),
                &uninstall_command(system, &current_exe()?, name)
            )
        );
    }
    let steps = match system {
        System::Unix(unix) => steps::uninstall_steps(unix, name),
        System::Windows => {
            let record = windows_steps::read_record(&existing).with_context(|| {
                format!(
                    "{} holds no project folder and binary",
                    definition.display()
                )
            })?;
            windows_steps::uninstall_steps(name, &windows_steps::services_folder(), &record)
        }
    };
    execute(&steps)?;
    outln!("Stopped and removed {id}.");
    Ok(())
}

/// `trust-dev service run <name> --project <folder> --binary <binary>`: the program of the Windows
/// service `trust-service-<name>`, which the service manager runs (specification 48 RU-64).
pub fn run_host(name: &str, project: &Path, binary: &Path) -> anyhow::Result<()> {
    plan::check_name(name)?;
    #[cfg(windows)]
    {
        windows::run_host(name, project, binary)
    }
    #[cfg(not(windows))]
    {
        bail!(
            "`trust-dev service run {name} --project {} --binary {}` is the program of a Windows \
             service; on Linux and macOS the service manager runs the service's binary itself",
            project.display(),
            binary.display()
        )
    }
}

/// The refusal of `action` without the rights it takes, naming `command`, which has them.
fn rights_refusal(system: System, action: &str, command: &str) -> String {
    match system {
        System::Unix(_) => format!(
            "{action} changes the computer's services, which takes root; nothing was changed. \
             Run:\n  {command}"
        ),
        System::Windows => format!(
            "{action} changes the computer's services, which takes an administrator; nothing was \
             changed. In PowerShell opened as administrator, run:\n  {command}"
        ),
    }
}

/// The command that installs the service of `plan` with the rights it takes: trust-dev's own path,
/// the project folder and the binary, each quoted for the shell where it needs it; on Linux and
/// macOS after `sudo`.
pub(crate) fn install_command(system: System, trust_dev: &Path, plan: &Plan) -> String {
    let arguments = |word: fn(&Path) -> String| {
        format!(
            "service install {} --project {} --binary {}",
            plan.name,
            word(&plan.project),
            word(&plan.binary)
        )
    };
    match system {
        System::Unix(_) => format!("sudo {} {}", shell_word(trust_dev), arguments(shell_word)),
        System::Windows => format!(
            "{} {}",
            powershell_call(trust_dev),
            arguments(powershell_word)
        ),
    }
}

/// The command that uninstalls the service `name` with the rights it takes.
pub(crate) fn uninstall_command(system: System, trust_dev: &Path, name: &str) -> String {
    match system {
        System::Unix(_) => format!("sudo {} service uninstall {name}", shell_word(trust_dev)),
        System::Windows => format!("{} service uninstall {name}", powershell_call(trust_dev)),
    }
}

/// `path` as one word of a POSIX shell: as it is when it holds only characters the shell takes
/// literally, else in single quotes.
fn shell_word(path: &Path) -> String {
    let text = path.to_string_lossy();
    let literal = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:=@%+,-".contains(c));
    if literal {
        text.into_owned()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// `path` as one word of PowerShell: as it is when it holds only characters PowerShell takes
/// literally, else in single quotes, which take every character but a quote literally.
fn powershell_word(path: &Path) -> String {
    let text = path.to_string_lossy();
    let literal = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.:\\/+-".contains(c));
    if literal {
        text.into_owned()
    } else {
        format!("'{}'", text.replace('\'', "''"))
    }
}

/// The program `path` called in PowerShell: a quoted path needs the call operator `&`.
fn powershell_call(path: &Path) -> String {
    let word = powershell_word(path);
    if word.starts_with('\'') {
        format!("& {word}")
    } else {
        word
    }
}

/// The project folder: the one the command names, else the one detected, else the current folder.
fn project_folder(project: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    match project {
        Some(path) => Ok(path),
        None => match detect_bundle_path(None) {
            Ok(path) => Ok(path),
            Err(_) => std::env::current_dir().context("failed to resolve current directory"),
        },
    }
}

/// This computer's service manager, which `install` writes for.
fn current_system() -> anyhow::Result<System> {
    System::current().context(
        "trust-dev service install puts a service under systemd on Linux, launchd on macOS and \
         Windows' service manager, and this computer has none of them (specification 48 RU-64)",
    )
}

/// trust-dev's own path, which the command named for the rights runs.
fn current_exe() -> anyhow::Result<PathBuf> {
    std::env::current_exe().context("find trust-dev's own path")
}

/// Whether this process has the rights to change `system`'s services.
fn has_rights(system: System) -> anyhow::Result<bool> {
    match system {
        System::Unix(_) => is_root(),
        System::Windows => Ok(may_create_services()),
    }
}

/// Whether this process runs as root: `id -u` prints 0.
#[cfg(unix)]
fn is_root() -> anyhow::Result<bool> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("run id, which tells whether trust-dev runs as root")?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "0")
}

/// Windows has no root.
#[cfg(not(unix))]
fn is_root() -> anyhow::Result<bool> {
    Ok(false)
}

/// Whether this process may create a Windows service.
#[cfg(windows)]
fn may_create_services() -> bool {
    windows::may_create_services()
}

/// Only Windows has Windows' service manager.
#[cfg(not(windows))]
fn may_create_services() -> bool {
    false
}

/// Runs `steps` in order; the first that fails ends the command, naming it.
fn execute(steps: &[Step]) -> anyhow::Result<()> {
    for step in steps {
        match step {
            Step::Write { path, text } => {
                std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
                readable_by_everyone(path)?;
            }
            Step::Log { path, uid } => {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .with_context(|| format!("create {}", path.display()))?;
                give_to(path, *uid)?;
            }
            Step::Run {
                program,
                args,
                may_fail,
            } => {
                let line = format!("{program} {}", args.join(" "));
                let mut command = Command::new(program);
                command.args(args);
                let succeeded = if *may_fail {
                    // Its refusal, a daemon that is not loaded, is no news to the user.
                    command.output().is_ok_and(|output| output.status.success())
                } else {
                    command
                        .status()
                        .with_context(|| format!("run `{line}`"))?
                        .success()
                };
                if !succeeded && !may_fail {
                    bail!("`{line}` failed");
                }
            }
            Step::Remove { path } => {
                std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
            }
            Step::Folder { path } => {
                std::fs::create_dir_all(path)
                    .with_context(|| format!("create {}", path.display()))?;
            }
            Step::Copy { from, to } => {
                std::fs::copy(from, to)
                    .with_context(|| format!("copy {} to {}", from.display(), to.display()))?;
            }
            windows_step => execute_windows(windows_step)?,
        }
    }
    Ok(())
}

/// Runs a step of Windows' service manager.
#[cfg(windows)]
fn execute_windows(step: &Step) -> anyhow::Result<()> {
    match step {
        Step::Grant {
            path,
            account,
            access,
        } => windows::grant(path, account, *access),
        Step::Revoke { path, account } => windows::revoke(path, account),
        Step::CreateService(service) => windows::create(service),
        Step::UpdateService(service) => windows::update(service),
        Step::RestartOnFailure { name, after } => windows::restart_on_failure(name, *after),
        Step::StartService { name } => windows::start(name),
        Step::StopService { name } => windows::stop(name),
        Step::DeleteService { name } => windows::delete(name),
        other => bail!("{other:?} is no step of Windows' service manager"),
    }
}

/// Only Windows runs the steps of Windows' service manager.
#[cfg(not(windows))]
fn execute_windows(step: &Step) -> anyhow::Result<()> {
    bail!("{step:?} is a step of Windows' service manager")
}

/// Makes the definition `path` readable by everyone and writable by its owner, root, as the
/// service managers ask of a definition.
#[cfg(unix)]
fn readable_by_everyone(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
        .with_context(|| format!("set the mode of {}", path.display()))
}

/// Windows gives a file the access of its folder.
#[cfg(not(unix))]
fn readable_by_everyone(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Gives the file `path` to the user `uid`.
#[cfg(unix)]
fn give_to(path: &Path, uid: u32) -> anyhow::Result<()> {
    std::os::unix::fs::chown(path, Some(uid), None)
        .with_context(|| format!("give {} to the user {uid}", path.display()))
}

/// Only launchd's log is given to a user.
#[cfg(not(unix))]
fn give_to(path: &Path, _uid: u32) -> anyhow::Result<()> {
    bail!("{}: no owner to give it to", path.display())
}

#[cfg(test)]
mod tests;
