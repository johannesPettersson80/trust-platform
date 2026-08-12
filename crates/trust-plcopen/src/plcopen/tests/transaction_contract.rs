fn transaction_contract_valid_program_xml(name: &str, body_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="{PLCOPEN_NAMESPACE}">
  <types>
    <pous>
      <pou name="{name}" pouType="program">
        <body><ST><![CDATA[
PROGRAM {body_name}
VAR
    Value : INT := 1;
END_VAR
END_PROGRAM
]]></ST></body>
      </pou>
    </pous>
  </types>
</project>
"#
    )
}

fn transaction_contract_two_program_xml(
    first_name: &str,
    first_body_name: &str,
    second_name: &str,
    second_body_name: &str,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="{PLCOPEN_NAMESPACE}">
  <types>
    <pous>
      <pou name="{first_name}" pouType="program">
        <body><ST><![CDATA[
PROGRAM {first_body_name}
END_PROGRAM
]]></ST></body>
      </pou>
      <pou name="{second_name}" pouType="program">
        <body><ST><![CDATA[
PROGRAM {second_body_name}
END_PROGRAM
]]></ST></body>
      </pou>
    </pous>
  </types>
</project>
"#
    )
}

fn transaction_contract_write_project_source(project: &Path, relative: &str, body: &str) {
    write(&project.join("src").join(relative), body);
}

fn transaction_contract_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, snapshot: &mut Vec<(String, Vec<u8>)>) {
        let mut entries = std::fs::read_dir(current)
            .expect("read snapshot directory")
            .map(|entry| entry.expect("read snapshot entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path below root")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = std::fs::symlink_metadata(&path).expect("snapshot metadata");
            if metadata.file_type().is_symlink() {
                let mut value = b"symlink:".to_vec();
                value.extend_from_slice(
                    std::fs::read_link(&path)
                        .expect("read snapshot symlink")
                        .to_string_lossy()
                        .as_bytes(),
                );
                snapshot.push((relative, value));
            } else if metadata.is_dir() {
                snapshot.push((relative, b"directory".to_vec()));
                walk(root, &path, snapshot);
            } else {
                let mut value = b"file:".to_vec();
                value.extend_from_slice(&std::fs::read(&path).expect("read snapshot file"));
                snapshot.push((relative, value));
            }
        }
    }

    let mut snapshot = Vec::new();
    walk(root, root, &mut snapshot);
    snapshot
}

fn transaction_contract_strip_generation_timestamp(xml: &str) -> String {
    let marker = " creationDateTime=\"";
    let Some(start) = xml.find(marker) else {
        return xml.to_string();
    };
    let value_start = start + marker.len();
    let Some(value_end_offset) = xml[value_start..].find('"') else {
        return xml.to_string();
    };
    let value_end = value_start + value_end_offset;
    let mut normalized = xml.to_string();
    normalized.replace_range(value_start..value_end, "<generated>");
    normalized
}

#[test]
fn transaction_contract_filename_sanitization_blocks_path_components() {
    for raw in [
        "../Main",
        "..\\Main",
        "/absolute/Main",
        "C:\\absolute\\Main",
        "folder/name",
        "folder\\name",
        ".",
        "..",
        "",
    ] {
        let sanitized = sanitize_filename(raw);
        assert!(!sanitized.contains('/'), "{raw:?} -> {sanitized:?}");
        assert!(!sanitized.contains('\\'), "{raw:?} -> {sanitized:?}");
        assert_ne!(sanitized, ".", "{raw:?}");
        assert_ne!(sanitized, "..", "{raw:?}");
    }
}

#[test]
fn transaction_contract_path_segments_are_nonempty_single_components() {
    for raw in ["", ".", "..", "///", "\\\\", "../..", "  "] {
        let sanitized = sanitize_path_segment(raw, "folder");
        assert_eq!(sanitized, "folder", "{raw:?}");
    }

    for raw in ["Cell/One", "Cell\\One", " Cell One ", "Cell:One"] {
        let sanitized = sanitize_path_segment(raw, "folder");
        assert!(!sanitized.is_empty(), "{raw:?}");
        assert!(!sanitized.contains('/'), "{raw:?} -> {sanitized:?}");
        assert!(!sanitized.contains('\\'), "{raw:?} -> {sanitized:?}");
        assert_ne!(sanitized, ".", "{raw:?}");
        assert_ne!(sanitized, "..", "{raw:?}");
    }
}

