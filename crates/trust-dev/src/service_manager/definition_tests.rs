//! The definitions of specification 48 RU-64, written out by hand from it: the systemd unit and the
//! launchd daemon of a service, with the characters each format gives a meaning escaped, and each
//! read by its service manager's own checker where the computer has it.

use std::path::PathBuf;

use super::super::plan::Plan;
use super::*;

/// The plan of the service recipe of the project folder `project`, running `binary`.
fn plan(project: &str, binary: &str) -> Plan {
    Plan {
        name: "recipe".into(),
        project: PathBuf::from(project),
        binary: PathBuf::from(binary),
    }
}

#[test]
fn the_unit_runs_the_binary_as_the_owner_in_the_project_and_restarts_it() {
    // RU-64's Linux unit, setting by setting: the owner, the project folder as the working folder
    // and as TRUST_PROJECT, the binary, Restart=always after 2 s as the runtime's own unit, after
    // and wanting the network, at every boot. Catches a unit that runs as root (no User=), one
    // whose service cannot find its trust.toml (no TRUST_PROJECT), and one that stays down after
    // the service ends or after a boot.
    let unit = systemd_unit(
        &plan(
            "/home/ada/plant",
            "/home/ada/plant/recipe-service/target/release/recipe-service",
        ),
        "ada",
    );
    assert_eq!(
        unit,
        r#"[Unit]
Description=truST service recipe of /home/ada/plant
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=ada
WorkingDirectory=/home/ada/plant
Environment="TRUST_PROJECT=/home/ada/plant"
ExecStart="/home/ada/plant/recipe-service/target/release/recipe-service"
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
"#
    );
}

