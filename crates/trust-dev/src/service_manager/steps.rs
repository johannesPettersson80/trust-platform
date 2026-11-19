//! The steps that put a service under each service manager and take it away again (specification
//! 48 RU-64), worked out before any of them runs, so that `install` and `uninstall` change nothing
//! when they refuse, and a test reads every step of every service manager on any computer. The
//! steps of a Windows service are in `windows_steps`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::definition::{
    launchd_log_path, launchd_plist, launchd_plist_path, launchd_project_entry, service_id,
    systemd_project_line, systemd_unit, systemd_unit_path,
};
use super::plan::{Owner, Plan};
use super::windows_steps;

/// The service managers of Linux and macOS, whose definitions run the service's binary as the
/// project folder's owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unix {
    /// systemd, on Linux.
    Systemd,
    /// launchd, on macOS.
    Launchd,
}

/// The service managers `install` writes for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum System {
    /// systemd or launchd.
    Unix(Unix),
    /// Windows' service manager.
    Windows,
}

impl System {
    /// This computer's service manager; `None` on a computer that has none of the three.
    pub(crate) fn current() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Unix(Unix::Systemd))
        } else if cfg!(target_os = "macos") {
            Some(Self::Unix(Unix::Launchd))
        } else if cfg!(windows) {
            Some(Self::Windows)
        } else {
            None
        }
    }

    /// The file that holds the service `name` once it is installed: the systemd unit, the launchd
    /// daemon, or the Windows service's `service.toml`.
    pub(crate) fn definition_path(self, name: &str) -> PathBuf {
        match self {
            Self::Unix(Unix::Systemd) => systemd_unit_path(name).into(),
            Self::Unix(Unix::Launchd) => launchd_plist_path(name).into(),
            Self::Windows => windows_steps::record_path(&windows_steps::services_folder(), name),
        }
    }

    /// Where the output of the service `name` goes, as the command reports it.
    pub(crate) fn output(self, name: &str) -> String {
        match self {
            Self::Unix(Unix::Systemd) => format!("journalctl -u {}", service_id(name)),
            Self::Unix(Unix::Launchd) => launchd_log_path(name),
            Self::Windows => windows_steps::log_path(&windows_steps::services_folder(), name)
                .display()
                .to_string(),
        }
    }
}

/// One step of `install` or `uninstall`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Write the definition `text` into the file `path`, readable by everyone.
    Write { path: PathBuf, text: String },
    /// Create the log `path` when it is missing and give it to the user `uid`, whom launchd opens
    /// it for.
    Log { path: PathBuf, uid: u32 },
    /// Run `program` with `args`; its failure ends the command unless it `may_fail`, as booting
    /// out a daemon that is not loaded does.
    Run {
        program: &'static str,
        args: Vec<String>,
        may_fail: bool,
    },
    /// Remove the file `path`.
    Remove { path: PathBuf },
    /// Create the folder `path` with the folders it is in.
    Folder { path: PathBuf },
    /// Copy the file `from` to `to`, replacing it.
    Copy { from: PathBuf, to: PathBuf },
    /// Give the account `account` the access `access` to the folder `path` and what it holds.
    Grant {
        path: PathBuf,
        account: String,
        access: Access,
    },
    /// Take the access the account `account` was given to the folder `path` away again.
    Revoke { path: PathBuf, account: String },
    /// Create the Windows service `service`.
    CreateService(WindowsService),
    /// Give the existing Windows service of the same name the settings of `service`.
    UpdateService(WindowsService),
    /// Have the service manager start the service `name` again `after` it ends with a failure,
    /// every time.
    RestartOnFailure { name: String, after: Duration },
    /// Start the service `name`.
    StartService { name: String },
    /// Stop the service `name` and wait until it has stopped; a service that does not run is
    /// stopped already.
    StopService { name: String },
    /// Delete the service `name`.
    DeleteService { name: String },
}

/// What an account may do in a folder and in what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    /// Read the files and run the programs.
    ReadExecute,
    /// Also write, change and delete files.
    Modify,
}

/// A Windows service as `install` creates it: the service manager runs `program` with
/// `arguments` under `account`, starting it with the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WindowsService {
    /// The service's name, `trust-service-<name>`.
    pub(crate) name: String,
    /// The name the service manager shows.
    pub(crate) display_name: String,
    /// The copy of trust-dev that answers the service manager.
    pub(crate) program: PathBuf,
    /// The copy's arguments: `service run <name> --project <folder> --binary <binary>`.
    pub(crate) arguments: Vec<String>,
    /// The service's virtual account, `NT SERVICE\trust-service-<name>`.
    pub(crate) account: String,
}

/// The steps that install the service of `plan`, running as `owner`, under `unix`, and start it;
/// for a service installed before for the same project folder, they replace and restart it.
pub(crate) fn install_steps(unix: Unix, plan: &Plan, owner: &Owner) -> Vec<Step> {
    let id = service_id(&plan.name);
    match unix {
        Unix::Systemd => {
            let unit = format!("{id}.service");
            vec![
                Step::Write {
                    path: systemd_unit_path(&plan.name).into(),
                    text: systemd_unit(plan, &owner.name),
                },
                run("systemctl", &["daemon-reload"]),
                run("systemctl", &["enable", &unit]),
                run("systemctl", &["restart", &unit]),
            ]
        }
        Unix::Launchd => {
            let job = format!("system/{id}");
            let plist = launchd_plist_path(&plan.name);
            vec![
                Step::Run {
                    program: "launchctl",
                    args: vec!["bootout".into(), job.clone()],
                    may_fail: true,
                },
                Step::Write {
                    path: plist.clone().into(),
                    text: launchd_plist(plan, &owner.name),
                },
                Step::Log {
                    path: launchd_log_path(&plan.name).into(),
                    uid: owner.uid,
                },
                run("launchctl", &["enable", &job]),
                run("launchctl", &["bootstrap", "system", &plist]),
            ]
        }
    }
}

/// The steps that stop the service `name` and remove it from `unix`.
pub(crate) fn uninstall_steps(unix: Unix, name: &str) -> Vec<Step> {
    let id = service_id(name);
    match unix {
        Unix::Systemd => vec![
            run("systemctl", &["disable", "--now", &format!("{id}.service")]),
            Step::Remove {
                path: systemd_unit_path(name).into(),
            },
            run("systemctl", &["daemon-reload"]),
        ],
        Unix::Launchd => vec![
            Step::Run {
                program: "launchctl",
                args: vec!["bootout".into(), format!("system/{id}")],
                may_fail: true,
            },
            Step::Remove {
                path: launchd_plist_path(name).into(),
            },
        ],
    }
}

/// Whether the definition `existing` of a service, which `system` holds, was written for a
/// project folder other than `project`: a computer has one service of a name.
pub(crate) fn installed_for_another(system: System, existing: &str, project: &Path) -> bool {
    match system {
        System::Unix(Unix::Systemd) => !existing.contains(&systemd_project_line(project)),
        System::Unix(Unix::Launchd) => !existing.contains(&launchd_project_entry(project)),
        System::Windows => {
            windows_steps::read_record(existing).is_none_or(|record| record.project != project)
        }
    }
}

/// A step that runs `program` with `args` and must succeed.
fn run(program: &'static str, args: &[&str]) -> Step {
    Step::Run {
        program,
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        may_fail: false,
    }
}
