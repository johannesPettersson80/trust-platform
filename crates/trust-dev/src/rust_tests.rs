//! `trust-dev rust declarations` writes a project's generated declarations (specification 48
//! RU-24); what the declarations hold is proven by trust-runtime's `native::rust_pous` tests.

use clap::Parser;

use super::run_declarations;
use crate::{Cli, Command, RustAction};

const MANIFEST: &str = "[package]\nname = \"line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

fn project(lib: &str) -> std::path::PathBuf {
    let root = trust_test_support::unique_temp_path("trust-dev-rust");
    std::fs::create_dir_all(root.join("rust/src")).expect("the crate's folder");
    std::fs::write(root.join("rust/Cargo.toml"), MANIFEST).expect("the manifest");
    std::fs::write(root.join("rust/src/lib.rs"), lib).expect("the root file");
    root
}

#[test]
fn the_command_line_names_the_project_and_writes_its_declarations() {
    // Catches a command that parses to something else or writes nothing: RU-24's command with the
    // project folder, then the file RU-23 names.
    let root = project(
        "#[trust_function(name = \"MAX_LIMITED\")]\n#[budget(\"2us\")]\n\
         pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {\n    Ok(a.max(b).min(limit))\n}\n",
    );
    let cli = Cli::try_parse_from([
        "trust-dev",
        "rust",
        "declarations",
        "--project",
        root.to_str().expect("a path"),
    ])
    .expect("the command line parses");
    let Command::Rust {
        action: RustAction::Declarations { project },
    } = cli.command
    else {
        panic!("rust declarations");
    };
    run_declarations(project).expect("the declarations are written");
    let written =
        std::fs::read_to_string(root.join("src/generated/rust_pous.st")).expect("the file");
    assert!(
        written.contains("FUNCTION MAX_LIMITED : DINT\n"),
        "{written}"
    );
}

#[test]
fn a_crate_with_an_error_fails_and_writes_nothing() {
    // RU-24: declarations that do not match the crate are never written.
    let root = project(
        "#[trust_function(name = \"F\")]\npub fn f(_cx: &mut Cx) -> Result<(), Fault> {\n    Ok(())\n}\n",
    );
    let error = run_declarations(Some(root.clone())).expect_err("the missing budget fails");
    assert!(error.to_string().contains("1 error(s)"), "{error}");
    assert!(!root.join("src/generated/rust_pous.st").exists());
}

#[test]
fn the_declarations_come_with_the_bindings_of_the_structured_text_the_crate_calls() {
    // RU-39, RU-77: a crate that calls `st::SCALE` gets its bindings from the command that writes
    // its declarations, before the project's first build, so that rust-analyzer and its own cargo
    // find `st`: `build/trust_st` holds the crate `trust_st` with the library `st` and SCALE's
    // binding, of SCALE's two LREAL inputs and LREAL result. Catches bindings that only a build
    // writes, which leave a fresh clone's crate without `st`.
    let root = project(
        "#[trust_function(name = \"TWICE\")]\n#[budget(\"1us\")]\n\
         pub fn twice(cx: &mut Cx, x: f64) -> Result<f64, Fault> {\n    st::SCALE(cx, x, 2.0)\n}\n",
    );
    std::fs::write(
        root.join("rust/Cargo.toml"),
        format!("{MANIFEST}\n[dependencies]\ntrust_st = {{ path = \"../build/trust_st\" }}\n"),
    )
    .expect("the manifest");
    std::fs::create_dir_all(root.join("src")).expect("the sources' folder");
    std::fs::write(
        root.join("src/main.st"),
        "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    factor : LREAL;
END_VAR
SCALE := x * factor;
END_FUNCTION

PROGRAM Line
VAR
    doubled : LREAL;
END_VAR
doubled := TWICE(x := 1.5);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
",
    )
    .expect("the program");
    run_declarations(Some(root.clone())).expect("the declarations and the bindings are written");
    let manifest =
        std::fs::read_to_string(root.join("build/trust_st/Cargo.toml")).expect("the manifest");
    assert!(
        manifest.contains("name = \"trust_st\"") && manifest.contains("[lib]\nname = \"st\""),
        "{manifest}"
    );
    let library =
        std::fs::read_to_string(root.join("build/trust_st/src/lib.rs")).expect("the library");
    assert!(
        library.contains(
            "pub fn SCALE(cx: &mut trust_plc::Cx, x: f64, factor: f64) -> Result<f64, trust_plc::Fault>"
        ),
        "{library}"
    );
}
