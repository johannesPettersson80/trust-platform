//! The heartbeats of a project's services as its program's globals (specification 48 RU-63): a
//! Structured Text project without a crate whose `trust.toml` declares a service builds with the
//! generated `services.st`, its program gates on the service's heartbeat with `SERVICE_FRESH`
//! (specification 08 §5A), and the build refuses the project while the committed file is not what
//! `trust.toml` declares.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use std::path::{Path, PathBuf};

use rust_pous_support::sources;
use trust_runtime::native::rust_pous::{write_generated, Written, SERVICES_FILE};
use trust_runtime::native::{
    build_project, load, Build, Device, HostCode, NativeProgram, NoHooks, Options, Target,
    Toolchain,
};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// A service with Appendix B's settings.
const TRUST_TOML: &str = "[services.recipe]
package = \"recipe-service\"
heartbeat = \"500ms\"
timeout = \"2s\"
";

/// A program that gates on the recipe service's heartbeat with a timeout of one second.
const PROGRAM: &str = "PROGRAM Line
VAR_EXTERNAL
    recipe_heartbeat : UDINT;
END_VAR
VAR
    fresh : SERVICE_FRESH;
    ok : BOOL;
    age : TIME;
END_VAR
fresh(HEARTBEAT := recipe_heartbeat, TIMEOUT := T#1s, FRESH => ok, AGE => age);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// A project of the program and `trust.toml` alone, without a crate.
fn project(name: &str) -> PathBuf {
    let root = trust_test_support::unique_temp_path(name);
    for (path, text) in [("trust.toml", TRUST_TOML), ("src/main.st", PROGRAM)] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    root
}

/// The project built for this computer.
fn build(root: &Path) -> Result<Build, String> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    build_project(
        root,
        &sources(root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .map_err(|error| error.to_string())
}

/// `ok` and `age` in milliseconds after the scan at `time` milliseconds.
fn after(program: &mut NativeProgram<'_, HostCode>, time: i64) -> (Value, Value) {
    program
        .scan(Duration::from_millis(time), &mut NoHooks)
        .expect("the scan runs");
    (
        program.value("MAIN.OK").expect("ok"),
        program.value("MAIN.AGE").expect("age"),
    )
}

fn fresh(ok: bool, age_ms: i64) -> (Value, Value) {
    (Value::Bool(ok), Value::Time(Duration::from_millis(age_ms)))
}

#[test]
fn a_program_gates_on_the_heartbeat_of_a_service_its_trust_toml_declares() {
    // RU-63 with specification 08 §5A, worked by hand on the task's logical time: the task is due
    // at its start (specification 10 §4.3), so its first call, at 0 ms, keeps heartbeat 0 without a
    // change: not fresh. The service then writes 1 into the global, as its SDK does: at 10 ms the
    // block counts the change, fresh with AGE 0; unchanged at 1010 ms, AGE is exactly the 1 s
    // timeout, still fresh; at 1020 ms it is stale. Catches a project whose services' globals the
    // build does not declare (the program would not compile), a heartbeat of another type, and a
    // global the program does not read.
    let root = project("service-heartbeats");
    let generated = write_generated(&root).expect("the generated files write");
    assert_eq!(generated.services, Written::Wrote(1));
    assert!(root.join(SERVICES_FILE).is_file());
    let built = build(&root).unwrap_or_else(|error| panic!("the project builds: {error}"));
    let device = Device {
        target: Target::this_cpu().expect("a computer target"),
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    assert_eq!(program.value("RECIPE_HEARTBEAT"), Some(Value::UDInt(0)));
    assert_eq!(after(&mut program, 0), fresh(false, 0));
    let (heartbeat, _) = program
        .image()
        .find_symbol("RECIPE_HEARTBEAT")
        .expect("the heartbeat's symbol");
    program
        .write_symbol(heartbeat, &Value::UDInt(1))
        .expect("the service's heartbeat");
    assert_eq!(after(&mut program, 10), fresh(true, 0));
    assert_eq!(after(&mut program, 1010), fresh(true, 1000));
    assert_eq!(after(&mut program, 1020), fresh(false, 1010));
}

#[test]
fn a_project_whose_heartbeats_are_out_of_date_does_not_build() {
    // RU-63, RU-24: a service added to `trust.toml` after the file was written makes the
    // committed file out of date, and the build refuses the project naming the file and the
    // command, whether the project has a crate or not. Catches a build that checks the generated
    // files only for a project with a crate.
    let root = project("service-heartbeats-stale");
    write_generated(&root).expect("the generated files write");
    std::fs::write(
        root.join("trust.toml"),
        format!("{TRUST_TOML}\n[services.camera]\nheartbeat = \"100ms\"\n"),
    )
    .expect("trust.toml");
    let Err(refused) = build(&root) else {
        panic!("the build is refused");
    };
    assert!(
        refused.contains(SERVICES_FILE) && refused.contains("trust-dev rust declarations"),
        "{refused}"
    );
}
