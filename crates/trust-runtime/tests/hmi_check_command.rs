//! `trust-runtime hmi check` (specification 39, §18 phase 5 item 6): the editor's checks over every
//! HMI file of a project, for its own CI: the files load and the bindings resolve in the program,
//! and an error fails the command.

use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAM: &str = "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n";
const SCREEN: &str = "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n";

fn page(path: &str) -> String {
    format!("format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"{path}\"\n")
}

fn project(name: &str, bound: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-hmi-check-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let write = |file: &str, text: &str| {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
        std::fs::write(path, text).expect("write");
    };
    write("src/main.st", PROGRAM);
    write("hmi/screen.toml", SCREEN);
    write("hmi/pages/overview.toml", &page(bound));
    root
}

fn check(root: &Path, json: bool) -> (Option<i32>, String) {
    let mut command = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    );
    command.args(["hmi", "check", "--project"]).arg(root);
    if json {
        command.arg("--json");
    }
    let output = command.output().expect("trust-runtime runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn a_project_whose_hmi_files_check_passes() {
    let root = project("good", "Main.Level");
    let (code, output) = check(&root, false);
    assert_eq!(code, Some(0), "{output}");
    assert!(output.contains("HMI check passed"), "{output}");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_binding_the_program_lacks_fails_the_check_at_its_file_and_line() {
    let root = project("broken", "Main.Height");
    let (code, output) = check(&root, false);
    assert_eq!(code, Some(1), "{output}");
    assert!(
        output.contains("hmi/pages/overview.toml:") && output.contains("Main.Height"),
        "{output}"
    );
    let (code, output) = check(&root, true);
    assert_eq!(code, Some(1), "{output}");
    let answer: serde_json::Value = serde_json::from_str(&output).expect("JSON");
    assert_eq!(answer["ok"], serde_json::json!(false), "{answer}");
    let problem = &answer["problems"][0];
    assert_eq!(
        problem["file"],
        serde_json::json!("hmi/pages/overview.toml"),
        "{answer}"
    );
    assert!(
        problem["line"].as_u64().is_some_and(|line| line > 1),
        "{answer}"
    );
    assert!(
        problem["message"]
            .as_str()
            .is_some_and(|text| text.contains("Main.Height")),
        "{answer}"
    );
    std::fs::remove_dir_all(root).ok();
}

/// Specification 39, §8.9 items 1 and 2: a placed faceplate whose instance's block lacks a field
/// the faceplate needs fails the check at its `instance` line.
#[test]
fn a_placed_faceplate_its_instance_does_not_fit_fails_the_check() {
    let root = project("placed", "Main.Level");
    let write = |file: &str, text: &str| {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
        std::fs::write(path, text).expect("write");
    };
    write(
        "src/main.st",
        "FUNCTION_BLOCK Tank\nVAR_OUTPUT\n    Level : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    Level : REAL;\n    T1 : Tank;\nEND_VAR\n    T1();\nEND_PROGRAM\n",
    );
    write(
        "hmi/panels/pump-card.toml",
        "format = 1\ntitle = \"Pump card\"\nsize = [320, 160]\n\n[fields.Running]\naccepts = [\"boolean\"]\n",
    );
    write(
        "hmi/pages/cards.toml",
        "format = 1\ntitle = \"Cards\"\n\n[[object]]\nid = \"card\"\ntype = \"faceplate\"\nfaceplate = \"pump-card\"\ninstance = \"Main.T1\"\nat = [0, 0]\nsize = [320, 204]\n",
    );
    let (code, output) = check(&root, false);
    assert_eq!(code, Some(1), "{output}");
    assert!(
        output.contains("hmi/pages/cards.toml:8:")
            && output.contains("Main.T1 has no member Running"),
        "{output}"
    );
    std::fs::remove_dir_all(root).ok();
}
