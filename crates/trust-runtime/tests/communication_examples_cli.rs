use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn communication_project_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/communication")
        .join(name)
}

fn communication_examples_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/communication")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst)
        .unwrap_or_else(|err| panic!("create directory {}: {err}", dst.display()));
    let entries = std::fs::read_dir(src)
        .unwrap_or_else(|err| panic!("read directory {}: {err}", src.display()));
    for entry in entries {
        let entry = entry.expect("directory entry");
        let source_path = entry.path();
        let dest_path = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("query file type {}: {err}", source_path.display()));
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &dest_path);
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &dest_path).unwrap_or_else(|err| {
                panic!(
                    "copy file {} -> {}: {err}",
                    source_path.display(),
                    dest_path.display()
                )
            });
        } else {
            panic!(
                "unsupported non-file/non-directory entry {} in communication example fixture",
                source_path.display()
            );
        }
    }
}

#[cfg(unix)]
fn normalize_runtime_endpoint_for_platform(_project: &Path) {}

#[cfg(not(unix))]
fn normalize_runtime_endpoint_for_platform(project: &Path) {
    let runtime_path = project.join("runtime.toml");
    let Ok(raw) = std::fs::read_to_string(&runtime_path) else {
        return;
    };
    if !raw.contains("endpoint = \"unix://") {
        return;
    }

    let mut changed = false;
    let mut normalized = String::with_capacity(raw.len());
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("endpoint = \"unix://") {
            normalized.push_str("endpoint = \"tcp://127.0.0.1:18082\"\n");
            changed = true;
        } else {
            normalized.push_str(line);
            normalized.push('\n');
        }
    }

    if changed {
        std::fs::write(&runtime_path, normalized).unwrap_or_else(|err| {
            panic!(
                "rewrite runtime endpoint for {}: {err}",
                runtime_path.display()
            )
        });
    }
}

#[test]
fn communication_examples_build_and_validate() {
    let root = communication_examples_root();
    let mut examples: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("read communication examples root {}: {err}", root.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            entry.file_name().to_str().map(|s| s.to_string())
        })
        .collect();
    examples.sort();

    assert!(
        !examples.is_empty(),
        "no communication example directories found under {}",
        root.display()
    );

    for name in examples {
        let fixture = communication_project_path(&name);
        assert!(
            fixture.is_dir(),
            "missing communication example fixture: {}",
            fixture.display()
        );
        let required_files = [
            "io.toml",
            "runtime.toml",
            "trust-lsp.toml",
            "src/main.st",
            "src/config.st",
        ];
        for file in required_files {
            let path = fixture.join(file);
            assert!(
                path.is_file(),
                "communication example {} is missing required file {}",
                fixture.display(),
                path.display()
            );
        }

        let temp_root = unique_temp_dir(&format!("communication-example-{name}"));
        let project = temp_root.join(&name);
        copy_dir_recursive(&fixture, &project);
        normalize_runtime_endpoint_for_platform(&project);

        let build = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
            .args(["build", "--project"])
            .arg(&project)
            .args(["--sources", "src"])
            .output()
            .expect("run trust-runtime build");
        assert!(
            build.status.success(),
            "expected build success for {name} example.\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let validate = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
            .args(["validate", "--project"])
            .arg(&project)
            .output()
            .expect("run trust-runtime validate");
        assert!(
            validate.status.success(),
            "expected validate success for {name} example.\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&validate.stdout),
            String::from_utf8_lossy(&validate.stderr)
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
