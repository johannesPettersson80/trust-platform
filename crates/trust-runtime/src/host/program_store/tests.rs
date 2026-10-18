//! The program store's tests. Expected values are worked out from specification 34 §11.2 and §5.1.1:
//! the first deployment of a store has install sequence 1, and each later one takes one more than the
//! newest of `current` and `previous`.

use std::fs;
use std::path::{Path, PathBuf};

use trust_runtime_core::deployment::{sha256, DeploymentRecord};
use trust_runtime_services::ports::{ProgramIdentities, ProgramIdentity};

use super::*;
use crate::deployment::{write_record, DEPLOYMENT_RECORD_FILE};

/// The runtime settings of every test store, the same as the deployment tests use.
const RUNTIME_TOML: &str = r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

const EMPTY_PROGRAM: &str = "PROGRAM Main\nEND_PROGRAM\n";
const COUNTING_PROGRAM: &str =
    "PROGRAM Main\nVAR\n    Counter : DINT;\nEND_VAR\nCounter := Counter + 1;\nEND_PROGRAM\n";

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "trust-program-store-{name}-{}",
        default_bundle_label()
    ));
    fs::create_dir_all(&path).expect("create the test folder");
    path
}

fn compile(source: &str) -> Vec<u8> {
    crate::harness::CompileSession::from_source(source)
        .build_bytecode_bytes()
        .expect("the test program compiles")
}

/// A deployment that brings the test runtime and `program`.
fn payload_of(program: &[u8]) -> Payload {
    Payload {
        files: vec![
            PayloadFile {
                path: "runtime.toml".to_string(),
                bytes: RUNTIME_TOML.as_bytes().to_vec(),
            },
            PayloadFile {
                path: PROGRAM_FILE.to_string(),
                bytes: program.to_vec(),
            },
        ],
    }
}