#[test]
fn the_unit_keeps_a_space_a_percent_a_dollar_a_quote_and_a_backslash_of_a_path() {
    // systemd expands `%` specifiers in every setting and `$` variables in ExecStart=, and unquotes
    // "..." with C escapes in ExecStart= and Environment=; WorkingDirectory= takes the rest of its
    // line as it is. So `%` is doubled everywhere, `$` in ExecStart=, and `"` and `\` are escaped
    // inside quotes. Catches a unit that runs another program than the binary, or gives the service
    // another folder than its project.
    let unit = systemd_unit(
        &plan("/srv/my plant 100%", "/srv/my plant 100%/bin/a$b\"c\\d"),
        "ada",
    );
    let lines: Vec<&str> = unit.lines().collect();
    assert!(lines.contains(&"Description=truST service recipe of /srv/my plant 100%%"));
    assert!(lines.contains(&"WorkingDirectory=/srv/my plant 100%%"));
    assert!(lines.contains(&r#"Environment="TRUST_PROJECT=/srv/my plant 100%%""#));
    assert!(lines.contains(&r#"ExecStart="/srv/my plant 100%%/bin/a$$b\"c\\d""#));
}

#[test]
fn the_daemon_runs_the_binary_as_the_owner_in_the_project_keeps_it_alive_and_logs() {
    // RU-64's macOS daemon, key by key: the service's name as its label, the binary, the owner, the
    // project folder as the working folder and as TRUST_PROJECT, started at load, kept alive and
    // restarted at most every 2 s, its output and errors in its log. Catches a daemon that runs as
    // root (no UserName), one that stays down after the service ends, and one whose output is lost.
    let plist = launchd_plist(
        &plan(
            "/Users/ada/plant",
            "/Users/ada/plant/recipe-service/target/release/recipe-service",
        ),
        "ada",
    );
    assert_eq!(
        plist,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>trust-service-recipe</string>
	<key>ProgramArguments</key>
	<array>
		<string>/Users/ada/plant/recipe-service/target/release/recipe-service</string>
	</array>
	<key>UserName</key>
	<string>ada</string>
	<key>WorkingDirectory</key>
	<string>/Users/ada/plant</string>
	<key>EnvironmentVariables</key>
	<dict>
		<key>TRUST_PROJECT</key>
		<string>/Users/ada/plant</string>
	</dict>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<true/>
	<key>ThrottleInterval</key>
	<integer>2</integer>
	<key>StandardOutPath</key>
	<string>/Library/Logs/trust-service-recipe.log</string>
	<key>StandardErrorPath</key>
	<string>/Library/Logs/trust-service-recipe.log</string>
</dict>
</plist>
"#
    );
}

#[test]
fn the_daemon_escapes_the_characters_xml_gives_a_meaning() {
    // `&`, `<` and `>` in a path would end or break the plist's XML. Catches a daemon that launchd
    // cannot read, or that names another folder.
    let plist = launchd_plist(&plan("/Users/ada/R&D <plant>", "/Users/ada/bin/a>b"), "ada");
    assert!(plist.contains("\t<string>/Users/ada/R&amp;D &lt;plant&gt;</string>\n"));
    assert!(plist.contains("\t\t<string>/Users/ada/bin/a&gt;b</string>\n"));
    assert!(!plist.contains("R&D"));
}

#[test]
fn every_computer_names_the_service_alike() {
    // RU-64: the service is trust-service-<name> on every computer, in the unit's, the daemon's and
    // the log's file names. Catches two names for one service.
    assert_eq!(service_id("recipe"), "trust-service-recipe");
    assert_eq!(
        systemd_unit_path("recipe"),
        "/etc/systemd/system/trust-service-recipe.service"
    );
    assert_eq!(
        launchd_plist_path("recipe"),
        "/Library/LaunchDaemons/trust-service-recipe.plist"
    );
    assert_eq!(
        launchd_log_path("recipe"),
        "/Library/Logs/trust-service-recipe.log"
    );
}

/// A project folder with a binary that runs, for the service managers' own checkers.
#[cfg(unix)]
fn project_with_binary() -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let project = trust_test_support::unique_temp_path("trust-dev-unit-check");
    std::fs::create_dir_all(&project).expect("the project folder");
    let binary = project.join("recipe-service");
    std::fs::write(&binary, "#!/bin/sh\nexit 0\n").expect("the binary");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
        .expect("the binary's mode");
    (project, binary)
}

/// The name of the user who runs the tests.
#[cfg(unix)]
fn user() -> String {
    let output = std::process::Command::new("id")
        .arg("-un")
        .output()
        .expect("id");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(target_os = "linux")]
#[test]
fn systemd_reads_the_unit_without_a_complaint() {
    // systemd's own reader, `systemd-analyze verify`, on the unit of a real project folder and
    // binary run as the user who runs the tests: it names an unknown setting, a bad quote and a
    // binary it cannot run. Catches a unit that only this file's expectations accept. A computer
    // without systemd (a container) has no reader, and says so.
    let (project, binary) = project_with_binary();
    let unit = systemd_unit(
        &plan(
            project.to_str().expect("a UTF-8 path"),
            binary.to_str().expect("a UTF-8 path"),
        ),
        &user(),
    );
    let file = project.join("trust-service-recipe.service");
    std::fs::write(&file, unit).expect("the unit");
    let Ok(output) = std::process::Command::new("systemd-analyze")
        .arg("verify")
        .arg(&file)
        .output()
    else {
        eprintln!("systemd-analyze is not on this computer: the unit is not read by systemd");
        return;
    };
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let about_the_unit: Vec<&str> = report
        .lines()
        .filter(|line| line.contains("trust-service-recipe"))
        .collect();
    assert!(
        output.status.success() && about_the_unit.is_empty(),
        "systemd-analyze verify: {}\n{report}",
        output.status
    );
}

#[cfg(target_os = "macos")]
#[test]
fn plutil_reads_the_daemon_without_a_complaint() {
    // macOS's own reader of property lists, `plutil -lint`. Catches a daemon file that only this
    // file's expectations accept.
    let (project, binary) = project_with_binary();
    let plist = launchd_plist(
        &plan(
            project.to_str().expect("a UTF-8 path"),
            binary.to_str().expect("a UTF-8 path"),
        ),
        &user(),
    );
    let file = project.join("trust-service-recipe.plist");
    std::fs::write(&file, plist).expect("the daemon");
    let output = std::process::Command::new("plutil")
        .arg("-lint")
        .arg(&file)
        .output()
        .expect("plutil");
    assert!(
        output.status.success(),
        "plutil -lint: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
