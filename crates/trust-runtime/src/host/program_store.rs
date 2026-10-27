//! The program store of a computer (specification 34 §11.2; DEPLOY-03, DEPLOY-04, DEPLOY-06).
//!
//! A store keeps each deployed project as a bundle under `bundles/<label>`. The link `current` names
//! the bundle that runs and `previous` the one before it; both are symbolic links into `bundles`. A
//! deployment stages its bundle, admits the program in it, and commits by replacing `current` with
//! one rename: that rename is the commit point. `previous` is replaced by a second rename. A rollback
//! replaces the same two links in the same order.
//!
//! A project folder that is not a store's `current` link has no store. Its files are replaced in
//! place, each through a temporary file and a rename, and only after its program is admitted.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use trust_runtime_core::deployment::sha256;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_services::ports::{ProgramIdentities, ProgramIdentity};

use super::DEPLOYMENT_RECORD_FILE;
use crate::config::RuntimeConfig;

#[path = "program_store/bundles.rs"]
mod bundles;
#[path = "program_store/links.rs"]
mod links;
#[path = "program_store/retained.rs"]
mod retained;
#[cfg(test)]
#[path = "program_store/tests.rs"]
mod tests;

pub use bundles::{
    bundle_targets, claim_label, copy_bundle, default_bundle_label, prune_bundles,
    validate_bundle_label, validate_bundle_relative_path,
};
pub use links::{
    read_link_target, replace_symlink, resolve_existing_bundle_link, restore_pointer_pair,
    update_deployment_pointers, validate_pointer_slot, PointerState,
};
pub use retained::check_retained;

use bundles::{copy_bundle_files, copy_sidecars, is_program_file, program_path, IMAGE_FILE};
use links::pointer_state;

/// The folder of a store's bundles.
pub const BUNDLES_DIR: &str = "bundles";
/// The link to the bundle that runs. Replacing it is a deployment's commit point.
pub const CURRENT_LINK: &str = "current";
/// The link to the bundle before the running one. A rollback swaps it with `current`.
pub const PREVIOUS_LINK: &str = "previous";
/// The file of a bundle that holds its install sequence as decimal text (specification 34 §11.2).
pub const SEQUENCE_FILE: &str = "install-sequence";

/// Counts the names this process gives to labels and to temporary files, so that none repeats.
static NAME_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Why the program store refuses a deployment or an operation, or cannot finish one. The text
/// states the reason alone.
#[derive(Debug)]
pub enum StoreError {
    /// The store, a bundle or a link refuses the operation, or a file could not be read or written.
    Refused(String),
    /// A runtime check refused the program or a bundle, with the runtime's own error.
    Runtime(RuntimeError),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(reason) => formatter.write_str(reason),
            Self::Runtime(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<RuntimeError> for StoreError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<StoreError> for RuntimeError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Refused(reason) => Self::ControlError(reason.into()),
            StoreError::Runtime(error) => error,
        }
    }
}

fn refused(reason: impl Into<String>) -> StoreError {
    StoreError::Refused(reason.into())
}

fn io_error(context: impl std::fmt::Display, error: io::Error) -> StoreError {
    StoreError::Refused(format!("{context}: {error}"))
}

/// One file that a deployment writes: its path inside the bundle, with `/` between the folders, and
/// its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadFile {
    /// The path inside the bundle, such as `runtime.toml` or `src/main.st`.
    pub path: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

/// The files a deployment brings. A file it leaves out keeps the project's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Payload {
    /// The files, in the order they are written and reported.
    pub files: Vec<PayloadFile>,
}

impl Payload {
    /// The program the payload brings, when it has one.
    #[must_use]
    pub fn program(&self) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|file| is_program_file(&file.path))
            .map(|file| file.bytes.as_slice())
    }

    /// Refuses an empty payload, a path outside the bundle, and a path that names the deployment
    /// record, which the store writes itself.
    fn validate(&self) -> Result<(), StoreError> {
        if self.files.is_empty() {
            return Err(refused("no deploy payload provided"));
        }
        for file in &self.files {
            bundles::validate_bundle_relative_path(Path::new(&file.path), "deploy path")?;
            if file.path == DEPLOYMENT_RECORD_FILE {
                return Err(refused(format!(
                    "{DEPLOYMENT_RECORD_FILE} is written by the program store"
                )));
            }
        }
        Ok(())
    }
}

