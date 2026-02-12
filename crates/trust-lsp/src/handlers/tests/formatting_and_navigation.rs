use super::*;

#[test]
fn lsp_signature_help_snapshot() {
    let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    x : INT;
END_VAR
    x := Add(1, 2|);
END_PROGRAM
"#;
    let cursor = source.find('|').expect("cursor");
    let mut cleaned = source.to_string();
    cleaned.remove(cursor);

    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, cleaned.to_string());

    let position = super::lsp_utils::offset_to_position(&cleaned, cursor as u32);
    let params = tower_lsp::lsp_types::SignatureHelpParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position,
        },
        work_done_progress_params: Default::default(),
        context: None,
    };

    let result = signature_help(&state, params).expect("signature help");
    let json = serde_json::to_string_pretty(&result).expect("serialize signature help");
    expect![[r#"
{
  "signatures": [
    {
      "label": "Add(A: INT, B: INT) : INT",
      "parameters": [
        {
          "label": "A: INT"
        },
        {
          "label": "B: INT"
        }
      ]
    }
  ],
  "activeSignature": 0,
  "activeParameter": 1
}"#]]
    .assert_eq(&json);
}

#[test]
fn lsp_formatting_snapshot() {
    let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nx:=1;\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = formatting(&state, params).expect("formatting edits");
    let formatted = edits
        .first()
        .map(|edit| edit.new_text.as_str())
        .unwrap_or("");
    expect![[r#"
PROGRAM Test
    VAR
        x: INT;
    END_VAR
    x := 1;
END_PROGRAM
"#]]
    .assert_eq(formatted);
}

#[test]
fn lsp_formatting_vendor_profile_applies_keyword_case() {
    let source = "program Test\nvar\nx:INT;\nend_var\nx:=1+2;\nend_program\n";
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: Some("siemens".to_string()),
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = formatting(&state, params).expect("formatting edits");
    assert!(!edits.is_empty());
    let formatted = edits[0].new_text.as_str();
    let expected = "PROGRAM Test\n  VAR\n    x:INT;\n  END_VAR\n  x:=1+2;\nEND_PROGRAM\n";
    assert_eq!(formatted, expected);
}

#[test]
fn lsp_formatting_siemens_profile_preserves_hash_prefixed_references() {
    let source = "program Test\nvar\nsum:int;\nend_var\n#sum := #sum + 1;\nend_program\n";
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: Some("siemens".to_string()),
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = formatting(&state, params).expect("formatting edits");
    assert!(!edits.is_empty());
    let formatted = edits[0].new_text.as_str();
    let expected = "PROGRAM Test\n  VAR\n    sum:INT;\n  END_VAR\n  #sum:=#sum+1;\nEND_PROGRAM\n";
    assert_eq!(formatted, expected);
}

#[test]
fn lsp_formatting_mitsubishi_profile_keeps_spaced_style() {
    let source = "program Test\nvar\npulse:bool;\nrise:difu;\nq_rise:bool;\nend_var\nrise(clk:=pulse,q=>q_rise);\nend_program\n";
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::parse("file:///workspace/").unwrap();
    state.set_workspace_config(
        root_uri,
        ProjectConfig {
            root: PathBuf::from("/workspace"),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: Some("mitsubishi".to_string()),
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        },
    );

    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = formatting(&state, params).expect("formatting edits");
    assert!(!edits.is_empty());
    let formatted = edits[0].new_text.as_str();
    let expected = "PROGRAM Test\n    VAR\n        pulse : BOOL;\n        rise  : difu;\n        q_rise: BOOL;\n    END_VAR\n    rise(clk := pulse, q => q_rise);\nEND_PROGRAM\n";
    assert_eq!(formatted, expected);
}

#[test]
fn lsp_formatting_accepts_snake_case_client_keys() {
    let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nx:=1;\nEND_PROGRAM\n";
    let state = ServerState::new();
    state.set_config(serde_json::json!({
        "trust_lsp": {
            "format": {
                "indent_width": 2,
                "insert_spaces": true,
                "keyword_case": "lower"
            }
        }
    }));
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = formatting(&state, params).expect("formatting edits");
    assert!(!edits.is_empty());
    let formatted = edits[0].new_text.as_str();
    assert!(formatted.contains("program Test"));
    assert!(formatted.contains("\n  var\n"));
    assert!(formatted.contains("\n  x := 1;\n"));
}

#[test]
fn lsp_formatting_prefers_camel_case_when_both_aliases_present() {
    let source = "program Test\nvar\nx:INT;\nend_var\nx:=1;\nend_program\n";
    let state = ServerState::new();
    state.set_config(serde_json::json!({
        "stLsp": {
            "format": {
                "indentWidth": 4,
                "indent_width": 2,
                "keywordCase": "upper",
                "keyword_case": "lower"
            }
        }
    }));
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = formatting(&state, params).expect("formatting edits");
    assert!(!edits.is_empty());
    let formatted = edits[0].new_text.as_str();
    assert!(formatted.contains("PROGRAM Test"));
    assert!(formatted.contains("\n    VAR\n"));
    assert!(formatted.contains("\n    x := 1;\n"));
}

#[test]
fn lsp_code_lens_references() {
    let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, 2);
    result := Add(2, 3);
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::CodeLensParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let lenses = code_lens(&state, params).expect("code lenses");
    let mut found = false;
    for lens in lenses {
        if let Some(cmd) = &lens.command {
            if let Some(count_str) = cmd.title.strip_prefix("References: ") {
                if let Ok(count) = count_str.trim().parse::<usize>() {
                    if count >= 2 {
                        found = true;
                    }
                }
            }
        }
    }
    assert!(found, "expected references code lens");
}

#[test]
fn lsp_document_link_using_directive() {
    let lib_source = r#"
NAMESPACE Lib
FUNCTION Foo : INT
VAR_INPUT
    A : INT;
END_VAR
    Foo := A;
END_FUNCTION
END_NAMESPACE
"#;
    let main_source = r#"
USING Lib;
FUNCTION Bar : INT
    Bar := Foo(1);
END_FUNCTION
"#;
    let state = ServerState::new();
    let lib_uri = tower_lsp::lsp_types::Url::parse("file:///lib.st").unwrap();
    let main_uri = tower_lsp::lsp_types::Url::parse("file:///main.st").unwrap();
    state.open_document(lib_uri.clone(), 1, lib_source.to_string());
    state.open_document(main_uri.clone(), 1, main_source.to_string());

    let params = tower_lsp::lsp_types::DocumentLinkParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
            uri: main_uri.clone(),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let links = document_link(&state, params).expect("document links");
    let start_offset = main_source.find("Lib").expect("Lib offset") as u32;
    let end_offset = start_offset + "Lib".len() as u32;
    assert!(links.iter().any(|link| {
        link.target.as_ref() == Some(&lib_uri)
            && super::lsp_utils::position_to_offset(main_source, link.range.start)
                .map(|start| start <= start_offset)
                .unwrap_or(false)
            && super::lsp_utils::position_to_offset(main_source, link.range.end)
                .map(|end| end >= end_offset)
                .unwrap_or(false)
    }));
}

#[test]
fn lsp_document_link_config_paths() {
    let source = r#"
[project]
include_paths = ["src", "lib"]
library_paths = ["vendor/lib"]

[[libraries]]
name = "Extra"
path = "extras/ExtraLib"
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///workspace/trust-lsp.toml").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let params = tower_lsp::lsp_types::DocumentLinkParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let links = document_link(&state, params).expect("document links");
    let src_target = tower_lsp::lsp_types::Url::parse("file:///workspace/src").unwrap();
    let lib_target = tower_lsp::lsp_types::Url::parse("file:///workspace/vendor/lib").unwrap();
    let extra_target =
        tower_lsp::lsp_types::Url::parse("file:///workspace/extras/ExtraLib").unwrap();

    assert!(links
        .iter()
        .any(|link| link.target.as_ref() == Some(&src_target)));
    assert!(links
        .iter()
        .any(|link| link.target.as_ref() == Some(&lib_target)));
    assert!(links
        .iter()
        .any(|link| link.target.as_ref() == Some(&extra_target)));
}

#[test]
fn lsp_call_hierarchy_incoming_outgoing() {
    let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, 2);
END_PROGRAM
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let prepare_main = tower_lsp::lsp_types::CallHierarchyPrepareParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "Main"),
        },
        work_done_progress_params: Default::default(),
    };
    let main_items = prepare_call_hierarchy(&state, prepare_main).expect("prepare main");
    let main_item = main_items.first().expect("main item").clone();

    let outgoing_params = tower_lsp::lsp_types::CallHierarchyOutgoingCallsParams {
        item: main_item,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let outgoing = outgoing_calls(&state, outgoing_params).expect("outgoing calls");
    assert!(outgoing.iter().any(|call| call.to.name.contains("Add")));

    let prepare_add = tower_lsp::lsp_types::CallHierarchyPrepareParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "Add : INT"),
        },
        work_done_progress_params: Default::default(),
    };
    let add_items = prepare_call_hierarchy(&state, prepare_add).expect("prepare add");
    let add_item = add_items.first().expect("add item").clone();

    let incoming_params = tower_lsp::lsp_types::CallHierarchyIncomingCallsParams {
        item: add_item,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let incoming = incoming_calls(&state, incoming_params).expect("incoming calls");
    assert!(incoming.iter().any(|call| call.from.name.contains("Main")));
}

