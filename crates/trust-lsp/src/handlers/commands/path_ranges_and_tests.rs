#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BuildConfig, DiagnosticSettings, IndexingConfig, ProjectConfig, RuntimeConfig,
        StdlibSettings, TelemetryConfig, WorkspaceSettings,
    };
    use crate::state::Document;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use trust_hir::db::FileId;

    #[derive(Clone, Default)]
    struct MockContext {
        workspace_configs: Vec<(Url, ProjectConfig)>,
        workspace_config_by_uri: HashMap<Url, ProjectConfig>,
        workspace_folders: Vec<Url>,
        documents_by_uri: HashMap<Url, Document>,
        documents_by_file_id: HashMap<FileId, Document>,
        rename_result: Option<RenameResult>,
    }

    impl MockContext {
        fn insert_document(&mut self, document: Document) {
            self.documents_by_file_id
                .insert(document.file_id, document.clone());
            self.documents_by_uri.insert(document.uri.clone(), document);
        }
    }

    impl ServerContext for MockContext {
        fn workspace_configs(&self) -> Vec<(Url, ProjectConfig)> {
            self.workspace_configs.clone()
        }

        fn workspace_config_for_uri(&self, uri: &Url) -> Option<ProjectConfig> {
            self.workspace_config_by_uri.get(uri).cloned()
        }

        fn workspace_folders(&self) -> Vec<Url> {
            self.workspace_folders.clone()
        }

        fn get_document(&self, uri: &Url) -> Option<Document> {
            self.documents_by_uri.get(uri).cloned()
        }

        fn document_for_file_id(&self, file_id: FileId) -> Option<Document> {
            self.documents_by_file_id.get(&file_id).cloned()
        }

        fn rename(
            &self,
            _file_id: FileId,
            _offset: TextSize,
            _new_name: &str,
        ) -> Option<RenameResult> {
            self.rename_result.clone()
        }
    }

    fn test_project_config(root: &str, target: &str) -> ProjectConfig {
        ProjectConfig {
            root: PathBuf::from(root),
            config_path: None,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig {
                target: Some(target.to_string()),
                ..BuildConfig::default()
            },
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        }
    }

    fn test_document(uri: &str, file_id: u32, content: &str) -> Document {
        Document::new(
            Url::parse(uri).expect("test uri"),
            1,
            content.to_string(),
            FileId(file_id),
            true,
            1,
        )
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn project_info_with_mock_context_uses_uri_mapping() {
        let root_a = Url::parse("file:///workspace/a/").expect("root a");
        let root_b = Url::parse("file:///workspace/b/").expect("root b");
        let config_a = test_project_config("/workspace/a", "x86_64");
        let config_b = test_project_config("/workspace/b", "armv7");
        let main_uri = Url::parse("file:///workspace/a/src/main.st").expect("main uri");

        let mut context = MockContext {
            workspace_configs: vec![
                (root_a.clone(), config_a.clone()),
                (root_b.clone(), config_b.clone()),
            ],
            ..MockContext::default()
        };
        context
            .workspace_config_by_uri
            .insert(main_uri.clone(), config_a);

        let info = project_info_value_with_context(
            &context,
            vec![json!({
                "text_document": {
                    "uri": main_uri,
                }
            })],
        )
        .expect("project info");
        let projects = info
            .get("projects")
            .and_then(Value::as_array)
            .expect("projects");
        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects[0]
                .get("build")
                .and_then(|build| build.get("target"))
                .and_then(Value::as_str),
            Some("x86_64")
        );
    }

    #[test]
    fn namespace_move_with_mock_context_generates_expected_operations() {
        let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
END_NAMESPACE
"#;
        let main = r#"
PROGRAM Main
VAR
    x : LibA.Foo;
END_VAR
END_PROGRAM
"#;
        let source_uri = Url::parse("file:///workspace/liba.st").expect("source uri");
        let main_uri = Url::parse("file:///workspace/main.st").expect("main uri");
        let target_uri = Url::parse("file:///workspace/Company/LibA.st").expect("target uri");

        let source_doc = test_document(source_uri.as_str(), 1, source);
        let main_doc = test_document(main_uri.as_str(), 2, main);

        let mut rename_result = RenameResult::new();
        let ns_start = source.find("LibA").expect("namespace name start");
        rename_result.add_edit(
            source_doc.file_id,
            IdeTextEdit {
                range: TextRange::new(
                    TextSize::from(ns_start as u32),
                    TextSize::from((ns_start + "LibA".len()) as u32),
                ),
                new_text: "Company.LibA".to_string(),
            },
        );
        let main_ref_start = main.find("LibA").expect("main namespace reference");
        rename_result.add_edit(
            main_doc.file_id,
            IdeTextEdit {
                range: TextRange::new(
                    TextSize::from(main_ref_start as u32),
                    TextSize::from((main_ref_start + "LibA".len()) as u32),
                ),
                new_text: "Company.LibA".to_string(),
            },
        );

        let mut context = MockContext {
            workspace_folders: vec![Url::parse("file:///workspace/").expect("root uri")],
            rename_result: Some(rename_result),
            ..MockContext::default()
        };
        context.insert_document(source_doc);
        context.insert_document(main_doc);

        let args = MoveNamespaceCommandArgs {
            text_document: TextDocumentIdentifier {
                uri: source_uri.clone(),
            },
            position: offset_to_position(source, source.find("LibA").expect("position") as u32),
            new_path: "Company.LibA".to_string(),
            target_uri: Some(target_uri.clone()),
            return_edit: false,
        };
        let edit = namespace_move_workspace_edit_with_context(&context, args).expect("edit");
        let ops = match edit.document_changes.expect("document changes") {
            DocumentChanges::Operations(ops) => ops,
            DocumentChanges::Edits(_) => panic!("expected operation list"),
        };

        assert!(
            ops.iter().any(|op| matches!(
                op,
                DocumentChangeOperation::Op(ResourceOp::Create(create)) if create.uri == target_uri
            )),
            "expected target file create operation"
        );
        assert!(
            ops.iter().any(|op| matches!(
                op,
                DocumentChangeOperation::Op(ResourceOp::Delete(delete)) if delete.uri == source_uri
            )),
            "expected source file delete operation"
        );

        let target_edit = ops.iter().find_map(|op| match op {
            DocumentChangeOperation::Edit(edit) if edit.text_document.uri == target_uri => {
                Some(edit)
            }
            _ => None,
        });
        let target_edit = target_edit.expect("target edit");
        let target_contains_renamed_namespace = target_edit.edits.iter().any(|edit| match edit {
            tower_lsp::lsp_types::OneOf::Left(edit) => {
                edit.new_text.contains("NAMESPACE Company.LibA")
            }
            tower_lsp::lsp_types::OneOf::Right(_) => false,
        });
        assert!(
            target_contains_renamed_namespace,
            "target insertion should include renamed namespace"
        );

        let main_edit = ops.iter().find_map(|op| match op {
            DocumentChangeOperation::Edit(edit) if edit.text_document.uri == main_uri => Some(edit),
            _ => None,
        });
        let main_edit = main_edit.expect("main edit");
        let main_updated = main_edit.edits.iter().any(|edit| match edit {
            tower_lsp::lsp_types::OneOf::Left(edit) => edit.new_text.contains("Company.LibA"),
            tower_lsp::lsp_types::OneOf::Right(_) => false,
        });
        assert!(main_updated, "main file should include renamed namespace");
    }

    #[test]
    fn namespace_move_apply_phases_keep_create_and_edits_before_delete() {
        let source_uri = Url::parse("file:///workspace/liba.st").expect("source uri");
        let target_uri = Url::parse("file:///workspace/Company/LibA.st").expect("target uri");
        let main_uri = Url::parse("file:///workspace/main.st").expect("main uri");

        let edit = WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Operations(vec![
                DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
                    uri: target_uri.clone(),
                    options: None,
                    annotation_id: None,
                })),
                DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: target_uri.clone(),
                        version: None,
                    },
                    edits: vec![tower_lsp::lsp_types::OneOf::Left(TextEdit {
                        range: Range {
                            start: Position::new(0, 0),
                            end: Position::new(0, 0),
                        },
                        new_text: "NAMESPACE Company.LibA\nEND_NAMESPACE\n".to_string(),
                    })],
                }),
                DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: main_uri.clone(),
                        version: None,
                    },
                    edits: vec![tower_lsp::lsp_types::OneOf::Left(TextEdit {
                        range: Range {
                            start: Position::new(1, 10),
                            end: Position::new(1, 14),
                        },
                        new_text: "Company.LibA".to_string(),
                    })],
                }),
                DocumentChangeOperation::Op(ResourceOp::Delete(DeleteFile {
                    uri: source_uri,
                    options: Some(DeleteFileOptions {
                        recursive: Some(false),
                        ignore_if_not_exists: Some(true),
                        annotation_id: None,
                    }),
                })),
            ])),
            change_annotations: None,
        };

        let phases = split_workspace_edit_for_safe_apply(edit);
        assert_eq!(phases.len(), 2, "expected apply/cleanup phases");

        let operations = phases
            .iter()
            .map(
                |phase| match phase.document_changes.as_ref().expect("document changes") {
                    DocumentChanges::Operations(operations) => operations,
                    DocumentChanges::Edits(_) => panic!("expected operation phases"),
                },
            )
            .collect::<Vec<_>>();

        assert!(
            operations[0].iter().any(|operation| {
                matches!(
                    operation,
                    DocumentChangeOperation::Op(ResourceOp::Create(_))
                )
            }),
            "first phase should create the target file"
        );
        assert!(
            operations[1]
                .iter()
                .all(|operation| matches!(
                    operation,
                    DocumentChangeOperation::Op(ResourceOp::Delete(_))
                )),
            "second phase should only clean up source files"
        );
        assert!(
            operations[0]
                .iter()
                .any(|operation| matches!(operation, DocumentChangeOperation::Edit(edit) if edit.text_document.uri == main_uri)),
            "first phase should include reference text edits before cleanup"
        );
        assert!(
            operations[0]
                .iter()
                .any(|operation| matches!(operation, DocumentChangeOperation::Edit(edit) if edit.text_document.uri == target_uri)),
            "first phase should insert namespace text into the target file before cleanup"
        );
        assert!(
            !operations[0].iter().any(|operation| {
                matches!(
                    operation,
                    DocumentChangeOperation::Op(ResourceOp::Delete(_))
                )
            }),
            "source cleanup must not happen with create/text edits"
        );
    }

    #[test]
    fn project_info_server_state_and_context_paths_match() {
        let state = ServerState::new();
        let root = Url::parse("file:///workspace/").expect("root");
        state.set_workspace_folders(vec![root.clone()]);
        state.set_workspace_config(root, test_project_config("/workspace", "x86_64"));

        let from_wrapper = project_info_value(&state, Vec::new()).expect("wrapper value");
        let from_context =
            project_info_value_with_context(&state, Vec::new()).expect("context value");
        assert_eq!(from_wrapper, from_context);
    }

    #[test]
    fn hmi_init_command_with_mock_context_generates_scaffold() {
        let root = temp_dir("trustlsp-hmi-init");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        let source_path = src_dir.join("pump.st");
        let source = r#"
PROGRAM PumpStation
VAR_INPUT
    speed_setpoint : REAL;
END_VAR
VAR_OUTPUT
    speed : REAL;
    running : BOOL;
END_VAR
END_PROGRAM
"#;
        std::fs::write(&source_path, source).expect("write source");

        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };

        let result = hmi_init_value_with_context(&context, vec![json!({ "style": "mint" })])
            .expect("hmi init response");
        assert_eq!(
            result.get("ok").and_then(Value::as_bool),
            Some(true),
            "unexpected hmi bindings response: {result}",
        );
        assert_eq!(result.get("style").and_then(Value::as_str), Some("mint"));
        assert!(root.join("hmi").join("_config.toml").is_file());
        assert!(root.join("hmi").join("overview.toml").is_file());

        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_init_command_rejects_invalid_style() {
        let context = MockContext::default();
        let result = hmi_init_value_with_context(&context, vec![json!({ "style": "retro" })])
            .expect("hmi init response");
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(false));
        let error = result.get("error").and_then(Value::as_str).unwrap_or("");
        assert!(error.contains("invalid style"));
    }

    #[test]
    fn hmi_bindings_command_with_mock_context_returns_external_contract_catalog() {
        let root = temp_dir("trustlsp-hmi-bindings");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        let source_path = src_dir.join("pump.st");
        let source = r#"
TYPE MODE : (OFF, AUTO); END_TYPE

PROGRAM PumpStation
VAR_INPUT
    speed_setpoint : REAL;
END_VAR
VAR_OUTPUT
    speed : REAL;
    mode : MODE := MODE#AUTO;
END_VAR
VAR
    internal_counter : DINT;
END_VAR
END_PROGRAM
"#;
        std::fs::write(&source_path, source).expect("write source");

        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };

        let result =
            hmi_bindings_value_with_context(&context, Vec::new()).expect("hmi bindings response");
        assert_eq!(
            result.get("ok").and_then(Value::as_bool),
            Some(true),
            "unexpected hmi bindings response: {result}",
        );
        assert_eq!(
            result.get("command").and_then(Value::as_str),
            Some(HMI_BINDINGS_COMMAND)
        );

        let programs = result
            .get("programs")
            .and_then(Value::as_array)
            .expect("programs");
        let pump = programs
            .iter()
            .find(|entry| entry.get("name").and_then(Value::as_str) == Some("PumpStation"))
            .expect("PumpStation program");
        let variables = pump
            .get("variables")
            .and_then(Value::as_array)
            .expect("program variables");

        assert!(variables.iter().any(|variable| {
            variable.get("name").and_then(Value::as_str) == Some("speed_setpoint")
                && variable.get("path").and_then(Value::as_str)
                    == Some("PumpStation.speed_setpoint")
                && variable.get("type").and_then(Value::as_str) == Some("REAL")
                && variable.get("qualifier").and_then(Value::as_str) == Some("VAR_INPUT")
                && variable.get("writable").and_then(Value::as_bool) == Some(true)
        }));
        assert!(variables.iter().any(|variable| {
            variable.get("name").and_then(Value::as_str) == Some("speed")
                && variable.get("path").and_then(Value::as_str) == Some("PumpStation.speed")
                && variable.get("qualifier").and_then(Value::as_str) == Some("VAR_OUTPUT")
                && variable.get("writable").and_then(Value::as_bool) == Some(false)
        }));
        assert!(variables.iter().any(|variable| {
            variable.get("name").and_then(Value::as_str) == Some("mode")
                && variable.get("type").and_then(Value::as_str) == Some("MODE")
                && variable
                    .get("enum_values")
                    .and_then(Value::as_array)
                    .is_some_and(|values| {
                        values.iter().any(|value| value.as_str() == Some("OFF"))
                            && values.iter().any(|value| value.as_str() == Some("AUTO"))
                    })
        }));
        assert!(!variables.iter().any(|variable| {
            variable.get("name").and_then(Value::as_str) == Some("internal_counter")
        }));
        assert!(pump
            .get("file")
            .and_then(Value::as_str)
            .is_some_and(|path| path.ends_with("pump.st")));

        assert!(result.get("globals").and_then(Value::as_array).is_some());

        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_bindings_command_rejects_invalid_argument_shape() {
        let context = MockContext::default();
        let result = hmi_bindings_value_with_context(
            &context,
            vec![
                json!({ "root_uri": "file:///tmp" }),
                json!({ "unexpected": true }),
            ],
        )
        .expect("hmi bindings response");
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(false));
        let error = result.get("error").and_then(Value::as_str).unwrap_or("");
        assert!(error.contains("expects zero or one argument object"));
    }
}
