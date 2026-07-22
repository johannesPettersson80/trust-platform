use std::process::{Command, Output};

fn run_runtime(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(args)
        .output()
        .expect("run trust-runtime")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf-8")
}

#[test]
fn mistyped_check_command_suggests_the_real_subcommand() {
    let output = run_runtime(&["chek"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Did you mean: check?"),
        "stderr was:\n{}",
        stderr(&output)
    );
}

#[test]
fn subcommand_suggestion_ignores_global_flags() {
    let output = run_runtime(&["--verbose", "rn"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Did you mean: run?"),
        "stderr was:\n{}",
        stderr(&output)
    );
}
