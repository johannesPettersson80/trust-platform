//! The command line of `trust-control-studio` (specification 35, §0.1b). Each verb calls the same
//! function as its method (§0.1a), so it adds no rule and drops none. Readable text goes to standard
//! output; with `--json` the answer is the object that the method answers. A refusal goes to
//! standard error, and with `--json` its failure object goes to standard output as well. A verb
//! that starts an operation (`identify`, `tune`, `validate`, `cancel`) follows it to its final
//! update, each progress update one line on standard error.

mod application;
mod application_output;
mod apply;
mod cancel;
mod follow;
mod identify;
mod keep;
mod output;
mod restore;
mod setup;
mod tune;
mod validate;

pub use application::ReviewArgs;
pub use setup::SetupArgs;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{json, Value};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::AcquisitionMethod;
use trust_control::messages::{OpenRequest, TuneChoice, TuneMethod};
use trust_control::tuning::structure::Structure;
use trust_runtime_client::ControlEndpoint;

use crate::commands;
use crate::interrupt::Presses;
use crate::serve;
use crate::settings::{self, Environment, Platform};

/// The runtime the verbs name for a project's loops: the simulator, which the extension selects
/// when no other runtime is chosen. Section 0.1b does not yet say how the command line names the
/// runtime of a physical target.
pub const SELECTED_RUNTIME: &str = "simulator";

/// The command line: `serve`, or one of the verbs.
#[derive(Debug, Parser)]
#[command(
    name = "trust-control-studio",
    version,
    about = "Control Studio's engine: `serve` for the extension, and its verbs from a terminal, a script or CI."
)]
pub struct CommandLine {
    /// The verb to run.
    #[command(subcommand)]
    pub verb: Verb,
}

/// The verbs of the command line.
#[derive(Debug, Subcommand)]
pub enum Verb {
    /// Serves the engine's methods as JSON-RPC 2.0 on standard input and output, until standard input ends.
    Serve,
    /// Lists the project's loops, or opens one with --loop: its stages and its journaled operations.
    Open(OpenArgs),
    /// Identifies the loop's plant: a live campaign, a kept recording fitted again, the journaled campaign followed to
    /// its end, or a new working identification study.
    Identify(IdentifyArgs),
    /// Cancels the loop's journaled operation: a running experiment ends through its restoration path, and the journal
    /// is cleared once the outcome is kept.
    Cancel(CancelArgs),
    /// Designs the loop's controller candidate from its study's model: the loop's method, or the one named, against the
    /// requirements (§0.1b). It never writes to the controller.
    Tune(TuneArgs),
    /// Validates the loop's candidate (§0.1b; B5): the independent model check when the model is provisional, then the
    /// actual ST in isolated simulations, with each requirement's result. It never writes to the controller.
    Validate(ValidateArgs),
    /// Sets up a new loop (§0.1b; B1.3): without --draft, what the compiled project offers; with --draft <file>, writes the
    /// confirmed loop definitions to control-loops/ (all or none) and prints their ids.
    Setup(SetupArgs),
    /// Applies the validated candidate (§0.1b; B6.1 to B6.3): without --review, prints the review and its identity; with it,
    /// applies the reviewed change and prints the assessed result. A physical target is refused here (A2.5).
    Apply(ReviewArgs),
    /// Restores the settings from before Apply (§0.1b; B6.4), by the same review and confirmation as apply.
    Restore(ReviewArgs),
    /// Keeps the running values in the project as the record's initial values (§0.1b; B6.5), by the same review and
    /// confirmation as apply.
    Keep(ReviewArgs),
}