/// What a deployment wrote, in the order it wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployed {
    /// The paths written, with `/` between the folders. The record is named after the program when
    /// it was written.
    pub written: Vec<String>,
    /// The install sequence the deployment gave its program: the new bundle's sequence in a store,
    /// and 0 for a project folder that is not a store (specification 34 §11.2).
    pub sequence: u64,
}

/// The bundles a rollback selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rollback {
    /// The bundle that `current` names after the rollback: the one that runs.
    pub current: PathBuf,
    /// The bundle that `previous` names after the rollback: the one the rollback set aside.
    pub previous: PathBuf,
}

/// The store a project folder belongs to: the folder that holds `project`, when `project` is a
/// store's `current` link. Any other folder has no store.
#[must_use]
pub fn store_root(project: &Path) -> Option<PathBuf> {
    let is_link =
        fs::symlink_metadata(project).is_ok_and(|metadata| metadata.file_type().is_symlink());
    if is_link && project.file_name() == Some(OsStr::new(CURRENT_LINK)) {
        project.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// Writes `bytes` to `path` through a temporary file and then a rename, so that a stop leaves either
/// the old file or the whole new one (specification 11 §6.7; G17).
///
/// # Errors
///
/// As [`publish_atomic`].
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    publish_atomic(path, |file| file.write_all(bytes))
}

/// Publishes a file at `path`: `write` fills a temporary file beside it, which is synced and then
/// renamed over `path`. The folder of `path` is synced after the rename, so that the rename survives
/// a power cut, as the retain file's writer does (specification 11 §6.7).
///
/// # Errors
///
/// The temporary file cannot be created, written or synced, or the rename fails. The old file is then
/// unchanged, and the temporary file is removed. A failure to sync the folder comes after the rename:
/// the new file is in place, but its durability is not claimed.
pub fn publish_atomic(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let temporary = scratch_path(path, "tmp");
    let result = write_then_rename(&temporary, path, write);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_then_rename(
    temporary: &Path,
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let mut file = fs::File::create(temporary)?;
    write(&mut file)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)?;
    sync_parent(path)
}

/// Syncs the folder that holds `path` (specification 11 §6.7). A path with no folder part is in the
/// current folder, as the retain writer treats it.
fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    crate::retain::sync_parent_dir(parent)
}

/// A name beside `path` that no other write of this process uses. `role` says what the name is for,
/// such as `new` for a link that is staged to replace `path`.
#[must_use]
pub fn scratch_path(path: &Path, role: &str) -> PathBuf {
    let sequence = NAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("deployment-pointer");
    path.with_file_name(format!(".{name}.{role}-{}-{sequence}", std::process::id()))
}

/// Deploys `payload` into `project`. When `project` is a store's `current` link, the deployment
/// stages a new bundle under `label` (a generated label when `None`) and commits it by replacing the
/// links. Otherwise the project's files are replaced in place. Either way the program is admitted
/// before anything the runtime starts from is written (specification 34 §11.2 steps 6 and 7; D2),
/// so a refused program leaves every bundle and link as it was.
///
/// # Errors
///
/// The payload is empty or names a path outside the bundle; the program is refused by its record,
/// or its admission; a bundle or link cannot be read or written; or the
/// label is the bundle of a link.
pub fn deploy(
    project: &Path,
    payload: &Payload,
    label: Option<&str>,
) -> Result<Deployed, StoreError> {
    payload.validate()?;
    match store_root(project) {
        Some(root) => deploy_to_store(&root, project, payload, label),
        None => deploy_in_place(project, payload),
    }
}

