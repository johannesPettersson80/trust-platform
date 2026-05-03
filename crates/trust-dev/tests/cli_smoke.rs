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
