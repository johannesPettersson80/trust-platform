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
