//! Batch-only source/case pin refresh. Historical evidence and case bodies are immutable.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn read(path: &Path) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}
fn records(path: &Path) -> Result<toml::Value> {
    Ok(toml::from_str(&read(path)?)?)
}
fn field<'a>(value: &'a toml::Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("missing {name}").into())
}
fn files(root: &Path, suffix: &str, result: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            files(&path, suffix, result)?;
        } else if path.extension().is_some_and(|ext| ext == suffix) {
            result.push(path);
        }
    }
    Ok(())
}
fn replace_pin(text: &str, old: &str, new: &str) -> Result<String> {
    if !text.contains(old) {
        return Err(format!("missing expected pin {old}").into());
    }
    Ok(text.replace(old, new))
}
fn record_replacement(
    text: &mut String,
    seen: &mut BTreeMap<String, String>,
    old: &str,
    new: &str,
) -> Result<()> {
    if let Some(previous) = seen.get(old) {
        if previous != new {
            return Err("one old pin maps to conflicting new digests".into());
        }
        return Ok(());
    }
    *text = replace_pin(text, old, new)?;
    seen.insert(old.to_owned(), new.to_owned());
    Ok(())
}
fn select_mutation<'a>(
    mutation: &toml::Value,
    candidates: &'a [serde_json::Value],
) -> Result<&'a serde_json::Value> {
    let function = field(mutation, "function")?;
    let genre = field(mutation, "genre")?;
    let replacement = field(mutation, "replacement")?;
    let exact = field(mutation, "selector_name")?;
    let matches: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate["function"]["function_name"].as_str() == Some(function)
                && candidate["genre"].as_str() == Some(genre)
                && candidate["replacement"].as_str() == Some(replacement)
        })
        .collect();
    matches.iter().find(|candidate| candidate["name"].as_str() == Some(exact)).copied()
        .or_else(|| (matches.len() == 1).then(|| matches[0]))
        .ok_or_else(|| format!(
            "mutation {} in {}: {} matching selectors for function={function:?}, genre={genre:?}, replacement={replacement:?}; requested {exact:?}; review before refreshing",
            field(mutation, "id").unwrap_or("<missing id>"),
            field(mutation, "source_file").unwrap_or("<missing source>"),
            matches.len(),
        ).into())
}

fn native_pin_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for package in ["trust-runtime", "trust-runtime-core"] {
        for directory in ["src", "tests"] {
            files(
                &root.join("crates").join(package).join(directory),
                "rs",
                &mut paths,
            )?;
        }
    }
    Ok(paths)
}
fn invariant_source_digest(invariant_path: &str) -> Result<String> {
    // Reuse the authoritative execution-contract projection. The CLI case
    // generator cannot refresh stale metadata because it validates it first.
    let output = Command::new("python3").args(["-c", "import sys,tomllib; from pathlib import Path; from scripts.verification.execution_contract import invariant_execution_contract_digest; print(invariant_execution_contract_digest(tomllib.loads(Path(sys.argv[1]).read_text())))", invariant_path]).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    let source_digest = String::from_utf8(output.stdout)?.trim().to_owned();
    if !source_digest.starts_with("sha256:") || source_digest.len() != 71 {
        return Err("invalid generator digest output".into());
    }
    Ok(source_digest)
}

