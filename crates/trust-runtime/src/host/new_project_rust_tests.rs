//! The Rust starts of a new project (specification 25 §3.2, specification 48 RU-51): the files
//! each writes, its generated Structured Text, the crate's package name and `rust-toolchain.toml`.

use std::path::{Path, PathBuf};

use super::super::{create_project_as, ProjectStart};
use super::{package_name, toolchain_file};
use crate::native::rust_pous::{write_generated, Generated, Written};

/// The files below `root`, relative with `/`, in byte order.
fn files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, folder: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(folder).expect("the folder reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let relative = path.strip_prefix(root).expect("below the root");
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

/// A new project of `start` named `name` in a folder of its own.
fn created(name: &str, start: ProjectStart) -> (PathBuf, PathBuf) {
    let parent = trust_test_support::unique_temp_path("new-rust-project");
    let project = create_project_as(&parent, name, start, false).expect("the project");
    (parent, project)
}

fn read(project: &Path, relative: &str) -> String {
    std::fs::read_to_string(project.join(relative))
        .unwrap_or_else(|error| panic!("{relative}: {error}"))
}

#[test]
fn a_structured_text_project_with_rust_holds_its_crate_and_the_declarations_of_its_block() {
    // Specification 25 §3.2, specification 48 RU-51: the files of the Structured Text start, with
    // `src/Main.st` calling FILTER_EMA, the crate in `rust/`, `rust-toolchain.toml` and the
    // generated declarations, which are what `trust-dev rust declarations` writes for the crate: a
    // second run finds them up to date. Catches a start that leaves out a file, writes one that
    // the build would refuse as out of date, or names the crate by the folder alone.
    let (parent, project) = created("Conveyor Line", ProjectStart::StructuredTextWithRust);
    assert_eq!(
        files(&project),
        [
            ".gitignore",
            ".vscode/launch.json",
            ".vscode/settings.json",
            "io.toml",
            "runtime.toml",
            "rust-toolchain.toml",
            "rust/Cargo.toml",
            "rust/src/lib.rs",
            "src/Main.st",
            "src/config.st",
            "src/generated/rust_pous.st",
            "trust-lsp.toml",
        ]
    );
    assert_eq!(
        read(&project, "rust/Cargo.toml"),
        format!(
            "[package]\nname = \"conveyor-line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ntrust-plc = \"={}\"\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    let main = read(&project, "src/Main.st");
    assert!(main.contains("Filter : FILTER_EMA;"), "{main}");
    assert!(main.contains("Filter(x := Raw, alpha := 0.1);"), "{main}");
    let declarations = read(&project, "src/generated/rust_pous.st");
    assert!(
        declarations.contains("FUNCTION_BLOCK FILTER_EMA"),
        "{declarations}"
    );
    assert_eq!(
        write_generated(&project).expect("the generated files"),
        Generated {
            declarations: Written::UpToDate,
            configuration: None,
            services: Written::Nothing,
        }
    );
    let gitignore = read(&project, ".gitignore");
    for line in ["/.trust-runtime/", "/build/", "/rust/target/"] {
        assert!(
            gitignore.lines().any(|held| held == line),
            "{line}: {gitignore}"
        );
    }
    assert!(project
        .join(ProjectStart::StructuredTextWithRust.focus())
        .is_file());
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn a_rust_first_project_is_its_crate_with_the_configuration_of_its_trust_toml() {
    // Specification 25 §3.2, specification 48 §14: the crate at the project's root with
    // `trust.toml`, the generated declarations and configuration, which a second run finds up to
    // date, and no hand-written Structured Text; the simulator runs the generated configuration.
    // Catches a Rust-first start that writes `src/Main.st` or `src/config.st`, whose configuration
    // is out of date, or whose launch names a file it has not.
    let (parent, project) = created("cell", ProjectStart::RustFirst);
    assert_eq!(
        files(&project),
        [
            ".gitignore",
            ".vscode/launch.json",
            ".vscode/settings.json",
            "Cargo.toml",
            "io.toml",
            "runtime.toml",
            "rust-toolchain.toml",
            "src/generated/trust/configuration.st",
            "src/generated/trust/rust_pous.st",
            "src/lib.rs",
            "trust-lsp.toml",
            "trust.toml",
        ]
    );
    assert!(read(&project, "Cargo.toml").contains("name = \"cell\"\n"));
    let configuration = read(&project, "src/generated/trust/configuration.st");
    assert!(
        configuration.contains("PROGRAM MainInstance WITH MainTask : MAIN;"),
        "{configuration}"
    );
    assert!(read(&project, ".vscode/launch.json")
        .contains("\"program\": \"${workspaceFolder}/src/generated/trust/configuration.st\""));
    assert_eq!(
        write_generated(&project).expect("the generated files"),
        Generated {
            declarations: Written::UpToDate,
            configuration: Some(Written::UpToDate),
            services: Written::Nothing,
        }
    );
    let gitignore = read(&project, ".gitignore");
    for line in ["/.trust-runtime/", "/build/", "/target/"] {
        assert!(
            gitignore.lines().any(|held| held == line),
            "{line}: {gitignore}"
        );
    }
    assert!(project.join(ProjectStart::RustFirst.focus()).is_file());
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn the_crates_package_is_named_after_the_project() {
    // Specification 25 §3.2, worked by hand: lower case, one `-` for each run of other characters,
    // no `-` or `_` at either end, `plc` for nothing left, `plc-` before a digit and `-plc` after a
    // name cargo refuses. Catches a package name cargo refuses: a space, a capital, a final `-`, a
    // leading digit, a keyword or a crate Rust builds in.
    for (project, package) in [
        ("Conveyor Line", "conveyor-line"),
        ("my_plc", "my_plc"),
        ("My PLC (v2)", "my-plc-v2"),
        ("--A..B__", "a-b"),
        ("Anlage Süd", "anlage-s-d"),
        ("2nd line", "plc-2nd-line"),
        ("Ü", "plc"),
        ("test", "test-plc"),
        ("fn", "fn-plc"),
        ("Core", "core-plc"),
        ("proc-macro", "proc-macro-plc"),
        ("examples", "examples-plc"),
    ] {
        assert_eq!(package_name(project), package, "{project}");
    }
}

#[test]
fn the_toolchain_file_names_the_bundles_rust_and_the_devices_targets() {
    // Specification 48 RU-77, specification 25 §3.2: the channel of the workspace's
    // `rust-toolchain.toml`, which the toolchain bundle's compiler is, the devices' module targets
    // and `rust-src`. Catches a file without the version, so that a user's rustup would give
    // rust-analyzer another compiler than the build's.
    let workspace = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rust-toolchain.toml"),
    )
    .expect("the workspace's rust-toolchain.toml");
    let channel = workspace
        .lines()
        .find_map(|line| line.strip_prefix("channel = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("its channel");
    let written = toolchain_file().expect("the toolchain file");
    let parsed = written
        .parse::<toml_edit::DocumentMut>()
        .expect("the file is TOML");
    let toolchain = parsed.get("toolchain").expect("[toolchain]");
    assert_eq!(
        toolchain.get("channel").and_then(|item| item.as_str()),
        Some(channel)
    );
    let strings = |key: &str| -> Vec<String> {
        toolchain
            .get(key)
            .and_then(|item| item.as_array())
            .map(|array| {
                array
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        strings("targets"),
        ["riscv32imac-unknown-none-elf", "thumbv8m.main-none-eabihf"]
    );
    assert_eq!(strings("components"), ["rust-src"]);
}

#[test]
fn force_keeps_a_gitignore_and_adds_only_the_lines_it_lacks() {
    // Specification 25 §3.2: an existing `.gitignore` is kept and gains each line of the start that
    // it lacks, once. Catches a start that replaces the user's `.gitignore` or repeats a line.
    let parent = trust_test_support::unique_temp_path("new-rust-project-force");
    let project = parent.join("cell");
    std::fs::create_dir_all(&project).expect("the folder");
    std::fs::write(project.join(".gitignore"), "node_modules/\n/build/").expect("the file");
    create_project_as(&parent, "cell", ProjectStart::RustFirst, true).expect("the project");
    let gitignore = read(&project, ".gitignore");
    assert!(
        gitignore.starts_with("node_modules/\n/build/\n"),
        "{gitignore}"
    );
    for line in ["/.trust-runtime/", "/build/", "/target/"] {
        assert_eq!(
            gitignore.lines().filter(|held| *held == line).count(),
            1,
            "{line}: {gitignore}"
        );
    }
    let _ = std::fs::remove_dir_all(parent);
}
