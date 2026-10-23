//! Version-selected temporal layout from source to installed execution and inspection.
#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::{
        build_module_from_runtime, build_module_from_runtime_with_sources, BytecodeModule,
        BytecodeVersion, HEADER_FLAG_LEGACY_TEMPORAL32,
    },
    harness::{parse_debug_expression_with_abi, parse_debug_lvalue_with_abi, CompileSession},
    value::{
        DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
        StorageAbi, TimeOfDayValue, Value,
    },
    Runtime,
};

const SIZES: &str = r#"
TYPE Pair : STRUCT elapsed : TIME; stamp : DATE; END_STRUCT; END_TYPE
VAR_GLOBAL CONSTANT folded_width : DINT := SIZEOF(TIME); END_VAR
VAR_GLOBAL
    initial_width : DINT := SIZEOF(TIME);
    elapsed : TIME; stamp : DATE; daytime : TOD; instant : DT;
    long_elapsed : LTIME; long_stamp : LDATE; long_daytime : LTOD; long_instant : LDT;
    time_bytes : DINT; date_bytes : DINT; tod_bytes : DINT; dt_bytes : DINT;
    long_bytes : DINT; array_bytes : DINT; pair_bytes : DINT;
    copied : TIME;
END_VAR
PROGRAM Main
    time_bytes := SIZEOF(TIME); date_bytes := SIZEOF(DATE);
    tod_bytes := SIZEOF(TOD); dt_bytes := SIZEOF(DT);
    long_bytes := SIZEOF(LTIME); array_bytes := SIZEOF(ARRAY[0..1] OF TIME);
    pair_bytes := SIZEOF(Pair); copied := elapsed;
END_PROGRAM
"#;

fn install(source: &str, version: BytecodeVersion) -> (BytecodeModule, Runtime) {
    let session = CompileSession::from_source(source);
    let module = session.build_bytecode_module_for_version(version).unwrap();
    let runtime = if version == BytecodeVersion::LEGACY {
        session.build_runtime_from_bytecode(&module, None).unwrap()
    } else {
        Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap()
    };
    (module, runtime)
}

fn evaluate(runtime: &Runtime, text: &str) -> Value {
    let mut registry = runtime.registry().clone();
    let expression = parse_debug_expression_with_abi(
        text,
        &mut registry,
        runtime.profile(),
        &[],
        runtime.prepared().storage_abi(),
    )
    .unwrap();
    runtime
        .evaluate_debug_expression(&expression, &registry, None)
        .unwrap()
}

#[test]
fn versions_preserve_temporal_values_and_select_sizeof_during_compile_run_and_inspection() {
    for (version, abi, short) in [
        (BytecodeVersion::LEGACY, StorageAbi::LegacyTemporal32, 4),
        (
            BytecodeVersion::SOURCE_FREE_V2,
            StorageAbi::LegacyTemporal32,
            4,
        ),
        (
            BytecodeVersion::SOURCE_FREE,
            StorageAbi::UniformTemporal64,
            8,
        ),
    ] {
        let (artifact, mut runtime) = install(SIZES, version);
        assert_eq!(artifact.storage_abi().unwrap(), abi);
        assert_eq!(runtime.prepared().storage_abi(), abi);
        assert_eq!(runtime.metadata_snapshot().storage_abi(), abi);
        assert_eq!(
            runtime.storage().get_global("folded_width"),
            Some(&Value::DInt(short))
        );
        assert_eq!(
            runtime.storage().get_global("initial_width"),
            Some(&Value::DInt(short))
        );
        // Ordinary logical storage retains its signed64 domain even under the old ABI.
        for ticks in [-5_000_000_000, 5_000_000_000] {
            for (name, value) in [
                ("elapsed", Value::Time(Duration::from_nanos(ticks))),
                ("stamp", Value::Date(DateValue::new(ticks))),
                ("daytime", Value::Tod(TimeOfDayValue::new(ticks))),
                ("instant", Value::Dt(DateTimeValue::new(ticks))),
                ("long_elapsed", Value::LTime(Duration::from_nanos(ticks))),
                ("long_stamp", Value::LDate(LDateValue::new(ticks))),
                ("long_daytime", Value::LTod(LTimeOfDayValue::new(ticks))),
                ("long_instant", Value::Ldt(LDateTimeValue::new(ticks))),
            ] {
                runtime.write_global(name, value.clone()).unwrap();
                assert_eq!(
                    runtime.storage().get_global(name),
                    Some(&value),
                    "{version:?} {name}"
                );
            }
            runtime.execute_cycle().unwrap();
            assert_eq!(
                runtime.storage().get_global("copied"),
                Some(&Value::Time(Duration::from_nanos(ticks)))
            );
        }
        for name in ["time_bytes", "date_bytes", "tod_bytes", "dt_bytes"] {
            assert_eq!(
                runtime.storage().get_global(name),
                Some(&Value::DInt(short)),
                "{version:?} {name}"
            );
        }
        assert_eq!(
            runtime.storage().get_global("long_bytes"),
            Some(&Value::DInt(8))
        );
        assert_eq!(
            runtime.storage().get_global("array_bytes"),
            Some(&Value::DInt(short * 2))
        );
        assert_eq!(
            runtime.storage().get_global("pair_bytes"),
            Some(&Value::DInt(short * 2))
        );
        assert_eq!(evaluate(&runtime, "SIZEOF(TIME)"), Value::DInt(short));
        assert_eq!(
            evaluate(&runtime, "SIZEOF(ARRAY[1..SIZEOF(TIME)] OF BYTE)"),
            Value::DInt(short)
        );
        let control = runtime.enable_debug();
        control.refresh_snapshot_from_storage_with_abi(
            runtime.storage(),
            runtime.current_time(),
            abi,
        );
        let mut snapshot = control.snapshot().unwrap();
        let mut registry = runtime.registry().clone();
        let expr = parse_debug_expression_with_abi(
            "SIZEOF(TIME)",
            &mut registry,
            runtime.profile(),
            &[],
            abi,
        )
        .unwrap();
        assert_eq!(
            snapshot
                .evaluate_expression(
                    &expr,
                    &registry,
                    Some(runtime.stdlib()),
                    &runtime.profile(),
                    None
                )
                .unwrap(),
            Value::DInt(short)
        );
    }
}