/// The arguments that the verbs share.
#[derive(Debug, Args)]
pub struct Common {
    /// The project folder (default: the current folder).
    #[arg(long, value_name = "FOLDER")]
    pub project: Option<PathBuf>,
    /// The runtime's control endpoint, `tcp://host:port` or `unix:///path` (default: the project's runtime.toml).
    #[arg(long, value_name = "ENDPOINT", value_parser = parse_endpoint)]
    pub endpoint: Option<String>,
    /// The file that holds the runtime's control token. Its one trailing line break is not part of it.
    #[arg(long, value_name = "FILE")]
    pub token_file: Option<PathBuf>,
    /// The folder of the operation journals (default: truST's folder in the user's local state folder).
    #[arg(long, value_name = "FOLDER")]
    pub journal_dir: Option<PathBuf>,
    /// Prints the answer as JSON: the object that the method answers.
    #[arg(long)]
    pub json: bool,
}

/// The arguments of `open`.
#[derive(Debug, Args)]
pub struct OpenArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop to open. Without it, the project's loops are listed.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: Option<String>,
}

/// The arguments of `identify`. `--method`, `--recording`, `--resume` and `--reset` exclude each
/// other; without any of them, `identify` runs a live campaign with the loop's own method (§0.1b): the one the loop
/// declares, or Automatic when it declares none.
#[derive(Debug, Args)]
pub struct IdentifyArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop whose plant is identified.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: String,
    /// The method of the live campaign (default: automatic).
    #[arg(
        long,
        value_enum,
        value_name = "METHOD",
        conflicts_with_all = ["recording", "resume", "reset"]
    )]
    pub method: Option<MethodWord>,
    /// Fits a kept recording of the study again, without live excitation.
    #[arg(long, value_name = "ID", conflicts_with_all = ["resume", "reset"])]
    pub recording: Option<String>,
    /// Follows the journaled identification campaign to its end.
    #[arg(long, conflicts_with = "reset")]
    pub resume: bool,
    /// Starts a new working identification study; the previous one is kept as history.
    #[arg(long)]
    pub reset: bool,
}

/// The method words of `identify --method` (D8; B3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MethodWord {
    /// Identify chooses the method from the loop's mode and policy.
    Automatic,
    /// A step of the plant input, in Manual.
    Step,
    /// A pulse of the plant input, in Manual.
    Pulse,
    /// A step of the setpoint, in Auto.
    SetpointStep,
    /// A feedback relay.
    Relay,
    /// A pseudo-random binary sequence.
    Prbs,
    /// A multisine.
    Multisine,
}

impl MethodWord {
    /// The acquisition method that the word names.
    #[must_use]
    pub const fn method(self) -> AcquisitionMethod {
        match self {
            Self::Automatic => AcquisitionMethod::Automatic,
            Self::Step => AcquisitionMethod::MvStep,
            Self::Pulse => AcquisitionMethod::MvPulse,
            Self::SetpointStep => AcquisitionMethod::SetpointStep,
            Self::Relay => AcquisitionMethod::Relay,
            Self::Prbs => AcquisitionMethod::Prbs,
            Self::Multisine => AcquisitionMethod::Multisine,
        }
    }
}

/// What `identify` is asked to do (§0.1b). The parser lets at most one of its action arguments
/// through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Requested<'a> {
    /// A live campaign with this method (B3.4).
    Live(AcquisitionMethod),
    /// A fit of the kept recording of this identifier, without excitation (B3.6).
    Recording(&'a str),
    /// The journaled campaign, followed to its end (A8.5).
    Resume,
    /// A new working identification study (A8.4).
    Reset,
}

impl IdentifyArgs {
    /// What the arguments ask for: without an action argument, a live campaign with the method
    /// named, or the loop's own method (§0.1b).
    fn requested(&self) -> Requested<'_> {
        if let Some(recording) = &self.recording {
            Requested::Recording(recording)
        } else if self.resume {
            Requested::Resume
        } else if self.reset {
            Requested::Reset
        } else {
            Requested::Live(
                self.method
                    .map_or(AcquisitionMethod::Configured, MethodWord::method),
            )
        }
    }
}