#[test]
fn lsp_call_hierarchy_cross_file_incoming() {
    let add_source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION
"#;
    let main_source = r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, 2);
END_PROGRAM
"#;
    let state = ServerState::new();
    let add_uri = tower_lsp::lsp_types::Url::parse("file:///add.st").unwrap();
    let main_uri = tower_lsp::lsp_types::Url::parse("file:///main.st").unwrap();
    state.open_document(add_uri.clone(), 1, add_source.to_string());
    state.open_document(main_uri.clone(), 1, main_source.to_string());

    let prepare_add = tower_lsp::lsp_types::CallHierarchyPrepareParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: add_uri },
            position: position_at(add_source, "Add : INT"),
        },
        work_done_progress_params: Default::default(),
    };
    let add_items = prepare_call_hierarchy(&state, prepare_add).expect("prepare add");
    let add_item = add_items.first().expect("add item").clone();

    let incoming_params = tower_lsp::lsp_types::CallHierarchyIncomingCallsParams {
        item: add_item,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let incoming = incoming_calls(&state, incoming_params).expect("incoming calls");
    assert!(incoming.iter().any(|call| call.from.name.contains("Main")));
}

#[test]
fn lsp_call_hierarchy_cross_file_incoming_named_args() {
    let add_source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION
"#;
    let main_source = r#"
PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(A := 1, B := 2);
END_PROGRAM
"#;
    let state = ServerState::new();
    let add_uri = tower_lsp::lsp_types::Url::parse("file:///add.st").unwrap();
    let main_uri = tower_lsp::lsp_types::Url::parse("file:///main.st").unwrap();
    state.open_document(add_uri.clone(), 1, add_source.to_string());
    state.open_document(main_uri.clone(), 1, main_source.to_string());

    let prepare_add = tower_lsp::lsp_types::CallHierarchyPrepareParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: add_uri },
            position: position_at(add_source, "Add : INT"),
        },
        work_done_progress_params: Default::default(),
    };
    let add_items = prepare_call_hierarchy(&state, prepare_add).expect("prepare add");
    let add_item = add_items.first().expect("add item").clone();

    let incoming_params = tower_lsp::lsp_types::CallHierarchyIncomingCallsParams {
        item: add_item,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let incoming = incoming_calls(&state, incoming_params).expect("incoming calls");
    assert!(incoming.iter().any(|call| call.from.name.contains("Main")));
}