#[test]
fn current_relative_arrays_keep_full_width_temporal_aliases() {
    let source = r#"
TYPE
    Cell : STRUCT OVERLAP elapsed AT %B0 : TIME; raw AT %B0 : LINT; END_STRUCT;
    PageType : STRUCT cells AT %B0 : ARRAY[1..2] OF Cell; tail AT %B16 : TIME; END_STRUCT;
END_TYPE
VAR_GLOBAL page : PageType; cell_bytes : DINT; page_bytes : DINT; observed : LINT; END_VAR
PROGRAM Main
    page.cells[1].elapsed := T#5s;
    page.cells[2].elapsed := T#-5s;
    page.tail := T#5s;
    cell_bytes := SIZEOF(Cell); page_bytes := SIZEOF(PageType);
    observed := page.cells[2].raw;
END_PROGRAM
"#;
    let (_, mut runtime) = install(source, BytecodeVersion::SOURCE_FREE);
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("cell_bytes"),
        Some(&Value::DInt(8))
    );
    assert_eq!(
        runtime.storage().get_global("page_bytes"),
        Some(&Value::DInt(24))
    );
    assert_eq!(
        runtime.storage().get_global("observed"),
        Some(&Value::LInt(-5_000_000_000))
    );
    assert_eq!(
        evaluate(&runtime, "page.cells[1].raw"),
        Value::LInt(5_000_000_000)
    );
    assert_eq!(
        evaluate(&runtime, "page.tail"),
        Value::Time(Duration::from_nanos(5_000_000_000))
    );
    assert_eq!(evaluate(&runtime, "SIZEOF(PageType)"), Value::DInt(24));
    assert!(
        CompileSession::from_source(source)
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE_V2)
            .is_err(),
        "source-free 2.0 must not silently discard relative layout"
    );
}

const OLD_PLAIN: &str = r#"
TYPE OverlayType : STRUCT elapsed : TIME; raw : LINT; END_STRUCT; END_TYPE
VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR
PROGRAM Main END_PROGRAM
"#;
const OLD_CONTEXT: &str = r#"
TYPE OverlayType : STRUCT OVERLAP elapsed AT %B0 : TIME; raw AT %B0 : LINT; END_STRUCT; END_TYPE
VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR
PROGRAM Main END_PROGRAM
"#;