/// The arguments of `cancel`.
#[derive(Debug, Args)]
pub struct CancelArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop whose journaled operation is cancelled.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: String,
    /// The journaled operation to cancel, as `open --loop` lists it; needed only when the loop's journal holds more
    /// than one.
    pub operation_id: Option<String>,
}

/// The arguments of `tune` (§0.1b; B4). Tune designs on the study's model and never reaches the runtime (B4.1), so
/// `--endpoint` and `--token-file` are accepted with the other common arguments and are not used.
#[derive(Debug, Args)]
pub struct TuneArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop whose controller is designed.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: String,
    /// The tuning method (default: automatic).
    #[arg(long, value_enum, value_name = "METHOD")]
    pub method: Option<TuneMethodWord>,
    /// The response of a formula method.
    #[arg(long, value_enum, value_name = "RESPONSE")]
    pub choice: Option<TuneChoiceWord>,
    /// Fixes the controller structure.
    #[arg(long, value_enum, value_name = "STRUCTURE")]
    pub structure: Option<TuneStructureWord>,
    /// A setting the engineer enters for the manual method, as SYMBOL=VALUE; repeat it for each setting (B4.2).
    #[arg(long, value_name = "SYMBOL=VALUE", value_parser = parse_setting)]
    pub setting: Vec<(String, f64)>,
}

/// The arguments of `validate` (§0.1b; B5). Validate takes no method and no action: it validates the loop's current candidate,
/// and it reaches the runtime for its model check and its isolated simulations, so it names the runtime as `identify` does.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop whose candidate is validated.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: String,
}

/// The method words of `tune --method` (B4.2; N5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TuneMethodWord {
    /// Tune recommends the simplest structure that meets every mandatory requirement (B4.4).
    Automatic,
    /// The SIMC formula of Skogestad (N5.2, N5.3).
    Simc,
    /// The AMIGO formula of Åström and Hägglund (N5.5).
    Amigo,
    /// The relay rule (N5.R), for a model fitted from a qualified Feedback relay record of the study.
    Relay,
    /// The settings that `--setting` enters (B4.2).
    Manual,
}

impl TuneMethodWord {
    /// The method that the word names; Automatic names none.
    #[must_use]
    pub const fn method(self) -> Option<TuneMethod> {
        match self {
            Self::Automatic => None,
            Self::Simc => Some(TuneMethod::Simc),
            Self::Amigo => Some(TuneMethod::Amigo),
            Self::Relay => Some(TuneMethod::Relay),
            Self::Manual => Some(TuneMethod::Manual),
        }
    }
}

/// The response words of `tune --choice` (B4.2; N5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TuneChoiceWord {
    /// The Faster closed-loop time.
    Faster,
    /// Twice the Faster closed-loop time.
    Balanced,
    /// Four times the Faster closed-loop time.
    #[value(name = "more-robust")]
    MoreRobust,
}

impl TuneChoiceWord {
    /// The declared response that the word names.
    #[must_use]
    pub const fn choice(self) -> TuneChoice {
        match self {
            Self::Faster => TuneChoice::Faster,
            Self::Balanced => TuneChoice::Balanced,
            Self::MoreRobust => TuneChoice::MoreRobust,
        }
    }
}

/// The structure words of `tune --structure` (B4.4; C0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TuneStructureWord {
    /// Proportional only.
    #[value(name = "p")]
    P,
    /// Proportional and integral.
    #[value(name = "pi")]
    Pi,
    /// Proportional and derivative.
    #[value(name = "pd")]
    Pd,
    /// Proportional, integral and derivative.
    #[value(name = "pid")]
    Pid,
}

impl TuneStructureWord {
    /// The structure that the word names.
    #[must_use]
    pub const fn structure(self) -> Structure {
        match self {
            Self::P => Structure::P,
            Self::Pi => Structure::PI,
            Self::Pd => Structure::PD,
            Self::Pid => Structure::PID,
        }
    }
}

