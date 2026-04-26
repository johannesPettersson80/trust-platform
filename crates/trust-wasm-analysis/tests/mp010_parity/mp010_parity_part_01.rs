use super::*;

#[test]
pub(super) fn diagnostics_parity_matches_native_analysis() {
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
pub(super) fn hover_and_completion_parity_matches_native_analysis() {
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
pub(super) fn completion_for_struct_member_access_returns_expected_members() {
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
pub(super) fn completion_for_statement_prefixes_exposes_program_variables() {
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
pub(super) fn hover_function_block_signature_in_wasm_uses_declared_types() {
    let documents = load_plant_demo_documents();
    let fb_uri = "memory:///plant_demo/fb_pump.st";
    let fb_text = documents
        .iter()
        .find(|doc| doc.uri == fb_uri)
        .map(|doc| doc.text.as_str())
        .expect("fb source exists");
    let hover_offset = fb_text
        .find("PumpController")
        .expect("pump controller name exists") as u32;

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
        hover.contents.contains("Command : PumpCommand;"),
        "hover should include declared input type; hover: {}",
        hover.contents
    );
    assert!(
        hover.contents.contains("Status : PumpStatus;"),
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