#[test]
fn lsp_type_hierarchy_super_and_subtypes() {
    let source = r#"
CLASS Base
END_CLASS

CLASS Derived EXTENDS Base
END_CLASS
"#;
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let prepare_derived = tower_lsp::lsp_types::TypeHierarchyPrepareParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "Derived"),
        },
        work_done_progress_params: Default::default(),
    };
    let derived_items = prepare_type_hierarchy(&state, prepare_derived).expect("prepare derived");
    let derived_item = derived_items.first().expect("derived item").clone();

    let super_params = tower_lsp::lsp_types::TypeHierarchySupertypesParams {
        item: derived_item,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let supertypes = type_hierarchy_supertypes(&state, super_params).expect("supertypes");
    assert!(supertypes.iter().any(|item| item.name.contains("Base")));

    let prepare_base = tower_lsp::lsp_types::TypeHierarchyPrepareParams {
        text_document_position_params: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position: position_at(source, "Base"),
        },
        work_done_progress_params: Default::default(),
    };
    let base_items = prepare_type_hierarchy(&state, prepare_base).expect("prepare base");
    let base_item = base_items.first().expect("base item").clone();

    let sub_params = tower_lsp::lsp_types::TypeHierarchySubtypesParams {
        item: base_item,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let subtypes = type_hierarchy_subtypes(&state, sub_params).expect("subtypes");
    assert!(subtypes.iter().any(|item| item.name.contains("Derived")));
}

