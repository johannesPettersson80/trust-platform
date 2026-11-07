// Specification 14 §6.6.2: a command that compiles the project compiles it under the project's
// `vendor_profile`, as its build does.

/// A tank program whose PI block has a `Reset` method without an access specifier, which the
/// program calls: PUBLIC under the CODESYS rules (DEV-028), PROTECTED under the IEC rules
/// (IEC 61131-3 Ed.3 6.6.5.9).
const CATALOG_PROFILE_TANK: &str = "FUNCTION_BLOCK FB_PI
VAR_INPUT
    rSetpoint : REAL;
    rActual : REAL;
END_VAR
VAR_OUTPUT
    rOutput : REAL;
END_VAR
VAR
    _rKp : REAL := 2.0;
    _rIntegral : REAL;
END_VAR
rOutput := _rKp * (rSetpoint - rActual) + _rIntegral;
METHOD Reset
_rIntegral := 0.0;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM PRG_Tank
VAR
    fbPI : FB_PI;
    bReset : BOOL;
END_VAR
IF bReset THEN
    fbPI.Reset();
END_IF
fbPI(rSetpoint := 50.0, rActual := 40.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : PRG_Tank;
END_RESOURCE
END_CONFIGURATION
";

/// The catalog answer of a project that holds the tank program, with `vendor_profile` set to
/// `profile` in its settings.
fn catalog_under_profile(profile: &str) -> Value {
    let root = temp_dir("trustlsp-catalog-profile");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("trust-lsp.toml"),
        format!("[project]\ninclude_paths = [\"src\"]\nvendor_profile = \"{profile}\"\n"),
    )
    .expect("write settings");
    std::fs::write(root.join("src").join("main.st"), CATALOG_PROFILE_TANK).expect("write source");
    let root_uri = Url::from_directory_path(&root).expect("root uri");
    let answer = hmi_bindings_value_with_context(
        &MockContext::default(),
        vec![json!({ "root_uri": root_uri.as_str() })],
    )
    .expect("catalog answer");
    std::fs::remove_dir_all(root).expect("remove temp dir");
    answer
}

/// Catches the catalog compiled under the IEC rules whatever the project's profile: Control
/// Studio's setup and the HMI Builder's variable list read it, and they refused a CODESYS project
/// whose method without an access specifier its own build accepts.
#[test]
fn the_catalog_compiles_the_project_under_its_vendor_profile() {
    let codesys = catalog_under_profile("codesys");
    assert_eq!(codesys["ok"], true, "{codesys}");
    let iec = catalog_under_profile("iec");
    assert_eq!(iec["ok"], false, "{iec}");
    assert!(
        iec["error"].as_str().unwrap_or_default().contains("Reset"),
        "{iec}"
    );
}

/// Catches a misspelled profile compiled under other rules: the catalog refuses it by name, as
/// the build does.
#[test]
fn the_catalog_refuses_an_unknown_vendor_profile_as_the_build_does() {
    let answer = catalog_under_profile("codesis");
    assert_eq!(answer["ok"], false, "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown vendor_profile 'codesis'"),
        "{answer}"
    );
}