/// `--endpoint` takes a `tcp://` or `unix://` endpoint, as the runtime protocol names them.
fn parse_endpoint(text: &str) -> Result<String, String> {
    match ControlEndpoint::parse(text) {
        Some(_) => Ok(text.to_string()),
        None => Err("expected tcp://host:port or unix:///absolute/path".to_string()),
    }
}

/// `--setting` takes `SYMBOL=VALUE`: a controller setting's record symbol and its number (B4.2). A malformed setting is a usage
/// error; which symbols a loop has, and whether the method takes them, is for trust-control to decide.
fn parse_setting(text: &str) -> Result<(String, f64), String> {
    const SHAPE: &str = "expected SYMBOL=VALUE with a number, for example Kp=2.5";
    let (symbol, value) = text.split_once('=').ok_or(SHAPE)?;
    if symbol.is_empty() {
        return Err(SHAPE.to_string());
    }
    let number = value.parse::<f64>().map_err(|_| SHAPE.to_string())?;
    Ok((symbol.to_string(), number))
}

/// The process status of a verb (§0.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The verb finished with its result (0).
    Finished,
    /// The verb was refused or failed (1).
    Failed,
    /// A usage error: an unknown verb, or a missing or invalid argument (2).
    Usage,
    /// An interruption ended the verb (130).
    Interrupted,
}

impl Exit {
    /// The process status number.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Finished => 0,
            Self::Failed => 1,
            Self::Usage => 2,
            Self::Interrupted => 130,
        }
    }
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit.code())
    }
}