#[test]
fn legacy_physical_write_and_cross_abi_retained_replacement_reject_atomically() {
    // The original1.1 wire format had no location fields. The matching construction
    // context supplies them; no ordinary instruction bytes are replaced.
    let original = CompileSession::from_source(OLD_PLAIN)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    let mut runtime = CompileSession::from_source(OLD_CONTEXT)
        .build_runtime_from_bytecode(&original, None)
        .unwrap();
    assert_eq!(
        runtime.prepared().storage_abi(),
        StorageAbi::LegacyTemporal32
    );
    let before = runtime.storage().get_global("overlay").cloned().unwrap();
    let image = runtime.io().outputs().to_vec();
    let before_export = build_module_from_runtime(&runtime).unwrap();
    let mut registry = runtime.registry().clone();
    let target = parse_debug_lvalue_with_abi(
        "overlay.elapsed",
        &mut registry,
        runtime.profile(),
        &[],
        StorageAbi::LegacyTemporal32,
    )
    .unwrap();
    for ticks in [-5_000_000_000, 5_000_000_000] {
        let error = runtime
            .write_lvalue(&target, Value::Time(Duration::from_nanos(ticks)), None)
            .unwrap_err();
        assert_eq!(error.stable_code().as_str(), "runtime_overflow");
        assert_eq!(runtime.storage().get_global("overlay"), Some(&before));
        assert_eq!(runtime.io().outputs(), image.as_slice());
        assert!(!runtime.faulted());
    }
    let candidate = CompileSession::from_source(OLD_CONTEXT)
        .build_bytecode_module()
        .unwrap();
    let error = runtime.apply_bytecode_module(&candidate, None).unwrap_err();
    assert_eq!(error.stable_code().as_str(), "runtime_retain_migration");
    assert_eq!(
        runtime.prepared().storage_abi(),
        StorageAbi::LegacyTemporal32
    );
    assert_eq!(runtime.storage().get_global("overlay"), Some(&before));
    assert_eq!(runtime.io().outputs(), image.as_slice());
    assert_eq!(build_module_from_runtime(&runtime).unwrap(), before_export);
    runtime.execute_cycle().unwrap();
}

#[test]
fn legacy_export_identity_changes_only_after_successful_resource_reconfiguration() {
    let source = "VAR_GLOBAL count : INT; END_VAR PROGRAM Main count := count + 1; END_PROGRAM";
    let (original, mut runtime) = install(source, BytecodeVersion::LEGACY);
    let renamed_on_install = CompileSession::from_source(source)
        .build_runtime_from_bytecode(&original, Some("AtInstall"))
        .unwrap();
    let renamed_artifact = build_module_from_runtime(&renamed_on_install).unwrap();
    assert_eq!(renamed_artifact.version, BytecodeVersion::SOURCE_FREE);
    assert_eq!(
        renamed_artifact.storage_abi().unwrap(),
        StorageAbi::LegacyTemporal32
    );
    assert_eq!(
        renamed_artifact.metadata().unwrap().resources[0].name,
        "AtInstall"
    );
    let before = build_module_from_runtime(&runtime).unwrap();
    assert_eq!(before.version, BytecodeVersion::LEGACY);
    let mut expected = original.clone();
    expected.sections.retain(|section| {
        !matches!(
            trust_runtime::bytecode::SectionId::from_raw(section.id),
            Some(
                trust_runtime::bytecode::SectionId::DebugMap
                    | trust_runtime::bytecode::SectionId::DebugStringTable
            )
        )
    });
    assert_eq!(before, expected);
    let mut resource = before.metadata().unwrap().resources[0].clone();
    resource.process_image.memory = usize::MAX;
    assert!(runtime.apply_resource_metadata(&resource).is_err());
    assert_eq!(build_module_from_runtime(&runtime).unwrap(), before);
    runtime.execute_cycle().unwrap();
    resource = before.metadata().unwrap().resources[0].clone();
    resource.name = "Renamed".into();
    runtime.apply_resource_metadata(&resource).unwrap();
    let updated = build_module_from_runtime(&runtime).unwrap();
    assert_eq!(updated.version, BytecodeVersion::SOURCE_FREE);
    assert_ne!(updated.flags & HEADER_FLAG_LEGACY_TEMPORAL32, 0);
    assert_eq!(updated.storage_abi().unwrap(), StorageAbi::LegacyTemporal32);
    assert_eq!(updated.metadata().unwrap().resources[0].name, "Renamed");
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("count"), Some(&Value::Int(2)));
}

#[test]
fn uniform_temporal_storage_keeps_the_explicit_time_dword_millisecond_codec() {
    let source = "PROGRAM Main VAR elapsed AT %QD0 : TIME; END_VAR elapsed := T#5s; END_PROGRAM";
    let (_, mut runtime) = install(source, BytecodeVersion::SOURCE_FREE);
    assert_eq!(evaluate(&runtime, "SIZEOF(TIME)"), Value::DInt(8));
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.io().outputs(), &5_000_u32.to_le_bytes());
}

