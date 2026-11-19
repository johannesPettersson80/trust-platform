//! The steps of a Windows service (specification 48 RU-64): a copy of trust-dev in the service's
//! own folder under `%ProgramData%\truST\services`, which answers the service manager and runs the
//! service's binary; the folder's `service.toml`, which records the project folder and the binary;
//! the access of the service's virtual account; and the service, which starts with the computer and
//! again 2 s after it ends. Worked out as data on every computer, as the other service managers'.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::definition::service_id;
use super::plan::Plan;
use super::steps::{Access, Step, WindowsService};

/// What a Windows service's `service.toml` records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Record {
    /// The project folder the service was installed for.
    pub(crate) project: PathBuf,
    /// The binary it runs.
    pub(crate) binary: PathBuf,
}

/// The folder of the services' own folders: `%ProgramData%\truST\services`.
pub(crate) fn services_folder() -> PathBuf {
    std::env::var_os("ProgramData")
        .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from)
        .join("truST")
        .join("services")
}

/// The service `name`'s own folder in `services`.
pub(crate) fn service_folder(services: &Path, name: &str) -> PathBuf {
    services.join(name)
}

/// The `service.toml` of the service `name`.
pub(crate) fn record_path(services: &Path, name: &str) -> PathBuf {
    service_folder(services, name).join("service.toml")
}

/// The log the copy of trust-dev of the service `name` appends the binary's output to.
pub(crate) fn log_path(services: &Path, name: &str) -> PathBuf {
    service_folder(services, name)
        .join("logs")
        .join(format!("{name}.log"))
}

/// The copy of trust-dev in the service's folder, the service's program.
pub(crate) fn host_path(services: &Path, name: &str) -> PathBuf {
    service_folder(services, name).join("trust-dev.exe")
}

/// The virtual account of the service `name`: `NT SERVICE\trust-service-<name>`.
pub(crate) fn account(name: &str) -> String {
    format!(r"NT SERVICE\{}", service_id(name))
}

/// The text of the `service.toml` of `plan`.
pub(crate) fn record(plan: &Plan) -> String {
    let mut table = toml::Table::new();
    table.insert(
        "project".into(),
        toml::Value::String(plan.project.display().to_string()),
    );
    table.insert(
        "binary".into(),
        toml::Value::String(plan.binary.display().to_string()),
    );
    format!(
        "# Written by trust-dev service install: the service {} (specification 48 RU-64).\n{}",
        service_id(&plan.name),
        toml::to_string(&table).unwrap_or_default()
    )
}

/// The record a `service.toml`'s `text` holds; `None` for a text that holds none.
pub(crate) fn read_record(text: &str) -> Option<Record> {
    let table: toml::Table = toml::from_str(text).ok()?;
    let path = |key: &str| table.get(key)?.as_str().map(PathBuf::from);
    Some(Record {
        project: path("project")?,
        binary: path("binary")?,
    })
}

/// The steps that install the service of `plan` and start it, with a copy of the trust-dev at
/// `host`, in its folder in `services`. For a service that is `installed` already, for the same
/// project folder, they stop it, replace the copy and the settings, and start it again.
pub(crate) fn install_steps(
    plan: &Plan,
    host: &Path,
    services: &Path,
    installed: bool,
) -> Vec<Step> {
    let id = service_id(&plan.name);
    let account = account(&plan.name);
    let folder = service_folder(services, &plan.name);
    let logs = folder.join("logs");
    let service = WindowsService {
        name: id.clone(),
        display_name: format!("truST service {}", plan.name),
        program: host_path(services, &plan.name),
        arguments: vec![
            "service".into(),
            "run".into(),
            plan.name.clone(),
            "--project".into(),
            plan.project.display().to_string(),
            "--binary".into(),
            plan.binary.display().to_string(),
        ],
        account: account.clone(),
    };
    let mut steps = Vec::new();
    if installed {
        steps.push(Step::StopService { name: id.clone() });
    }
    steps.push(Step::Folder { path: logs.clone() });
    steps.push(Step::Copy {
        from: host.to_path_buf(),
        to: service.program.clone(),
    });
    steps.push(Step::Write {
        path: record_path(services, &plan.name),
        text: record(plan),
    });
    steps.push(if installed {
        Step::UpdateService(service)
    } else {
        Step::CreateService(service)
    });
    steps.push(Step::RestartOnFailure {
        name: id.clone(),
        after: Duration::from_secs(2),
    });
    // The virtual account exists from the service's creation on, so its access comes after it.
    for path in read_folders(&plan.project, &plan.binary, &folder) {
        steps.push(Step::Grant {
            path,
            account: account.clone(),
            access: Access::ReadExecute,
        });
    }
    steps.push(Step::Grant {
        path: logs,
        account,
        access: Access::Modify,
    });
    steps.push(Step::StartService { name: id });
    steps
}

/// The steps that stop the service `name`, installed as `record` says, take the access of its
/// virtual account away again while the account exists, delete the service, and remove its copy of
/// trust-dev and its `service.toml` from its folder in `services`; its log stays.
pub(crate) fn uninstall_steps(name: &str, services: &Path, record: &Record) -> Vec<Step> {
    let id = service_id(name);
    let account = account(name);
    let folder = service_folder(services, name);
    let mut steps = vec![Step::StopService { name: id.clone() }];
    for path in read_folders(&record.project, &record.binary, &folder) {
        steps.push(Step::Revoke {
            path,
            account: account.clone(),
        });
    }
    steps.push(Step::Revoke {
        path: folder.join("logs"),
        account,
    });
    steps.push(Step::DeleteService { name: id });
    steps.push(Step::Remove {
        path: host_path(services, name),
    });
    steps.push(Step::Remove {
        path: record_path(services, name),
    });
    steps
}

/// The folders the service's virtual account reads and runs programs from: the project folder, the
/// binary's folder when it lies outside it, and the service's own folder.
fn read_folders(project: &Path, binary: &Path, folder: &Path) -> Vec<PathBuf> {
    let mut folders = vec![project.to_path_buf()];
    if let Some(release) = binary
        .parent()
        .filter(|release| !release.starts_with(project))
    {
        folders.push(release.to_path_buf());
    }
    folders.push(folder.to_path_buf());
    folders
}

#[cfg(test)]
#[path = "windows_steps_tests.rs"]
mod tests;
