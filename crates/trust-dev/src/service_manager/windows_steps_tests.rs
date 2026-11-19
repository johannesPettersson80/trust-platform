//! The steps of a Windows service (specification 48 RU-64), worked out by hand from it: the copy of
//! trust-dev, `service.toml`, the virtual account's access, the service and its restart, on a first
//! install, on an install again and on uninstall; and the command for an administrator. The steps'
//! paths use `/`, which Windows takes as well, so that the steps read the same on every computer.

use std::path::Path;
use std::time::Duration;

use super::super::plan::Plan;
use super::super::steps::{installed_for_another, Access, Step, System, WindowsService};
use super::super::{install_command, uninstall_command};
use super::*;

/// `%ProgramData%\truST\services` on a computer whose `ProgramData` is `C:/ProgramData`.
const SERVICES: &str = "C:/ProgramData/truST/services";

/// The virtual account of the service recipe.
const ACCOUNT: &str = r"NT SERVICE\trust-service-recipe";

/// The plan of the service recipe of the project folder `project`, running `binary`.
fn plan(project: &str, binary: &str) -> Plan {
    Plan {
        name: "recipe".into(),
        project: project.into(),
        binary: binary.into(),
    }
}

#[test]
fn a_first_install_copies_trust_dev_records_the_folder_gives_access_and_creates_the_service() {
    // RU-64, in order: the service's folder with its logs folder, the copy of trust-dev, its
    // service.toml, the service started with the computer under its virtual account and running
    // the copy with the service's name, project and binary, restarted 2 s after every failure; then
    // read and execute access for the virtual account, which exists from the service's creation on,
    // to the project folder and the service's folder and write access only to its logs; and the
    // start. Catches a service that runs as LocalSystem, one that cannot read its project, one that
    // can replace its own program, one that a truST update breaks, one that stays down after its
    // binary ends, and access given to an account that does not exist yet (the probe's icacls
    // failure on Windows, 19 November).
    let services = Path::new(SERVICES);
    let folder = services.join("recipe");
    let program = folder.join("trust-dev.exe");
    let plan = plan(
        "C:/Users/ada/plant",
        "C:/Users/ada/plant/recipe-service/target/release/recipe-service.exe",
    );
    assert_eq!(
        install_steps(&plan, Path::new("C:/tools/trust-dev.exe"), services, false),
        vec![
            Step::Folder {
                path: folder.join("logs")
            },
            Step::Copy {
                from: "C:/tools/trust-dev.exe".into(),
                to: program.clone(),
            },
            Step::Write {
                path: folder.join("service.toml"),
                text: record(&plan),
            },
            Step::CreateService(WindowsService {
                name: "trust-service-recipe".into(),
                display_name: "truST service recipe".into(),
                program,
                arguments: [
                    "service",
                    "run",
                    "recipe",
                    "--project",
                    "C:/Users/ada/plant",
                    "--binary",
                    "C:/Users/ada/plant/recipe-service/target/release/recipe-service.exe",
                ]
                .map(String::from)
                .to_vec(),
                account: ACCOUNT.into(),
            }),
            Step::RestartOnFailure {
                name: "trust-service-recipe".into(),
                after: Duration::from_secs(2),
            },
            Step::Grant {
                path: "C:/Users/ada/plant".into(),
                account: ACCOUNT.into(),
                access: Access::ReadExecute,
            },
            Step::Grant {
                path: folder.clone(),
                account: ACCOUNT.into(),
                access: Access::ReadExecute,
            },
            Step::Grant {
                path: folder.join("logs"),
                account: ACCOUNT.into(),
                access: Access::Modify,
            },
            Step::StartService {
                name: "trust-service-recipe".into()
            },
        ]
    );
}

#[test]
fn an_install_again_stops_and_changes_the_service_and_a_binary_elsewhere_is_readable() {
    // RU-64: installed for the same folder, the service is stopped before its copy is replaced,
    // its settings are changed instead of a second service created, and it is started again; a
    // binary outside the project folder (a target folder of CARGO_TARGET_DIR) is given read and
    // execute access too. Catches a copy that cannot be replaced while it runs, a second service
    // of a name, and a service that cannot run its binary.
    let plan = plan(
        "C:/Users/ada/plant",
        "D:/targets/release/recipe-service.exe",
    );
    let steps = install_steps(
        &plan,
        Path::new("C:/tools/trust-dev.exe"),
        Path::new(SERVICES),
        true,
    );
    assert_eq!(
        steps.first(),
        Some(&Step::StopService {
            name: "trust-service-recipe".into()
        })
    );
    assert!(steps.contains(&Step::Grant {
        path: "D:/targets/release".into(),
        account: ACCOUNT.into(),
        access: Access::ReadExecute,
    }));
    assert!(steps
        .iter()
        .any(|step| matches!(step, Step::UpdateService(_))));
    assert!(!steps
        .iter()
        .any(|step| matches!(step, Step::CreateService(_))));
    assert_eq!(
        steps.last(),
        Some(&Step::StartService {
            name: "trust-service-recipe".into()
        })
    );
}