#[test]
fn relative_temporal_io_arrays_use_complete_wire_extents_without_element_aliasing() {
    let source = r#"
TYPE TimedPair : STRUCT first AT %B0 : TIME; second AT %B8 : TIME; END_STRUCT; END_TYPE
VAR_GLOBAL pairs AT %QB0 : ARRAY[0..1] OF TimedPair; storage_width : DINT; END_VAR
PROGRAM Main
    pairs[0].first := T#5s; pairs[0].second := T#7s;
    pairs[1].first := T#11s; pairs[1].second := T#13s;
    storage_width := SIZEOF(ARRAY[0..1] OF TimedPair);
END_PROGRAM
"#;
    for (version, storage_width) in [
        (BytecodeVersion::LEGACY, 16),
        (BytecodeVersion::SOURCE_FREE, 32),
    ] {
        let (_, mut runtime) = install(source, version);
        runtime.execute_cycle().unwrap();
        let mut expected = [0_u8; 24];
        for (offset, milliseconds) in [(0, 5_000_u32), (8, 7_000), (12, 11_000), (20, 13_000)] {
            expected[offset..offset + 4].copy_from_slice(&milliseconds.to_le_bytes());
        }
        assert_eq!(runtime.io().outputs(), &expected, "{version:?}");
        assert_eq!(
            runtime.storage().get_global("storage_width"),
            Some(&Value::DInt(storage_width))
        );
    }
}

#[test]
fn non_temporal_retained_overlay_keeps_its_compatible_layout_across_abi_change() {
    let plain = "TYPE FlagsType : STRUCT full : DWORD; low : BOOL; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN flags : FlagsType; END_VAR PROGRAM Main END_PROGRAM";
    let context = "TYPE FlagsType : STRUCT OVERLAP full AT %B0 : DWORD; low AT %X0.0 : BOOL; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN flags : FlagsType; END_VAR PROGRAM Main END_PROGRAM";
    let original = CompileSession::from_source(plain)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    let mut runtime = CompileSession::from_source(context)
        .build_runtime_from_bytecode(&original, None)
        .unwrap();
    let mut registry = runtime.registry().clone();
    let target = parse_debug_lvalue_with_abi(
        "flags.full",
        &mut registry,
        runtime.profile(),
        &[],
        StorageAbi::LegacyTemporal32,
    )
    .unwrap();
    runtime
        .write_lvalue(&target, Value::DWord(0x8000_0101), None)
        .unwrap();
    let candidate = CompileSession::from_source(context)
        .build_bytecode_module()
        .unwrap();
    runtime
        .apply_bytecode_module(&candidate, None)
        .expect("unchanged non-temporal physical geometry remains compatible");
    assert_eq!(
        runtime.prepared().storage_abi(),
        StorageAbi::UniformTemporal64
    );
    assert_eq!(evaluate(&runtime, "flags.full"), Value::DWord(0x8000_0101));
    assert_eq!(evaluate(&runtime, "flags.low"), Value::Bool(true));
    runtime.execute_cycle().unwrap();
}

#[test]
fn retained_overlay_cannot_lose_physical_storage_during_warm_replacement() {
    let program =
        "VAR_GLOBAL published AT %QD0 : DINT; END_VAR PROGRAM Main published := 9; END_PROGRAM";
    let source = OLD_CONTEXT.replace("PROGRAM Main END_PROGRAM", program);
    let ordinary_source = OLD_PLAIN.replace("PROGRAM Main END_PROGRAM", program);
    let (_, mut installed) = install(&source, BytecodeVersion::SOURCE_FREE);
    installed.execute_cycle().unwrap();
    let mut registry = installed.registry().clone();
    let target = parse_debug_lvalue_with_abi(
        "overlay.raw",
        &mut registry,
        installed.profile(),
        &[],
        StorageAbi::UniformTemporal64,
    )
    .unwrap();
    installed
        .write_lvalue(&target, Value::LInt(42), None)
        .unwrap();
    let before = installed.storage().get_global("overlay").cloned();
    let archive = build_module_from_runtime(&installed).unwrap();
    let outputs = installed.io().outputs().to_vec();
    let now = installed.current_time();
    let candidate = CompileSession::from_source(&ordinary_source)
        .build_bytecode_module()
        .unwrap();
    let error = installed
        .apply_bytecode_module(&candidate, None)
        .unwrap_err();
    assert_eq!(error.stable_code().as_str(), "runtime_retain_migration");
    assert_eq!(installed.storage().get_global("overlay"), before.as_ref());
    assert_eq!(
        evaluate(&installed, "overlay.elapsed"),
        Value::Time(Duration::from_nanos(42))
    );
    assert_eq!(build_module_from_runtime(&installed).unwrap(), archive);
    assert_eq!(installed.io().outputs(), outputs);
    assert_eq!(installed.current_time(), now);
    assert!(!installed.faulted());
    installed.execute_cycle().unwrap();

    // Refusal protects the retained live representation; the new ordinary
    // declaration remains valid for a deliberate fresh installation.
    let mut cold = Runtime::from_bytes(&candidate.encode().unwrap(), Default::default()).unwrap();
    cold.execute_cycle().unwrap();
    assert_eq!(
        evaluate(&cold, "overlay.elapsed"),
        Value::Time(Duration::ZERO)
    );
    assert_eq!(cold.io().outputs(), &9_i32.to_le_bytes());
}

