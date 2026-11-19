//! The steps that put a service under each service manager and take it away again (specification
//! 48 RU-64), worked out before any of them runs, so that `install` and `uninstall` change nothing
//! when they refuse, and a test reads every step of every service manager on any computer.

use std::path::Path;

use super::definition::{
    launchd_log_path, launchd_plist, launchd_plist_path, launchd_project_entry, service_id,
    systemd_project_line, systemd_unit, systemd_unit_path,
};
use super::plan::{Owner, Plan};

/// The service managers `install` writes for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum System {
    /// systemd, on Linux.
    Systemd,
    /// launchd, on macOS.
    Launchd,
}

impl System {
    /// This computer's service manager; `None` where `install` has none yet.
    pub(crate) fn current() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Systemd)
        } else if cfg!(target_os = "macos") {
            Some(Self::Launchd)
        } else {
            None
        }
    }

    /// The file of the definition of the service `name`.
    pub(crate) fn definition_path(self, name: &str) -> String {
        match self {
            Self::Systemd => systemd_unit_path(name),
            Self::Launchd => launchd_plist_path(name),
        }
    }

    /// Where the output of the service `name` goes, as the command reports it.
    pub(crate) fn output(self, name: &str) -> String {
        match self {
            Self::Systemd => format!("journalctl -u {}", service_id(name)),
            Self::Launchd => launchd_log_path(name),
        }
    }
}

/// One step of `install` or `uninstall`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Write the definition `text` into the file `path`, readable by everyone.
    Write { path: String, text: String },
    /// Create the log `path` when it is missing and give it to the user `uid`, whom launchd opens
    /// it for.
    Log { path: String, uid: u32 },
    /// Run `program` with `args`; its failure ends the command unless it `may_fail`, as booting
    /// out a daemon that is not loaded does.
    Run {
        program: &'static str,
        args: Vec<String>,
        may_fail: bool,
    },
    /// Remove the file `path`.
    Remove { path: String },
}

/// The steps that install the service of `plan`, running as `owner`, under `system`, and start it;
/// for a service installed before for the same project folder, they replace and restart it.
pub(crate) fn install_steps(system: System, plan: &Plan, owner: &Owner) -> Vec<Step> {
    let id = service_id(&plan.name);
    match system {
        System::Systemd => {
            let unit = format!("{id}.service");
            vec![
                Step::Write {
                    path: systemd_unit_path(&plan.name),
                    text: systemd_unit(plan, &owner.name),
                },
                run("systemctl", &["daemon-reload"]),
                run("systemctl", &["enable", &unit]),
                run("systemctl", &["restart", &unit]),
            ]
        }
        System::Launchd => {
            let job = format!("system/{id}");
            let plist = launchd_plist_path(&plan.name);
            vec![
                Step::Run {
                    program: "launchctl",
                    args: vec!["bootout".into(), job.clone()],
                    may_fail: true,
                },
                Step::Write {
                    path: plist.clone(),
                    text: launchd_plist(plan, &owner.name),
                },
                Step::Log {
                    path: launchd_log_path(&plan.name),
                    uid: owner.uid,
                },
                run("launchctl", &["enable", &job]),
                run("launchctl", &["bootstrap", "system", &plist]),
            ]
        }
    }
}

/// The steps that stop the service `name` and remove it from `system`.
pub(crate) fn uninstall_steps(system: System, name: &str) -> Vec<Step> {
    let id = service_id(name);
    match system {
        System::Systemd => vec![
            run("systemctl", &["disable", "--now", &format!("{id}.service")]),
            Step::Remove {
                path: systemd_unit_path(name),
            },
            run("systemctl", &["daemon-reload"]),
        ],
        System::Launchd => vec![
            Step::Run {
                program: "launchctl",
                args: vec!["bootout".into(), format!("system/{id}")],
                may_fail: true,
            },
            Step::Remove {
                path: launchd_plist_path(name),
            },
        ],
    }
}

/// Whether the definition `existing` of a service, which `system` holds, was written for a
/// project folder other than `project`: a computer has one service of a name.
pub(crate) fn installed_for_another(system: System, existing: &str, project: &Path) -> bool {
    let marker = match system {
        System::Systemd => systemd_project_line(project),
        System::Launchd => launchd_project_entry(project),
    };
    !existing.contains(&marker)
}

/// A step that runs `program` with `args` and must succeed.
fn run(program: &'static str, args: &[&str]) -> Step {
    Step::Run {
        program,
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        may_fail: false,
    }
}
