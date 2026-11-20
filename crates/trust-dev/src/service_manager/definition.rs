//! The definitions `trust-dev service install` writes (specification 48 RU-64): the systemd unit
//! of Linux and the launchd daemon of macOS, each running the service's binary as the project
//! folder's owner, in the project folder and with `TRUST_PROJECT` naming it.

use std::fmt::Write as _;
use std::path::Path;

use super::Plan;

/// Where `systemctl` finds the units the administrator installs.
pub(super) const SYSTEMD_FOLDER: &str = "/etc/systemd/system";
/// Where launchd finds the daemons of the system.
pub(super) const LAUNCHD_FOLDER: &str = "/Library/LaunchDaemons";
/// Where a launchd daemon's output goes, a file per service.
pub(super) const LAUNCHD_LOG_FOLDER: &str = "/Library/Logs";

/// The name of the service `name` on every computer: `trust-service-<name>`.
pub(super) fn service_id(name: &str) -> String {
    format!("trust-service-{name}")
}

/// The file of the systemd unit of the service `name`.
pub(super) fn systemd_unit_path(name: &str) -> String {
    format!("{SYSTEMD_FOLDER}/{}.service", service_id(name))
}

/// The file of the launchd daemon of the service `name`.
pub(super) fn launchd_plist_path(name: &str) -> String {
    format!("{LAUNCHD_FOLDER}/{}.plist", service_id(name))
}

/// The file the launchd daemon of the service `name` appends its output and errors to.
pub(super) fn launchd_log_path(name: &str) -> String {
    format!("{LAUNCHD_LOG_FOLDER}/{}.log", service_id(name))
}

/// The systemd unit of `plan`: `User=` the owner, `WorkingDirectory=` and `TRUST_PROJECT` the
/// project folder, `ExecStart=` the binary, restarted 2 s after it ends as the runtime's own unit
/// is, started after the network is up and at every boot.
pub(super) fn systemd_unit(plan: &Plan, user: &str) -> String {
    let project = text(&plan.project);
    let mut unit = String::new();
    let _ = writeln!(unit, "[Unit]");
    let _ = writeln!(
        unit,
        "Description=truST service {} of {}",
        plan.name,
        systemd_specifiers(project)
    );
    let _ = writeln!(unit, "Wants=network-online.target");
    let _ = writeln!(unit, "After=network-online.target");
    let _ = writeln!(unit);
    let _ = writeln!(unit, "[Service]");
    let _ = writeln!(unit, "Type=simple");
    let _ = writeln!(unit, "User={user}");
    let _ = writeln!(unit, "WorkingDirectory={}", systemd_specifiers(project));
    let _ = writeln!(unit, "{}", systemd_project_line(&plan.project));
    let _ = writeln!(
        unit,
        "ExecStart={}",
        systemd_quoted(&systemd_specifiers(text(&plan.binary)).replace('$', "$$"))
    );
    let _ = writeln!(unit, "Restart=always");
    let _ = writeln!(unit, "RestartSec=2");
    let _ = writeln!(unit);
    let _ = writeln!(unit, "[Install]");
    let _ = writeln!(unit, "WantedBy=multi-user.target");
    unit
}

/// The line of a systemd unit that names the project folder `project` in the service's
/// environment, by which `install` knows a unit it wrote for that folder.
pub(super) fn systemd_project_line(project: &Path) -> String {
    let assignment = format!("TRUST_PROJECT={}", systemd_specifiers(text(project)));
    format!("Environment={}", systemd_quoted(&assignment))
}

/// The launchd daemon of `plan`: `ProgramArguments` the binary, `UserName` the owner,
/// `WorkingDirectory` and `TRUST_PROJECT` the project folder, started at load and kept alive,
/// restarted at most every 2 s, its output and errors appended to its log.
pub(super) fn launchd_plist(plan: &Plan, user: &str) -> String {
    let id = service_id(&plan.name);
    let log = xml(&launchd_log_path(&plan.name));
    let mut plist = String::new();
    plist.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    plist.push_str(
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    );
    plist.push_str("<plist version=\"1.0\">\n<dict>\n");
    let _ = writeln!(plist, "\t<key>Label</key>\n\t<string>{id}</string>");
    let _ = writeln!(
        plist,
        "\t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t</array>",
        xml(text(&plan.binary))
    );
    let _ = writeln!(
        plist,
        "\t<key>UserName</key>\n\t<string>{}</string>",
        xml(user)
    );
    let _ = writeln!(
        plist,
        "\t<key>WorkingDirectory</key>\n\t<string>{}</string>",
        xml(text(&plan.project))
    );
    let _ = writeln!(
        plist,
        "\t<key>EnvironmentVariables</key>\n\t<dict>\n{}\n\t</dict>",
        launchd_project_entry(&plan.project)
    );
    plist.push_str("\t<key>RunAtLoad</key>\n\t<true/>\n");
    plist.push_str("\t<key>KeepAlive</key>\n\t<true/>\n");
    plist.push_str("\t<key>ThrottleInterval</key>\n\t<integer>2</integer>\n");
    let _ = writeln!(
        plist,
        "\t<key>StandardOutPath</key>\n\t<string>{log}</string>"
    );
    let _ = writeln!(
        plist,
        "\t<key>StandardErrorPath</key>\n\t<string>{log}</string>"
    );
    plist.push_str("</dict>\n</plist>\n");
    plist
}

/// The entry of a launchd daemon's environment that names the project folder `project`, by which
/// `install` knows a daemon it wrote for that folder.
pub(super) fn launchd_project_entry(project: &Path) -> String {
    format!(
        "\t\t<key>TRUST_PROJECT</key>\n\t\t<string>{}</string>",
        xml(text(project))
    )
}

/// The path as text; `install` refuses a path that is not UTF-8 before it writes a definition.
fn text(path: &Path) -> &str {
    path.to_str().unwrap_or_default()
}

/// `value` with systemd's specifier character doubled, so that `%` stays itself.
fn systemd_specifiers(value: &str) -> String {
    value.replace('%', "%%")
}

/// `value` in double quotes, with a backslash and a quote escaped, as systemd unquotes a setting.
fn systemd_quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `value` with the characters XML gives a meaning escaped.
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "definition_tests.rs"]
mod tests;
