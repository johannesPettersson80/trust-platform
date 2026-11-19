//! Windows' service manager (specification 48 RU-64): the steps of a Windows service through the
//! service manager's own interface and `icacls`, the check for an administrator's rights, and
//! `trust-dev service run`, the service's program, which answers the service manager and runs the
//! service's binary as its child. Nothing here needs `unsafe`: the service manager calls the
//! service's main function with its start parameters, which the service does not take, so it
//! reads what to run from its own command line instead.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept,
    ServiceErrorControl, ServiceExitCode, ServiceFailureActions, ServiceFailureResetPeriod,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use super::definition::service_id;
use super::steps::{Access, WindowsService};

/// The service manager's answer for a service that does not exist, `ERROR_SERVICE_DOES_NOT_EXIST`.
const NO_SUCH_SERVICE: i32 = 1060;

/// How long `stop` waits for a service to stop.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// Whether this process may create a service, which takes an administrator.
pub(super) fn may_create_services() -> bool {
    manager().is_ok()
}

/// Creates the service `service`.
pub(super) fn create(service: &WindowsService) -> anyhow::Result<()> {
    manager()?
        .create_service(&info(service), ServiceAccess::QUERY_STATUS)
        .with_context(|| format!("create the service {}", service.name))?;
    Ok(())
}

/// Gives the service of `service`'s name its settings; creates it when it does not exist, as after
/// it was deleted by hand.
pub(super) fn update(service: &WindowsService) -> anyhow::Result<()> {
    let manager = manager()?;
    match manager.open_service(&service.name, ServiceAccess::CHANGE_CONFIG) {
        Ok(existing) => existing
            .change_config(&info(service))
            .with_context(|| format!("change the service {}", service.name)),
        Err(error) if missing(&error) => {
            manager
                .create_service(&info(service), ServiceAccess::QUERY_STATUS)
                .with_context(|| format!("create the service {}", service.name))?;
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("open the service {}", service.name)),
    }
}

/// Has the service manager start the service `name` again `after` every failure: a failure is also
/// the service's own stop with an error, which `run_host` reports when the binary ends.
pub(super) fn restart_on_failure(name: &str, after: Duration) -> anyhow::Result<()> {
    let service = manager()?
        .open_service(name, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
        .with_context(|| format!("open the service {name}"))?;
    service
        .update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 60 * 60)),
            reboot_msg: None,
            command: None,
            actions: Some(vec![ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: after,
            }]),
        })
        .with_context(|| format!("set the failure actions of the service {name}"))?;
    service
        .set_failure_actions_on_non_crash_failures(true)
        .with_context(|| format!("set the failure actions of the service {name}"))
}

/// Starts the service `name`.
pub(super) fn start(name: &str) -> anyhow::Result<()> {
    manager()?
        .open_service(name, ServiceAccess::START)
        .with_context(|| format!("open the service {name}"))?
        .start::<&str>(&[])
        .with_context(|| format!("start the service {name}"))
}

