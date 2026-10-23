//! Pin rejection when release packaging loses the locked tool or target binaries.
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn vsix_contract_rejects_unlocked_tool_missing_target_and_missing_binaries() {
    let workspace = super::workspace_root();
    let workflow = fs::read_to_string(workspace.join(".github/workflows/release.yml")).unwrap();
    let fixture = std::env::temp_dir().join(format!(
        "trust-vsix-contract-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(fixture.join(".github/workflows")).unwrap();
    let fixtures = std::iter::once(("complete", workflow.clone())).chain(
        [
            "--no-install ",
            "--target ${{ matrix.platform }}",
            "cp target/${{ matrix.target }}/release/trust-lsp",
            "cp target/${{ matrix.target }}/release/trust-debug",
            "cp target/${{ matrix.target }}/release/trust-runtime",
            "trust-lsp.exe editors/vscode/bin/",
            "trust-debug.exe editors/vscode/bin/",
            "trust-runtime.exe editors/vscode/bin/",
        ]
        .into_iter()
        .map(|fragment| {
            assert!(workflow.contains(fragment), "fixture is missing {fragment}");
            (fragment, workflow.replace(fragment, "REMOVED"))
        }),
    );
    for (label, source) in fixtures {
        fs::write(fixture.join(".github/workflows/release.yml"), source).unwrap();
        // Fixture wiring only: the existing checker owns validation; Rust owns
        // mutation construction and executable acceptance/rejection assertions.
        let output = Command::new(super::python_program())
            .args(["-c", "import json,sys; from pathlib import Path; from scripts import release_claim_contract as contract; contract.ROOT=Path(sys.argv[1]); print(json.dumps(contract.audit_vsix()))"])
            .arg(&fixture).current_dir(&workspace).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if label == "complete" {
            assert!(output.status.success(), "{stderr}");
            let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result["embedded_binaries"], 3);
            assert_eq!(result["targets"].as_array().unwrap().len(), 5);
        } else {
            assert!(!output.status.success(), "accepted missing {label}");
            assert!(
                stderr.contains("VSIX target contract missing reviewed fragments"),
                "{stderr}"
            );
        }
    }
    fs::remove_dir_all(fixture).unwrap();
}
