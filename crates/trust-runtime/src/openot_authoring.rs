//! OpenOT attribute authoring support.
//!
//! This module implements the first compiler-side lowering target for
//! declaration-adjacent `{attribute 'oot' := ...}` pragmas. The user source
//! remains pure ST; the compile session instruments hidden `OPENOT_Producer`
//! calls before bytecode is built.

use crate::harness::SourceFile;
use trust_hir::openot_authoring as hir_openot;

const DEFAULT_SOURCE_ID: u32 = 1;
const DEFAULT_STATE_CATEGORY: u16 = hir_openot::STATE_CATEGORY_PROCESS;
const ST_PRODUCER_MAX_RECORD_SIZE: u16 = 256;
const SAMPLING_MODE_DEFAULT: u16 = 0;
const SAMPLING_MODE_PERIODIC: u16 = 1;
const SAMPLING_MODE_HYSTERESIS: u16 = 2;
const PRODUCER_NAME: &str = "OotProducer";
const USE_SOURCE_TIME_NAME: &str = "OotUseSourceTimeInput";
const SOURCE_TIME_NAME: &str = "OotSourceTime";

/// Name of the hidden producer instance generated for attributed programs.
pub const GENERATED_PRODUCER_NAME: &str = PRODUCER_NAME;
/// Name of the hidden boolean that enables host-supplied source timestamps.
pub const GENERATED_USE_SOURCE_TIME_NAME: &str = USE_SOURCE_TIME_NAME;
/// Name of the hidden ULINT source timestamp in Unix nanoseconds.
pub const GENERATED_SOURCE_TIME_NAME: &str = SOURCE_TIME_NAME;

mod definition;
mod instrumentation;
mod model;
mod types;
mod validation;

pub use definition::definition_json_from_sources;
use instrumentation::instrument_source_text;
use validation::{collect_authoring_model, validate_authoring_sources};

/// Instrument source files that contain OpenOT declaration attributes.
///
/// Files without OpenOT attributes are returned unchanged. The lowering is
/// intentionally conservative in this slice: it supports simple scalar
/// declarations inside `PROGRAM ... VAR ... END_VAR` blocks.
#[must_use]
pub fn instrument_source_files(sources: &[SourceFile]) -> Vec<SourceFile> {
    try_instrument_source_files(sources)
        .unwrap_or_else(|error| source_files_with_openot_authoring_error(sources, &error))
}

/// Fallible OpenOT instrumentation for compile paths that must surface the
/// validation error instead of returning a lossy source rewrite.
pub fn try_instrument_source_files(sources: &[SourceFile]) -> Result<Vec<SourceFile>, String> {
    let validation_errors = validate_authoring_sources(sources);
    if !validation_errors.is_empty() {
        return Err(validation_errors.join("; "));
    }
    Ok(instrument_source_files_unchecked(sources))
}

fn instrument_source_files_unchecked(sources: &[SourceFile]) -> Vec<SourceFile> {
    let model = collect_authoring_model(sources);
    sources
        .iter()
        .enumerate()
        .map(|(idx, source)| SourceFile {
            path: source.path.clone(),
            text: instrument_source_text(&source.text, &model.files[idx].programs),
        })
        .collect()
}

fn source_files_with_openot_authoring_error(
    sources: &[SourceFile],
    error: &str,
) -> Vec<SourceFile> {
    let mut result = sources.to_vec();
    let escaped = error.replace("*)", "* )");
    result.push(SourceFile::with_path(
        "__trust_openot_authoring_error.st",
        format!(
            "(* OpenOT authoring validation failed: {escaped} *)
             PROGRAM __TRUST_OPENOT_AUTHORING_ERROR
             VAR
                 OpenOTAuthoringError : __TRUST_OPENOT_AUTHORING_VALIDATION_FAILED;
             END_VAR
             END_PROGRAM
"
        ),
    ));
    result
}

#[cfg(test)]
mod tests;
