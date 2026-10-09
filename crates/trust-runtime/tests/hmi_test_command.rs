//! `trust-runtime hmi test` (specification 39, §18 phase 5 item 6): every page drawn in a headless
//! browser and compared with its approved screenshot kept in the project, failing on a change;
//! `--update` approves the new look. It needs Chromium; where none is found the test says so and
//! checks nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAM: &str = "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n";
const SCREEN: &str = "format = 1\nsize = [640, 360]\ntheme = \"dark\"\nsymbols = \"flat\"\n";

fn page(x: u32) -> String {
    format!("format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [{x}, 40]\nsize = [240, 96]\nlabel = \"Level\"\n\n[object.bind]\nvalue = \"Main.Level\"\n")
}

fn project() -> PathBuf {
    let root = empty_project();
    let write = |file: &str, text: &str| {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
        std::fs::write(path, text).expect("write");
    };
    write("src/main.st", PROGRAM);
    write("hmi/screen.toml", SCREEN);
    write("hmi/pages/overview.toml", &page(40));
    root
}

fn empty_project() -> PathBuf {
    std::env::temp_dir().join(format!(
        "trust-hmi-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

/// A Chromium to draw with, as `hmi test` looks for one.
fn chromium() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("CHROMIUM") {
        return Some(PathBuf::from(named));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let cache = std::fs::read_dir(home.join(".cache/ms-playwright")).ok()?;
    cache
        .filter_map(Result::ok)
        .flat_map(|entry| {
            ["chrome-linux64/chrome", "chrome-linux/chrome"].map(|tail| entry.path().join(tail))
        })
        .find(|path| path.is_file())
}

fn hmi_test(root: &Path, update: bool) -> (Option<i32>, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command.args(["hmi", "test", "--project"]).arg(root);
    if update {
        command.arg("--update");
    }
    if let Some(chromium) = chromium() {
        command.arg("--chromium").arg(chromium);
    }
    let output = command.output().expect("trust-runtime runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn a_page_is_compared_with_its_approved_screenshot_and_a_change_fails() {
    if chromium().is_none() && Command::new("chromium").arg("--version").output().is_err() {
        eprintln!("no Chromium here: hmi test is not exercised");
        return;
    }
    let root = project();
    let (code, output) = hmi_test(&root, false);
    assert_eq!(code, Some(1), "no approved screenshot yet: {output}");
    assert!(
        output.contains("overview") && output.contains("no approved screenshot"),
        "{output}"
    );
    let (code, output) = hmi_test(&root, true);
    assert_eq!(code, Some(0), "{output}");
    let approved = root.join("hmi/screens/overview.png");
    assert!(approved.is_file(), "approved: {output}");
    let (code, output) = hmi_test(&root, false);
    assert_eq!(code, Some(0), "unchanged: {output}");
    assert!(output.contains("overview: same"), "{output}");
    std::fs::write(root.join("hmi/pages/overview.toml"), page(320)).expect("moved");
    let (code, output) = hmi_test(&root, false);
    assert_eq!(code, Some(1), "{output}");
    assert!(output.contains("overview: changed"), "{output}");
    assert!(
        root.join("hmi/screens/overview.actual.png").is_file(),
        "the new look to review: {output}"
    );
    std::fs::remove_dir_all(root).ok();
}

const TANKS: &str = "FUNCTION_BLOCK Tank\nVAR_OUTPUT\n    Level : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    T1 : Tank;\nEND_VAR\n    T1();\nEND_PROGRAM\n";

const PLACED: &str = "format = 1\ntitle = \"Tanks\"\n\n[[object]]\nid = \"card-1\"\ntype = \"faceplate\"\nfaceplate = \"tank\"\ninstance = \"Main.T1\"\nat = [40, 40]\nsize = [320, 204]\n";

fn faceplate(x: u32) -> String {
    format!("format = 1\ntitle = \"Tank\"\nsize = [320, 160]\n\n[fields.Level]\naccepts = [\"number\"]\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [{x}, 24]\nsize = [200, 96]\nlabel = \"Level\"\n\n[object.bind]\nvalue = \"$Level\"\n")
}

/// Specification 39, §8.9: a placed faceplate is drawn from its faceplate file, so a change to
/// that file changes the page's look.
#[test]
fn a_placed_faceplate_is_drawn_from_its_faceplate_file() {
    if chromium().is_none() && Command::new("chromium").arg("--version").output().is_err() {
        eprintln!("no Chromium here: hmi test is not exercised");
        return;
    }
    let root = empty_project();
    for (file, text) in [
        ("src/main.st", TANKS.to_string()),
        ("hmi/screen.toml", SCREEN.to_string()),
        ("hmi/pages/tanks.toml", PLACED.to_string()),
        ("hmi/panels/tank.toml", faceplate(16)),
    ] {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
        std::fs::write(path, text).expect("write");
    }
    let (code, output) = hmi_test(&root, true);
    assert_eq!(code, Some(0), "{output}");
    std::fs::write(root.join("hmi/panels/tank.toml"), faceplate(104)).expect("moved");
    let (code, output) = hmi_test(&root, false);
    assert_eq!(
        code,
        Some(1),
        "the faceplate's value moved on the page: {output}"
    );
    assert!(output.contains("tanks: changed"), "{output}");
    std::fs::remove_dir_all(root).ok();
}