#[test]
fn uninstall_deletes_the_service_takes_its_access_away_and_keeps_its_log() {
    // RU-64: stop the service, take the virtual account's access to every folder it was given away
    // again (from service.toml: the project folder and the binary's folder) while the account still
    // exists, delete the service, remove the copy and service.toml, and keep the log. Catches a
    // service left behind, access left to a deleted account, which no longer has a name to remove
    // it by, and a log lost with the service.
    let services = Path::new(SERVICES);
    let folder = services.join("recipe");
    let record = Record {
        project: "C:/Users/ada/plant".into(),
        binary: "D:/targets/release/recipe-service.exe".into(),
    };
    assert_eq!(
        uninstall_steps("recipe", services, &record),
        vec![
            Step::StopService {
                name: "trust-service-recipe".into()
            },
            Step::Revoke {
                path: "C:/Users/ada/plant".into(),
                account: ACCOUNT.into(),
            },
            Step::Revoke {
                path: "D:/targets/release".into(),
                account: ACCOUNT.into(),
            },
            Step::Revoke {
                path: folder.clone(),
                account: ACCOUNT.into(),
            },
            Step::Revoke {
                path: folder.join("logs"),
                account: ACCOUNT.into(),
            },
            Step::DeleteService {
                name: "trust-service-recipe".into()
            },
            Step::Remove {
                path: folder.join("trust-dev.exe")
            },
            Step::Remove {
                path: folder.join("service.toml")
            },
        ]
    );
}

#[test]
fn service_toml_records_the_project_folder_and_the_binary() {
    // RU-64: service.toml reads back as it was written, with Windows' backslashes and a space, so
    // that install knows the folder a service was installed for and uninstall the access it gave.
    // Catches a record whose paths do not read back.
    let plan = plan(
        r"C:\Users\ada\my plant",
        r"C:\Users\ada\my plant\target\release\recipe-service.exe",
    );
    let text = record(&plan);
    assert!(
        text.starts_with(
            "# Written by trust-dev service install: the service trust-service-recipe"
        ),
        "{text}"
    );
    assert_eq!(
        read_record(&text),
        Some(Record {
            project: plan.project.clone(),
            binary: plan.binary.clone(),
        })
    );
    assert_eq!(read_record("project = 1"), None);
}

#[test]
fn a_windows_service_installed_for_another_project_folder_is_not_replaced() {
    // RU-64: a computer has one trust-service-<name>; install replaces it only when its
    // service.toml names the same project folder. Catches an install that takes another project's
    // service, and one that takes a folder whose service.toml does not read for its own.
    let existing = record(&plan(r"C:\Users\ada\plant", r"C:\x.exe"));
    assert!(!installed_for_another(
        System::Windows,
        &existing,
        Path::new(r"C:\Users\ada\plant")
    ));
    assert!(installed_for_another(
        System::Windows,
        &existing,
        Path::new(r"C:\Users\ada\plant2")
    ));
    assert!(installed_for_another(
        System::Windows,
        "not a record",
        Path::new(r"C:\Users\ada\plant")
    ));
}

#[test]
fn an_administrator_s_command_is_named_for_powershell() {
    // RU-64: on Windows the command for the rights is the same command without sudo, for PowerShell
    // opened as administrator; a path with a space is quoted, and a quoted program is called with
    // `&`, without which PowerShell takes it for text. Catches a command that does not run as
    // printed.
    let plain = plan(
        r"C:\Users\ada\plant",
        r"C:\Users\ada\plant\target\release\recipe-service.exe",
    );
    assert_eq!(
        install_command(
            System::Windows,
            Path::new(r"C:\tools\trust-dev.exe"),
            &plain
        ),
        r"C:\tools\trust-dev.exe service install recipe --project C:\Users\ada\plant --binary C:\Users\ada\plant\target\release\recipe-service.exe"
    );
    let spaced = plan(
        r"C:\Users\Ada Lovelace\plant",
        r"C:\Users\Ada Lovelace\plant\target\release\recipe-service.exe",
    );
    assert_eq!(
        install_command(
            System::Windows,
            Path::new(r"C:\Users\Ada Lovelace\bin\trust-dev.exe"),
            &spaced
        ),
        r"& 'C:\Users\Ada Lovelace\bin\trust-dev.exe' service install recipe --project 'C:\Users\Ada Lovelace\plant' --binary 'C:\Users\Ada Lovelace\plant\target\release\recipe-service.exe'"
    );
    assert_eq!(
        uninstall_command(
            System::Windows,
            Path::new(r"C:\tools\trust-dev.exe"),
            "recipe"
        ),
        r"C:\tools\trust-dev.exe service uninstall recipe"
    );
}