/// Installs `program` as the project's program (specification 34 §11.2, `program.install`): a
/// deployment of the program alone, through the same store, so that the program is admitted before
/// anything the runtime starts from changes. The reply's identity is the one `program.active` then
/// reports. The caller runs the retained-value check, [`check_retained`], before this.
///
/// # Errors
///
/// As [`deploy`]: the program is refused by its record or its admission, or
/// a bundle, link or file cannot be read or written.
pub fn install(project: &Path, program: &[u8]) -> Result<ProgramIdentity, StoreError> {
    let payload = Payload {
        files: vec![PayloadFile {
            path: IMAGE_FILE.to_string(),
            bytes: program.to_vec(),
        }],
    };
    let deployed = deploy(project, &payload, None)?;
    Ok(ProgramIdentity {
        sequence: deployed.sequence,
        program_sha256: sha256(program),
        bytes: program.len(),
    })
}

fn deploy_to_store(
    root: &Path,
    current: &Path,
    payload: &Payload,
    label: Option<&str>,
) -> Result<Deployed, StoreError> {
    let bundles_dir = root.join(BUNDLES_DIR);
    let previous_link = root.join(PREVIOUS_LINK);
    validate_pointer_slot(current)?;
    validate_pointer_slot(&previous_link)?;
    let base = resolve_existing_bundle_link(&bundles_dir, current)?
        .ok_or_else(|| refused(format!("no current project link at {}", current.display())))?;
    let previous = resolve_existing_bundle_link(&bundles_dir, &previous_link)?;
    let sequence = next_sequence(&base, previous.as_deref())?;
    let label = label.map_or_else(default_bundle_label, str::to_owned);
    let in_use: Vec<PathBuf> = std::iter::once(base.clone()).chain(previous).collect();
    let stage = claim_label(&bundles_dir, &label, &in_use)?;
    let written = build_stage(&base, &stage, payload, sequence).inspect_err(|_| {
        let _ = fs::remove_dir_all(&stage);
    })?;
    if let Err(error) = update_deployment_pointers(current, &previous_link, &stage, Some(&base)) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    prune_bundles(&bundles_dir, &bundle_targets(&stage, Some(&base)))?;
    Ok(Deployed { written, sequence })
}

/// Stages a bundle at `stage`: a copy of `base` with the payload written over it, the record of its
/// program, the sidecars its `runtime.toml` enables, and the install sequence. The program is admitted
/// before the sidecars are copied, and a refusal returns the error and leaves the caller to remove
/// `stage`.
fn build_stage(
    base: &Path,
    stage: &Path,
    payload: &Payload,
    sequence: u64,
) -> Result<Vec<String>, StoreError> {
    copy_bundle_files(base, stage)?;
    write_payload(stage, payload)?;
    if let Some(program) = payload.program() {
        admit(stage, program)?;
    }
    let runtime = RuntimeConfig::load(stage.join("runtime.toml"))?;
    copy_sidecars(base, stage, &runtime)?;
    write_atomic(
        &stage.join(SEQUENCE_FILE),
        format!("{sequence}\n").as_bytes(),
    )
    .map_err(|error| io_error(format!("write {SEQUENCE_FILE}"), error))?;
    Ok(written_names(payload))
}

/// The install sequence a new bundle takes: one more than the newest of the bundles `current` and
/// `previous` name. A bundle with no sequence file counts as 0.
fn next_sequence(base: &Path, previous: Option<&Path>) -> Result<u64, StoreError> {
    let previous_sequence = match previous {
        Some(previous) => read_sequence(previous)?,
        None => 0,
    };
    read_sequence(base)?
        .max(previous_sequence)
        .checked_add(1)
        .ok_or_else(|| refused("the install sequence cannot count higher"))
}

