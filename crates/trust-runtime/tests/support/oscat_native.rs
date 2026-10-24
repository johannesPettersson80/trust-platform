//! An OSCAT fixture project compiled for this computer, loaded and run scan by scan: the support of
//! the tests that check the OSCAT blocks on compiled programs (specification 44). The tests need
//! the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::Path;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::native::{
    build, load, Device, HostCode, NativeProgram, NoHooks, Options, Target, Toolchain,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::native::image::{FileRecord, StmtRecord};

/// The image of the fixture project `fixtures/oscat/<name>`, compiled for this computer.
pub fn image(name: &str) -> &'static [u8] {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/oscat")
        .join(name);
    let sources = collect_project_source_files(&project, None).expect("the fixture's sources");
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-oscat-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let built = build(
        &sources,
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the fixture compiles");
    let _ = std::fs::remove_dir_all(&work);
    Box::leak(built.image.into_boxed_slice())
}

/// The program of `image`, cold-started at logical time zero.
pub fn program(image: &'static [u8]) -> NativeProgram<'static, HostCode> {
    let device = Device {
        target: Target::this_cpu().expect("a computer target"),
        program_memory: 1 << 24,
        task_stack: 1 << 20,
        max_statements: 10_000_000,
        trusted_keys: &[],
    };
    let mut program = load(image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
}

/// Runs one scan at the logical time `at_ms`; it must end without a fault.
pub fn scan(program: &mut NativeProgram<'static, HostCode>, at_ms: i64) {
    if let Err(error) = program.scan(Duration::from_millis(at_ms), &mut NoHooks) {
        let fault = program.last_fault();
        panic!(
            "the scan at {at_ms} ms: {error} (statement {} at {}, detail {:?})",
            fault.stmt,
            position(program, fault.stmt),
            fault.detail
        );
    }
}

/// Where statement `id` stands, `file:line:column`, by the image's statement table.
fn position(program: &NativeProgram<'static, HostCode>, id: u32) -> String {
    let image = program.image();
    let statement = image
        .table::<StmtRecord>()
        .ok()
        .and_then(|statements| statements.get(id));
    let Some(statement) = statement else {
        return String::from("no statement of the image");
    };
    let file = image
        .table::<FileRecord>()
        .ok()
        .and_then(|files| files.get(u32::from(statement.file)))
        .and_then(|file| image.string(file.label))
        .unwrap_or("?");
    format!("{file}:{}:{}", statement.start_line, statement.start_column)
}

/// The value of the variable `name`, by its canonical name (`MAIN.LAG.OUT`).
pub fn value(program: &NativeProgram<'static, HostCode>, name: &str) -> Value {
    program
        .value(name)
        .unwrap_or_else(|| panic!("{name} has no value"))
}

/// The `REAL` value of `name`.
pub fn real(program: &NativeProgram<'static, HostCode>, name: &str) -> f32 {
    match value(program, name) {
        Value::Real(value) => value,
        other => panic!("{name} is {other:?}"),
    }
}