/// Runs the command line. `arguments` begin with the program's name, `environment` reads the
/// environment, and the verbs write their answers to `stdout` and their messages to `stderr`.
/// `serve` reads and writes the standard streams itself.
pub fn run<I, T>(
    arguments: I,
    environment: Environment<'_>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let line = match CommandLine::try_parse_from(arguments) {
        Ok(line) => line,
        Err(error) => {
            // Help and version are answers, and usage errors are not. clap prints either one.
            let _ = error.print();
            return if error.exit_code() == 0 {
                Exit::Finished
            } else {
                Exit::Usage
            };
        }
    };
    match line.verb {
        Verb::Serve => serve_standard_streams(),
        Verb::Open(args) => watched(stderr, |stderr| open(&args, environment, stdout, stderr)),
        Verb::Identify(args) => following(stderr, |presses, stderr| {
            identify::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Cancel(args) => following(stderr, |presses, stderr| {
            cancel::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Tune(args) => following(stderr, |presses, stderr| {
            tune::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Validate(args) => following(stderr, |presses, stderr| {
            validate::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Apply(args) => following(stderr, |presses, stderr| {
            apply::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Restore(args) => following(stderr, |presses, stderr| {
            restore::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Keep(args) => following(stderr, |presses, stderr| {
            keep::run(&args, environment, presses, stdout, stderr)
        }),
        Verb::Setup(args) => watched(stderr, |stderr| {
            setup::run(&args, environment, stdout, stderr)
        }),
    }
}

/// Runs a verb that answers at once, with Ctrl-C counted while it runs (§0.1b).
fn watched(stderr: &mut dyn Write, verb: impl FnOnce(&mut dyn Write) -> Exit) -> Exit {
    following(stderr, |presses, stderr| {
        settle(verb(stderr), presses.count())
    })
}

/// Runs a verb with Ctrl-C counted for it (§0.1b). A verb that follows an operation reads the
/// presses while it waits for the operation's final update.
fn following(
    stderr: &mut dyn Write,
    verb: impl FnOnce(&mut Presses, &mut dyn Write) -> Exit,
) -> Exit {
    match Presses::register() {
        Ok(mut presses) => verb(&mut presses, stderr),
        Err(error) => {
            let _ = writeln!(stderr, "Ctrl-C cannot be watched: {error}.");
            Exit::Failed
        }
    }
}

/// A verb that answers at once still ends with status 130 when Ctrl-C was pressed while it ran.
fn settle(exit: Exit, presses: u32) -> Exit {
    if presses > 0 {
        Exit::Interrupted
    } else {
        exit
    }
}

/// `serve`: the engine's methods on the standard streams, until standard input ends.
fn serve_standard_streams() -> Exit {
    match serve::serve(std::io::stdin().lock(), std::io::stdout()) {
        Ok(()) => Exit::Finished,
        Err(error) => {
            eprintln!("{error}");
            Exit::Failed
        }
    }
}

/// `open`: lists the project's loops, or opens one with its stages and journaled operations. It
/// starts no test, tuning, validation or write (A8.6).
fn open(
    args: &OpenArgs,
    environment: Environment<'_>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let answer = places(&args.common, environment).and_then(|places| {
        commands::open(&OpenRequest {
            project_root: text_of(&places.project),
            runtime_id: SELECTED_RUNTIME.to_string(),
            loop_id: args.loop_id.clone(),
            storage_folder: text_of(&places.journal),
        })
    });
    answer_of(answer, args.common.json, output::open_text, stdout, stderr)
}

/// Writes an answer: its JSON object with `--json`, else its text. A refusal is its failure on
/// standard error, and with `--json` its failure object on standard output as well.
fn answer_of<T: Serialize>(
    answer: Result<T, Failure>,
    as_json: bool,
    text: impl FnOnce(&T) -> String,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    match answer {
        Ok(result) => {
            let shown = if as_json {
                json!({ "result": result }).to_string()
            } else {
                text(&result)
            };
            let _ = writeln!(stdout, "{shown}");
            Exit::Finished
        }
        Err(failure) => {
            if as_json {
                let _ = writeln!(stdout, "{}", json!({ "failure": failure }));
            }
            let _ = writeln!(stderr, "{}", output::failure_text(&failure));
            Exit::Failed
        }
    }
}

/// Writes an answer that a method gave as JSON, as [`answer_of`] writes a typed one: with `--json`
/// the object itself, else the text of its result; a `{"failure": …}` is its failure's text on
/// standard error, and with `--json` the object on standard output as well.
fn json_answer_of(
    answer: &Value,
    as_json: bool,
    text: impl FnOnce(&Value) -> String,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    if let Some(failure) = answer.get("failure") {
        if as_json {
            let _ = writeln!(stdout, "{answer}");
        }
        let _ = writeln!(stderr, "{}", output::answered_failure_text(failure));
        return Exit::Failed;
    }
    let shown = if as_json {
        answer.to_string()
    } else {
        text(answer.get("result").unwrap_or(&Value::Null))
    };
    let _ = writeln!(stdout, "{shown}");
    Exit::Finished
}

/// Writes a refusal that came before any operation started, as [`answer_of`] writes one, with the
/// status of a verb that answered at once.
fn refused(
    failure: Failure,
    as_json: bool,
    presses: &mut Presses,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let exit = answer_of::<Value>(Err(failure), as_json, |_| String::new(), stdout, stderr);
    settle(exit, presses.count())
}

/// The folders that a verb works in.
struct Places {
    project: PathBuf,
    journal: PathBuf,
}

/// The project folder (the current folder by default) and the journal folder (`--journal-dir`, else
/// the platform's folder).
fn places(common: &Common, environment: Environment<'_>) -> Result<Places, Failure> {
    let project = match &common.project {
        Some(project) => project.clone(),
        None => std::env::current_dir().map_err(|error| {
            Failure::new(
                &format!("The current folder cannot be read: {error}."),
                Cause::Settings,
                NextAction::None,
            )
        })?,
    };
    let journal = match &common.journal_dir {
        Some(journal) => journal.clone(),
        None => settings::default_journal_folder(Platform::current(), environment)?,
    };
    Ok(Places { project, journal })
}

fn text_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

    /// The arguments of `identify --loop tank` followed by `extra`, as the parser reads them.
    fn identify_args(extra: &[&str]) -> IdentifyArgs {
        let arguments = ["trust-control-studio", "identify", "--loop", "tank"]
            .into_iter()
            .chain(extra.iter().copied());
        let Verb::Identify(args) = CommandLine::try_parse_from(arguments)
            .expect("accepted")
            .verb
        else {
            panic!("identify is read as identify");
        };
        args
    }

    /// The arguments of `tune --loop tank` followed by `extra`, as the parser reads them.
    fn tune_args(extra: &[&str]) -> TuneArgs {
        let arguments = ["trust-control-studio", "tune", "--loop", "tank"]
            .into_iter()
            .chain(extra.iter().copied());
        let Verb::Tune(args) = CommandLine::try_parse_from(arguments)
            .expect("accepted")
            .verb
        else {
            panic!("tune is read as tune");
        };
        args
    }

    #[test]
    fn each_outcome_has_its_process_status() {
        assert_eq!(
            [Exit::Finished, Exit::Failed, Exit::Usage, Exit::Interrupted].map(Exit::code),
            [0, 1, 2, 130]
        );
    }

    #[test]
    fn a_press_ends_a_verb_that_answers_at_once_with_status_130() {
        assert_eq!(settle(Exit::Finished, 0), Exit::Finished);
        assert_eq!(settle(Exit::Failed, 0), Exit::Failed);
        assert_eq!(settle(Exit::Usage, 0), Exit::Usage);
        assert_eq!(settle(Exit::Finished, 1), Exit::Interrupted);
        assert_eq!(settle(Exit::Failed, 2), Exit::Interrupted);
    }

    #[test]
    fn a_usage_error_is_refused_by_the_parser_with_status_2() {
        for arguments in [
            vec!["trust-control-studio", "frobnicate"],
            vec!["trust-control-studio", "open", "--token", "secret"],
            vec![
                "trust-control-studio",
                "open",
                "--endpoint",
                "http://runtime",
            ],
            vec![
                "trust-control-studio",
                "cancel",
                "--loop",
                "tank",
                "--journal-dir",
            ],
            vec!["trust-control-studio", "identify", "--method", "step"],
            vec![
                "trust-control-studio",
                "identify",
                "--loop",
                "tank",
                "--token",
                "secret",
            ],
        ] {
            let error = CommandLine::try_parse_from(arguments.clone()).expect_err("a usage error");
            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }

    #[test]
    fn cancel_names_its_operation_optionally_but_needs_its_loop() {
        assert!(CommandLine::try_parse_from([
            "trust-control-studio",
            "cancel",
            "--loop",
            "tank",
            "operation-1",
        ])
        .is_ok());
        assert!(
            CommandLine::try_parse_from(["trust-control-studio", "cancel", "--loop", "tank",])
                .is_ok()
        );
        let missing_loop =
            CommandLine::try_parse_from(["trust-control-studio", "cancel", "operation-1"])
                .expect_err("the loop is required");
        assert_eq!(missing_loop.exit_code(), 2);
    }

    #[test]
    fn identify_takes_at_most_one_of_method_recording_resume_and_reset() {
        let actions: [&[&str]; 4] = [
            &["--method", "step"],
            &["--recording", "rec-1"],
            &["--resume"],
            &["--reset"],
        ];
        for (index, first) in actions.iter().enumerate() {
            for second in &actions[index + 1..] {
                let arguments: Vec<&str> = ["trust-control-studio", "identify", "--loop", "tank"]
                    .into_iter()
                    .chain(first.iter().copied())
                    .chain(second.iter().copied())
                    .collect();
                let error =
                    CommandLine::try_parse_from(arguments.clone()).expect_err("one action at most");
                assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{arguments:?}");
                assert_eq!(error.exit_code(), 2, "{arguments:?}");
            }
        }
    }

    #[test]
    fn identify_runs_live_with_the_loops_method_unless_an_action_or_a_method_is_named() {
        assert_eq!(
            identify_args(&[]).requested(),
            Requested::Live(AcquisitionMethod::Configured)
        );
        assert_eq!(
            identify_args(&["--method", "pulse"]).requested(),
            Requested::Live(AcquisitionMethod::MvPulse),
            "--method alone is accepted"
        );
        assert_eq!(
            identify_args(&["--recording", "rec-1"]).requested(),
            Requested::Recording("rec-1")
        );
        assert_eq!(identify_args(&["--resume"]).requested(), Requested::Resume);
        assert_eq!(identify_args(&["--reset"]).requested(), Requested::Reset);
    }

    #[test]
    fn every_method_word_names_its_acquisition_method() {
        let words = [
            ("automatic", AcquisitionMethod::Automatic),
            ("step", AcquisitionMethod::MvStep),
            ("pulse", AcquisitionMethod::MvPulse),
            ("setpoint-step", AcquisitionMethod::SetpointStep),
            ("relay", AcquisitionMethod::Relay),
            ("prbs", AcquisitionMethod::Prbs),
            ("multisine", AcquisitionMethod::Multisine),
        ];
        for (word, method) in words {
            assert_eq!(
                identify_args(&["--method", word]).requested(),
                Requested::Live(method),
                "{word}"
            );
        }
        assert_eq!(
            MethodWord::value_variants().len(),
            words.len(),
            "no other method word"
        );
    }

    #[test]
    fn an_unknown_method_word_is_a_usage_error() {
        for word in ["chirp", "Step", "configured", "mv_step"] {
            let error = CommandLine::try_parse_from([
                "trust-control-studio",
                "identify",
                "--loop",
                "tank",
                "--method",
                word,
            ])
            .expect_err("not a method word");
            assert_eq!(error.kind(), ErrorKind::InvalidValue, "{word}");
            assert_eq!(error.exit_code(), 2, "{word}");
        }
    }

    #[test]
    fn tune_needs_its_loop() {
        for arguments in [
            vec!["trust-control-studio", "tune"],
            vec!["trust-control-studio", "tune", "--method", "simc"],
            vec!["trust-control-studio", "tune", "--json"],
        ] {
            let error =
                CommandLine::try_parse_from(arguments.clone()).expect_err("the loop is required");
            assert_eq!(
                error.kind(),
                ErrorKind::MissingRequiredArgument,
                "{arguments:?}"
            );
            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }

    #[test]
    fn tune_reads_its_method_choice_and_structure_words_and_the_common_arguments() {
        let args = tune_args(&[
            "--method",
            "simc",
            "--choice",
            "more-robust",
            "--structure",
            "pi",
            "--json",
        ]);
        assert_eq!(args.loop_id, "tank");
        assert_eq!(args.method, Some(TuneMethodWord::Simc));
        assert_eq!(args.choice, Some(TuneChoiceWord::MoreRobust));
        assert_eq!(args.structure, Some(TuneStructureWord::Pi));
        assert!(args.common.json);
        let bare = tune_args(&[]);
        assert_eq!(
            (bare.method, bare.choice, bare.structure),
            (None, None, None),
            "every word is optional"
        );
        assert!(!bare.common.json);
        let common = tune_args(&[
            "--project",
            "/project",
            "--endpoint",
            "tcp://127.0.0.1:1",
            "--token-file",
            "/token",
            "--journal-dir",
            "/journal",
        ])
        .common;
        assert_eq!(
            (
                common.project,
                common.endpoint,
                common.token_file,
                common.journal_dir
            ),
            (
                Some(PathBuf::from("/project")),
                Some("tcp://127.0.0.1:1".to_string()),
                Some(PathBuf::from("/token")),
                Some(PathBuf::from("/journal"))
            ),
            "the common arguments are accepted, though Tune never reaches the runtime"
        );
    }

    #[test]
    fn every_tune_word_names_its_method_response_and_structure() {
        let methods = [
            ("automatic", None),
            ("simc", Some(TuneMethod::Simc)),
            ("amigo", Some(TuneMethod::Amigo)),
            ("relay", Some(TuneMethod::Relay)),
            ("manual", Some(TuneMethod::Manual)),
        ];
        for (word, method) in methods {
            assert_eq!(
                tune_args(&["--method", word])
                    .method
                    .and_then(TuneMethodWord::method),
                method,
                "{word}"
            );
        }
        assert_eq!(
            TuneMethodWord::value_variants().len(),
            methods.len(),
            "no other method word"
        );
        let choices = [
            ("faster", TuneChoice::Faster),
            ("balanced", TuneChoice::Balanced),
            ("more-robust", TuneChoice::MoreRobust),
        ];
        assert_eq!(
            TuneChoiceWord::value_variants().len(),
            choices.len(),
            "no other response word"
        );
        for (word, choice) in choices {
            assert_eq!(
                tune_args(&["--choice", word])
                    .choice
                    .map(TuneChoiceWord::choice),
                Some(choice),
                "{word}"
            );
        }
        let structures = [
            ("p", Structure::P),
            ("pi", Structure::PI),
            ("pd", Structure::PD),
            ("pid", Structure::PID),
        ];
        for (word, structure) in structures {
            assert_eq!(
                tune_args(&["--structure", word])
                    .structure
                    .map(TuneStructureWord::structure),
                Some(structure),
                "{word}"
            );
        }
        assert_eq!(
            TuneStructureWord::value_variants().len(),
            structures.len(),
            "no other structure word"
        );
    }

    #[test]
    fn tune_takes_the_relay_and_manual_words_and_each_setting_as_a_symbol_and_a_number() {
        assert_eq!(
            tune_args(&["--method", "relay"]).method,
            Some(TuneMethodWord::Relay)
        );
        let manual = tune_args(&[
            "--method",
            "manual",
            "--setting",
            "Kp=2.5",
            "--setting",
            "Ki=0.5",
        ]);
        assert_eq!(manual.method, Some(TuneMethodWord::Manual));
        assert_eq!(
            manual.setting,
            vec![("Kp".to_string(), 2.5), ("Ki".to_string(), 0.5)]
        );
        assert!(
            tune_args(&[]).setting.is_empty(),
            "no setting is named by default"
        );
    }

    #[test]
    fn a_setting_without_an_equals_sign_a_symbol_or_a_number_is_a_usage_error() {
        for setting in ["Kp", "=2.5", "Kp=", "Kp=two", "Kp=2,5"] {
            let error = CommandLine::try_parse_from([
                "trust-control-studio",
                "tune",
                "--loop",
                "tank",
                "--setting",
                setting,
            ])
            .expect_err("not SYMBOL=VALUE with a number");
            assert_eq!(error.kind(), ErrorKind::ValueValidation, "{setting}");
            assert_eq!(error.exit_code(), 2, "{setting}");
        }
    }

    #[test]
    fn an_unknown_tune_word_is_a_usage_error() {
        for (option, word) in [
            ("--method", "ziegler-nichols"),
            ("--method", "SIMC"),
            ("--choice", "fastest"),
            ("--choice", "Balanced"),
            ("--choice", "more_robust"),
            ("--structure", "i"),
            ("--structure", "PI"),
        ] {
            let error = CommandLine::try_parse_from([
                "trust-control-studio",
                "tune",
                "--loop",
                "tank",
                option,
                word,
            ])
            .expect_err("not a word of the option");
            assert_eq!(error.kind(), ErrorKind::InvalidValue, "{option} {word}");
            assert_eq!(error.exit_code(), 2, "{option} {word}");
        }
    }
}