/// Stops the service `name` and waits until it has stopped; a service that does not exist or does
/// not run is stopped already.
pub(super) fn stop(name: &str) -> anyhow::Result<()> {
    let service =
        match manager()?.open_service(name, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS) {
            Ok(service) => service,
            Err(error) if missing(&error) => return Ok(()),
            Err(error) => return Err(error).with_context(|| format!("open the service {name}")),
        };
    let state = || -> anyhow::Result<ServiceState> {
        Ok(service
            .query_status()
            .with_context(|| format!("ask the state of the service {name}"))?
            .current_state)
    };
    if state()? == ServiceState::Stopped {
        return Ok(());
    }
    // A service that is stopping already refuses the request; the wait below covers both.
    let _ = service.stop();
    let deadline = Instant::now() + STOP_WAIT;
    while state()? != ServiceState::Stopped {
        if Instant::now() > deadline {
            bail!(
                "the service {name} did not stop within {} s",
                STOP_WAIT.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

/// Deletes the service `name`; a service that does not exist is deleted already.
pub(super) fn delete(name: &str) -> anyhow::Result<()> {
    match manager()?.open_service(name, ServiceAccess::DELETE) {
        Ok(service) => service
            .delete()
            .with_context(|| format!("delete the service {name}")),
        Err(error) if missing(&error) => Ok(()),
        Err(error) => Err(error).with_context(|| format!("open the service {name}")),
    }
}

/// Gives the account `account` the access `access` to the folder `path` and what it holds.
pub(super) fn grant(path: &Path, account: &str, access: Access) -> anyhow::Result<()> {
    let rights = match access {
        Access::ReadExecute => "(OI)(CI)RX",
        Access::Modify => "(OI)(CI)M",
    };
    icacls(path, &["/grant", &format!("{account}:{rights}")])
}

/// Takes the access given to the account `account` to the folder `path` away; a folder that no
/// longer exists has none to take.
pub(super) fn revoke(path: &Path, account: &str) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    icacls(path, &["/remove:g", account])
}

/// Runs `icacls` on `path` with `args`.
fn icacls(path: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("icacls")
        .arg(path)
        .args(args)
        .output()
        .context("run icacls")?;
    if !output.status.success() {
        bail!(
            "icacls {} {}: {}",
            path.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout).trim()
        );
    }
    Ok(())
}

/// The service manager, opened to create services.
fn manager() -> anyhow::Result<ServiceManager> {
    ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .context("open Windows' service manager to create a service")
}

/// The service manager's settings of `service`: its own process, started with the computer, under
/// its virtual account, which needs no password.
fn info(service: &WindowsService) -> ServiceInfo {
    ServiceInfo {
        name: service.name.clone().into(),
        display_name: service.display_name.clone().into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service.program.clone(),
        launch_arguments: service.arguments.iter().map(OsString::from).collect(),
        dependencies: Vec::new(),
        account_name: Some(service.account.clone().into()),
        account_password: None,
    }
}

/// Whether the service manager answered that the service does not exist.
fn missing(error: &windows_service::Error) -> bool {
    matches!(error, windows_service::Error::Winapi(io) if io.raw_os_error() == Some(NO_SUCH_SERVICE))
}

/// What the service's program runs, set before the service manager calls the service's main
/// function on a thread of its own.
static HOST: OnceLock<Host> = OnceLock::new();

/// The service a copy of trust-dev runs.
struct Host {
    /// The service's name, `[services.<name>]`.
    name: String,
    /// The project folder, the binary's working folder and `TRUST_PROJECT`.
    project: PathBuf,
    /// The service's binary.
    binary: PathBuf,
    /// The log the binary's output and errors are appended to.
    log: PathBuf,
}

/// `trust-dev service run <name> --project <folder> --binary <binary>`: answers Windows' service
/// manager as the service `trust-service-<name>` until the service stops.
pub(super) fn run_host(name: &str, project: &Path, binary: &Path) -> anyhow::Result<()> {
    let folder = std::env::current_exe()
        .context("find trust-dev's own path")?
        .parent()
        .map(Path::to_path_buf)
        .context("find trust-dev's own folder")?;
    let log = folder.join("logs").join(format!("{name}.log"));
    let _ = HOST.set(Host {
        name: name.to_string(),
        project: project.to_path_buf(),
        binary: binary.to_path_buf(),
        log,
    });
    service_dispatcher::start(service_id(name), service_main).context(
        "answer Windows' service manager: `trust-dev service run` is the program of a Windows \
         service, which the service manager runs (`trust-dev service install`)",
    )
}

/// The service's main function, which the service manager calls with the service's start
/// parameters; the service takes none, so it does not read them.
extern "system" fn service_main(_count: u32, _parameters: *mut *mut u16) {
    if let Some(host) = HOST.get()
        && let Err(error) = supervise(host)
    {
        note(&host.log, &format!("trust-dev service run: {error:#}"));
    }
}

/// Reports the service running while its binary runs: stops the binary when the service is
/// stopped, and stops with an error when the binary ends, so that the service manager starts it
/// again.
fn supervise(host: &Host) -> anyhow::Result<()> {
    let (stop, stopped) = mpsc::channel();
    let status =
        service_control_handler::register(service_id(&host.name), move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })
        .context("register with Windows' service manager")?;
    let report = |state: ServiceState, failed: bool| {
        status
            .set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: if state == ServiceState::Running {
                    ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
                } else {
                    ServiceControlAccept::empty()
                },
                exit_code: if failed {
                    ServiceExitCode::ServiceSpecific(1)
                } else {
                    ServiceExitCode::NO_ERROR
                },
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            })
            .context("report the service's state")
    };
    let mut child = match spawn(host) {
        Ok(child) => child,
        Err(error) => {
            report(ServiceState::Stopped, true)?;
            return Err(error);
        }
    };
    report(ServiceState::Running, false)?;
    loop {
        if stopped.recv_timeout(Duration::from_millis(250)).is_ok() {
            let _ = child.kill();
            let _ = child.wait();
            return report(ServiceState::Stopped, false);
        }
        if let Some(exit) = child.try_wait().context("watch the service's binary")? {
            note(
                &host.log,
                &format!(
                    "trust-dev service run: {} ended ({exit}); the service manager starts it again",
                    host.binary.display()
                ),
            );
            return report(ServiceState::Stopped, true);
        }
    }
}

/// Starts the service's binary in the project folder, with `TRUST_PROJECT` naming it, and its
/// output and errors appended to the log.
fn spawn(host: &Host) -> anyhow::Result<Child> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&host.log)
        .with_context(|| format!("open {}", host.log.display()))?;
    let errors = log
        .try_clone()
        .with_context(|| format!("open {}", host.log.display()))?;
    Command::new(&host.binary)
        .current_dir(&host.project)
        .env("TRUST_PROJECT", &host.project)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(errors)
        .spawn()
        .with_context(|| format!("start {}", host.binary.display()))
}

/// Appends `line` to the log `log`; a log that cannot be written leaves nowhere to say so.
fn note(log: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log) {
        let _ = writeln!(file, "{line}");
    }
}
