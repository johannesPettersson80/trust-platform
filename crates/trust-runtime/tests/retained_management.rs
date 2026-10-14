#![cfg(feature = "full")]
use trust_runtime::{debug::RuntimeEvent, harness::CompileSession, value::Value};
const SOURCE: &str = "VAR_GLOBAL RETAIN kept : INT := 1; END_VAR VAR_GLOBAL plain : INT := 2; END_VAR PROGRAM Main VAR RETAIN member : INT := 3; END_VAR END_PROGRAM";

#[test]
fn retained_targets_rebind_actual_globals_and_program_members_without_a_bank() {
    let mut runtime = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let control = runtime.enable_debug();
    control.enqueue_retain_write("kept", Value::Int(7));
    control.enqueue_retain_write("@program/Main/member", Value::Int(8));
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(7)));
    let Some(Value::Instance(root)) = runtime.storage().get_global("Main") else {
        panic!("program")
    };
    let root = *root;
    assert_eq!(
        runtime.storage().get_instance_var(root, "member"),
        Some(&Value::Int(8))
    );
    assert!(runtime.storage().retain().is_empty());
    control.force_retain("kept", Value::Int(9));
    control.force_retain("@program/Main/member", Value::Int(10));
    runtime.execute_cycle().unwrap();
    runtime.write_global("kept", Value::Int(11)).unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(9)));
    assert_eq!(
        runtime.storage().get_instance_var(root, "member"),
        Some(&Value::Int(10))
    );
    control.release_retain("kept");
    control.release_retain("@program/Main/member");
    runtime.write_global("kept", Value::Int(12)).unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(12)));
    assert!(control
        .drain_runtime_events()
        .iter()
        .all(|event| !matches!(event, RuntimeEvent::WriteRejected { .. })));
}

#[test]
fn retained_label_index_rejects_nonretain_missing_wrong_type_and_stale_roots() {
    let mut runtime = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let metadata = runtime.metadata_snapshot();
    assert!(metadata
        .retained_reference(runtime.storage(), "KEPT")
        .is_some());
    assert!(metadata
        .retained_reference_exact(runtime.storage(), "KEPT")
        .is_none());
    assert!(metadata
        .retained_reference(runtime.storage(), "plain")
        .is_none());
    let listed = metadata.retained_references(runtime.storage());
    assert_eq!(
        listed
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["kept", "@program/Main/member"]
    );
    let mut stale = runtime.storage().clone();
    stale.set_global("Main", Value::Null);
    assert!(metadata
        .retained_reference(&stale, "@program/Main/member")
        .is_none());
    assert!(metadata
        .retained_reference(runtime.storage(), "@PROGRAM/main/MEMBER")
        .is_some());
    let control = runtime.enable_debug();
    for (name, value) in [
        ("missing", Value::Int(4)),
        ("plain", Value::Int(4)),
        ("kept", Value::Bool(false)),
    ] {
        control.enqueue_retain_write(name, value);
    }
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(1)));
    assert_eq!(runtime.storage().get_global("plain"), Some(&Value::Int(2)));
    assert!(!runtime.faulted());
    let rejected = control
        .drain_runtime_events()
        .into_iter()
        .filter_map(|event| match event {
            RuntimeEvent::WriteRejected { target, .. } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rejected, vec!["missing", "plain", "kept"]);
}

#[test]
fn retained_program_target_resolves_the_current_root_after_schema_replacement() {
    let mut runtime = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let metadata = runtime.metadata_snapshot();
    let old_reference = metadata
        .retained_reference(runtime.storage(), "@program/Main/member")
        .unwrap();
    let source = format!("FUNCTION_BLOCK Extra VAR item : INT; END_VAR END_FUNCTION_BLOCK VAR_GLOBAL leading_instance : Extra; END_VAR {SOURCE}");
    let module = CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap();
    runtime.apply_bytecode_module(&module, None).unwrap();
    let current = runtime
        .metadata_snapshot()
        .retained_reference(runtime.storage(), "@program/Main/member")
        .unwrap();
    assert_ne!(
        current.location, old_reference.location,
        "new leading object shifts the live root identity"
    );
    assert_eq!(
        metadata.retained_reference(runtime.storage(), "@program/Main/member"),
        Some(current.clone()),
        "immutable symbolic binding must not retain the previous instance id"
    );
    let control = runtime.enable_debug();
    control.enqueue_retain_write("@program/Main/member", Value::Int(17));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().read_by_ref(current),
        Some(&Value::Int(17))
    );
    assert!(!runtime.faulted());
}

