use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use text_size::{TextRange, TextSize};
use trust_hir::project::{Project, SourceKey};
use trust_hir::DiagnosticSeverity;
use trust_ide::StdlibFilter;
use trust_wasm_analysis::{
    ApplyDocumentsResult, BrowserAnalysisEngine, CompletionItem, CompletionRequest,
    DefinitionRequest, DocumentInput, EngineStatus, HoverItem, HoverRequest, Position, Range,
    ReferencesRequest, RelatedInfoItem, RenameRequest, WasmAnalysisEngine,
};

#[test]
fn diagnostics_parity_matches_native_analysis() {
    let document = DocumentInput {
        uri: "memory:///diagnostics.st".to_string(),
        text: r#"PROGRAM Main
VAR
    value : INT;
END_VAR

value := UnknownSymbol + 1;
END_PROGRAM
"#
        .to_string(),
    };

    let mut engine = BrowserAnalysisEngine::new();
    let apply = engine
        .replace_documents(vec![document.clone()])
        .expect("load documents");
    assert_eq!(apply.documents.len(), 1);

    let adapter = engine
        .diagnostics(&document.uri)
        .expect("adapter diagnostics");
    let native = native_diagnostics(&[document], "memory:///diagnostics.st");
    assert_eq!(adapter, native);
}

#[test]
fn hover_and_completion_parity_matches_native_analysis() {
    let hover_doc = DocumentInput {
        uri: "memory:///hover.st".to_string(),
        text: r#"PROGRAM Main
VAR
    value : INT;
END_VAR

value := value + 1;
END_PROGRAM
"#
        .to_string(),
    };
    let completion_doc = DocumentInput {
        uri: "memory:///completion.st".to_string(),
        text: r#"PROGRAM Main
VAR
    value : INT;
END_VAR

val
END_PROGRAM
"#
        .to_string(),
    };
    let documents = vec![hover_doc.clone(), completion_doc.clone()];

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents.clone())
        .expect("load documents");

    let hover_offset = hover_doc
        .text
        .find("value + 1;")
        .expect("hover anchor exists") as u32;
    let hover_request = HoverRequest {
        uri: hover_doc.uri.clone(),
        position: offset_to_position_utf16(&hover_doc.text, hover_offset),
    };
    let adapter_hover = engine.hover(hover_request.clone()).expect("adapter hover");
    let native_hover = native_hover(&documents, &hover_request);
    assert_eq!(adapter_hover, native_hover);

    let completion_offset = completion_doc
        .text
        .find("val")
        .expect("completion anchor exists") as u32
        + 3;
    let completion_request = CompletionRequest {
        uri: completion_doc.uri.clone(),
        position: offset_to_position_utf16(&completion_doc.text, completion_offset),
        limit: Some(30),
    };
    let adapter_completion = engine
        .completion(completion_request.clone())
        .expect("adapter completion");
    let native_completion = native_completion(&documents, &completion_request);
    assert_eq!(adapter_completion, native_completion);
}

#[test]
fn completion_for_struct_member_access_returns_expected_members() {
    let documents = load_plant_demo_documents();
    let program_uri = "memory:///plant_demo/program.st";
    let program_text = documents
        .iter()
        .find(|doc| doc.uri == program_uri)
        .map(|doc| doc.text.as_str())
        .expect("program source exists");

    let completion_offset = program_text
        .find("Status.State")
        .map(|idx| idx as u32 + "Status.".len() as u32)
        .expect("status member access anchor exists");
    let request = CompletionRequest {
        uri: program_uri.to_string(),
        position: offset_to_position_utf16(program_text, completion_offset),
        limit: Some(80),
    };

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plant demo documents");
    let completion = engine.completion(request).expect("completion");

    let labels = completion
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"State"),
        "completion should include struct field 'State', got: {labels:?}"
    );
    assert!(
        labels.contains(&"Running"),
        "completion should include struct field 'Running', got: {labels:?}"
    );
    assert!(
        labels.contains(&"ActualSpeed"),
        "completion should include struct field 'ActualSpeed', got: {labels:?}"
    );
}

