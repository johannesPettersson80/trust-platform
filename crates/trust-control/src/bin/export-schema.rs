//! Writes the JSON Schema of every engine message type to one folder: `<Type>.json` for each type that a
//! `controlStudio/*` request or answer carries, the operation update and the failure (specification 35,
//! §0.1a; checklist V0.9). The extension's declarations are generated from these files by
//! `editors/vscode/scripts/generate-control-studio-types.js`.
//!
//! Usage: `cargo run -p trust-control --bin export-schema -- <folder>`.

use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use schemars::{JsonSchema, SchemaGenerator};
use trust_control::commissioning::application_records::{ApplyOutcome, RestoreOutcome};
use trust_control::failure::Failure;
use trust_control::identification::campaign_review::IdentifyReview;
use trust_control::messages::{
    ApplicationAnswer, ApplicationRequest, CancelResult, IdentifyRequest, IdentifyResult,
    InputsRequest, InputsResult, KeepRequest, KeepResult, KeepReviewAnswer, OpenRequest,
    OpenResult, OperationRequest, OperationStarted, OperationUpdate, PageRequest, PreviewRequest,
    ResetResult, RunRequest, RunResult, TuneRequest, TuneResult, ValidateRequest, ValidateResult,
};
use trust_control::setup::messages::{
    SetupDiscoverRequest, SetupDiscoverResult, SetupSaveRequest, SetupSaveResult,
};

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let [folder] = arguments.as_slice() else {
        eprintln!("usage: export-schema <folder>");
        return ExitCode::from(2);
    };
    match export(Path::new(folder)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("export-schema: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Writes the schema of each named type, as `<TypeName>.json`, into the folder.
macro_rules! write_all {
    ($folder:expr, $($message:ident),+ $(,)?) => {
        $( write_schema::<$message>($folder, stringify!($message))?; )+
    };
}

/// Writes the schemas of every engine message type into `folder`, which is created when it does not exist.
///
/// # Errors
/// The folder cannot be created, or a schema file cannot be written.
fn export(folder: &Path) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    write_all!(
        folder,
        OpenRequest,
        OpenResult,
        OperationRequest,
        OperationUpdate,
        OperationStarted,
        IdentifyRequest,
        IdentifyReview,
        IdentifyResult,
        ResetResult,
        CancelResult,
        PageRequest,
        InputsRequest,
        InputsResult,
        RunRequest,
        RunResult,
        TuneRequest,
        TuneResult,
        PreviewRequest,
        ValidateRequest,
        ValidateResult,
        ApplicationRequest,
        ApplicationAnswer,
        ApplyOutcome,
        RestoreOutcome,
        KeepRequest,
        KeepReviewAnswer,
        KeepResult,
        SetupDiscoverRequest,
        SetupDiscoverResult,
        SetupSaveRequest,
        SetupSaveResult,
        Failure,
    );
    Ok(())
}

/// Writes the schema of `T` as `<name>.json` in `folder`, pretty-printed with a final newline. The generator
/// runs on the same schema for every build, so the output is deterministic.
fn write_schema<T: JsonSchema>(folder: &Path, name: &str) -> io::Result<()> {
    let schema = SchemaGenerator::default().into_root_schema_for::<T>();
    let text = serde_json::to_string_pretty(&schema).map_err(io::Error::other)?;
    fs::write(folder.join(format!("{name}.json")), format!("{text}\n"))
}
