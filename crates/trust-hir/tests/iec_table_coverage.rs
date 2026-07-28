use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

macro_rules! repository_source_tree_read_to_string {
    (
        $path_and_root:expr,
        roots = [$($root:literal),+ $(,)?],
        extension = $extension:literal $(,)?
    ) => {{
        let (path, repository_root) = $path_and_root;
        let result = path.strip_prefix(repository_root).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "repository source path escaped root",
            )
        });
        result.and_then(|relative| {
            let allowed_root = relative.components().next().is_some_and(
                |component| {
                    let component = component.as_os_str();
                    false $(|| component == std::ffi::OsStr::new($root))+
                },
            );
            let allowed_extension =
                relative.extension().and_then(|value| value.to_str())
                    == Some($extension);
            if !allowed_root || !allowed_extension {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "repository source path is outside the declared tree",
                ));
            }
            fs::read_to_string(path)
        })
    }};
}

#[derive(Debug, Deserialize)]
struct TableMap {
    table: Vec<TableEntry>,
}

#[derive(Debug, Deserialize)]
struct TableEntry {
    id: String,
    spec: Option<String>,
    tests: Vec<String>,
}

#[test]
fn iec_table_coverage_report() {
    let repo_root = repo_root();
    let map_source = include_str!("../../../docs/specs/coverage/iec-table-test-map.toml");
    let map = parse_table_map(map_source).expect("parse IEC table map");

    let test_names = collect_test_names(&repo_root);
    let (report, missing) = render_coverage_report(&map, &test_names, &repo_root);

    insta::assert_snapshot!(report);
    assert_no_missing_tests(&missing).expect("all IEC table tests resolve");
}

#[test]
fn malformed_iec_table_map_is_rejected() {
    let malformed = r#"[[table]]
id = "IEC_TABLE"
tests = "not-an-array"
"#;

    let error = parse_table_map(malformed).expect_err("malformed map must reject");

    assert!(error.to_string().contains("invalid type"));
}

#[test]
fn missing_iec_table_test_name_is_reported_and_rejected() {
    let map = parse_table_map(
        r#"[[table]]
id = "IEC_TABLE"
spec = "docs/specs/example.md"
tests = ["does_not_exist"]
"#,
    )
    .expect("parse missing-name fixture");
    let repo_root = PathBuf::from("/repo");
    let (report, missing) = render_coverage_report(&map, &BTreeMap::new(), &repo_root);

    assert!(report.contains("does_not_exist: MISSING"));
    assert_eq!(missing, vec!["does_not_exist"]);
    assert_eq!(
        assert_no_missing_tests(&missing).unwrap_err(),
        "missing IEC table tests: does_not_exist"
    );
}

fn parse_table_map(source: &str) -> Result<TableMap, toml::de::Error> {
    toml::from_str(source)
}

fn render_coverage_report(
    map: &TableMap,
    test_names: &BTreeMap<String, Vec<PathBuf>>,
    repo_root: &Path,
) -> (String, Vec<String>) {
    let mut missing = Vec::new();
    let mut report = String::new();

    for entry in &map.table {
        let spec = entry.spec.as_deref().unwrap_or("-");
        report.push_str(&format!("{} ({})\n", entry.id, spec));
        for test_name in &entry.tests {
            let status = if let Some(paths) = test_names.get(test_name) {
                let mut shown = paths
                    .iter()
                    .map(|path| display_path(path, repo_root))
                    .collect::<Vec<_>>();
                shown.sort();
                format!("ok [{}]", shown.join(", "))
            } else {
                missing.push(test_name.clone());
                "MISSING".to_string()
            };
            report.push_str(&format!("  - {test_name}: {status}\n"));
        }
        report.push('\n');
    }

    (report, missing)
}

fn assert_no_missing_tests(missing: &[String]) -> Result<(), String> {
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("missing IEC table tests: {}", missing.join(", ")))
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn collect_test_names(repo_root: &Path) -> BTreeMap<String, Vec<PathBuf>> {
    let mut names: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut dirs = vec![repo_root.join("crates"), repo_root.join("tests")];

    while let Some(dir) = dirs.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if should_skip(&path) {
                continue;
            }
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                let Ok(contents) = repository_source_tree_read_to_string!(
                    (&path, repo_root),
                    roots = ["crates", "tests"],
                    extension = "rs",
                ) else {
                    continue;
                };
                for test_name in parse_test_names(&contents) {
                    names.entry(test_name).or_default().push(path.clone());
                }
            }
        }
    }

    names
}

fn parse_test_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut pending = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[test]") || trimmed.starts_with("#[tokio::test]") {
            pending = true;
            continue;
        }
        if pending && trimmed.starts_with("#[") {
            continue;
        }
        if pending {
            if let Some(name) = parse_fn_name(trimmed) {
                names.insert(name);
                pending = false;
            }
        }
    }
    names
}

fn parse_fn_name(line: &str) -> Option<String> {
    let line = line.trim_start();
    let line = line
        .strip_prefix("pub fn ")
        .or_else(|| line.strip_prefix("pub(crate) fn "))
        .or_else(|| line.strip_prefix("fn "))
        .or_else(|| line.strip_prefix("async fn "))
        .or_else(|| line.strip_prefix("pub async fn "))
        .or_else(|| line.strip_prefix("pub(crate) async fn "))?;
    let name = line
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .next()
        .filter(|name| !name.is_empty())?;
    Some(name.to_string())
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "target" || name == ".git" || name == "node_modules"
    })
}

fn display_path(path: &Path, repo_root: &Path) -> String {
    let display = path
        .strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    display.replace('\\', "/")
}