#[test]
fn completion_for_statement_prefixes_exposes_program_variables() {
    let cases = [
        ("Cm", "Cmd"),
        ("Sta", "Status"),
        ("Pu", "Pump"),
        ("Ha", "HaltReq"),
    ];
    for (prefix, expected) in cases {
        let labels = completion_labels_for_program_prefix(prefix);
        assert!(
            labels
                .iter()
                .any(|label| label.eq_ignore_ascii_case(expected)),
            "completion should include '{expected}' for prefix '{prefix}', got: {labels:?}"
        );
    }
}

#[test]
fn hover_function_block_signature_in_wasm_uses_declared_types() {
    let documents = load_plant_demo_documents();
    let fb_uri = "memory:///plant_demo/fb_pump.st";
    let fb_text = documents
        .iter()
        .find(|doc| doc.uri == fb_uri)
        .map(|doc| doc.text.as_str())
        .expect("fb source exists");
    let hover_offset = fb_text.find("FB_Pump").expect("fb name exists") as u32;

    let request = HoverRequest {
        uri: fb_uri.to_string(),
        position: offset_to_position_utf16(fb_text, hover_offset),
    };
    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plant demo documents");
    let hover = engine
        .hover(request)
        .expect("hover request should succeed")
        .expect("hover payload should exist");

    assert!(
        hover.contents.contains("Command : ST_PumpCommand;"),
        "hover should include declared input type; hover: {}",
        hover.contents
    );
    assert!(
        hover.contents.contains("Status : ST_PumpStatus;"),
        "hover should include declared output type; hover: {}",
        hover.contents
    );
    assert!(
        !hover.contents.contains("Command : ?;"),
        "hover should not use unknown placeholder for Command; hover: {}",
        hover.contents
    );
    assert!(
        !hover.contents.contains("Status : ?;"),
        "hover should not use unknown placeholder for Status; hover: {}",
        hover.contents
    );
}

