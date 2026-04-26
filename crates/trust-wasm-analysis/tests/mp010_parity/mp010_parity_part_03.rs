use super::*;

#[test]
pub(super) fn definition_for_pump_controller_type_with_plain_demo_uris_returns_target_uri() {
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
    let program_text = documents
        .iter()
        .find(|doc| doc.uri == "program.st")
        .map(|doc| doc.text.clone())
        .expect("program source exists");

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plain-uri documents");

    let offset = program_text
        .find("PumpController;")
        .expect("PumpController use exists") as u32;
    let definition = engine
        .definition(DefinitionRequest {
            uri: "program.st".to_string(),
            position: offset_to_position_utf16(&program_text, offset),
        })
        .expect("definition request should succeed")
        .expect("definition should exist");

    assert_eq!(
        definition.uri, "fb_pump.st",
        "PumpController definition should resolve to fb_pump.st"
    );
}

#[test]
pub(super) fn references_for_program_variable_work_with_plain_demo_uris() {
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
    let program_text = documents
        .iter()
        .find(|doc| doc.uri == "program.st")
        .map(|doc| doc.text.clone())
        .expect("program source exists");

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plain-uri documents");

    let haltreq_offset = program_text
        .find("HaltReq : BOOL;")
        .expect("HaltReq declaration exists") as u32;
    let refs = engine
        .references(ReferencesRequest {
            uri: "program.st".to_string(),
            position: offset_to_position_utf16(&program_text, haltreq_offset),
            include_declaration: Some(true),
        })
        .expect("references request should succeed");

    assert!(
        refs.iter().any(|item| item.uri == "program.st"),
        "program variable references should stay in program.st, got: {:?}",
        refs.iter()
            .map(|item| item.uri.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        refs.len() >= 3,
        "expected declaration + multiple HaltReq usages, got {}",
        refs.len()
    );
}

#[test]
pub(super) fn document_highlight_for_local_symbol_returns_multiple_occurrences() {
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

    let ramp_offset = fb_text
        .find("Ramp := Ramp + 0.2;")
        .expect("Ramp expression exists") as u32;
    let highlights = engine
        .document_highlight(DocumentHighlightRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, ramp_offset),
        })
        .expect("document highlight request should succeed");

    assert!(
        highlights.len() >= 3,
        "expected multiple highlights for local symbol 'Ramp', got {}",
        highlights.len()
    );
}

#[test]
pub(super) fn definition_references_and_rename_accept_punctuation_adjacent_cursor_positions() {
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
    let fb_text = documents
        .iter()
        .find(|doc| doc.uri == "fb_pump.st")
        .map(|doc| doc.text.clone())
        .expect("fb source exists");

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plain-uri documents");

    let ramp_plus_offset = fb_text
        .find("Ramp + 0.2")
        .map(|idx| idx as u32 + "Ramp +".len() as u32 - 1)
        .expect("Ramp expression anchor exists");
    let enum_def = engine
        .definition(DefinitionRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, ramp_plus_offset),
        })
        .expect("definition request at punctuation should succeed");
    assert!(
        enum_def.is_some(),
        "definition should resolve when cursor is at punctuation adjacent to symbol"
    );

    let enable_colon_offset = types_text
        .find("Enable : BOOL;")
        .map(|idx| idx as u32 + "Enable ".len() as u32)
        .expect("Enable declaration anchor exists");
    let enable_refs = engine
        .references(ReferencesRequest {
            uri: "types.st".to_string(),
            position: offset_to_position_utf16(&types_text, enable_colon_offset),
            include_declaration: Some(true),
        })
        .expect("references request at punctuation should succeed");
    assert!(
        !enable_refs.is_empty(),
        "references should resolve when cursor is at punctuation adjacent to field declaration"
    );

    let actual_speed_colon_offset = types_text
        .find("ActualSpeed : REAL;")
        .map(|idx| idx as u32 + "ActualSpeed ".len() as u32)
        .expect("ActualSpeed declaration anchor exists");
    let rename_edits = engine
        .rename(RenameRequest {
            uri: "types.st".to_string(),
            position: offset_to_position_utf16(&types_text, actual_speed_colon_offset),
            new_name: "ActualSpeedRpm".to_string(),
        })
        .expect("rename request at punctuation should succeed");
    assert!(
        !rename_edits.is_empty(),
        "rename should resolve when cursor is at punctuation adjacent to declaration"
    );
}

#[test]
pub(super) fn wasm_json_adapter_contract_is_stable() {
    let mut engine = WasmAnalysisEngine::new();
    let bad_json = engine
        .apply_documents_json("{\"broken\"")
        .expect_err("bad json should fail");
    assert!(bad_json.contains("invalid documents json"));

    let payload = serde_json::to_string(&vec![DocumentInput {
        uri: "memory:///json.st".to_string(),
        text: "PROGRAM Main\nEND_PROGRAM\n".to_string(),
    }])
    .expect("serialize docs");
    let apply_json = engine
        .apply_documents_json(&payload)
        .expect("apply docs json");
    let apply: ApplyDocumentsResult = serde_json::from_str(&apply_json).expect("parse apply json");
    assert_eq!(apply.documents.len(), 1);

    let status_json = engine.status_json().expect("status json");
    let status: EngineStatus = serde_json::from_str(&status_json).expect("parse status json");
    assert_eq!(status.document_count, 1);
    assert_eq!(status.uris, vec!["memory:///json.st".to_string()]);
}

#[test]
pub(super) fn browser_host_smoke_apply_documents_then_diagnostics_round_trip() {
    let mut engine = WasmAnalysisEngine::new();
    let docs = vec![DocumentInput {
        uri: "memory:///smoke.st".to_string(),
        text: "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nCounter := UnknownSymbol + 1;\nEND_PROGRAM\n"
            .to_string(),
    }];
    let payload = serde_json::to_string(&docs).expect("serialize docs");
    let apply = engine
        .apply_documents_json(&payload)
        .expect("apply documents json");
    let parsed_apply: ApplyDocumentsResult =
        serde_json::from_str(&apply).expect("parse apply result");
    assert_eq!(parsed_apply.documents.len(), 1);

    let diagnostics = engine
        .diagnostics_json("memory:///smoke.st")
        .expect("diagnostics json");
    let parsed: Vec<trust_wasm_analysis::DiagnosticItem> =
        serde_json::from_str(&diagnostics).expect("parse diagnostics");
    assert!(
        parsed
            .iter()
            .any(|item| item.message.contains("UnknownSymbol")),
        "expected unresolved symbol diagnostic in smoke round-trip"
    );
}
