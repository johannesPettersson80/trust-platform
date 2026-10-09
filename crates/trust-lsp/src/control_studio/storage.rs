//! The study files of a project (specification 35, A8.2 to A8.4): `control-studio/<loop id>/`
//! holds `study.json` and one file per recording under `recordings/`. Earlier studies of the loop
//! stay readable under `history/`. This module is the only writer of these files. Writes go
//! through a temporary file, so a failed write leaves the previous file whole, and the whole loop
//! folder is checked against the storage limit of A7 before anything is written.

use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::recording::RecordingFile;
use trust_control::study::{check_size, has_current_format, Study};

const STUDY_FILE: &str = "study.json";
/// Numbers the temporary files of this process, so two writes never share one.
static TEMPORARY_FILES: AtomicU64 = AtomicU64::new(0);
const RECORDINGS: &str = "recordings";
const HISTORY: &str = "history";

/// A folder name for a loop identifier or revision: letters, digits, `.`, `-` and `_` are kept,
/// anything else becomes `_`, and a short hash is added when that changes the name.
#[must_use]
pub fn folder_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if safe == name && !safe.is_empty() && !safe.chars().all(|character| character == '.') {
        return safe;
    }
    let digest = Sha256::digest(name.as_bytes());
    let mut hash = String::new();
    for byte in &digest[..4] {
        let _ = write!(hash, "{byte:02x}");
    }
    let stem: String = safe.trim_start_matches('.').chars().take(48).collect();
    format!(
        "{}-{hash}",
        if stem.is_empty() {
            "loop"
        } else {
            stem.as_str()
        }
    )
}

/// The folder of one loop's study in a project.
#[must_use]
pub fn study_directory(project_root: &Path, loop_id: &str) -> PathBuf {
    project_root
        .join("control-studio")
        .join(folder_name(loop_id))
}

/// Reads the loop's study for the loop revision `loop_revision`. A study of another revision, or of
/// an earlier format, moves to `history/` and a new study starts (A8.2, A8.3); no study yet starts
/// one too.
///
/// # Errors
/// The study file cannot be read or is not a study of this loop, or the move fails.
pub fn open_study(
    project_root: &Path,
    loop_id: &str,
    loop_revision: &str,
) -> Result<Study, Failure> {
    let directory = study_directory(project_root, loop_id);
    let Some(text) = read_optional(&directory.join(STUDY_FILE))? else {
        return Ok(Study::new(loop_id, loop_revision));
    };
    if !has_current_format(&text) {
        archive_working_study(&directory, "earlier-format")?;
        return Ok(Study::new(loop_id, loop_revision));
    }
    let study = Study::decode(&text, loop_id)?;
    if study.loop_revision == loop_revision {
        return Ok(study);
    }
    archive_working_study(&directory, &study.loop_revision)?;
    Ok(Study::new(loop_id, loop_revision))
}

/// Writes the study and the new recording files it refers to. A recording already on disk is
/// never rewritten. Nothing is written when the loop folder would exceed the storage limit.
///
/// # Errors
/// A recording is not referred to by the study, the limit would be exceeded, or a write fails.
pub fn save_study(
    project_root: &Path,
    study: &Study,
    recordings: &[RecordingFile],
) -> Result<(), Failure> {
    let directory = study_directory(project_root, &study.loop_id);
    let document = study.encode()?;
    let mut incoming = Vec::new();
    for recording in recordings {
        let id = recording.id()?;
        if recording.loop_id != study.loop_id || !study.recordings.contains(&id) {
            return Err(storage_failure(&format!(
                "recording {id} is not part of the study of loop {}",
                study.loop_id
            )));
        }
        let file = recording_file(&directory, &id);
        if !file.exists() {
            incoming.push((file, recording.encode()?));
        }
    }
    let kept = bytes_in(&directory).saturating_sub(size_of(&directory.join(STUDY_FILE)));
    let total = kept
        + document.len() as u64
        + incoming
            .iter()
            .map(|(_, text)| text.len() as u64)
            .sum::<u64>();
    check_size(total)?;
    for (file, text) in &incoming {
        write_atomic(file, text)?;
    }
    write_atomic(&directory.join(STUDY_FILE), &document)
}

