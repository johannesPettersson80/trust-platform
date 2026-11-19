//! `trust-dev service install` and `trust-dev service uninstall`: a service of the project under
//! the computer's service manager (specification 48 RU-64). `install` reads the service's settings
//! and finds its release binary as the user who builds it, works out every step, and runs them only
//! with root's rights; without them it changes nothing and names the command that has them, with
//! the binary's path, so that the command needs neither cargo nor the user's `PATH` under `sudo`.

mod definition;
mod plan;
mod steps;

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
    let owner = plan::owner(&plan.project)?;
    let id = service_id(name);
    let definition = system.definition_path(name);
    if let Ok(existing) = std::fs::read_to_string(&definition)
        && steps::installed_for_another(system, &existing, &plan.project)
    {
        bail!(
            "{id} is installed for another project folder ({definition}): a computer has one \
             service of a name, so uninstall it first with `trust-dev service uninstall {name}`"
        );
    }
    if !is_root()? {
        bail!(
            "installing {id} changes the computer's services, which takes root; nothing was \
             changed. Run:\n  {}",
            install_command(&current_exe()?, &plan)
        );
    }
    execute(&steps::install_steps(system, &plan, &owner))?;
    println!(
        "Installed and started {id}: {} runs as {} in {}.",
        plan.binary.display(),
        owner.name,
        plan.project.display()
    );
    println!("Its output: {}", system.output(name));
    Ok(())
}

/// `trust-dev service uninstall <name>`: stops the service `name` and removes it from the
/// computer's service manager (specification 48 RU-64).
pub fn run_uninstall(name: &str) -> anyhow::Result<()> {
    plan::check_name(name)?;
    let system = current_system()?;
    let id = service_id(name);
    let definition = system.definition_path(name);
    if !Path::new(&definition).exists() {
        bail!("{id} is not installed: there is no {definition}");
    }
    if !is_root()? {
        bail!(
            "removing {id} changes the computer's services, which takes root; nothing was changed. \
             Run:\n  {}",
            uninstall_command(&current_exe()?, name)
        );
    }
    execute(&steps::uninstall_steps(system, name))?;
    println!("Stopped and removed {id}.");
    Ok(())
}

/// The command that installs the service of `plan` with root's rights: `sudo`, trust-dev's own
/// path, the project folder and the binary, each quoted for the shell where it needs it.
pub(crate) fn install_command(trust_dev: &Path, plan: &Plan) -> String {
    format!(
        "sudo {} service install {} --project {} --binary {}",
        shell_word(trust_dev),
        plan.name,
        shell_word(&plan.project),
        shell_word(&plan.binary)
    )
}

/// The command that uninstalls the service `name` with root's rights.
pub(crate) fn uninstall_command(trust_dev: &Path, name: &str) -> String {
    format!("sudo {} service uninstall {name}", shell_word(trust_dev))
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
        "trust-dev service install does not put a service under Windows' service manager yet; \
         it installs a systemd unit on Linux and a launchd daemon on macOS (specification 48 RU-64)",
    )
}

/// trust-dev's own path, which the command named for root runs.
fn current_exe() -> anyhow::Result<PathBuf> {
    std::env::current_exe().context("find trust-dev's own path")
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

/// No computer without a service manager of `install` gets this far.
#[cfg(not(unix))]
fn is_root() -> anyhow::Result<bool> {
    Ok(false)
}

/// Runs `steps` in order; the first that fails ends the command, naming it.
fn execute(steps: &[Step]) -> anyhow::Result<()> {
    for step in steps {
        match step {
            Step::Write { path, text } => {
                std::fs::write(path, text).with_context(|| format!("write {path}"))?;
                readable_by_everyone(Path::new(path))?;
            }
            Step::Log { path, uid } => {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .with_context(|| format!("create {path}"))?;
                give_to(Path::new(path), *uid)?;
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
                std::fs::remove_file(path).with_context(|| format!("remove {path}"))?;
            }
        }
    }
    Ok(())
}

/// Makes the definition `path` readable by everyone and writable by its owner, root, as the
/// service managers ask of a definition.
#[cfg(unix)]
fn readable_by_everyone(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
        .with_context(|| format!("set the mode of {}", path.display()))
}

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

#[cfg(not(unix))]
fn give_to(path: &Path, _uid: u32) -> anyhow::Result<()> {
    bail!("{}: no owner to give it to", path.display())
}

#[cfg(test)]
mod tests;