fn main() -> Result<()> {
    // Run from repository root after formatting, before freezing or native tests.
    let evidence = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("missing task evidence directory")?,
    );
    fs::create_dir_all(&evidence)?;
    let mut writes = BTreeMap::<PathBuf, String>::new();
    let native = native_pin_paths(Path::new(""))?;
    let mut cases = Vec::new();
    files(
        Path::new("verification/cases/bytecode_vm"),
        "toml",
        &mut cases,
    )?;
    let catalog = PathBuf::from("verification/test-catalog.toml");
    writes.insert(catalog.clone(), read(&catalog)?);
    for path in cases {
        let original = read(&path)?;
        let record: toml::Value = toml::from_str(&original)?;
        let invariant = field(&record, "invariant")?;
        let invariant_path = format!("verification/invariants/bytecode_vm/{invariant}.toml");
        let source_digest = invariant_source_digest(&invariant_path)?;
        let updated = replace_pin(&original, field(&record, "source_digest")?, &source_digest)?;
        let old = digest(original.as_bytes());
        let new = digest(updated.as_bytes());
        if old == new {
            continue;
        }
        let path_text = path.to_str().ok_or("non-UTF8 case path")?;
        let catalog_text = writes.get_mut(&catalog).ok_or("catalog missing")?;
        if catalog_text.contains(&old) {
            *catalog_text = replace_pin(catalog_text, &old, &new)?;
        }
        for source in &native {
            let text = writes.get(source).cloned().unwrap_or(read(source)?);
            if text.contains(path_text) {
                writes.insert(source.clone(), replace_pin(&text, &old, &new)?);
            }
        }
        writes.insert(path, updated);
    }
    // Refresh active planned selectors only. Never rewrite dated measurement files.
    let manifest = PathBuf::from("verification/mutation-program.toml");
    let record = records(&manifest)?;
    let mut text = read(&manifest)?;
    let mut digest_updates = BTreeMap::new();
    let mut selector_updates = BTreeMap::new();
    for shard in record
        .get("shards")
        .and_then(toml::Value::as_array)
        .ok_or("shards missing")?
    {
        if field(shard, "execution_status")? != "planned" {
            return Err("refuse to repin a measured shard".into());
        }
        for mutation in shard
            .get("mutations")
            .and_then(toml::Value::as_array)
            .ok_or("mutations missing")?
        {
            let source = field(mutation, "source_file")?;
            let source_digest = digest(&fs::read(source)?);
            record_replacement(
                &mut text,
                &mut digest_updates,
                field(mutation, "source_digest")?,
                &source_digest,
            )?;
            let output = Command::new("cargo")
                .args(["mutants", "--Zmutate-file", source, "--list", "--json"])
                .output()?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
            }
            fs::write(
                evidence.join(format!("{}-selectors.json", field(mutation, "id")?)),
                &output.stdout,
            )?;
            let candidates: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
            let selected = select_mutation(mutation, &candidates)?;
            let exact = field(mutation, "selector_name")?;
            record_replacement(
                &mut text,
                &mut selector_updates,
                exact,
                selected["name"].as_str().ok_or("selector name missing")?,
            )?;
        }
    }
    writes.insert(manifest, text);
    // Prepare all changes before writing; a discovery/provenance failure leaves
    // the candidate source untouched. Static literal test pins stay explicit.
    for (path, text) in writes {
        if read(&path)? != text {
            fs::write(&path, text)?;
            println!("updated {}", path.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_mutation() -> toml::Value {
        toml::from_str(
            r#"
id = "MUTANT_PROBE"
source_file = "crates/probe/src/current.rs"
function = "probe"
genre = "BinaryOperator"
replacement = "!="
selector_name = "current.rs:10:5: replace == with != in probe"
"#,
        )
        .unwrap()
    }

    fn candidate(name: &str) -> serde_json::Value {
        serde_json::json!({
            "function": { "function_name": "probe" },
            "genre": "BinaryOperator", "replacement": "!=", "name": name,
        })
    }

    #[test]
    fn selection_preserves_exact_operator_identity_and_reports_ambiguity() {
        let mutation = synthetic_mutation();
        let candidates = vec![
            candidate("first"),
            candidate(field(&mutation, "selector_name").unwrap()),
            candidate("third"),
        ];
        assert_eq!(
            select_mutation(&mutation, &candidates).unwrap(),
            &candidates[1]
        );
        let ambiguous = vec![candidate("first"), candidate("third")];
        let error = select_mutation(&mutation, &ambiguous)
            .unwrap_err()
            .to_string();
        assert!(error.contains("MUTANT_PROBE in crates/probe/src/current.rs"));
        assert!(error.contains("2 matching selectors"));
        assert!(error.contains("probe"));
    }

    #[test]
    fn missing_owner_reports_zero_matches_and_unique_relocation_remains_supported() {
        let mutation = synthetic_mutation();
        let error = select_mutation(&mutation, &[]).unwrap_err().to_string();
        assert!(error.contains("MUTANT_PROBE in crates/probe/src/current.rs"));
        assert!(error.contains("0 matching selectors"));
        let mut function = mutation;
        function["genre"] = toml::Value::String("FnValue".into());
        function["replacement"] = toml::Value::String("false".into());
        let candidates = vec![serde_json::json!({
            "function": { "function_name": "probe" }, "genre": "FnValue",
            "replacement": "false", "name": "moved.rs:63:5: replace probe -> bool with false",
        })];
        assert_eq!(
            select_mutation(&function, &candidates).unwrap(),
            &candidates[0]
        );
    }

    #[test]
    fn shared_source_digests_refresh_once_without_accepting_missing_or_conflicting_pins() {
        let mut text = "source_digest = old\nsource_digest = old\n".to_owned();
        let mut seen = BTreeMap::new();
        record_replacement(&mut text, &mut seen, "old", "new").unwrap();
        record_replacement(&mut text, &mut seen, "old", "new").unwrap();
        assert_eq!(text, "source_digest = new\nsource_digest = new\n");
        assert!(record_replacement(&mut text, &mut seen, "old", "different").is_err());
        assert!(record_replacement(&mut text, &mut seen, "missing", "new").is_err());
    }

    #[test]
    fn discovery_includes_unit_and_integration_pin_files_in_both_crates() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-a4-provenance-{}-{stamp}",
            std::process::id()
        ));
        let mut expected = Vec::new();
        for package in ["trust-runtime", "trust-runtime-core"] {
            for directory in ["src/runtime/vm/type_policy", "tests"] {
                let directory = root.join("crates").join(package).join(directory);
                fs::create_dir_all(&directory).unwrap();
                let path = directory.join("pins.rs");
                fs::write(&path, "const CASE_FILE: &str = \"verification/cases/bytecode_vm/probe.toml\";\nconst PIN: &str = \"sha256:old\";\n").unwrap();
                expected.push(path);
            }
        }
        let mut actual = native_pin_paths(&root).unwrap();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        for path in actual {
            let updated = replace_pin(&read(&path).unwrap(), "sha256:old", "sha256:new").unwrap();
            assert!(updated.contains("sha256:new"));
            assert!(!updated.contains("sha256:old"));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
