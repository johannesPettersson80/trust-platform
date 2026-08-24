use std::process::Command;

fn trust_dev() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trust-dev"))
}

#[test]
fn trust_dev_help_surfaces_workbench_commands() {
    let output = trust_dev().arg("--help").output().expect("run trust-dev");

    assert!(
        output.status.success(),
        "trust-dev --help failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agent"));
    assert!(stdout.contains("commit"));
    assert!(stdout.contains("docs"));
    assert!(stdout.contains("test"));
}

#[test]
fn trust_dev_subcommand_help_is_stable() {
    for args in [
        &["agent", "serve", "--help"][..],
        &["commit", "--help"][..],
        &["docs", "--help"][..],
        &["test", "--help"][..],
    ] {
        let output = trust_dev().args(args).output().expect("run trust-dev");
        assert!(
            output.status.success(),
            "trust-dev {} failed.\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn oscat_replaced_upstream_name_remains_absent() {
    let project = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../trust-runtime/tests/fixtures/oscat/negative_public_surface");
    let main_source = include_str!(
        "../../trust-runtime/tests/fixtures/oscat/negative_public_surface/src/main.st"
    );
    let compile_fixture_source = include_str!(
        "../../trust-runtime/tests/fixtures/oscat/negative_public_surface/src/tests.st"
    );
    assert!(main_source.contains("OVERRIDE("));
    assert!(compile_fixture_source.contains("OVERRIDE("));
    let output = trust_dev()
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run OSCAT negative public-surface fixture");

    assert!(
        !output.status.success(),
        "upstream OSCAT OVERRIDE must not resolve as a duplicate public API"
    );
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace('\\', "/");
    assert!(diagnostics.contains("src/main.st: expected expression"));
    assert!(diagnostics.contains("src/tests.st: expected expression"));
}