#[test]
fn transaction_contract_portable_names_avoid_device_and_trailing_aliases() {
    for raw in ["CON", "con", "PRN", "AUX", "NUL", "COM1", "LPT9"] {
        let sanitized = sanitize_filename(raw);
        assert!(
            !sanitized.eq_ignore_ascii_case(raw),
            "reserved device name remained addressable: {raw:?}"
        );
    }

    for raw in ["Main.", "Main ", "Main. "] {
        let sanitized = sanitize_filename(raw);
        assert!(
            !sanitized.ends_with('.') && !sanitized.ends_with(' '),
            "non-portable trailing component: {raw:?} -> {sanitized:?}"
        );
    }
}

#[test]
fn transaction_contract_unique_source_path_reserves_batch_case_insensitively() {
    let project = temp_dir("plcopen-transaction-batch-case");
    let source_root = project.join("src");
    std::fs::create_dir_all(&source_root).expect("create source root");
    let mut seen = HashSet::new();

    let first = unique_source_path(&source_root, "Main", &mut seen);
    let second = unique_source_path(&source_root, "main", &mut seen);
    let third = unique_source_path(&source_root, "MAIN", &mut seen);

    assert_eq!(first, source_root.join("Main.st"));
    assert_eq!(second, source_root.join("main_2.st"));
    assert_eq!(third, source_root.join("MAIN_3.st"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_unique_source_path_reserves_existing_entries() {
    let project = temp_dir("plcopen-transaction-existing-path");
    let source_root = project.join("src");
    write(&source_root.join("Main.st"), "existing");
    std::fs::create_dir_all(source_root.join("Main_2.st")).expect("create colliding directory");
    let mut seen = HashSet::new();

    let candidate = unique_source_path(&source_root, "Main", &mut seen);

    assert_eq!(candidate, source_root.join("Main_3.st"));
    assert_eq!(
        std::fs::read_to_string(source_root.join("Main.st")).expect("read existing source"),
        "existing"
    );
    assert!(source_root.join("Main_2.st").is_dir());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_unique_source_path_reserves_existing_case_alias() {
    let project = temp_dir("plcopen-transaction-existing-case");
    let source_root = project.join("src");
    write(&source_root.join("MAIN.ST"), "existing");
    let mut seen = HashSet::new();

    let candidate = unique_source_path(&source_root, "main", &mut seen);

    assert_eq!(candidate, source_root.join("main_2.st"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_unique_nested_path_stays_below_source_root() {
    let project = temp_dir("plcopen-transaction-nested-path");
    let source_root = project.join("src");
    std::fs::create_dir_all(&source_root).expect("create source root");
    let mut seen = HashSet::new();
    let segments = vec![
        "..".to_string(),
        "/absolute".to_string(),
        "Cell\\One".to_string(),
    ];

    let candidate =
        unique_source_path_with_segments(&source_root, &segments, "../../Main", &mut seen);

    assert!(candidate.starts_with(&source_root), "{}", candidate.display());
    let relative = candidate
        .strip_prefix(&source_root)
        .expect("candidate below source root");
    assert!(relative.components().all(|component| {
        matches!(component, std::path::Component::Normal(_))
    }));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_identifier_uniqueness_is_ascii_case_insensitive() {
    let mut used = HashSet::new();
    assert_eq!(unique_identifier("Cell".to_string(), &mut used), "Cell");
    assert_eq!(unique_identifier("cell".to_string(), &mut used), "cell_2");
    assert_eq!(unique_identifier("CELL".to_string(), &mut used), "CELL_3");
}

#[test]
fn transaction_contract_export_discovers_every_ascii_case_extension() {
    let project = temp_dir("plcopen-transaction-extension-case");
    for (name, pou) in [
        ("a.st", "A"),
        ("b.ST", "B"),
        ("c.St", "C"),
        ("d.sT", "D"),
        ("e.pou", "E"),
        ("f.POU", "F"),
        ("g.Pou", "G"),
        ("h.pOu", "H"),
    ] {
        transaction_contract_write_project_source(
            &project,
            name,
            &format!("PROGRAM {pou}\nEND_PROGRAM\n"),
        );
    }

    let sources = load_sources(&project, &project.join("src")).expect("discover sources");
    assert_eq!(sources.len(), 8);
    assert_eq!(
        sources
            .iter()
            .map(|source| source.path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>(),
        [
            "src/a.st",
            "src/b.ST",
            "src/c.St",
            "src/d.sT",
            "src/e.pou",
            "src/f.POU",
            "src/g.Pou",
            "src/h.pOu",
        ]
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_discovery_is_recursive_literal_and_deterministic() {
    let project = temp_dir("plcopen-transaction-discovery-order");
    for (name, pou) in [
        ("z last/Ωmega.st", "Omega"),
        ("a[first]/two.pou", "Two"),
        ("a[first]/one.st", "One"),
        ("literal*/three.ST", "Three"),
    ] {
        transaction_contract_write_project_source(
            &project,
            name,
            &format!("PROGRAM {pou}\nEND_PROGRAM\n"),
        );
    }
    transaction_contract_write_project_source(
        &project,
        "ignored.txt",
        "PROGRAM Ignored\nEND_PROGRAM\n",
    );

    let first = load_sources(&project, &project.join("src")).expect("first discovery");
    let second = load_sources(&project, &project.join("src")).expect("second discovery");
    let first_paths = first
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let second_paths = second
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();

    assert_eq!(first_paths, second_paths);
    assert_eq!(first_paths.len(), 4);
    assert!(first_paths.windows(2).all(|pair| pair[0] < pair[1]));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_rejects_matching_directory_entry() {
    let project = temp_dir("plcopen-transaction-source-directory");
    std::fs::create_dir_all(project.join("src/not-a-file.st"))
        .expect("create matching source directory");

    let error = load_sources(&project, &project.join("src"))
        .expect_err("matching directory must be rejected");

    assert!(error.to_string().contains("source"));
    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn transaction_contract_export_rejects_source_symlink() {
    use std::os::unix::fs::symlink;

    let project = temp_dir("plcopen-transaction-source-symlink");
    write(
        &project.join("outside.st"),
        "PROGRAM Outside\nEND_PROGRAM\n",
    );
    std::fs::create_dir_all(project.join("src")).expect("create source root");
    symlink("../outside.st", project.join("src/link.st")).expect("create source symlink");

    let error =
        load_sources(&project, &project.join("src")).expect_err("source symlink must be rejected");

    assert!(error.to_string().to_ascii_lowercase().contains("symbolic"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_cdata_split_round_trips_original_bytes() {
    let payload = "before ]]> middle ]]> after";
    let xml = format!(
        "<project><data><text><![CDATA[{}]]></text></data></project>",
        escape_cdata(payload)
    );
    let document = roxmltree::Document::parse(&xml).expect("escaped CDATA parses");
    let text = document
        .descendants()
        .find(|node| is_element_named(*node, "text"))
        .and_then(extract_text_content)
        .expect("CDATA payload");

    assert_eq!(text, payload);
}

#[test]
fn transaction_contract_export_vendor_hook_cdata_round_trips() {
    let project = temp_dir("plcopen-transaction-vendor-cdata");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let payload = "<vendor>before ]]> after</vendor>";
    write(&project.join(VENDOR_EXTENSION_HOOK_FILE), payload);
    let output = project.join("out/plcopen.xml");

    export_project_to_xml(&project, &output).expect("export with vendor hook");

    let xml = std::fs::read_to_string(&output).expect("read exported XML");
    let document = roxmltree::Document::parse(&xml).expect("exported XML parses");
    let preserved = document
        .descendants()
        .find(|node| {
            is_element_named(*node, "data")
                && node.attribute("name") == Some(VENDOR_EXT_DATA_NAME)
        })
        .and_then(|data| {
            data.descendants()
                .find(|node| is_element_named(*node, "text"))
        })
        .and_then(extract_text_content)
        .expect("vendor payload");
    assert_eq!(preserved, payload);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_embedded_and_sidecar_source_maps_match() {
    let project = temp_dir("plcopen-transaction-source-map-coherence");
    transaction_contract_write_project_source(
        &project,
        "nested/main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    transaction_contract_write_project_source(
        &project,
        "calc.pou",
        "FUNCTION Calc : INT\nCalc := 1;\nEND_FUNCTION\n",
    );
    let output = project.join("out/plcopen.xml");

    let report = export_project_to_xml(&project, &output).expect("export project");

    let xml = std::fs::read_to_string(&output).expect("read XML");
    let document = roxmltree::Document::parse(&xml).expect("parse XML");
    let embedded =
        parse_embedded_source_map(document.root_element()).expect("embedded source map");
    let sidecar: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&report.source_map_path).expect("read sidecar"),
    )
    .expect("parse sidecar");
    assert_eq!(
        serde_json::to_value(&embedded).expect("serialize embedded"),
        sidecar
    );
    assert_eq!(embedded.entries.len(), report.pou_count);
    assert!(embedded.entries.iter().all(|entry| entry.line > 0));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_is_deterministic_except_generation_timestamp() {
    let project = temp_dir("plcopen-transaction-export-determinism");
    transaction_contract_write_project_source(
        &project,
        "b.st",
        "PROGRAM B\nEND_PROGRAM\n",
    );
    transaction_contract_write_project_source(
        &project,
        "a.st",
        "PROGRAM A\nEND_PROGRAM\n",
    );
    let first_output = project.join("out/first.xml");
    let second_output = project.join("out/second.xml");

    let first = export_project_to_xml(&project, &first_output).expect("first export");
    let second = export_project_to_xml(&project, &second_output).expect("second export");

    let first_xml = std::fs::read_to_string(first_output).expect("read first XML");
    let second_xml = std::fs::read_to_string(second_output).expect("read second XML");
    assert_eq!(
        transaction_contract_strip_generation_timestamp(&first_xml),
        transaction_contract_strip_generation_timestamp(&second_xml)
    );
    assert_eq!(
        std::fs::read(first.source_map_path).expect("read first map"),
        std::fs::read(second.source_map_path).expect("read second map")
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_rejects_duplicate_pou_identity_before_mutation() {
    let project = temp_dir("plcopen-transaction-duplicate-pou");
    transaction_contract_write_project_source(
        &project,
        "first.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    transaction_contract_write_project_source(
        &project,
        "second.st",
        "PROGRAM main\nEND_PROGRAM\n",
    );
    let output = project.join("out/plcopen.xml");
    write(&output, "existing XML");
    write(
        &output.with_extension("source-map.json"),
        "existing source map",
    );
    let before = transaction_contract_snapshot(&project.join("out"));

    let error = export_project_to_xml(&project, &output)
        .expect_err("case-insensitive duplicate POU identity must fail");

    assert!(error.to_string().to_ascii_lowercase().contains("duplicate"));
    assert_eq!(transaction_contract_snapshot(&project.join("out")), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_rolls_back_when_source_map_publish_fails() {
    let project = temp_dir("plcopen-transaction-map-failure");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let output = project.join("out/plcopen.xml");
    write(&output, "old XML bytes");
    let source_map = output.with_extension("source-map.json");
    std::fs::create_dir_all(&source_map).expect("create source-map collision");
    write(&source_map.join("keep.txt"), "keep source-map directory");
    let before = transaction_contract_snapshot(&project.join("out"));

    export_project_to_xml(&project, &output).expect_err("source-map publication must fail");

    assert_eq!(transaction_contract_snapshot(&project.join("out")), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_rolls_back_when_adapter_report_publish_fails() {
    let project = temp_dir("plcopen-transaction-adapter-failure");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let output = project.join("out/plcopen.ab.xml");
    write(&output, "old XML bytes");
    write(
        &output.with_extension("source-map.json"),
        "old source-map bytes",
    );
    let adapter = adapter_report_path_for_output(&output);
    std::fs::create_dir_all(&adapter).expect("create adapter collision");
    write(&adapter.join("keep.txt"), "keep adapter directory");
    let before = transaction_contract_snapshot(&project.join("out"));

    export_project_to_xml_with_target(
        &project,
        &output,
        PlcopenExportTarget::AllenBradley,
    )
    .expect_err("adapter publication must fail");

    assert_eq!(transaction_contract_snapshot(&project.join("out")), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_export_rolls_back_when_siemens_bundle_publish_fails() {
    let project = temp_dir("plcopen-transaction-siemens-failure");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let output = project.join("out/plcopen.siemens.xml");
    write(&output, "old XML bytes");
    write(
        &output.with_extension("source-map.json"),
        "old source-map bytes",
    );
    write(
        &siemens_scl_bundle_dir_for_output(&output),
        "existing bundle collision",
    );
    let before = transaction_contract_snapshot(&project.join("out"));

    export_project_to_xml_with_target(&project, &output, PlcopenExportTarget::Siemens)
        .expect_err("Siemens bundle publication must fail");

    assert_eq!(transaction_contract_snapshot(&project.join("out")), before);
    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn transaction_contract_export_rejects_output_symlink_without_following() {
    use std::os::unix::fs::symlink;

    let project = temp_dir("plcopen-transaction-output-symlink");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let outside = project.join("outside.xml");
    write(&outside, "outside sentinel");
    let output = project.join("out/plcopen.xml");
    std::fs::create_dir_all(output.parent().expect("output parent")).expect("create output parent");
    symlink("../outside.xml", &output).expect("create output symlink");

    let error =
        export_project_to_xml(&project, &output).expect_err("output symlink must be rejected");

    assert!(error.to_string().to_ascii_lowercase().contains("symbolic"));
    assert_eq!(
        std::fs::read_to_string(&outside).expect("read outside target"),
        "outside sentinel"
    );
    assert!(std::fs::symlink_metadata(&output)
        .expect("output metadata")
        .file_type()
        .is_symlink());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_successful_export_publishes_complete_generic_set() {
    let project = temp_dir("plcopen-transaction-generic-set");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let output = project.join("new/deep/plcopen.xml");

    let report = export_project_to_xml(&project, &output).expect("generic export");

    assert_eq!(report.output_path, output);
    assert!(report.output_path.is_file());
    assert!(report.source_map_path.is_file());
    assert!(report.adapter_report_path.is_none());
    assert!(report.siemens_scl_bundle_dir.is_none());
    assert!(report.siemens_scl_files.is_empty());
    assert_eq!(report.pou_count, 1);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_successful_adapter_report_binds_committed_paths() {
    let project = temp_dir("plcopen-transaction-adapter-coherence");
    transaction_contract_write_project_source(
        &project,
        "main.st",
        "PROGRAM Main\nEND_PROGRAM\n",
    );
    let output = project.join("out/plcopen.ab.xml");

    let report = export_project_to_xml_with_target(
        &project,
        &output,
        PlcopenExportTarget::AllenBradley,
    )
    .expect("adapter export");

    let adapter_path = report.adapter_report_path.expect("adapter report path");
    let adapter: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&adapter_path).expect("read adapter report"),
    )
    .expect("parse adapter report");
    assert_eq!(adapter["target"], "allen-bradley");
    assert_eq!(
        adapter["source_xml"],
        serde_json::Value::String(output.to_string_lossy().to_string())
    );
    assert_eq!(
        adapter["source_map_path"],
        serde_json::Value::String(report.source_map_path.to_string_lossy().to_string())
    );
    assert!(adapter["siemens_scl_bundle_dir"].is_null());
    assert!(adapter["siemens_scl_files"]
        .as_array()
        .expect("SCL file array")
        .is_empty());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_malformed_import_is_side_effect_free() {
    let project = temp_dir("plcopen-transaction-malformed-import");
    write(&project.join("keep.txt"), "keep");
    let input = project.join("input.xml");
    write(&input, "<project><types>");
    let before = transaction_contract_snapshot(&project);

    import_xml_to_project(&input, &project).expect_err("malformed import must fail");

    assert_eq!(transaction_contract_snapshot(&project), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_invalid_root_import_is_side_effect_free() {
    let project = temp_dir("plcopen-transaction-invalid-root");
    write(&project.join("keep.txt"), "keep");
    let input = project.join("input.xml");
    write(&input, "<notProject/>");
    let before = transaction_contract_snapshot(&project);

    import_xml_to_project(&input, &project).expect_err("invalid root import must fail");

    assert_eq!(transaction_contract_snapshot(&project), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_no_importable_content_is_report_only() {
    let project = temp_dir("plcopen-transaction-report-only");
    let input = project.join("input.xml");
    write(
        &input,
        &format!(
            r#"<project xmlns="{PLCOPEN_NAMESPACE}"><types><pous>
<pou name="Graph" pouType="program"><body><FBD/></body></pou>
</pous></types></project>"#
        ),
    );

    let error =
        import_xml_to_project(&input, &project).expect_err("non-importable content must fail");

    assert!(error.to_string().contains("no importable PLCopen ST content"));
    assert!(!project.join("src").exists());
    assert!(!project.join(IMPORTED_VENDOR_EXTENSION_FILE).exists());
    let report_path = project.join(MIGRATION_REPORT_FILE);
    assert!(report_path.is_file());
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(report_path).expect("read migration report"),
    )
    .expect("parse migration report");
    assert_eq!(report["imported_pous"], 0);
    assert_eq!(report["skipped_pous"], 1);
    assert_eq!(report["unsupported_diagnostics"][0]["code"], "PLCO215");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_preserves_existing_exact_source_with_suffix() {
    let project = temp_dir("plcopen-transaction-import-existing");
    write(&project.join("src/Main.st"), "existing source bytes");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_valid_program_xml("Main", "ImportedMain"),
    );

    let report = import_xml_to_project(&input, &project).expect("import with collision");

    assert_eq!(
        std::fs::read_to_string(project.join("src/Main.st")).expect("read existing source"),
        "existing source bytes"
    );
    assert_eq!(report.written_sources, [project.join("src/Main_2.st")]);
    assert!(project.join("src/Main_2.st").is_file());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_preserves_existing_case_alias_with_suffix() {
    let project = temp_dir("plcopen-transaction-import-existing-case");
    write(&project.join("src/MAIN.ST"), "existing source bytes");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_valid_program_xml("main", "ImportedMain"),
    );

    let report = import_xml_to_project(&input, &project).expect("import with case collision");

    assert_eq!(
        std::fs::read_to_string(project.join("src/MAIN.ST")).expect("read existing source"),
        "existing source bytes"
    );
    assert_eq!(report.written_sources, [project.join("src/main_2.st")]);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_resolves_batch_case_collision_with_suffix() {
    let project = temp_dir("plcopen-transaction-import-batch-case");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_two_program_xml("Main", "First", "main", "Second"),
    );

    let report = import_xml_to_project(&input, &project).expect("import colliding batch");

    assert_eq!(
        report.written_sources,
        [project.join("src/Main.st"), project.join("src/main_2.st")]
    );
    assert!(report.written_sources.iter().all(|path| path.is_file()));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_resolves_sanitized_collision_deterministically() {
    let project = temp_dir("plcopen-transaction-import-sanitized-case");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_two_program_xml("Cell/Main", "First", "Cell?Main", "Second"),
    );

    let report = import_xml_to_project(&input, &project).expect("import sanitized collision");

    assert_eq!(
        report.written_sources,
        [
            project.join("src/Cell_Main.st"),
            project.join("src/Cell_Main_2.st"),
        ]
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_path_like_name_remains_below_source_root() {
    let project = temp_dir("plcopen-transaction-import-path-name");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_valid_program_xml("../../Escape", "SafeBody"),
    );

    let report = import_xml_to_project(&input, &project).expect("import path-like name");

    assert_eq!(report.written_sources.len(), 1);
    assert!(report.written_sources[0].starts_with(project.join("src")));
    assert!(!project.join("Escape.st").exists());
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_rolls_back_when_report_publish_fails() {
    let project = temp_dir("plcopen-transaction-import-report-failure");
    write(&project.join("src/keep.st"), "keep source");
    let report_path = project.join(MIGRATION_REPORT_FILE);
    std::fs::create_dir_all(&report_path).expect("create report collision");
    write(&report_path.join("keep.txt"), "keep report directory");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_valid_program_xml("Main", "Main"),
    );
    let before = transaction_contract_snapshot(&project);

    import_xml_to_project(&input, &project).expect_err("report publication must fail");

    assert_eq!(transaction_contract_snapshot(&project), before);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_import_rolls_back_when_vendor_publish_fails() {
    let project = temp_dir("plcopen-transaction-import-vendor-failure");
    write(&project.join("src/keep.st"), "keep source");
    let vendor_path = project.join(IMPORTED_VENDOR_EXTENSION_FILE);
    std::fs::create_dir_all(&vendor_path).expect("create vendor collision");
    write(&vendor_path.join("keep.txt"), "keep vendor directory");
    let input = project.join("input.xml");
    let xml = transaction_contract_valid_program_xml("Main", "Main").replace(
        "</project>",
        r#"<addData><data name="vendor.raw"><text>opaque</text></data></addData></project>"#,
    );
    write(&input, &xml);
    let before = transaction_contract_snapshot(&project);

    import_xml_to_project(&input, &project).expect_err("vendor publication must fail");

    assert_eq!(transaction_contract_snapshot(&project), before);
    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn transaction_contract_import_rejects_source_symlink_without_following() {
    use std::os::unix::fs::symlink;

    let project = temp_dir("plcopen-transaction-import-symlink");
    let outside = project.join("outside.st");
    write(&outside, "outside sentinel");
    std::fs::create_dir_all(project.join("src")).expect("create source root");
    symlink("../outside.st", project.join("src/Main.st")).expect("create source symlink");
    let input = project.join("input.xml");
    write(
        &input,
        &transaction_contract_valid_program_xml("Main", "Main"),
    );

    let error =
        import_xml_to_project(&input, &project).expect_err("source symlink must be rejected");

    assert!(error.to_string().to_ascii_lowercase().contains("symbolic"));
    assert_eq!(
        std::fs::read_to_string(&outside).expect("read outside source"),
        "outside sentinel"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn transaction_contract_successful_import_publishes_coherent_artifact_set() {
    let project = temp_dir("plcopen-transaction-import-coherence");
    let input = project.join("input.xml");
    let xml = transaction_contract_valid_program_xml("Main", "Main").replace(
        "</project>",
        r#"<addData><data name="vendor.raw"><text>opaque</text></data></addData></project>"#,
    );
    write(&input, &xml);

    let report = import_xml_to_project(&input, &project).expect("import project");

    assert_eq!(report.project_root, project);
    assert_eq!(report.imported_pous, 1);
    assert_eq!(report.discovered_pous, 1);
    assert_eq!(report.written_sources, [project.join("src/Main.st")]);
    assert!(report.written_sources[0].is_file());
    assert_eq!(
        report.preserved_vendor_extensions,
        Some(project.join(IMPORTED_VENDOR_EXTENSION_FILE))
    );
    assert!(project.join(IMPORTED_VENDOR_EXTENSION_FILE).is_file());
    assert_eq!(report.migration_report_path, project.join(MIGRATION_REPORT_FILE));
    assert!(report.migration_report_path.is_file());

    let migration: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&report.migration_report_path)
            .expect("read migration report"),
    )
    .expect("parse migration report");
    assert_eq!(migration["project_root"], project.to_string_lossy().as_ref());
    assert_eq!(migration["source_xml"], input.to_string_lossy().as_ref());
    assert_eq!(migration["imported_pous"], 1);
    assert_eq!(migration["skipped_pous"], 0);
    let _ = std::fs::remove_dir_all(project);
}
