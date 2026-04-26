use super::*;

#[test]
pub(super) fn definition_references_and_rename_work_with_plain_demo_uris() {
    let mut documents = load_plant_demo_documents();
    for doc in &mut documents {
        let file_name = doc
            .uri
            .rsplit('/')
            .next()
            .expect("document uri should have file name")
            .to_string();
        doc.uri = file_name;
    }

    let fb_text = documents
        .iter()
        .find(|doc| doc.uri == "fb_pump.st")
        .map(|doc| doc.text.clone())
        .expect("fb source exists");
    let program_text = documents
        .iter()
        .find(|doc| doc.uri == "program.st")
        .map(|doc| doc.text.clone())
        .expect("program source exists");
    let types_text = documents
        .iter()
        .find(|doc| doc.uri == "types.st")
        .map(|doc| doc.text.clone())
        .expect("types source exists");

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plain-uri documents");

    let ramp_offset = fb_text.find("Ramp + 0.2").expect("Ramp use anchor exists") as u32;
    let ramp_def = engine
        .definition(DefinitionRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, ramp_offset),
        })
        .expect("ramp definition request should succeed");
    assert!(ramp_def.is_some(), "local definition for ramp should exist");

    let fb_type_offset = program_text
        .find("PumpController;")
        .expect("PumpController type use anchor exists") as u32;
    let fb_type_def = engine
        .definition(DefinitionRequest {
            uri: "program.st".to_string(),
            position: offset_to_position_utf16(&program_text, fb_type_offset),
        })
        .expect("PumpController definition request should succeed");
    assert!(
        fb_type_def.is_some(),
        "definition for PumpController type use should exist"
    );

    let def_offset = fb_text
        .find("PumpState#Idle")
        .expect("enum use anchor exists") as u32;
    let native = native_project(&[
        DocumentInput {
            uri: "types.st".to_string(),
            text: types_text.clone(),
        },
        DocumentInput {
            uri: "fb_pump.st".to_string(),
            text: fb_text.clone(),
        },
        DocumentInput {
            uri: "program.st".to_string(),
            text: program_text.clone(),
        },
    ]);
    let fb_file = native
        .file_id_for_key(&SourceKey::from_virtual("fb_pump.st".to_string()))
        .expect("fb file id");
    let resolved_name = native.with_database(|db| {
        trust_ide::symbol_name_at_position(db, fb_file, TextSize::from(def_offset))
    });
    assert!(
        resolved_name.is_some(),
        "symbol resolution at enum type prefix should not be None"
    );
    let def = engine
        .definition(DefinitionRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, def_offset),
        })
        .expect("definition request should succeed");
    assert!(
        def.is_some(),
        "definition for enum type used in qualified literal should exist"
    );
}

#[test]
pub(super) fn definition_supports_boundary_cursor_positions_with_plain_demo_uris() {
    let mut documents = load_plant_demo_documents();
    for doc in &mut documents {
        let file_name = doc
            .uri
            .rsplit('/')
            .next()
            .expect("document uri should have file name")
            .to_string();
        doc.uri = file_name;
    }

    let fb_text = documents
        .iter()
        .find(|doc| doc.uri == "fb_pump.st")
        .map(|doc| doc.text.clone())
        .expect("fb source exists");

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plain-uri documents");

    let enum_hash_offset = fb_text
        .find("PumpState#Idle")
        .map(|idx| idx as u32 + "PumpState".len() as u32)
        .expect("enum typed-literal anchor exists");
    let enum_def = engine
        .definition(DefinitionRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, enum_hash_offset),
        })
        .expect("enum hash-boundary definition request should succeed");
    assert!(
        enum_def.is_some(),
        "definition should resolve when cursor is on typed-literal '#' boundary"
    );

    let ramp_boundary_offset = fb_text
        .find("Ramp + 0.2")
        .map(|idx| idx as u32 + "Ramp".len() as u32)
        .expect("Ramp usage anchor exists");
    let ramp_def = engine
        .definition(DefinitionRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, ramp_boundary_offset),
        })
        .expect("ramp boundary definition request should succeed");
    assert!(
        ramp_def.is_some(),
        "definition should resolve when cursor is at local variable boundary"
    );
}

#[test]
pub(super) fn references_and_rename_work_with_plain_demo_uris() {
    let mut documents = load_plant_demo_documents();
    for doc in &mut documents {
        let file_name = doc
            .uri
            .rsplit('/')
            .next()
            .expect("document uri should have file name")
            .to_string();
        doc.uri = file_name;
    }

    let types_text = documents
        .iter()
        .find(|doc| doc.uri == "types.st")
        .map(|doc| doc.text.clone())
        .expect("types source exists");

    let mut engine = BrowserAnalysisEngine::new();
    let native_documents = documents.clone();
    engine
        .replace_documents(documents)
        .expect("load plain-uri documents");

    let refs_offset = types_text
        .find("Enable : BOOL;")
        .expect("Enable decl exists") as u32;
    let native = native_project(&native_documents);
    let native_types_file = native
        .file_id_for_key(&SourceKey::from_virtual("types.st".to_string()))
        .expect("native types file id");
    let native_refs = native.with_database(|db| {
        trust_ide::find_references(
            db,
            native_types_file,
            TextSize::from(refs_offset),
            trust_ide::FindReferencesOptions {
                include_declaration: true,
            },
        )
    });
    assert!(
        !native_refs.is_empty(),
        "native references for Enable declaration should not be empty"
    );
    let refs = engine
        .references(ReferencesRequest {
            uri: "types.st".to_string(),
            position: offset_to_position_utf16(&types_text, refs_offset),
            include_declaration: Some(true),
        })
        .expect("references request should succeed");
    assert!(
        refs.iter().any(|item| item.uri == "types.st"),
        "references should include declaration in types.st, got: {:?}",
        refs.iter()
            .map(|item| item.uri.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        refs.iter().any(|item| item.uri == "fb_pump.st"),
        "references should include Command.Enable usage in fb_pump.st, got: {:?}",
        refs.iter()
            .map(|item| item.uri.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        refs.iter().any(|item| item.uri == "program.st"),
        "references should include Cmd.Enable usage in program.st, got: {:?}",
        refs.iter()
            .map(|item| item.uri.as_str())
            .collect::<Vec<_>>()
    );

    let rename_offset = types_text
        .find("ActualSpeed : REAL;")
        .expect("ActualSpeed decl exists") as u32;
    let rename_edits = engine
        .rename(RenameRequest {
            uri: "types.st".to_string(),
            position: offset_to_position_utf16(&types_text, rename_offset),
            new_name: "ActualSpeedRpm".to_string(),
        })
        .expect("rename request should succeed");
    assert!(
        !rename_edits.is_empty(),
        "rename should produce edits for ActualSpeed"
    );
    assert!(
        rename_edits.iter().any(|edit| edit.uri == "types.st"),
        "rename edits should include declaration in types.st"
    );
    assert!(
        rename_edits.iter().any(|edit| edit.uri == "fb_pump.st"),
        "rename edits should include usage in fb_pump.st"
    );
}