/// Starts a new working study (A8.4 Reset). The previous one moves to `history/` first, so nothing
/// is lost if the new study cannot be written.
///
/// # Errors
/// The move or the write fails.
pub fn start_new_study(project_root: &Path, study: &Study) -> Result<Study, Failure> {
    let directory = study_directory(project_root, &study.loop_id);
    archive_working_study(&directory, &study.loop_revision)?;
    let fresh = Study::new(&study.loop_id, &study.loop_revision);
    save_study(project_root, &fresh, &[])?;
    Ok(fresh)
}

/// The recording `id` of the loop's working study, checked against its identifier.
///
/// # Errors
/// The file is missing, unreadable, or does not match its identifier.
pub fn read_recording(
    project_root: &Path,
    loop_id: &str,
    id: &str,
) -> Result<RecordingFile, Failure> {
    let file = recording_file(&study_directory(project_root, loop_id), id);
    let text = read_optional(&file)?.ok_or_else(|| {
        read_failure(&format!(
            "the study refers to recording {id}, which is missing"
        ))
    })?;
    RecordingFile::decode(&text, loop_id, id)
}

/// Moves `study.json` and `recordings/` into `history/<revision>/`. Returns false when there was
/// no working study.
fn archive_working_study(directory: &Path, revision: &str) -> Result<bool, Failure> {
    let study = directory.join(STUDY_FILE);
    if !study.exists() {
        return Ok(false);
    }
    let folder = unique_folder(&directory.join(HISTORY), &folder_name(revision));
    fs::create_dir_all(&folder).map_err(|error| io_failure(&folder, &error))?;
    let recordings = directory.join(RECORDINGS);
    let moved = recordings.exists();
    if moved {
        fs::rename(&recordings, folder.join(RECORDINGS))
            .map_err(|error| io_failure(&recordings, &error))?;
    }
    if let Err(error) = fs::rename(&study, folder.join(STUDY_FILE)) {
        if moved {
            let _ = fs::rename(folder.join(RECORDINGS), &recordings);
        }
        return Err(io_failure(&study, &error));
    }
    Ok(true)
}

fn unique_folder(parent: &Path, name: &str) -> PathBuf {
    (1..)
        .map(|index| {
            parent.join(if index == 1 {
                name.to_string()
            } else {
                format!("{name}-{index}")
            })
        })
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| parent.join(name))
}

fn recording_file(directory: &Path, id: &str) -> PathBuf {
    directory.join(RECORDINGS).join(format!("{id}.json"))
}

/// Writes `text` to `file` through a temporary file in the same folder.
pub(crate) fn write_atomic(file: &Path, text: &str) -> Result<(), Failure> {
    let directory = file
        .parent()
        .ok_or_else(|| storage_failure(&format!("{} has no folder", file.display())))?;
    fs::create_dir_all(directory).map_err(|error| io_failure(directory, &error))?;
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let unique = TEMPORARY_FILES.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(".{name}.{}.{unique}.tmp", std::process::id()));
    write_then_rename(&temporary, file, text).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        io_failure(file, &error)
    })
}

/// Writes and syncs `temporary`, then renames it to `file`.
fn write_then_rename(temporary: &Path, file: &Path, text: &str) -> std::io::Result<()> {
    let mut handle = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    handle.write_all(text.as_bytes())?;
    handle.sync_all()?;
    fs::rename(temporary, file)
}

fn read_optional(file: &Path) -> Result<Option<String>, Failure> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(read_failure(&format!("{}: {error}", file.display()))),
    }
}

fn size_of(file: &Path) -> u64 {
    fs::metadata(file)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn bytes_in(directory: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                bytes_in(&path)
            } else {
                size_of(&path)
            }
        })
        .sum()
}

fn io_failure(path: &Path, error: &std::io::Error) -> Failure {
    storage_failure(&format!("{}: {error}", path.display()))
}

fn storage_failure(reason: &str) -> Failure {
    Failure::new(
        &format!("The study could not be stored: {reason}."),
        Cause::Settings,
        NextAction::None,
    )
}

fn read_failure(reason: &str) -> Failure {
    Failure::new(
        &format!("The study could not be read: {reason}. It was left in place."),
        Cause::Settings,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