#[test]
fn ambiguous_source_free_retained_labels_reject_without_replacing_the_controller() {
    use trust_runtime::bytecode::{build_module_from_runtime, SectionData, SectionId};
    for label in ["@program/Main/member", "@PROGRAM/main/MEMBER"] {
        let mut runtime = CompileSession::from_source(SOURCE).build_runtime().unwrap();
        runtime.write_global("kept", Value::Int(23)).unwrap();
        let control = runtime.enable_debug();
        control.enqueue_retain_write("kept", Value::Int(29));
        let before = build_module_from_runtime(&runtime).unwrap();
        let root = runtime.storage().get_global("Main").cloned();
        let mut forged = before.clone();
        let Some(SectionData::StringTable(strings)) = forged.section_mut(SectionId::StringTable)
        else {
            panic!("strings")
        };
        let name = strings
            .entries
            .iter_mut()
            .find(|name| name.as_str() == "kept")
            .unwrap();
        *name = label.into();
        forged
            .validate()
            .expect("opaque artifact names are valid wire data");
        let error = runtime.apply_bytecode_module(&forged, None).unwrap_err();
        // Replacement reaches shared retained-value migration before hosted
        // management metadata. Pin that owning rejection, not the later host
        // construction layer originally (incorrectly) expected by this new test.
        assert!(
            matches!(&error, trust_runtime::error::RuntimeError::RetainMigration(detail)
            if detail.as_str() == "ambiguous retained destination label")
        );
        assert_eq!(
            error.stable_code(),
            trust_runtime::error::RuntimeError::RetainMigration("".into()).stable_code()
        );
        let construction =
            trust_runtime::Runtime::from_bytes(&forged.encode().unwrap(), Default::default())
                .expect_err("hosted management labels must also reject ambiguous construction");
        assert!(
            matches!(&construction, trust_runtime::error::RuntimeError::InvalidConfig(detail)
            if detail.contains("ambiguous retained management label"))
        );
        assert_eq!(build_module_from_runtime(&runtime).unwrap(), before);
        assert_eq!(runtime.storage().get_global("Main").cloned(), root);
        assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(23)));
        assert!(!runtime.faulted());
        runtime.execute_cycle().unwrap();
        assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(29)));
    }
}

#[test]
fn shared_checkpoint_capture_rejects_duplicate_labels_without_losing_retained_values() {
    use trust_runtime::bytecode::{SectionData, SectionId};
    use trust_runtime::runtime_core::vm::PreparedModule;
    let mut artifact = CompileSession::from_source(SOURCE)
        .build_bytecode_module()
        .unwrap();
    let Some(SectionData::StringTable(strings)) = artifact.section_mut(SectionId::StringTable)
    else {
        panic!("strings")
    };
    *strings
        .entries
        .iter_mut()
        .find(|name| name.as_str() == "kept")
        .unwrap() = "@program/Main/member".into();
    // Wire names are opaque. The shared capture owner must protect consumers
    // that do not construct hosted management metadata.
    artifact.validate().unwrap();
    let prepared = std::sync::Arc::new(
        PreparedModule::from_bytes(&artifact.encode().unwrap(), Default::default()).unwrap(),
    );
    let mut borrowed = prepared.instantiate(0).unwrap();
    let mut owned = std::sync::Arc::clone(&prepared)
        .instantiate_owned(0)
        .unwrap();
    for result in [borrowed.retain_snapshot(), owned.retain_snapshot()] {
        assert!(
            matches!(result, Err(trust_runtime::error::RuntimeError::RetainMigration(detail))
            if detail.as_str() == "ambiguous retained checkpoint label")
        );
    }
    for storage in [borrowed.storage(), owned.storage()] {
        assert_eq!(
            storage.get_global("@program/Main/member"),
            Some(&Value::Int(1))
        );
        let Some(Value::Instance(main)) = storage.get_global("Main") else {
            panic!("root")
        };
        assert_eq!(
            storage.get_instance_var(*main, "member"),
            Some(&Value::Int(3))
        );
    }
    assert!(borrowed.fault().is_none());
    assert!(owned.fault().is_none());
}

#[test]
fn shared_checkpoint_restore_rejects_casefolded_destination_collisions_atomically() {
    use trust_runtime::bytecode::{SectionData, SectionId};
    use trust_runtime::runtime_core::{retain::RetainSnapshot, vm::PreparedModule};
    for label in ["@program/Main/member", "@PROGRAM/main/MEMBER"] {
        let mut artifact = CompileSession::from_source(SOURCE)
            .build_bytecode_module()
            .unwrap();
        let Some(SectionData::StringTable(strings)) = artifact.section_mut(SectionId::StringTable)
        else {
            panic!("strings")
        };
        *strings
            .entries
            .iter_mut()
            .find(|name| name.as_str() == "kept")
            .unwrap() = label.into();
        let prepared = std::sync::Arc::new(
            PreparedModule::from_bytes(&artifact.encode().unwrap(), Default::default()).unwrap(),
        );
        let mut borrowed = prepared.instantiate(0).unwrap();
        let mut owned = std::sync::Arc::clone(&prepared)
            .instantiate_owned(0)
            .unwrap();
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert("plain", Value::Int(91));
        snapshot.insert(label, Value::Int(92));
        snapshot.insert("@program/Main/member", Value::Int(93));
        for result in [
            borrowed.apply_retain_snapshot(&snapshot),
            owned.apply_retain_snapshot(&snapshot),
        ] {
            assert!(
                matches!(result, Err(trust_runtime::error::RuntimeError::RetainMigration(detail))
                if detail.as_str() == "ambiguous retained destination label")
            );
        }
        for storage in [borrowed.storage(), owned.storage()] {
            assert_eq!(storage.get_global(label), Some(&Value::Int(1)));
            assert_eq!(storage.get_global("plain"), Some(&Value::Int(2)));
            let Some(Value::Instance(main)) = storage.get_global("Main") else {
                panic!("root")
            };
            assert_eq!(
                storage.get_instance_var(*main, "member"),
                Some(&Value::Int(3))
            );
        }
        assert!(borrowed.fault().is_none());
        assert!(owned.fault().is_none());
    }
}
