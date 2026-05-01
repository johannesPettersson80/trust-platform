mod bytecode_helpers;

use smol_str::SmolStr;
use trust_runtime::bytecode::*;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::task::TaskConfig;
use trust_runtime::value::{Duration, Value};

#[test]
fn tasks_from_metadata() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let task = TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 0,
        programs: vec!["Main".into()],
        fb_instances: Vec::new(),
    };
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![ResourceMetadata {
            name: "R".into(),
            process_image: ProcessImageConfig::default(),
            tasks: vec![task],
        }],
    };

    runtime.apply_bytecode_metadata(&metadata, None).unwrap();
    runtime.set_current_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();

    let program_id = match runtime.storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };
    let value = runtime.storage().get_instance_var(program_id, "counter");
    assert!(matches!(
        value,
        Some(Value::Int(1) | Value::DInt(1) | Value::LInt(1))
    ));
}

#[test]
fn apply_bytecode_bytes() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM

CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
PROGRAM Main : Main;
END_CONFIGURATION
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let module = build_module_from_runtime(&runtime).unwrap();
    let bytes = module.encode().unwrap();

    runtime.apply_bytecode_bytes(&bytes, None).unwrap();
    runtime.set_current_time(Duration::from_millis(1));
    runtime.execute_cycle().unwrap();

    let program_id = match runtime.storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };
    let value = runtime.storage().get_instance_var(program_id, "counter");
    assert!(matches!(
        value,
        Some(Value::Int(1) | Value::DInt(1) | Value::LInt(1))
    ));
}

#[test]
fn version_gate() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION + 1, 0),
        resources: Vec::new(),
    };

    let err = runtime
        .apply_bytecode_metadata(&metadata, None)
        .unwrap_err();
    assert!(matches!(
        err,
        RuntimeError::UnsupportedBytecodeVersion { .. }
    ));
}

#[test]
fn pou_associations() {
    let source = r#"
FUNCTION_BLOCK FB
VAR_INPUT
    IN : BOOL;
END_VAR
VAR_OUTPUT
    OUT : BOOL;
END_VAR
OUT := IN;
END_FUNCTION_BLOCK

PROGRAM P
VAR
    fb : FB;
END_VAR
END_PROGRAM

CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
PROGRAM P : P;
END_CONFIGURATION
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();

    let program_id = match runtime.storage().get_global("P") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };
    let fb_ref = runtime
        .storage()
        .ref_for_instance(program_id, "fb")
        .expect("expected FB reference");
    let fb_instance_id = match runtime.storage().read_by_ref(fb_ref.clone()) {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected FB instance, got {other:?}"),
    };

    runtime
        .storage_mut()
        .set_instance_var(fb_instance_id, "IN", Value::Bool(true));

    let task = TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(0),
        single: Some("trigger".into()),
        priority: 0,
        programs: Vec::new(),
        fb_instances: vec![fb_ref.clone()],
    };
    let metadata = BytecodeMetadata {
        version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, 0),
        resources: vec![ResourceMetadata {
            name: "R".into(),
            process_image: ProcessImageConfig::default(),
            tasks: vec![task],
        }],
    };

    runtime.apply_bytecode_metadata(&metadata, None).unwrap();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();

    let out = runtime.storage().get_instance_var(fb_instance_id, "OUT");
    assert_eq!(out, Some(&Value::Bool(true)));
}

fn ensure_string(strings: &mut StringTable, name: &str) -> u32 {
    if let Some(index) = strings
        .entries
        .iter()
        .position(|entry| entry.as_str() == name)
    {
        return index as u32;
    }
    strings.entries.push(name.into());
    (strings.entries.len() - 1) as u32
}

fn ref_entry_from_value_ref(
    value_ref: &trust_runtime::value::ValueRef,
    strings: &mut StringTable,
) -> RefEntry {
    let (location, owner_id) = match value_ref.location {
        trust_runtime::memory::MemoryLocation::Global => (RefLocation::Global, 0),
        trust_runtime::memory::MemoryLocation::Local(frame) => (RefLocation::Local, frame.0),
        trust_runtime::memory::MemoryLocation::Instance(instance) => {
            (RefLocation::Instance, instance.0)
        }
        trust_runtime::memory::MemoryLocation::Io(area) => {
            let owner_id = match area {
                trust_runtime::memory::IoArea::Input => 0,
                trust_runtime::memory::IoArea::Output => 1,
                trust_runtime::memory::IoArea::Memory => 2,
            };
            (RefLocation::Io, owner_id)
        }
        trust_runtime::memory::MemoryLocation::Retain => (RefLocation::Retain, 0),
    };
    let segments = value_ref
        .path
        .iter()
        .map(|segment| match segment {
            trust_runtime::value::RefSegment::Index(indices) => RefSegment::Index(indices.to_vec()),
            trust_runtime::value::RefSegment::Field(name) => {
                let idx = ensure_string(strings, name.as_str());
                RefSegment::Field { name_idx: idx }
            }
        })
        .collect();
    RefEntry {
        location,
        owner_id,
        offset: value_ref.offset as u32,
        segments,
    }
}

