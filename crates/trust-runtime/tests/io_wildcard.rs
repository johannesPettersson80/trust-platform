use trust_runtime::harness::TestHarness;

#[test]
fn wildcard_requires_var_config() {
    let source = r#"
PROGRAM Main
VAR
    out AT %Q* : BOOL;
END_VAR
out := TRUE;
END_PROGRAM
"#;

    assert!(TestHarness::from_source(source).is_err());
}

#[test]
fn wildcard_area_mismatch() {
    let source = r#"
PROGRAM Main
VAR
    out AT %Q* : BOOL;
END_VAR
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_CONFIG
    P1.out AT %IX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#;

    assert!(TestHarness::from_source(source).is_err());
}

#[test]
fn wildcard_not_allowed_in_var_input() {
    let source = r#"
PROGRAM Main
VAR_INPUT
    inp AT %I* : BOOL;
END_VAR
END_PROGRAM
"#;

    assert!(TestHarness::from_source(source).is_err());
}

#[test]
fn wildcard_memory_area_mismatch() {
    let source = r#"
PROGRAM Main
VAR
    marker AT %M* : BOOL;
END_VAR
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_CONFIG
    P1.marker AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#;

    assert!(TestHarness::from_source(source).is_err());
}
