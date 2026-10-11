//! Freeze source bytes separately from expected rendered validation outputs.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path, process::Command};

const GENERATED: [&str; 3] = [
    "docs/diagrams/generated",
    "docs/diagrams/manifest.json",
    "docs/public/assets/images",
];

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FileIdentity {
    path: String,
    sha256: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    source_root: String,
    base: String,
    generated_output_paths: Vec<String>,
    files: Vec<FileIdentity>,
}

fn generated(path: &str) -> bool {
    GENERATED
        .iter()
        .any(|output| Path::new(path).starts_with(output))
}

fn git(root: &Path, arguments: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()?;
    ensure!(output.status.success(), "Git source inventory failed");
    String::from_utf8(output.stdout).context("Git source inventory is not UTF-8")
}

fn additional_paths(root: &Path, directory: &Path, paths: &mut BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(directory).context("read additional source directory")? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name() == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            additional_paths(root, &path, paths)?;
        } else if path.is_file() {
            paths.insert(
                path.strip_prefix(root)?
                    .to_str()
                    .context("instruction path is not UTF-8")?
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

fn inventory(root: &Path) -> Result<Vec<FileIdentity>> {
    let list = git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let mut paths: BTreeSet<String> = list
        .split_terminator('\0')
        .filter(|path| !generated(path))
        .map(str::to_owned)
        .collect();
    for name in ["AGENTS.md", "CLAUDE.md"] {
        ensure!(
            root.join(name).is_file(),
            "canonical instruction file is missing"
        );
        paths.insert(name.to_owned());
    }
    for directory in [
        ".codex/skills",
        "docs/internal/prototypes/browser_analysis_wasm_spike/web",
    ] {
        additional_paths(root, &root.join(directory), &mut paths)?;
    }
    paths
        .into_iter()
        .map(|path| {
            let file = root.join(&path);
            let sha256 = match fs::read(file) {
                Ok(bytes) => Some(super::digest(&bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error).context("read frozen source"),
            };
            Ok(FileIdentity { path, sha256 })
        })
        .collect()
}

pub(super) fn freeze(root: &Path, output: &str) -> Result<()> {
    let root = root.canonicalize()?;
    ensure!(
        !Path::new(output).exists(),
        "refusing to replace a source freeze"
    );
    let snapshot = Snapshot {
        source_root: root
            .to_str()
            .context("source root is not UTF-8")?
            .to_owned(),
        base: git(&root, &["rev-parse", "HEAD"])?.trim().to_owned(),
        generated_output_paths: GENERATED.map(str::to_owned).to_vec(),
        files: inventory(&root)?,
    };
    fs::write(output, serde_json::to_vec_pretty(&snapshot)?)?;
    println!("Frozen {} source records", snapshot.files.len());
    Ok(())
}

pub(super) fn check(root: &Path, input: &str) -> Result<()> {
    let snapshot: Snapshot = serde_json::from_slice(&fs::read(input)?)?;
    let root = root.canonicalize()?;
    ensure!(
        root.to_str() == Some(snapshot.source_root.as_str()),
        "source checkout differs from freeze"
    );
    ensure!(
        git(&root, &["rev-parse", "HEAD"])?.trim() == snapshot.base,
        "source base differs from freeze"
    );
    ensure!(
        snapshot
            .generated_output_paths
            .iter()
            .map(String::as_str)
            .eq(GENERATED),
        "generated-output policy differs from freeze"
    );
    verify_files(&snapshot.files, &inventory(&root)?)?;
    println!(
        "Frozen source matches: {} records; rendered outputs are separate evidence",
        snapshot.files.len()
    );
    Ok(())
}

fn verify_files(expected: &[FileIdentity], current: &[FileIdentity]) -> Result<()> {
    ensure!(
        expected == current,
        "source paths or bytes differ from freeze"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_expected_rendered_outputs_are_excluded() {
        for path in [
            "docs/diagrams/generated/flow.svg",
            "docs/diagrams/manifest.json",
            "docs/public/assets/images/ide.png",
        ] {
            assert!(generated(path));
        }
        for path in [
            "docs/diagrams/architecture/flow.puml",
            "docs/diagrams/manifest.json.backup",
            "docs/specs/34-runtime-portability.md",
            "crates/trust-runtime/src/web/ui/assets/ide-monaco.20260215.js",
            "editors/vscode/media/stateChartWebview.js",
            "AGENTS.md",
        ] {
            assert!(!generated(path));
        }
    }

    #[test]
    fn source_identity_distinguishes_changed_added_and_deleted_records() {
        let record = |path: &str, bytes: Option<&[u8]>| FileIdentity {
            path: path.to_owned(),
            sha256: bytes.map(super::super::digest),
        };
        let expected = vec![record("source.rs", Some(b"source"))];
        assert!(verify_files(&expected, &[record("source.rs", Some(b"source"))]).is_ok());
        assert!(verify_files(&expected, &[record("source.rs", Some(b"changed"))]).is_err());
        assert!(verify_files(&expected, &[record("source.rs", None)]).is_err());
        assert!(verify_files(
            &expected,
            &[
                record("source.rs", Some(b"source")),
                record("added.rs", Some(b"new"))
            ]
        )
        .is_err());
    }
}