#[test]
fn resources_from_container() {
    let strings = StringTable {
        entries: vec!["R".into(), "T".into(), "Main".into(), "trigger".into()],
    };
    let resource_meta = ResourceMeta {
        resources: vec![ResourceEntry {
            name_idx: 0,
            inputs_size: 4,
            outputs_size: 8,
            memory_size: 16,
            tasks: vec![TaskEntry {
                name_idx: 1,
                priority: 0,
                interval_nanos: 1_000_000,
                single_name_idx: Some(3),
                program_name_idx: vec![2],
                fb_ref_idx: Vec::new(),
            }],
        }],
    };
    let module = BytecodeModule {
        version: BytecodeVersion::new(1, 0),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings.clone()),
            },
            Section {
                id: SectionId::RefTable.as_raw(),
                flags: 0,
                data: SectionData::RefTable(RefTable::default()),
            },
            Section {
                id: SectionId::ResourceMeta.as_raw(),
                flags: 0,
                data: SectionData::ResourceMeta(resource_meta),
            },
        ],
    };
    let bytes = module.encode().unwrap();
    let decoded = BytecodeModule::decode(&bytes).unwrap();
    let metadata = decoded.metadata().unwrap();
    let resource = metadata.resources.first().expect("resource");
    assert_eq!(resource.process_image.inputs, 4);
    assert_eq!(resource.process_image.outputs, 8);
    assert_eq!(resource.process_image.memory, 16);
}

#[test]
fn task_associations() {
    let strings = StringTable {
        entries: vec![
            "R".into(),
            "T".into(),
            "Main".into(),
            "Aux".into(),
            "trigger".into(),
        ],
    };
    let resource_meta = ResourceMeta {
        resources: vec![ResourceEntry {
            name_idx: 0,
            inputs_size: 0,
            outputs_size: 0,
            memory_size: 0,
            tasks: vec![TaskEntry {
                name_idx: 1,
                priority: 1,
                interval_nanos: 0,
                single_name_idx: Some(4),
                program_name_idx: vec![2, 3],
                fb_ref_idx: Vec::new(),
            }],
        }],
    };
    let module = BytecodeModule {
        version: BytecodeVersion::new(1, 0),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings.clone()),
            },
            Section {
                id: SectionId::RefTable.as_raw(),
                flags: 0,
                data: SectionData::RefTable(RefTable::default()),
            },
            Section {
                id: SectionId::ResourceMeta.as_raw(),
                flags: 0,
                data: SectionData::ResourceMeta(resource_meta),
            },
        ],
    };
    let decoded = BytecodeModule::decode(&module.encode().unwrap()).unwrap();
    let metadata = decoded.metadata().unwrap();
    let task = &metadata.resources[0].tasks[0];
    assert_eq!(
        task.programs,
        vec![SmolStr::new("Main"), SmolStr::new("Aux")]
    );
    assert_eq!(task.single, Some("trigger".into()));
}

#[test]
fn fb_refs_from_container() {
    let source = r#"
FUNCTION_BLOCK FB
VAR_INPUT
    IN : BOOL;
END_VAR
VAR_OUTPUT
    OUT : BOOL;
END_VAR
OUT := IN;
END_FUNCTION_BLOCK

PROGRAM P
VAR
    fb : FB;
END_VAR
END_PROGRAM

CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
PROGRAM P : P;
END_CONFIGURATION
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let program_id = match runtime.storage().get_global("P") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };
    let fb_ref = runtime
        .storage()
        .ref_for_instance(program_id, "fb")
        .expect("expected FB reference");

    let mut strings = StringTable {
        entries: vec!["R".into(), "T".into(), "P".into(), "trigger".into()],
    };
    let ref_entry = ref_entry_from_value_ref(&fb_ref, &mut strings);
    let ref_table = RefTable {
        entries: vec![ref_entry],
    };
    let resource_meta = ResourceMeta {
        resources: vec![ResourceEntry {
            name_idx: 0,
            inputs_size: 0,
            outputs_size: 0,
            memory_size: 0,
            tasks: vec![TaskEntry {
                name_idx: 1,
                priority: 0,
                interval_nanos: 0,
                single_name_idx: Some(3),
                program_name_idx: Vec::new(),
                fb_ref_idx: vec![0],
            }],
        }],
    };

    let module = BytecodeModule {
        version: BytecodeVersion::new(1, 0),
        flags: 0,
        sections: vec![
            Section {
                id: SectionId::StringTable.as_raw(),
                flags: 0,
                data: SectionData::StringTable(strings),
            },
            Section {
                id: SectionId::RefTable.as_raw(),
                flags: 0,
                data: SectionData::RefTable(ref_table),
            },
            Section {
                id: SectionId::ResourceMeta.as_raw(),
                flags: 0,
                data: SectionData::ResourceMeta(resource_meta),
            },
        ],
    };

    let decoded = BytecodeModule::decode(&module.encode().unwrap()).unwrap();
    let metadata = decoded.metadata().unwrap();
    runtime.apply_bytecode_metadata(&metadata, None).unwrap();
    let fb_instance_id = match runtime.storage().read_by_ref(fb_ref.clone()) {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected FB instance, got {other:?}"),
    };
    runtime
        .storage_mut()
        .set_instance_var(fb_instance_id, "IN", Value::Bool(true));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();

    let out = runtime.storage().get_instance_var(fb_instance_id, "OUT");
    assert_eq!(out, Some(&Value::Bool(true)));
}
