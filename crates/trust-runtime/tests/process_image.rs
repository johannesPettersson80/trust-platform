use trust_runtime::bytecode::{
    BytecodeMetadata, BytecodeVersion, ProcessImageConfig, ResourceMetadata,
    SUPPORTED_MAJOR_VERSION,
};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::PROCESS_IMAGE_AREA_LIMIT;
use trust_runtime::task::TaskConfig;
use trust_runtime::value::Duration;

#[test]
fn sized_from_metadata() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let resource = ResourceMetadata {
        name: "R".into(),
        process_image: ProcessImageConfig {
            inputs: 16,
            outputs: 8,
            memory: 4,
        },
        tasks: Vec::new(),
    };
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![resource],
    };

    runtime.apply_bytecode_metadata(&metadata, None).unwrap();

    assert_eq!(runtime.io().inputs().len(), 16);
    assert_eq!(runtime.io().outputs().len(), 8);
    assert_eq!(runtime.io().memory().len(), 4);
}

#[test]
fn named_resource_selection_rejects_unknown_name_without_primary_fallback() {
    let source = "PROGRAM Main\nEND_PROGRAM";
    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    runtime
        .apply_resource_metadata(&ResourceMetadata {
            name: "R".into(),
            process_image: ProcessImageConfig {
                inputs: 4,
                outputs: 2,
                memory: 1,
            },
            tasks: Vec::new(),
        })
        .unwrap();
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![
            ResourceMetadata {
                name: "ResourceA".into(),
                process_image: ProcessImageConfig {
                    inputs: 16,
                    outputs: 8,
                    memory: 4,
                },
                tasks: Vec::new(),
            },
            ResourceMetadata {
                name: "ResourceB".into(),
                process_image: ProcessImageConfig {
                    inputs: 32,
                    outputs: 16,
                    memory: 8,
                },
                tasks: Vec::new(),
            },
        ],
    };

    let error = runtime
        .apply_bytecode_metadata(&metadata, Some("MissingResource"))
        .expect_err("unknown named resource must not select the primary entry");

    assert!(error.to_string().contains("MissingResource"), "{error}");
    assert_eq!(runtime.io().inputs().len(), 4);
    assert_eq!(runtime.io().outputs().len(), 2);
    assert_eq!(runtime.io().memory().len(), 1);
}

#[test]
fn named_resource_selection_accepts_single_legacy_placeholder() {
    let source = "PROGRAM Main\nEND_PROGRAM";
    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![ResourceMetadata {
            name: "RESOURCE".into(),
            process_image: ProcessImageConfig {
                inputs: 7,
                outputs: 5,
                memory: 3,
            },
            tasks: Vec::new(),
        }],
    };

    runtime
        .apply_bytecode_metadata(&metadata, Some("LegacyProjectName"))
        .expect("single legacy placeholder remains compatible");

    assert_eq!(runtime.io().inputs().len(), 7);
    assert_eq!(runtime.io().outputs().len(), 5);
    assert_eq!(runtime.io().memory().len(), 3);
    let rebuilt = trust_runtime::bytecode::build_module_from_runtime(&runtime)
        .expect("re-encode runtime after legacy compatibility load");
    assert!(rebuilt
        .metadata()
        .expect("read rebuilt metadata")
        .resource("LegacyProjectName")
        .is_some());
}

#[test]
fn metadata_size_above_process_image_cap_is_rejected() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![ResourceMetadata {
            name: "R".into(),
            process_image: ProcessImageConfig {
                inputs: PROCESS_IMAGE_AREA_LIMIT + 1,
                outputs: 0,
                memory: 0,
            },
            tasks: Vec::new(),
        }],
    };

    let err = runtime
        .apply_bytecode_metadata(&metadata, None)
        .expect_err("oversized bytecode process image must be rejected");
    assert!(
        err.to_string().contains("process image area limit"),
        "expected process-image cap error, got {err}"
    );
    // apply_resource_metadata checks the input area first (installed_lifecycle.rs), and the
    // shared helper builds this exact InvalidIoAddress text (io_image/mod.rs:237-249).
    assert_eq!(
        err,
        RuntimeError::InvalidIoAddress(
            format!("input exceeds {PROCESS_IMAGE_AREA_LIMIT} byte process image area limit")
                .into()
        )
    );
}

#[test]
fn invalid_metadata_task_does_not_partially_resize_process_image() {
    let source = r#"
PROGRAM Main
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    runtime
        .apply_resource_metadata(&ResourceMetadata {
            name: "R".into(),
            process_image: ProcessImageConfig {
                inputs: 4,
                outputs: 2,
                memory: 1,
            },
            tasks: Vec::new(),
        })
        .unwrap();
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![ResourceMetadata {
            name: "R".into(),
            process_image: ProcessImageConfig {
                inputs: 16,
                outputs: 8,
                memory: 4,
            },
            tasks: vec![TaskConfig {
                name: "Broken".into(),
                interval: Duration::from_millis(10),
                single: None,
                priority: 0,
                programs: vec!["MissingProgram".into()],
                fb_instances: Vec::new(),
            }],
        }],
    };

    runtime
        .apply_bytecode_metadata(&metadata, None)
        .expect_err("metadata with missing program must fail");

    assert_eq!(runtime.io().inputs().len(), 4);
    assert_eq!(runtime.io().outputs().len(), 2);
    assert_eq!(runtime.io().memory().len(), 1);
}

#[test]
fn source_binding_above_process_image_cap_is_rejected() {
    let source = format!(
        r#"
PROGRAM Main
VAR
    marker AT %MB{} : BYTE;
END_VAR
END_PROGRAM
"#,
        PROCESS_IMAGE_AREA_LIMIT
    );

    let err = match TestHarness::from_source(&source) {
        Ok(_) => panic!("oversized ST binding should fail compilation"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("process image area limit"),
        "expected process-image cap error, got {err}"
    );
}

#[test]
fn unused_declared_globals_and_program_members_can_be_bound_after_admission() {
    let mut harness = TestHarness::from_source(
        "VAR_GLOBAL unused_global : BOOL := TRUE; END_VAR PROGRAM Main VAR unused_member : WORD := WORD#16#1234; END_VAR END_PROGRAM"
    ).unwrap();
    harness.bind_direct("unused_global", "%QX0.0").unwrap();
    harness.bind_direct("unused_member", "%QW2").unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        harness.get_direct_output("%QX0.0").unwrap(),
        trust_runtime::value::Value::Bool(true)
    );
    assert_eq!(
        harness.get_direct_output("%QW2").unwrap(),
        trust_runtime::value::Value::Word(0x1234)
    );
}