#[test]
fn exported_legacy_import_recertifies_matching_sources_without_replacing_bodies() {
    let source = "VAR_GLOBAL count : INT; END_VAR PROGRAM Main count := count + 1; END_PROGRAM";
    let session = CompileSession::from_source(source);
    let legacy = session
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    let installed = session
        .build_runtime_from_bytecode(&legacy, Some("Renamed"))
        .unwrap();
    let exported = build_module_from_runtime_with_sources(&installed, &[source]).unwrap();
    assert_eq!(exported.version, BytecodeVersion::SOURCE_FREE);
    assert_eq!(
        exported.storage_abi().unwrap(),
        StorageAbi::LegacyTemporal32
    );

    let mut reloaded = session
        .build_runtime_from_bytecode(&exported, Some("Renamed"))
        .unwrap();
    assert!(reloaded
        .metadata_snapshot()
        .statement_locations(0)
        .is_some());
    assert_eq!(
        build_module_from_runtime_with_sources(&reloaded, &[source]).unwrap(),
        exported
    );
    reloaded.execute_cycle().unwrap();
    assert_eq!(reloaded.storage().get_global("count"), Some(&Value::Int(1)));

    let changed = source.replace("count + 1", "count + 2");
    let mut mismatched = CompileSession::from_source(&changed)
        .build_runtime_from_bytecode(&exported, Some("Renamed"))
        .unwrap();
    assert!(build_module_from_runtime_with_sources(&mismatched, &[changed.as_str()]).is_err());
    mismatched.execute_cycle().unwrap();
    assert_eq!(
        mismatched.storage().get_global("count"),
        Some(&Value::Int(1))
    );
}

#[test]
fn exported_legacy_relative_context_is_checked_under_its_original_widths() {
    let plain = "TYPE PairType : STRUCT elapsed : TIME; tail : INT; END_STRUCT; END_TYPE VAR_GLOBAL pair : PairType; width : DINT; END_VAR PROGRAM Main width := SIZEOF(PairType); END_PROGRAM";
    let context = plain.replace(
        "elapsed : TIME; tail : INT",
        "elapsed AT %B0 : TIME; tail AT %B4 : INT",
    );
    // This historical layout is valid with TIME occupying four bytes. The
    // current source default correctly refuses its overlapping ordinary fields.
    assert!(CompileSession::from_source(&context)
        .build_bytecode_module()
        .is_err());
    let legacy = CompileSession::from_source(&context)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    assert!(
        CompileSession::from_sources(vec![])
            .build_runtime_from_bytecode(&legacy, None)
            .is_err(),
        "explicit 1.1 relative layout still requires construction context"
    );
    let session = CompileSession::from_source(&context);
    let installed = session
        .build_runtime_from_bytecode(&legacy, Some("Renamed"))
        .unwrap();
    let exported = build_module_from_runtime_with_sources(&installed, &[context.as_str()]).unwrap();
    let mut reloaded = session
        .build_runtime_from_bytecode(&exported, Some("Renamed"))
        .unwrap();
    assert_eq!(
        reloaded.prepared().storage_abi(),
        StorageAbi::LegacyTemporal32
    );
    reloaded.execute_cycle().unwrap();
    assert_eq!(
        reloaded.storage().get_global("width"),
        Some(&Value::DInt(6))
    );
    assert_eq!(evaluate(&reloaded, "SIZEOF(PairType)"), Value::DInt(6));
    assert!(reloaded
        .metadata_snapshot()
        .statement_locations(0)
        .is_some());
    assert_eq!(
        build_module_from_runtime_with_sources(&reloaded, &[context.as_str()]).unwrap(),
        exported
    );
}