/// The install sequence a bundle holds: its [`SEQUENCE_FILE`], or 0 for a bundle that the store
/// never installed.
fn read_sequence(bundle: &Path) -> Result<u64, StoreError> {
    let path = bundle.join(SEQUENCE_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => text
            .trim()
            .parse::<u64>()
            .map_err(|_| refused(format!("{} does not hold a whole number", path.display()))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(io_error(format!("read {}", path.display()), error)),
    }
}

fn deploy_in_place(project: &Path, payload: &Payload) -> Result<Deployed, StoreError> {
    admit_in_place(project, payload)?;
    write_payload(project, payload)?;
    if let Some(program) = payload.program() {
        write_program_record(project, program)?;
    }
    Ok(Deployed {
        written: written_names(payload),
        sequence: 0,
    })
}

/// Admits the payload's program before the project folder changes. The record and the admission run
/// in a scratch folder that holds the configuration the project will have: the payload's
/// `runtime.toml` and `io.toml`, or the project's own where the payload leaves them out.
fn admit_in_place(project: &Path, payload: &Payload) -> Result<(), StoreError> {
    let Some(program) = payload.program() else {
        return Ok(());
    };
    let scratch = scratch_path(&std::env::temp_dir().join("trust-program-store"), "admit");
    let result = admit_in_scratch(&scratch, project, payload, program);
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn admit_in_scratch(
    scratch: &Path,
    project: &Path,
    payload: &Payload,
    program: &[u8],
) -> Result<(), StoreError> {
    stage_configuration(scratch, project, payload)?;
    admit(scratch, program)
}

/// Makes the folder `scratch` hold the configuration the project will have: the payload's
/// `runtime.toml` and `io.toml`, or the project's own where the payload leaves them out.
fn stage_configuration(
    scratch: &Path,
    project: &Path,
    payload: &Payload,
) -> Result<(), StoreError> {
    fs::create_dir_all(scratch).map_err(|error| io_error("create the admission folder", error))?;
    for name in ["runtime.toml", "io.toml"] {
        match payload.files.iter().find(|file| file.path == name) {
            Some(file) => fs::write(scratch.join(name), &file.bytes)
                .map_err(|error| io_error(format!("write {name}"), error))?,
            None => {
                let source = project.join(name);
                if source.is_file() {
                    fs::copy(&source, scratch.join(name))
                        .map_err(|error| io_error(format!("copy {name}"), error))?;
                }
            }
        }
    }
    Ok(())
}

/// Writes the record of `program` into `dir`, which holds the runtime and I/O configuration that the
/// record describes, and admits the program on this computer as the runtime admits it at its start
/// (specification 34 §5.1.1, DEPLOY-04; specification 44 §10.2, §8.8). A refused program is an
/// error, and the caller writes nothing the runtime starts from.
fn admit(dir: &Path, program: &[u8]) -> Result<(), StoreError> {
    if !program.starts_with(&trust_runtime_core::native::image::MAGIC) {
        return Err(refused(
            "the program is not a program image, which this runtime runs: rebuild the project",
        ));
    }
    write_program_record(dir, program)?;
    crate::native::admit_on_this_computer(program).map_err(|error| refused(error.to_string()))
}

/// Writes the deployment record of the program image `program` into `dir` (specification 44 §10.1).
fn write_program_record(dir: &Path, program: &[u8]) -> Result<(), StoreError> {
    let record = super::image_record(dir, program)?;
    write_atomic(&dir.join(DEPLOYMENT_RECORD_FILE), &record)
        .map_err(|error| io_error(format!("write {DEPLOYMENT_RECORD_FILE}"), error))
}

/// Writes every payload file into `dir`, each through a temporary file and a rename.
fn write_payload(dir: &Path, payload: &Payload) -> Result<(), StoreError> {
    for file in &payload.files {
        let path = dir.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| io_error(format!("create '{}'", parent.display()), error))?;
        }
        write_atomic(&path, &file.bytes)
            .map_err(|error| io_error(format!("write {}", file.path), error))?;
    }
    Ok(())
}

/// The paths a deployment wrote, with the record after the program.
fn written_names(payload: &Payload) -> Vec<String> {
    let mut written = Vec::new();
    for file in &payload.files {
        written.push(file.path.clone());
        if is_program_file(&file.path) {
            written.push(DEPLOYMENT_RECORD_FILE.to_string());
        }
    }
    written
}

