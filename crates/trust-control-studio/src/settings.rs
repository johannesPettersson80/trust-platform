//! The command line's settings (specification 35, §0.1b): the journal folder by the platform's
//! convention, the control token from the environment or from a file, and the runtime's endpoint
//! from `--endpoint` or from the project's `runtime.toml`. This module reads the environment and the
//! files it is given, and decides no Control Studio rule.

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::runtime::RuntimeConnection;
use trust_runtime_client::ControlEndpoint;

use crate::runtime_file;

/// The environment variable that holds the runtime's control token (§0.1b).
pub const TOKEN_VARIABLE: &str = "TRUST_CONTROL_TOKEN";

/// The environment, read by variable name.
pub type Environment<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The platforms whose folder convention the journal folder follows (§0.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux and the other Unix systems.
    Unix,
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
}

impl Platform {
    /// The platform this program was built for.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// truST's folder for the operation journals, by the platform's convention: `$XDG_STATE_HOME/trust`
/// (an absolute value only) or `$HOME/.local/state/trust` on Unix, `$HOME/Library/Application
/// Support/trust` on macOS, and `%LOCALAPPDATA%\trust` on Windows.
///
/// # Errors
/// The variable that the platform's folder starts from is not set.
pub fn default_journal_folder(
    platform: Platform,
    environment: Environment<'_>,
) -> Result<PathBuf, Failure> {
    let variable = |name: &str| {
        environment(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let folder = match platform {
        Platform::Unix => variable("XDG_STATE_HOME")
            .filter(|path| path.is_absolute())
            .or_else(|| variable("HOME").map(|home| home.join(".local").join("state"))),
        Platform::MacOs => {
            variable("HOME").map(|home| home.join("Library").join("Application Support"))
        }
        Platform::Windows => variable("LOCALAPPDATA"),
    };
    folder.map(|folder| folder.join("trust")).ok_or_else(|| {
        Failure::new(
            "The operation journal folder cannot be found: the environment does not give the user's state folder. Pass --journal-dir.",
            Cause::Settings,
            NextAction::None,
        )
    })
}

/// The runtime's control token: the file named by `--token-file` when there is one, else the
/// `TRUST_CONTROL_TOKEN` variable. The one trailing line break of the file is not part of the token,
/// and a blank token is no token. A token is never an argument (§0.1b).
///
/// # Errors
/// The token file cannot be read, or the variable is not text.
pub fn control_token(
    token_file: Option<&Path>,
    environment: Environment<'_>,
) -> Result<Option<String>, Failure> {
    if let Some(file) = token_file {
        let text = fs::read_to_string(file).map_err(|error| {
            Failure::new(
                &format!("The token file {} cannot be read: {error}.", file.display()),
                Cause::Settings,
                NextAction::None,
            )
        })?;
        return Ok(non_blank(strip_one_line_break(&text)));
    }
    match environment(TOKEN_VARIABLE) {
        None => Ok(None),
        Some(value) => value
            .into_string()
            .map(|text| non_blank(&text))
            .map_err(|_| {
                Failure::new(
                    &format!("{TOKEN_VARIABLE} is not text."),
                    Cause::Settings,
                    NextAction::None,
                )
            }),
    }
}

/// The runtime a live verb connects to: `--endpoint` when it is given, else the endpoint that the
/// project's `runtime.toml` configures. The control token comes from [`control_token`].
///
/// # Errors
/// No endpoint is given or configured, the endpoint is neither `tcp://` nor `unix://`, or the token
/// cannot be read.
pub fn runtime_connection(
    endpoint: Option<&str>,
    project: &Path,
    token_file: Option<&Path>,
    environment: Environment<'_>,
) -> Result<RuntimeConnection, Failure> {
    let endpoint = match endpoint {
        Some(endpoint) => endpoint.to_string(),
        None => configured_endpoint(project)?,
    };
    if ControlEndpoint::parse(&endpoint).is_none() {
        return Err(Failure::new(
            &format!(
                "{endpoint} is not a runtime endpoint. Use tcp://host:port or unix:///absolute/path."
            ),
            Cause::Settings,
            NextAction::ConnectRuntime,
        ));
    }
    Ok(RuntimeConnection {
        endpoint,
        token: control_token(token_file, environment)?,
    })
}

/// The endpoint that the project's `runtime.toml` configures.
fn configured_endpoint(project: &Path) -> Result<String, Failure> {
    let file = project.join("runtime.toml");
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(Failure::new(
                "This project has no runtime.toml, so it names no runtime. Pass --endpoint tcp://host:port.",
                Cause::Settings,
                NextAction::ConnectRuntime,
            ));
        }
        Err(error) => {
            return Err(Failure::new(
                &format!("{} cannot be read: {error}.", file.display()),
                Cause::Settings,
                NextAction::ConnectRuntime,
            ));
        }
    };
    runtime_file::control_endpoint(&text).ok_or_else(|| {
        Failure::new(
            "runtime.toml does not configure the control endpoint in [runtime.control]. Pass --endpoint tcp://host:port.",
            Cause::Settings,
            NextAction::ConnectRuntime,
        )
    })
}

/// `text` without its one trailing line break, `\n` or `\r\n`.
fn strip_one_line_break(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

/// The text as a token, or none when it is blank.
fn non_blank(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// A folder of the test, removed when the test ends, whether or not it passed.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> Scratch {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).expect("the scratch folder");
        Scratch(folder)
    }

    /// An environment with the given variables.
    fn environment(variables: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let variables: BTreeMap<String, OsString> = variables
            .iter()
            .map(|(name, value)| ((*name).to_string(), OsString::from(value)))
            .collect();
        move |name: &str| variables.get(name).cloned()
    }

    #[test]
    fn unix_uses_the_absolute_xdg_state_home_then_the_home_folder() {
        let state = environment(&[("XDG_STATE_HOME", "/state"), ("HOME", "/home/u")]);
        assert_eq!(
            default_journal_folder(Platform::Unix, &state).expect("found"),
            Path::new("/state").join("trust")
        );
        let relative = environment(&[("XDG_STATE_HOME", "state"), ("HOME", "/home/u")]);
        assert_eq!(
            default_journal_folder(Platform::Unix, &relative).expect("found"),
            Path::new("/home/u")
                .join(".local")
                .join("state")
                .join("trust"),
            "a relative XDG_STATE_HOME is ignored"
        );
        let home = environment(&[("HOME", "/home/u")]);
        assert_eq!(
            default_journal_folder(Platform::Unix, &home).expect("found"),
            Path::new("/home/u")
                .join(".local")
                .join("state")
                .join("trust")
        );
        let none =
            default_journal_folder(Platform::Unix, &environment(&[])).expect_err("no home folder");
        assert_eq!(none.next_action, NextAction::None);
    }

    #[test]
    fn macos_uses_the_application_support_folder_of_the_home_folder() {
        let home = environment(&[("HOME", "/Users/u"), ("XDG_STATE_HOME", "/state")]);
        assert_eq!(
            default_journal_folder(Platform::MacOs, &home).expect("found"),
            Path::new("/Users/u")
                .join("Library")
                .join("Application Support")
                .join("trust"),
            "macOS does not follow XDG_STATE_HOME"
        );
        assert!(default_journal_folder(Platform::MacOs, &environment(&[])).is_err());
    }

    #[test]
    fn windows_uses_the_local_application_data_folder() {
        let local = environment(&[("LOCALAPPDATA", "C:/Users/u/AppData/Local")]);
        assert_eq!(
            default_journal_folder(Platform::Windows, &local).expect("found"),
            Path::new("C:/Users/u/AppData/Local").join("trust")
        );
        assert!(default_journal_folder(Platform::Windows, &environment(&[])).is_err());
    }

    #[test]
    fn the_token_comes_from_the_file_or_the_environment_never_from_an_argument() {
        let folder = scratch("token");
        let file = folder.0.join("token.txt");
        let variable = environment(&[(TOKEN_VARIABLE, "from-environment")]);
        assert_eq!(
            control_token(None, &variable).expect("read"),
            Some("from-environment".to_string())
        );
        assert_eq!(
            control_token(None, &environment(&[])).expect("read"),
            None,
            "no variable is no token"
        );
        assert_eq!(
            control_token(None, &environment(&[(TOKEN_VARIABLE, "  ")])).expect("read"),
            None,
            "a blank token is no token"
        );
        fs::write(&file, "from-file\n").expect("the token file");
        assert_eq!(
            control_token(Some(file.as_path()), &variable).expect("read"),
            Some("from-file".to_string()),
            "the file wins, and its one trailing line break is not part of the token"
        );
        fs::write(&file, "crlf-token\r\n").expect("the token file");
        assert_eq!(
            control_token(Some(file.as_path()), &variable).expect("read"),
            Some("crlf-token".to_string())
        );
        let missing = folder.0.join("missing.txt");
        let failure = control_token(Some(missing.as_path()), &variable).expect_err("unreadable");
        assert_eq!(failure.next_action, NextAction::None);
    }

    #[test]
    fn the_endpoint_is_the_argument_or_the_projects_runtime_toml() {
        let folder = scratch("connection");
        let variables = environment(&[]);
        let given = runtime_connection(Some("tcp://127.0.0.1:5195"), &folder.0, None, &variables)
            .expect("the argument");
        assert_eq!(given.endpoint, "tcp://127.0.0.1:5195");
        assert_eq!(given.token, None);
        let missing =
            runtime_connection(None, &folder.0, None, &variables).expect_err("no runtime.toml");
        assert_eq!(missing.next_action, NextAction::ConnectRuntime);
        fs::write(
            folder.0.join("runtime.toml"),
            "[runtime.control]\nendpoint = \"tcp://127.0.0.1:5196\"\n",
        )
        .expect("runtime.toml");
        assert_eq!(
            runtime_connection(None, &folder.0, None, &variables)
                .expect("configured")
                .endpoint,
            "tcp://127.0.0.1:5196"
        );
        assert!(
            runtime_connection(Some("http://runtime"), &folder.0, None, &variables).is_err(),
            "only tcp:// and unix:// endpoints"
        );
    }
}