#[test]
fn lsp_range_formatting_formats_selection() {
    let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nx:=1+2;\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "x:=1+2");
    let end = super::lsp_utils::offset_to_position(
        source,
        (source.find("x:=1+2").unwrap() + "x:=1+2;".len()) as u32,
    );

    let params = tower_lsp::lsp_types::DocumentRangeFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range { start, end },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = range_formatting(&state, params).expect("range formatting");
    assert_eq!(edits.len(), 1);
    assert!(edits[0].new_text.contains("x := 1 + 2;"));
}

#[test]
fn lsp_range_formatting_expands_to_syntax_block() {
    let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nIF x=1 THEN\ny:=1;\nEND_IF\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///range-block.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "y:=1");
    let end = super::lsp_utils::offset_to_position(
        source,
        (source.find("y:=1").unwrap() + "y:=1;".len()) as u32,
    );

    let params = tower_lsp::lsp_types::DocumentRangeFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range { start, end },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = range_formatting(&state, params).expect("range formatting");
    assert_eq!(edits.len(), 1);
    let edit = &edits[0];
    let if_pos = position_at(source, "IF x=1 THEN");
    let end_if_pos = position_at(source, "END_IF");
    assert_eq!(edit.range.start.line, if_pos.line);
    assert_eq!(edit.range.end.line, end_if_pos.line + 1);
    assert!(edit.new_text.contains("IF x = 1 THEN"));
    assert!(edit.new_text.contains("END_IF"));
}

#[test]
fn lsp_range_formatting_aligns_assignment_groups() {
    let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nshort:=1;\nlonger :=2;\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///range-align.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let start = position_at(source, "short:=1");
    let end = super::lsp_utils::offset_to_position(
        source,
        (source.find("longer :=2").unwrap() + "longer :=2;".len()) as u32,
    );

    let params = tower_lsp::lsp_types::DocumentRangeFormattingParams {
        text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        range: tower_lsp::lsp_types::Range { start, end },
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
        work_done_progress_params: Default::default(),
    };

    let edits = range_formatting(&state, params).expect("range formatting");
    assert_eq!(edits.len(), 1);
    let lines: Vec<&str> = edits[0].new_text.lines().collect();
    let short_line = lines.iter().find(|line| line.contains("short")).unwrap();
    let longer_line = lines.iter().find(|line| line.contains("longer")).unwrap();
    assert_eq!(
        short_line.find(":=").unwrap(),
        longer_line.find(":=").unwrap()
    );
}

#[test]
fn lsp_on_type_formatting_formats_line() {
    let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nx:=1+2;\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = tower_lsp::lsp_types::Url::parse("file:///test.st").unwrap();
    state.open_document(uri.clone(), 1, source.to_string());

    let position = position_at(source, "x:=1+2");
    let params = tower_lsp::lsp_types::DocumentOnTypeFormattingParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier { uri: uri.clone() },
            position,
        },
        ch: ";".to_string(),
        options: tower_lsp::lsp_types::FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            ..Default::default()
        },
    };

    let edits = on_type_formatting(&state, params).expect("on type formatting");
    assert_eq!(edits.len(), 1);
    assert!(edits[0].new_text.contains("x := 1 + 2;"));
}
