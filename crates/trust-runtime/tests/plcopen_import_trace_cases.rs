use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use trust_runtime::plcopen::import_xml_to_project;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_PLCOPEN_IMPORT_TRACE_001";
const CASE_FILE: &str = "verification/cases/plcopen_devtools/PLCO_IMPORT_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:1a44c22e641c00fdc7ec9a0f0ddb3cdf1cec59b3626d22558cc3731e7914101e";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[test]
fn plcopen_import_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = PlcopenProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_import_case);

    let artifact = result.expect("PLCopen case artifact must be written");
    let failed = artifact
        .cases
        .iter()
        .filter(|case| case.result != CaseResult::Passed)
        .map(|case| {
            format!(
                "{}: {}",
                case.id,
                case.observed_error.as_deref().unwrap_or("not passed")
            )
        })
        .collect::<Vec<_>>();
    assert!(
        failed.is_empty(),
        "PLCopen import failures: {}",
        failed.join("; ")
    );
}

fn run_import_case(case: &CaseRecord, probe: &mut PlcopenProbe) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let root = unique_temp_dir(scenario)?;
    let xml_path = root.join("input.xml");
    let project = root.join("project");
    std::fs::create_dir_all(&project).map_err(|error| error.to_string())?;
    std::fs::write(&xml_path, xml_for_scenario(scenario)?).map_err(|error| error.to_string())?;

    let outcome = inspect_import(scenario, &xml_path, &project);
    let _ = std::fs::remove_dir_all(&root);
    let observed = outcome?;
    probe.observed = Some(observed.clone());

    if observed["passed"] == true {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("import_contract_match".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(
                observed["detail"]
                    .as_str()
                    .unwrap_or("PLCopen import contract mismatch")
                    .to_string(),
            ),
            observed_status: Some("import_contract_mismatch".to_string()),
        })
    }
}

fn inspect_import(
    scenario: &str,
    xml_path: &Path,
    project: &Path,
) -> Result<serde_json::Value, String> {
    let result = import_xml_to_project(xml_path, project);
    let expected_code = match scenario {
        "FBD_BODY_REJECTED" => Some("PLCO215"),
        "LD_BODY_REJECTED" => Some("PLCO216"),
        "SFC_BODY_REJECTED" => Some("PLCO217"),
        "UNKNOWN_EXECUTABLE_BODY_REJECTED" => Some("PLCO218"),
        _ => None,
    };

    if let Some(code) = expected_code {
        let error = match result {
            Ok(report) => {
                return Ok(serde_json::json!({
                    "detail": format!("unsupported body unexpectedly imported {} sources", report.written_sources.len()),
                    "passed": false,
                }))
            }
            Err(error) => error,
        };
        let migration = read_migration_report(project)?;
        let has_code = migration["unsupported_diagnostics"]
            .as_array()
            .is_some_and(|diagnostics| {
                diagnostics
                    .iter()
                    .any(|entry| entry["code"] == code && entry["severity"] == "error")
            });
        let no_sources = !contains_st_file(&project.join("src"));
        let rejected = error
            .to_string()
            .contains("no importable PLCopen ST content");
        return Ok(serde_json::json!({
            "diagnostic": code,
            "detail": format!("rejected={rejected}, diagnostic={has_code}, no_sources={no_sources}"),
            "passed": rejected && has_code && no_sources,
        }));
    }

    let report = result.map_err(|error| format!("supported PLCopen import failed: {error:#}"))?;
    let sources = report
        .written_sources
        .iter()
        .map(|path| std::fs::read_to_string(path).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let diagnostics = report
        .unsupported_diagnostics
        .iter()
        .map(|entry| entry.code.as_str())
        .collect::<Vec<_>>();
    let passed = match scenario {
        "ST_BODY_IMPORT" => {
            report.imported_pous == 1
                && report.discovered_pous == 1
                && sources.len() == 1
                && sources[0].contains("PROGRAM Main")
                && diagnostics.is_empty()
        }
        "ST_BODY_WITH_BENIGN_METADATA" => {
            report.imported_pous == 1
                && report.discovered_pous == 1
                && sources.len() == 1
                && sources[0].contains("PROGRAM Main")
                && !diagnostics.contains(&"PLCO218")
        }
        "MIXED_ST_AND_UNSUPPORTED_BODY" => {
            report.imported_pous == 1
                && report.discovered_pous == 2
                && sources.len() == 1
                && sources[0].contains("PROGRAM Main")
                && diagnostics.contains(&"PLCO215")
        }
        other => return Err(format!("unreviewed PLCopen scenario {other}")),
    };
    Ok(serde_json::json!({
        "detail": format!(
            "discovered={}, imported={}, sources={}, diagnostics={diagnostics:?}",
            report.discovered_pous,
            report.imported_pous,
            sources.len()
        ),
        "passed": passed,
    }))
}

fn xml_for_scenario(scenario: &str) -> Result<String, String> {
    let bodies = match scenario {
        "ST_BODY_IMPORT" => st_pou("Main", ""),
        "ST_BODY_WITH_BENIGN_METADATA" => st_pou(
            "Main",
            "<addData><data name=\"vendor.lineMap\"><text>line metadata only</text></data></addData>",
        ),
        "FBD_BODY_REJECTED" => non_st_pou("FbdPump", "FBD", "<network/>") ,
        "LD_BODY_REJECTED" => non_st_pou("LadderPump", "LD", "<leftPowerRail/>") ,
        "SFC_BODY_REJECTED" => non_st_pou("Sequence", "SFC", "<step name=\"Init\"/>") ,
        "UNKNOWN_EXECUTABLE_BODY_REJECTED" => {
            non_st_pou("VendorGraph", "vendorGraph", "<label>not ST</label>")
        }
        "MIXED_ST_AND_UNSUPPORTED_BODY" => format!(
            "{}{}",
            st_pou("Main", ""),
            non_st_pou("FbdPump", "FBD", "<network/>")
        ),
        other => return Err(format!("unreviewed PLCopen scenario {other}")),
    };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://www.plcopen.org/xml/tc6_0200\"><types><pous>{bodies}</pous></types></project>\n"
    ))
}

fn st_pou(name: &str, metadata: &str) -> String {
    format!(
        "<pou name=\"{name}\" pouType=\"program\"><body><ST><![CDATA[PROGRAM {name}\nVAR\n  value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM]]></ST>{metadata}</body></pou>"
    )
}

fn non_st_pou(name: &str, tag: &str, body: &str) -> String {
    format!("<pou name=\"{name}\" pouType=\"program\"><body><{tag}>{body}</{tag}></body></pou>")
}

fn read_migration_report(project: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(project.join("interop/plcopen-migration-report.json"))
        .map_err(|error| format!("migration report missing: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("migration report invalid: {error}"))
}

fn contains_st_file(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        (path.is_dir() && contains_st_file(&path))
            || path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("st"))
    })
}

fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "trust-plcopen-case-{}-{serial}-{prefix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(root)
}

#[derive(Default)]
struct PlcopenProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for PlcopenProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_before {
            self.observed = None;
        }
        self.next_snapshot_is_before = !self.next_snapshot_is_before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.observed.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