#[test]
fn definition_references_and_rename_work_with_plain_demo_uris() {
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

    let ramp_offset = fb_text.find("ramp + 0.2").expect("ramp use anchor exists") as u32;
    let ramp_def = engine
        .definition(DefinitionRequest {
            uri: "fb_pump.st".to_string(),
            position: offset_to_position_utf16(&fb_text, ramp_offset),
        })
        .expect("ramp definition request should succeed");
    assert!(ramp_def.is_some(), "local definition for ramp should exist");

    let fb_type_offset = program_text
        .find("FB_Pump;")
        .expect("FB_Pump type use anchor exists") as u32;
    let fb_type_def = engine
        .definition(DefinitionRequest {
            uri: "program.st".to_string(),
            position: offset_to_position_utf16(&program_text, fb_type_offset),
        })
        .expect("FB_Pump definition request should succeed");
    assert!(
        fb_type_def.is_some(),
        "definition for FB_Pump type use should exist"
    );

    let def_offset = fb_text
        .find("E_PumpState#Idle")
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
fn definition_supports_boundary_cursor_positions_with_plain_demo_uris() {
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
        .find("E_PumpState#Idle")
        .map(|idx| idx as u32 + "E_PumpState".len() as u32)
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
        .find("ramp + 0.2")
        .map(|idx| idx as u32 + "ramp".len() as u32)
        .expect("ramp usage anchor exists");
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
fn references_and_rename_work_with_plain_demo_uris() {
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

#[test]
fn definition_for_fb_pump_type_with_plain_demo_uris_returns_target_uri() {
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

    let offset = program_text.find("FB_Pump;").expect("FB_Pump use exists") as u32;
    let definition = engine
        .definition(DefinitionRequest {
            uri: "program.st".to_string(),
            position: offset_to_position_utf16(&program_text, offset),
        })
        .expect("definition request should succeed")
        .expect("definition should exist");

    assert_eq!(
        definition.uri, "fb_pump.st",
        "FB_Pump definition should resolve to fb_pump.st"
    );
}

#[test]
fn references_for_program_variable_work_with_plain_demo_uris() {
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
fn wasm_json_adapter_contract_is_stable() {
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
fn browser_host_smoke_apply_documents_then_diagnostics_round_trip() {
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

#[test]
fn browser_analysis_latency_budget_against_native_is_within_spike_limits() {
    let documents = load_plant_demo_documents();
    let main_uri = "memory:///plant_demo/program.st";
    let main_text = documents
        .iter()
        .find(|doc| doc.uri == main_uri)
        .map(|doc| doc.text.clone())
        .expect("program document present");

    let mut adapter = BrowserAnalysisEngine::new();
    adapter
        .replace_documents(documents.clone())
        .expect("load adapter docs");

    let native_project = native_project(&documents);
    let native_file = native_project
        .file_id_for_key(&SourceKey::from_virtual(main_uri.to_string()))
        .expect("native file id");

    let hover_offset = main_text.find("Pump.Status").expect("hover anchor exists") as u32;
    let hover_position = offset_to_position_utf16(&main_text, hover_offset);
    let completion_offset = main_text
        .find("Status := ")
        .expect("completion anchor exists") as u32
        + 10;
    let completion_position = offset_to_position_utf16(&main_text, completion_offset);

    let hover_request = HoverRequest {
        uri: main_uri.to_string(),
        position: hover_position,
    };
    let completion_request = CompletionRequest {
        uri: main_uri.to_string(),
        position: completion_position,
        limit: Some(50),
    };

    // Warm both paths before timing to reduce first-query cache noise.
    black_box(
        adapter
            .diagnostics(main_uri)
            .expect("adapter diagnostics warmup"),
    );
    black_box(
        adapter
            .hover(hover_request.clone())
            .expect("adapter hover warmup"),
    );
    black_box(
        adapter
            .completion(completion_request.clone())
            .expect("adapter completion warmup"),
    );
    native_project.with_database(|db| {
        black_box(trust_ide::diagnostics::collect_diagnostics(db, native_file));
        black_box(trust_ide::hover_with_filter(
            db,
            native_file,
            TextSize::from(hover_offset),
            &StdlibFilter::allow_all(),
        ));
        black_box(trust_ide::complete_with_filter(
            db,
            native_file,
            TextSize::from(completion_offset),
            &StdlibFilter::allow_all(),
        ));
    });

    let iterations = 24;
    let adapter_diagnostics = measure_iterations(iterations, || {
        black_box(adapter.diagnostics(main_uri).expect("adapter diagnostics"))
    });
    let adapter_hover = measure_iterations(iterations, || {
        black_box(adapter.hover(hover_request.clone()).expect("adapter hover"))
    });
    let adapter_completion = measure_iterations(iterations, || {
        black_box(
            adapter
                .completion(completion_request.clone())
                .expect("adapter completion"),
        )
    });

    let native_diagnostics = measure_iterations(iterations, || {
        native_project.with_database(|db| {
            black_box(trust_ide::diagnostics::collect_diagnostics(db, native_file))
        })
    });
    let native_hover = measure_iterations(iterations, || {
        native_project.with_database(|db| {
            black_box(trust_ide::hover_with_filter(
                db,
                native_file,
                TextSize::from(hover_offset),
                &StdlibFilter::allow_all(),
            ))
        })
    });
    let native_completion = measure_iterations(iterations, || {
        native_project.with_database(|db| {
            black_box(trust_ide::complete_with_filter(
                db,
                native_file,
                TextSize::from(completion_offset),
                &StdlibFilter::allow_all(),
            ))
        })
    });

    assert_budget("diagnostics", adapter_diagnostics, native_diagnostics);
    assert_budget("hover", adapter_hover, native_hover);
    assert_budget("completion", adapter_completion, native_completion);
}

#[test]
fn multi_document_incremental_update_flow_handles_realistic_edit_streams() {
    let mut engine = BrowserAnalysisEngine::new();
    let mut documents = vec![
        DocumentInput {
            uri: "memory:///workspace/main.st".to_string(),
            text:
                "PROGRAM Main\nVAR\ncounter : INT;\nEND_VAR\ncounter := counter + 1;\nEND_PROGRAM\n"
                    .to_string(),
        },
        DocumentInput {
            uri: "memory:///workspace/helpers.st".to_string(),
            text: "FUNCTION Helper : INT\nHelper := 1;\nEND_FUNCTION\n".to_string(),
        },
        DocumentInput {
            uri: "memory:///workspace/io.st".to_string(),
            text: "PROGRAM Io\nVAR\nInputA : BOOL;\nEND_VAR\nEND_PROGRAM\n".to_string(),
        },
    ];

    engine
        .replace_documents(documents.clone())
        .expect("initial documents");
    assert_eq!(engine.status().document_count, 3);

    for step in 0..40_u32 {
        documents[0].text = format!(
            "PROGRAM Main\nVAR\ncounter : INT;\nEND_VAR\ncounter := counter + {};\nEND_PROGRAM\n",
            step
        );

        if step % 5 == 0 {
            documents[1].text =
                format!("FUNCTION Helper : INT\nHelper := {};\nEND_FUNCTION\n", step);
        }

        if step % 7 == 0 {
            documents[2].text =
                "PROGRAM Io\nVAR\nInputA : BOOL;\nEND_VAR\nInputA := UnknownSymbol;\nEND_PROGRAM\n"
                    .to_string();
        } else {
            documents[2].text =
                "PROGRAM Io\nVAR\nInputA : BOOL;\nEND_VAR\nEND_PROGRAM\n".to_string();
        }

        engine
            .replace_documents(documents.clone())
            .expect("replace documents");

        let status = engine.status();
        assert_eq!(status.document_count, 3);
        assert!(status
            .uris
            .iter()
            .any(|uri| uri == "memory:///workspace/main.st"));

        let diagnostics = engine
            .diagnostics("memory:///workspace/io.st")
            .expect("diagnostics");
        if step % 7 == 0 {
            assert!(
                diagnostics
                    .iter()
                    .any(|item| item.message.contains("UnknownSymbol")),
                "expected unresolved symbol diagnostic on step {step}"
            );
        } else {
            assert!(
                diagnostics
                    .iter()
                    .all(|item| !item.message.contains("UnknownSymbol")),
                "unexpected unresolved symbol diagnostic on step {step}"
            );
        }
    }
}

#[test]
fn representative_corpus_memory_budget_gate() {
    let base = load_plant_demo_documents();
    let mut corpus = Vec::new();
    for replica in 0..12_u32 {
        for doc in &base {
            corpus.push(DocumentInput {
                uri: doc
                    .uri
                    .replace("memory:///", &format!("memory:///replica-{replica}/")),
                text: doc.text.clone(),
            });
        }
    }

    let before_kib = process_memory_kib();

    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(corpus.clone())
        .expect("load representative corpus");

    assert_eq!(engine.status().document_count, corpus.len());

    black_box(
        engine
            .diagnostics("memory:///replica-0/plant_demo/program.st")
            .expect("diagnostics"),
    );
    black_box(
        engine
            .completion(CompletionRequest {
                uri: "memory:///replica-0/plant_demo/program.st".to_string(),
                position: Position {
                    line: 18,
                    character: 12,
                },
                limit: Some(40),
            })
            .expect("completion"),
    );

    if let (Some(before), Some(after)) = (before_kib, process_memory_kib()) {
        let delta = after.saturating_sub(before);
        let absolute = after;
        assert!(
            delta <= 350 * 1024,
            "RSS delta exceeded memory budget: before={} KiB after={} KiB delta={} KiB",
            before,
            after,
            delta
        );
        assert!(
            absolute <= 700 * 1024,
            "RSS absolute exceeded memory budget: {} KiB",
            absolute
        );
    }
}

fn assert_budget(name: &str, adapter: Duration, native: Duration) {
    let adapter_us = adapter.as_micros();
    let native_us = native.as_micros();
    let ratio_limit = native_us.saturating_mul(4);
    let headroom = 120_000_u128;
    let allowed = ratio_limit.saturating_add(headroom);

    eprintln!(
        "{name}: adapter={}us native={}us allowed={}us",
        adapter_us, native_us, allowed
    );

    assert!(
        adapter_us <= allowed,
        "{name} exceeded spike budget (adapter={}us native={}us allowed={}us)",
        adapter_us,
        native_us,
        allowed
    );
    assert!(
        adapter <= Duration::from_secs(2),
        "{name} exceeded absolute 2s spike limit: {adapter:?}"
    );
}

fn measure_iterations<T>(iterations: usize, mut op: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(op());
    }
    start.elapsed()
}

#[cfg(target_os = "linux")]
fn process_memory_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let value = rest
                .split_whitespace()
                .next()
                .and_then(|text| text.parse::<u64>().ok())?;
            return Some(value);
        }
    }
    None
}

fn completion_labels_for_program_prefix(prefix: &str) -> Vec<String> {
    let mut documents = load_plant_demo_documents();
    let program_uri = "memory:///plant_demo/program.st";
    let program_index = documents
        .iter()
        .position(|doc| doc.uri == program_uri)
        .expect("program source exists");
    let anchor = "Pump(Command := Cmd);";
    let anchor_offset = documents[program_index]
        .text
        .find(anchor)
        .expect("anchor statement exists");
    let (before, after) = documents[program_index].text.split_at(anchor_offset);
    let updated_program = format!("{before}{prefix}\n{after}");
    let completion_offset = anchor_offset as u32 + prefix.len() as u32;
    documents[program_index].text = updated_program;

    let request = CompletionRequest {
        uri: program_uri.to_string(),
        position: offset_to_position_utf16(&documents[program_index].text, completion_offset),
        limit: Some(80),
    };
    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plant demo documents");
    engine
        .completion(request)
        .expect("completion should succeed")
        .into_iter()
        .map(|item| item.label)
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn process_memory_kib() -> Option<u64> {
    None
}

fn native_diagnostics(
    documents: &[DocumentInput],
    uri: &str,
) -> Vec<trust_wasm_analysis::DiagnosticItem> {
    let project = native_project(documents);
    let source = documents
        .iter()
        .find(|doc| doc.uri == uri)
        .map(|doc| doc.text.as_str())
        .expect("source exists");
    let file_id = project
        .file_id_for_key(&SourceKey::from_virtual(uri.to_string()))
        .expect("file id exists");

    let mut items = project.with_database(|db| {
        trust_ide::diagnostics::collect_diagnostics(db, file_id)
            .into_iter()
            .map(|diagnostic| {
                let mut related = diagnostic
                    .related
                    .into_iter()
                    .map(|item| RelatedInfoItem {
                        range: text_range_to_lsp(source, item.range),
                        message: item.message,
                    })
                    .collect::<Vec<_>>();
                related.sort_by(|left, right| {
                    left.range
                        .cmp(&right.range)
                        .then_with(|| left.message.cmp(&right.message))
                });
                trust_wasm_analysis::DiagnosticItem {
                    code: diagnostic.code.code().to_string(),
                    severity: severity_label(diagnostic.severity).to_string(),
                    message: diagnostic.message,
                    range: text_range_to_lsp(source, diagnostic.range),
                    related,
                }
            })
            .collect::<Vec<_>>()
    });
    items.sort_by(|left, right| {
        left.range
            .cmp(&right.range)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
            .then_with(|| left.severity.cmp(&right.severity))
    });
    items
}

fn native_hover(documents: &[DocumentInput], request: &HoverRequest) -> Option<HoverItem> {
    let project = native_project(documents);
    let source = documents
        .iter()
        .find(|doc| doc.uri == request.uri)
        .map(|doc| doc.text.as_str())
        .expect("source exists");
    let file_id = project
        .file_id_for_key(&SourceKey::from_virtual(request.uri.clone()))
        .expect("file id exists");
    let offset = position_to_offset_utf16(source, request.position.clone()).expect("offset");

    project.with_database(|db| {
        trust_ide::hover_with_filter(
            db,
            file_id,
            TextSize::from(offset),
            &StdlibFilter::allow_all(),
        )
        .map(|hover| HoverItem {
            contents: hover.contents,
            range: hover.range.map(|range| text_range_to_lsp(source, range)),
        })
    })
}

fn native_completion(
    documents: &[DocumentInput],
    request: &CompletionRequest,
) -> Vec<CompletionItem> {
    let project = native_project(documents);
    let source = documents
        .iter()
        .find(|doc| doc.uri == request.uri)
        .map(|doc| doc.text.as_str())
        .expect("source exists");
    let file_id = project
        .file_id_for_key(&SourceKey::from_virtual(request.uri.clone()))
        .expect("file id exists");
    let offset = position_to_offset_utf16(source, request.position.clone()).expect("offset");

    let mut items = project.with_database(|db| {
        trust_ide::complete_with_filter(
            db,
            file_id,
            TextSize::from(offset),
            &StdlibFilter::allow_all(),
        )
    });
    let typed_prefix = completion_prefix_at_offset(source, offset);
    items.sort_by(|left, right| {
        completion_match_rank(left.label.as_str(), typed_prefix.as_deref())
            .cmp(&completion_match_rank(
                right.label.as_str(),
                typed_prefix.as_deref(),
            ))
            .then_with(|| left.sort_priority.cmp(&right.sort_priority))
            .then_with(|| left.label.cmp(&right.label))
    });
    let limit = request.limit.unwrap_or(50).clamp(1, 500) as usize;
    items
        .into_iter()
        .take(limit)
        .map(|item| CompletionItem {
            label: item.label.to_string(),
            kind: completion_kind_label(item.kind).to_string(),
            detail: item.detail.map(|value| value.to_string()),
            documentation: item.documentation.map(|value| value.to_string()),
            insert_text: item.insert_text.map(|value| value.to_string()),
            text_edit: item
                .text_edit
                .map(|edit| trust_wasm_analysis::CompletionTextEditItem {
                    range: text_range_to_lsp(source, edit.range),
                    new_text: edit.new_text.to_string(),
                }),
            sort_priority: item.sort_priority,
        })
        .collect()
}

fn completion_prefix_at_offset(source: &str, offset: u32) -> Option<String> {
    let bytes = source.as_bytes();
    let mut cursor = (offset as usize).min(bytes.len());
    let end = cursor;
    while cursor > 0 && is_ident_byte(bytes[cursor - 1]) {
        cursor -= 1;
    }
    if cursor == end {
        return None;
    }
    let prefix = &source[cursor..end];
    if prefix.is_empty() {
        return None;
    }
    Some(prefix.to_ascii_uppercase())
}

fn completion_match_rank(label: &str, typed_prefix: Option<&str>) -> u8 {
    let Some(prefix) = typed_prefix else {
        return 2;
    };
    if prefix.is_empty() {
        return 2;
    }
    let label_upper = label.to_ascii_uppercase();
    if label_upper == prefix {
        return 0;
    }
    if label_upper.starts_with(prefix) {
        return 1;
    }
    if label_upper.contains(prefix) {
        return 2;
    }
    3
}

fn is_ident_byte(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

fn completion_kind_label(kind: trust_ide::CompletionKind) -> &'static str {
    match kind {
        trust_ide::CompletionKind::Keyword => "keyword",
        trust_ide::CompletionKind::Function => "function",
        trust_ide::CompletionKind::FunctionBlock => "function_block",
        trust_ide::CompletionKind::Method => "method",
        trust_ide::CompletionKind::Property => "property",
        trust_ide::CompletionKind::Variable => "variable",
        trust_ide::CompletionKind::Constant => "constant",
        trust_ide::CompletionKind::Type => "type",
        trust_ide::CompletionKind::EnumValue => "enum_value",
        trust_ide::CompletionKind::Snippet => "snippet",
    }
}

fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

fn native_project(documents: &[DocumentInput]) -> Project {
    let mut project = Project::default();
    for document in documents {
        project.set_source_text(
            SourceKey::from_virtual(document.uri.clone()),
            document.text.clone(),
        );
    }
    project
}

fn text_range_to_lsp(content: &str, range: TextRange) -> Range {
    Range {
        start: offset_to_position_utf16(content, u32::from(range.start())),
        end: offset_to_position_utf16(content, u32::from(range.end())),
    }
}

fn offset_to_position_utf16(content: &str, offset: u32) -> Position {
    let clamped_offset = (offset as usize).min(content.len());
    let mut line = 0u32;
    let mut character = 0u32;
    for (index, ch) in content.char_indices() {
        if index >= clamped_offset {
            break;
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        } else {
            character = character.saturating_add(ch.len_utf16() as u32);
        }
    }
    Position { line, character }
}

fn position_to_offset_utf16(content: &str, position: Position) -> Option<u32> {
    let mut line = 0u32;
    let mut character = 0u32;
    for (index, ch) in content.char_indices() {
        if line == position.line {
            if character == position.character {
                return Some(index as u32);
            }
            if ch == '\n' {
                return Some(index as u32);
            }
            let width = ch.len_utf16() as u32;
            if character.saturating_add(width) > position.character {
                return Some(index as u32);
            }
            character = character.saturating_add(width);
            continue;
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        }
    }

    if line == position.line {
        Some(content.len() as u32)
    } else {
        None
    }
}

fn load_plant_demo_documents() -> Vec<DocumentInput> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/plant_demo/src")
        .canonicalize()
        .expect("canonicalize plant_demo path");
    let files = ["types.st", "fb_pump.st", "program.st", "config.st"];
    files
        .iter()
        .map(|name| {
            let path = root.join(name);
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read {} failed: {err}", path.display()));
            DocumentInput {
                uri: format!("memory:///plant_demo/{name}"),
                text,
            }
        })
        .collect()
}