/// Rolls a store back: `current` and `previous` change bundles. Each link is replaced by one rename,
/// and `current` first, because it is the commit point. A stop between the two renames therefore
/// leaves `current` on the bundle the rollback selected, which the next start runs; `previous` is only
/// the slot the rollback fills, and no start reads it.
///
/// # Errors
///
/// `current` or `previous` names no bundle, or a link cannot be replaced. When the second rename
/// fails, the first is undone.
pub fn rollback(root: &Path) -> Result<Rollback, StoreError> {
    let plan = plan_rollback(root)?;
    rollback_current(&plan)?;
    if let Err(error) = rollback_previous(&plan) {
        return Err(
            match links::set_pointer_state(&plan.current_link, &plan.current_state) {
                Ok(()) => error,
                Err(restore) => refused(format!(
                    "{error}; also failed to restore the current link: {restore}"
                )),
            },
        );
    }
    Ok(Rollback {
        current: plan.new_current,
        previous: plan.new_previous,
    })
}

/// What a rollback changes: the two links, the state `current` had, and the bundles they move to.
struct RollbackPlan {
    current_link: PathBuf,
    previous_link: PathBuf,
    current_state: PointerState,
    new_current: PathBuf,
    new_previous: PathBuf,
}

fn plan_rollback(root: &Path) -> Result<RollbackPlan, StoreError> {
    let bundles_dir = root.join(BUNDLES_DIR);
    let current_link = root.join(CURRENT_LINK);
    let previous_link = root.join(PREVIOUS_LINK);
    let current = resolve_existing_bundle_link(&bundles_dir, &current_link)?.ok_or_else(|| {
        refused(format!(
            "no current project link at {}",
            current_link.display()
        ))
    })?;
    let previous =
        resolve_existing_bundle_link(&bundles_dir, &previous_link)?.ok_or_else(|| {
            refused(format!(
                "no previous project link at {} (nothing to rollback)",
                previous_link.display()
            ))
        })?;
    Ok(RollbackPlan {
        current_state: pointer_state(&current_link)?,
        current_link,
        previous_link,
        new_current: previous,
        new_previous: current,
    })
}

/// The commit of a rollback: `current` names the bundle the rollback selects.
fn rollback_current(plan: &RollbackPlan) -> Result<(), StoreError> {
    replace_symlink(&plan.current_link, &plan.new_current)
}

/// The second step of a rollback: `previous` names the bundle that `current` replaced.
fn rollback_previous(plan: &RollbackPlan) -> Result<(), StoreError> {
    replace_symlink(&plan.previous_link, &plan.new_previous)
}

/// The program `program.active` reports for a project folder. For a store's `current` link it is the
/// bundle that `current` names and the one that `previous` names, each with its install sequence, the
/// SHA-256 and the length of its program; a member is `None` when its bundle cannot be read, and the
/// result is `None` when `current` names no bundle. Any other folder has no store: its own
/// `program.tpi` is the active program, with no previous one (see `plain_identity`).
#[must_use]
pub fn identities(project: &Path) -> Option<ProgramIdentities> {
    let Some(root) = store_root(project) else {
        return Some(ProgramIdentities {
            active: plain_identity(project),
            previous: None,
        });
    };
    let bundles_dir = root.join(BUNDLES_DIR);
    let active = resolve_existing_bundle_link(&bundles_dir, project).ok()??;
    let previous = resolve_existing_bundle_link(&bundles_dir, &root.join(PREVIOUS_LINK))
        .ok()
        .flatten();
    Some(ProgramIdentities {
        active: identity_of(&active),
        previous: previous.as_deref().and_then(identity_of),
    })
}

/// The program of a folder with no store, as the runtime starts it (specification 34 §11.2): its own
/// `program.tpi`, with sequence 0 and its SHA-256 and length; none when the deployment record beside
/// it does not admit it, as the runtime then starts none.
fn plain_identity(project: &Path) -> Option<ProgramIdentity> {
    let program = fs::read(program_path(project)?).ok()?;
    crate::deployment::admit_project_image(project, &program).ok()?;
    Some(ProgramIdentity {
        sequence: 0,
        program_sha256: sha256(&program),
        bytes: program.len(),
    })
}

/// The identity of the program in `bundle`, or `None` when the program or its sequence cannot be read.
fn identity_of(bundle: &Path) -> Option<ProgramIdentity> {
    let program = fs::read(program_path(bundle)?).ok()?;
    let sequence = read_sequence(bundle).ok()?;
    Some(ProgramIdentity {
        sequence,
        program_sha256: sha256(&program),
        bytes: program.len(),
    })
}