/// Makes `bundles/<label>` the first bundle of the store at `root`, as a store holds it after its
/// first deployment: its program, its record and install sequence 1, with `current` naming it.
fn first_deployment(root: &Path, label: &str, program: &[u8]) -> PathBuf {
    let bundle = root.join(BUNDLES_DIR).join(label);
    fs::create_dir_all(&bundle).expect("create the first bundle");
    fs::write(bundle.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    fs::write(bundle.join(PROGRAM_FILE), program).expect("write program.stbc");
    write_record(&bundle, program).expect("write the record");
    fs::write(bundle.join(SEQUENCE_FILE), "1\n").expect("write the install sequence");
    replace_symlink(&root.join(CURRENT_LINK), &bundle).expect("point current at the first bundle");
    bundle
}

/// The folder a link names, resolved, so that two paths to one bundle compare equal.
fn resolved(link: &Path) -> PathBuf {
    fs::canonicalize(link).expect("the link names a bundle")
}

/// The identity of `program` installed with `sequence`.
fn identity(program: &[u8], sequence: u64) -> ProgramIdentity {
    ProgramIdentity {
        sequence,
        program_sha256: sha256(program),
        bytes: program.len(),
    }
}

/// Spec 34 §11.2 steps 6 and 7 (D2): a deployment admits its program before anything the runtime
/// starts from is written. A program that does not prepare leaves `current` on the first bundle, and
/// no second bundle appears. Catches a deployment that stages and commits first and refuses later.
#[test]
fn a_deploy_whose_program_does_not_admit_leaves_the_current_bundle_unchanged() {
    let root = scratch("refused");
    let first = compile(EMPTY_PROGRAM);
    first_deployment(&root, "first", &first);
    let project = root.join(CURRENT_LINK);
    let error = deploy(&project, &payload_of(&[1, 2, 3]), None)
        .expect_err("a program that does not prepare is refused");
    assert!(
        error.to_string().contains("does not prepare"),
        "unexpected refusal: {error}"
    );
    assert_eq!(
        identities(&project),
        Some(ProgramIdentities {
            active: Some(identity(&first, 1)),
            previous: None,
        })
    );
    assert_eq!(
        resolved(&project),
        resolved(&root.join(BUNDLES_DIR).join("first"))
    );
    let bundles: Vec<String> = fs::read_dir(root.join(BUNDLES_DIR))
        .expect("read the bundles")
        .map(|entry| {
            entry
                .expect("read a bundle entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        bundles,
        vec!["first".to_string()],
        "a refused deployment leaves no bundle"
    );
    let _ = fs::remove_dir_all(root);
}

/// Spec 34 §11.2 (D3): a rollback replaces `current` with one rename, then `previous` with one rename.
/// A stop between the two leaves `current` on the bundle the rollback selected, which the next start
/// runs, and `previous` still names that bundle. Catches a rollback that removes a link before it
/// renames it, which leaves no `current`, and one that renames `previous` first, which leaves the
/// rolled-back program unselected.
#[test]
fn rollback_is_one_rename_per_link_and_survives_a_stop_between_them() {
    let root = scratch("rollback");
    let first = compile(EMPTY_PROGRAM);
    let second = compile(COUNTING_PROGRAM);
    let first_bundle = first_deployment(&root, "first", &first);
    let project = root.join(CURRENT_LINK);
    deploy(&project, &payload_of(&second), Some("second")).expect("the second program deploys");
    assert_eq!(
        identities(&project),
        Some(ProgramIdentities {
            active: Some(identity(&second, 2)),
            previous: Some(identity(&first, 1)),
        })
    );

    let plan = plan_rollback(&root).expect("the rollback is planned");
    rollback_current(&plan).expect("the first rename");
    assert_eq!(resolved(&project), resolved(&first_bundle));
    // The stop: `previous` still names the first bundle, which `current` now names too, so the next
    // start runs the first program.
    assert_eq!(resolved(&root.join(PREVIOUS_LINK)), resolved(&first_bundle));
    assert_eq!(
        identities(&project).map(|reported| reported.active),
        Some(Some(identity(&first, 1)))
    );

    rollback_previous(&plan).expect("the second rename");
    assert_eq!(
        identities(&project),
        Some(ProgramIdentities {
            active: Some(identity(&first, 1)),
            previous: Some(identity(&second, 2)),
        })
    );
    let _ = fs::remove_dir_all(root);
}

/// Spec 34 §11.2 (cleanup): a stage writes its bundle under its label, and nothing the runtime starts
/// from changes until the commit. A stop before the commit leaves `current` on the first bundle, and
/// the staged bundle is on disk, unselected. Catches a stage that replaces `current` while it writes,
/// or a partial bundle that the next start runs.
#[test]
fn an_interrupted_stage_leaves_current_selected() {
    let root = scratch("interrupted");
    let first = compile(EMPTY_PROGRAM);
    let second = compile(COUNTING_PROGRAM);
    let first_bundle = first_deployment(&root, "first", &first);
    let project = root.join(CURRENT_LINK);
    let stage = root.join(BUNDLES_DIR).join("staged");
    build_stage(&resolved(&project), &stage, &payload_of(&second), 2)
        .expect("the stage is written");
    // The stop: the commit never runs.
    assert_eq!(resolved(&project), resolved(&first_bundle));
    assert_eq!(
        identities(&project).map(|reported| reported.active),
        Some(Some(identity(&first, 1)))
    );
    assert_eq!(
        fs::read(stage.join(PROGRAM_FILE)).expect("the staged program is on disk"),
        second
    );
    let _ = fs::remove_dir_all(root);
}

/// Spec 34 §11.2 (cleanup): a folder that no link names is a stage that was interrupted, and it blocks
/// nothing. A deployment under its label removes it and commits the new bundle. Catches a leftover
/// that refuses its label with `deployment already exists` for ever.
#[test]
fn a_stage_interrupted_before_its_rename_does_not_block_its_label() {
    let root = scratch("leftover");
    let first = compile(EMPTY_PROGRAM);
    let second = compile(COUNTING_PROGRAM);
    first_deployment(&root, "first", &first);
    let project = root.join(CURRENT_LINK);
    let leftover = root.join(BUNDLES_DIR).join("second");
    fs::create_dir_all(&leftover).expect("the interrupted stage's folder");
    fs::write(leftover.join("runtime.toml"), "[resource\n").expect("a half-written file");
    deploy(&project, &payload_of(&second), Some("second"))
        .expect("the leftover does not block its label");
    assert_eq!(resolved(&project), resolved(&leftover));
    assert_eq!(
        identities(&project).map(|reported| reported.active),
        Some(Some(identity(&second, 2)))
    );
    let _ = fs::remove_dir_all(root);
}

/// Spec 34 §11.2 (program identities): `program.active` reports the bundle that `current` names, with
/// its install sequence, SHA-256 and length, and the bundle that `previous` names. Catches a report that
/// still shows the first program as active after a deployment, or that drops `previous`.
#[test]
fn program_active_on_a_computer_reports_current_and_previous() {
    let root = scratch("identities");
    let first = compile(EMPTY_PROGRAM);
    let second = compile(COUNTING_PROGRAM);
    first_deployment(&root, "first", &first);
    let project = root.join(CURRENT_LINK);
    deploy(&project, &payload_of(&second), Some("second")).expect("the second program deploys");
    assert_eq!(
        identities(&project),
        Some(ProgramIdentities {
            active: Some(identity(&second, 2)),
            previous: Some(identity(&first, 1)),
        })
    );
    let _ = fs::remove_dir_all(root);
}

/// Spec 34 §11.2 (program identities): a project folder with no store runs its own `program.stbc`, so
/// `program.active` reports that program as active, with sequence 0, its SHA-256 and length, and no
/// previous program. Catches a plain project that answers `unsupported request`, or that reports a
/// previous program it does not have.
#[test]
fn program_active_on_a_plain_project_reports_its_program_and_no_previous() {
    let project = scratch("plain-identities");
    let program = compile(COUNTING_PROGRAM);
    fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("a plain runtime.toml");
    fs::write(project.join(PROGRAM_FILE), &program).expect("a plain program.stbc");
    write_record(&project, &program).expect("the plain project's record");
    assert_eq!(
        identities(&project),
        Some(ProgramIdentities {
            active: Some(ProgramIdentity {
                sequence: 0,
                program_sha256: sha256(&program),
                bytes: program.len(),
            }),
            previous: None,
        })
    );
    let _ = fs::remove_dir_all(project);
}

/// Spec 34 §11.2 and §5.1.1: a project folder whose record does not admit its program does not start, so
/// `program.active` reports no program for it. Catches a report that names a program the runtime refuses
/// to load.
#[test]
fn program_active_on_a_plain_project_whose_record_does_not_admit_its_program_reports_nothing() {
    let project = scratch("plain-refused-identities");
    let program = compile(EMPTY_PROGRAM);
    let other = compile(COUNTING_PROGRAM);
    fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("a plain runtime.toml");
    fs::write(project.join(PROGRAM_FILE), &program).expect("a plain program.stbc");
    write_record(&project, &other).expect("the record of another program");
    assert_eq!(
        identities(&project),
        Some(ProgramIdentities {
            active: None,
            previous: None,
        })
    );
    let _ = fs::remove_dir_all(project);
}

/// Spec 34 §11.2 step 7 and G17: the staged program and its deployment record are written as
/// `bundles/<label>/program.stbc` and `deployment-record.bin`, each through a temporary file and a
/// rename, and the commit is the `current` link. Catches a bundle without its record, a record of
/// another program, or a temporary file left beside the bundle's files.
#[test]
fn a_deployment_writes_its_program_and_record_into_its_bundle_and_commits_at_current() {
    let root = scratch("layout");
    let first = compile(EMPTY_PROGRAM);
    let second = compile(COUNTING_PROGRAM);
    first_deployment(&root, "first", &first);
    let project = root.join(CURRENT_LINK);
    let deployed =
        deploy(&project, &payload_of(&second), Some("second")).expect("the second program deploys");
    assert_eq!(
        deployed.written,
        vec![
            "runtime.toml".to_string(),
            PROGRAM_FILE.to_string(),
            DEPLOYMENT_RECORD_FILE.to_string(),
        ]
    );
    let bundle = root.join(BUNDLES_DIR).join("second");
    assert_eq!(resolved(&project), resolved(&bundle));
    assert_eq!(
        fs::read(bundle.join(PROGRAM_FILE)).expect("the program"),
        second
    );
    let record = DeploymentRecord::decode(
        &fs::read(bundle.join(DEPLOYMENT_RECORD_FILE)).expect("the record"),
    )
    .expect("the record decodes");
    assert_eq!(record.application.program_sha256, sha256(&second));
    for entry in fs::read_dir(&bundle).expect("list the bundle") {
        let name = entry
            .expect("read a bundle entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        assert!(!name.starts_with('.'), "a temporary file is left: {name}");
    }
    let _ = fs::remove_dir_all(root);
}

/// Spec 34 §11.2 step 6 (D2): a project folder with no store is refused before any of its files is
/// written, when its program does not admit. Catches an in-place deployment that replaces the
/// project's `runtime.toml` and program and only then finds the program refused.
#[test]
fn a_plain_project_whose_program_does_not_admit_keeps_its_files() {
    let project = scratch("plain-refused");
    let first = compile(EMPTY_PROGRAM);
    fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("a plain runtime.toml");
    fs::write(project.join(PROGRAM_FILE), &first).expect("a plain program.stbc");
    let changed = RUNTIME_TOML.replace("cycle_interval_ms = 100", "cycle_interval_ms = 50");
    let payload = Payload {
        files: vec![
            PayloadFile {
                path: "runtime.toml".to_string(),
                bytes: changed.into_bytes(),
            },
            PayloadFile {
                path: PROGRAM_FILE.to_string(),
                bytes: vec![1, 2, 3],
            },
        ],
    };
    let _ =
        deploy(&project, &payload, None).expect_err("a program that does not prepare is refused");
    assert_eq!(
        fs::read(project.join(PROGRAM_FILE)).expect("the program"),
        first
    );
    assert_eq!(
        fs::read_to_string(project.join("runtime.toml")).expect("the runtime"),
        RUNTIME_TOML
    );
    assert!(!project.join(DEPLOYMENT_RECORD_FILE).exists());
    let _ = fs::remove_dir_all(project);
}
