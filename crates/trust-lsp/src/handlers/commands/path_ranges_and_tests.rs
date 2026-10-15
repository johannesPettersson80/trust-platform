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
            operations[1].iter().all(|operation| matches!(
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

    /// Specification 39: the legacy `trust-lsp.hmiInit` command is removed (decision 12); the HMI
    /// Builder's file commands are offered (§17.3).
    #[test]
    fn advertised_commands_are_exactly_the_supported_set() {
        assert_eq!(
            EXECUTE_COMMANDS,
            &[
                "trust-lsp.moveNamespace",
                "trust-lsp.projectInfo",
                "trust-lsp.hmiBindings",
                "trust-lsp.hmiFileParse",
                "trust-lsp.hmiFileFormat",
                "trust-lsp.hmiFileCheck",
                "trust-lsp.hmiCatalogue",
                "trust-lsp.hmiBundle",
                "trust-lsp.hmiTexts",
                "trust-lsp.hmiFaceplates",
                "trust-lsp.hmiFaceplateInstances",
                "trust-lsp.hmiPlacedFaceplates",
                "trust-lsp.hmiAddMissingVariables",
                "trust-lsp.hmiEquipment",
                "trust-lsp.hmiOwnEquipment",
                "trust-lsp.hmiScreensProgram",
                "trust-lsp.hmiProposePage",
                "trust-lsp.hmiProposeSettingsPage",
                "trust-lsp.hmiRenameInstance",
                "trust-lsp.controlKeepParameters",
                "trust-lsp.hmiAgent.context",
                "trust-lsp.hmiAgent.library",
                "trust-lsp.hmiAgent.inspect",
                "trust-lsp.hmiAgent.compose",
                "trust-lsp.hmiAgent.edit",
                "trust-lsp.hmiAgent.validate",
                "trust-lsp.hmiAgent.drafts",
                "trust-lsp.hmiAgent.review",
                "trust-lsp.hmiAgent.apply",
                "trust-lsp.hmiAgent.discard",
                "trust-lsp.hmiAgent.checkInput",
            ]
        );
    }

    /// The language client registers every command the server advertises as a VS Code command, so
    /// one that shares a name with a command of the extension stops the client from starting
    /// (specification 35, §0.1a).
    #[test]
    fn advertised_commands_never_share_a_name_with_an_extension_command() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../../../../../editors/vscode/package.json"))
            .expect("the extension's package.json");
        let contributed = manifest["contributes"]["commands"]
            .as_array()
            .expect("the extension contributes commands")
            .iter()
            .filter_map(|command| command["command"].as_str());
        let activating = manifest["activationEvents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|event| event.as_str()?.strip_prefix("onCommand:"));
        let shared: Vec<&str> = contributed.chain(activating).filter(|command| EXECUTE_COMMANDS.contains(command)).collect();
        assert!(shared.is_empty(), "server commands that the extension also defines: {shared:?}");
    }

    /// Specification 39, §7.1: the picker names a configured program instance by the type its
    /// configuration declares, not by the instance name compiled storage gives it.
    #[test]
    fn hmi_bindings_variable_tree_names_a_configured_program_by_its_type() {
        let root = temp_dir("trustlsp-hmi-program-type");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        let source = r#"
PROGRAM Main
VAR
    Level : REAL;
END_VAR
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;
        std::fs::write(src_dir.join("main.st"), source).expect("write source");
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
        let p1 = result["variables"]
            .as_array()
            .and_then(|roots| roots.iter().find(|node| node["name"] == "P1"))
            .unwrap_or_else(|| panic!("P1 in the variable tree: {result}"));
        assert_eq!(
            (p1["kind"].as_str(), p1["type"].as_str()),
            (Some("program"), Some("Main")),
            "{p1}"
        );
        assert_eq!(p1["children"][0]["path"], "P1.Level", "{p1}");
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

        // Control Studio and Network Canvas read these two parts.
        let process_image = result.get("processImage").expect("processImage");
        for part in ["commands", "feedbacks", "memory"] {
            assert!(
                process_image.get(part).and_then(Value::as_array).is_some(),
                "processImage.{part}: {result}"
            );
        }
        assert!(result.get("controlSymbols").is_some(), "{result}");
        // Specification 39, decision 12: the legacy HMI binding catalogue is removed.
        assert!(result.get("programs").is_none(), "{result}");
        assert!(result.get("globals").is_none(), "{result}");
        // Specification 39, §7.1: the variable tree of the compiled program for the picker.
        let station = result["variables"]
            .as_array()
            .and_then(|roots| roots.iter().find(|root| root["name"] == "PumpStation"))
            .unwrap_or_else(|| panic!("PumpStation in the variable tree: {result}"));
        assert_eq!(station["kind"], "program");
        let members = station["children"]
            .as_array()
            .expect("program members")
            .iter()
            .map(|member| {
                (
                    member["name"].as_str().unwrap_or_default(),
                    member["type"].as_str().unwrap_or_default(),
                    member["path"].as_str().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            members,
            vec![
                ("speed_setpoint", "REAL", "PumpStation.speed_setpoint"),
                ("speed", "REAL", "PumpStation.speed"),
                ("mode", "MODE", "PumpStation.mode"),
                ("internal_counter", "DINT", "PumpStation.internal_counter"),
            ]
        );

        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    /// Specification 39, §7.6: the editor's binding checks, each problem on its line.
    #[test]
    fn hmi_file_check_reports_problems_on_their_lines() {
        let root = temp_dir("trustlsp-hmi-check");
        std::fs::create_dir_all(root.join("src")).expect("create src dir");
        std::fs::write(
            root.join("src/main.st"),
            "PROGRAM Main\nVAR\n    Level : REAL;\n    Run : BOOL;\nEND_VAR\nEND_PROGRAM\n",
        )
        .expect("write source");
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };
        let text = "format = 1\ntitle = \"Checks\"\n\n[[object]]\nid = \"level\"\ntype = \"bar\"\nat = [0, 0]\nsize = [80, 40]\n\n[object.bind]\nvalue = \"Main.Run\"\n\n[[object]]\nid = \"dial\"\ntype = \"dial\"\nat = [0, 60]\nsize = [80, 40]\n";

        let result = hmi_checks::check_value_with_context(
            &context,
            vec![json!({ "text": text, "file": "hmi/pages/checks.toml" })],
        );
        assert_eq!(result["ok"], true, "{result}");
        let problems = result["problems"].as_array().expect("problems");
        assert_eq!(problems.len(), 2, "{result}");
        assert_eq!(
            (
                &problems[0]["object"],
                &problems[0]["property"],
                &problems[0]["line"]
            ),
            (&json!("level"), &json!("value"), &json!(11))
        );
        assert!(problems[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Main.Run")));
        assert_eq!(
            (&problems[1]["object"], &problems[1]["line"]),
            (&json!("dial"), &json!(15))
        );

        let broken = hmi_checks::check_value_with_context(
            &context,
            vec![json!({ "text": "format = 1\ntitle = 3\n" })],
        );
        assert_eq!(broken["ok"], false, "{broken}");
        assert_eq!(broken["errors"][0]["line"], 2);

        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    include!("hmi_check_tests.rs");
    include!("hmi_placed_tests.rs");
    include!("hmi_symbol_tests.rs");
    include!("hmi_program_edit_tests.rs");
    include!("hmi_page_proposal_tests.rs");

    include!("hmi_rename_tests.rs");
    include!("control_evidence_tests.rs");

    #[test]
    fn hmi_bindings_excludes_runtime_ignored_lesson_tests() {
        for lesson in [
            "01-cold-storage-loop-anatomy",
            "02-cold-storage-on-off",
            "03-cold-storage-hysteresis-dwell",
        ] {
            let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/control/01-feedback-foundations-and-temperature-control")
                .join(lesson);
            let root = temp_dir("trustlsp-lesson-bindings");
            let src = root.join("src");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::copy(project.join("src/Main.st"), src.join("Main.st")).unwrap();
            std::fs::copy(project.join("src/tests.st"), src.join("tests.st")).unwrap();
            let root_uri = Url::from_directory_path(&root).unwrap();
            let context = MockContext {
                workspace_configs: vec![(
                    root_uri.clone(),
                    test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
                )],
                workspace_folders: vec![root_uri],
                ..MockContext::default()
            };
            let result = hmi_bindings_value_with_context(&context, Vec::new()).unwrap();
            assert_eq!(
                result.get("ok").and_then(Value::as_bool),
                Some(true),
                "actual lesson bindings must compile without test programs: {result}"
            );
            let text = result.to_string();
            for name in [
                "ProcessValue",
                "ManipulatedValue",
                "EffectiveSurroundings",
                "DoorOpenIndication",
            ] {
                assert!(
                    text.contains(name),
                    "missing configured signal {name}: {result}"
                );
            }
            assert!(!text.contains("Lesson1ManualDemandIsDirectAndBounded"));
            // Removing the opt-out must expose the unbound program error again.
            std::fs::write(src.join("tests.st"), "PROGRAM UnboundTest END_PROGRAM").unwrap();
            let invalid = hmi_bindings_value_with_context(&context, Vec::new()).unwrap();
            assert_eq!(invalid.get("ok").and_then(Value::as_bool), Some(false));
            std::fs::remove_dir_all(root).unwrap();
        }
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

    #[test]
    fn hmi_bindings_command_compiles_the_project_with_its_library_dependencies() {
        let root = temp_dir("trustlsp-library-dependency");
        let library = root.join("libraries").join("control");
        std::fs::create_dir_all(library.join("src")).expect("create library");
        std::fs::create_dir_all(root.join("src")).expect("create src dir");
        std::fs::write(
            library.join("trust-lsp.toml"),
            "[package]\nversion = \"0.1.0\"\n\n[project]\ninclude_paths = [\"src\"]\n",
        )
        .expect("write library manifest");
        std::fs::write(
            library.join("src").join("control.st"),
            "NAMESPACE Control\nFUNCTION_BLOCK PID\nVAR Gain : LREAL; END_VAR\nEND_FUNCTION_BLOCK\nEND_NAMESPACE\n",
        )
        .expect("write library source");
        std::fs::write(
            root.join("trust-lsp.toml"),
            "[project]\ninclude_paths = [\"src\"]\n\n[dependencies]\nControl = { path = \"libraries/control\", version = \"0.1.0\" }\n",
        )
        .expect("write project manifest");
        std::fs::write(
            root.join("src").join("main.st"),
            "PROGRAM Main\nVAR Loop : Control.PID; Output : LREAL; END_VAR\nEND_PROGRAM\n\nCONFIGURATION Config\nRESOURCE Res ON PLC\n    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nVAR_CONFIG\n    P1.Output AT %QL0 : LREAL;\nEND_VAR\nEND_CONFIGURATION\n",
        )
        .expect("write source");
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };
        let result = hmi_bindings_value_with_context(&context, Vec::new())
            .expect("compiled bindings response");
        assert_eq!(
            result.get("ok").and_then(Value::as_bool),
            Some(true),
            "a declared library dependency is part of the compiled project: {result}"
        );
        let controllers = result["controlSymbols"]["controllers"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            controllers
                .iter()
                .any(|entry| entry["instance"] == "P1.Loop" && entry["type"] == "Control.PID"),
            "the library's controller type is resolved: {controllers:?}"
        );
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_bindings_command_reports_the_compiled_links_between_cascaded_controllers() {
        let root = temp_dir("trustlsp-controller-links");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        std::fs::write(
            src_dir.join("main.st"),
            r#"
NAMESPACE Control
TYPE PIDInput :
STRUCT
    Setpoint : LREAL;
    ProcessValue : LREAL;
END_STRUCT
END_TYPE
FUNCTION_BLOCK PID
VAR Last : LREAL; END_VAR
METHOD PUBLIC Update : LREAL
VAR_INPUT Input : PIDInput; END_VAR
Update := Input.Setpoint - Input.ProcessValue;
END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE

PROGRAM Main
VAR
    Outer : Control.PID;
    Inner : Control.PID;
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    VesselSetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
    JacketSetpointC : LREAL;
    HeaterPct : LREAL;
END_VAR
OuterInput.Setpoint := VesselSetpointC;
OuterInput.ProcessValue := VesselC;
JacketSetpointC := Outer.Update(Input := OuterInput);
InnerInput.Setpoint := JacketSetpointC;
InnerInput.ProcessValue := JacketC;
HeaterPct := Inner.Update(Input := InnerInput);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.HeaterPct AT %QL0 : LREAL;
    P1.VesselC AT %IL0 : LREAL;
    P1.JacketC AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
"#,
        )
        .expect("write source");
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };
        let result = hmi_bindings_value_with_context(&context, Vec::new())
            .expect("compiled bindings response");
        let catalog = &result["controlSymbols"];
        let links = catalog["links"].as_array().cloned().unwrap_or_default();
        let inner = links.iter().find(|link| link["instance"] == "P1.Inner");
        let outer = links.iter().find(|link| link["instance"] == "P1.Outer");
        assert!(
            inner.is_some_and(|link| link["fields"]["Setpoint"] == json!(["P1.JacketSetpointC"]))
                && outer.is_some_and(|link| link["output"] == "P1.JacketSetpointC"),
            "the catalog must show the outer output feeding the inner setpoint; controllers {} links {}",
            catalog["controllers"],
            catalog["links"]
        );
        assert_eq!(
            catalog["writes"],
            json!([
                "P1.HeaterPct",
                "P1.InnerInput.ProcessValue",
                "P1.InnerInput.Setpoint",
                "P1.JacketSetpointC",
                "P1.OuterInput.ProcessValue",
                "P1.OuterInput.Setpoint"
            ]),
            "the catalog lists every symbol the program assigns"
        );
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_bindings_command_reports_each_program_instance_with_its_program_type() {
        let root = temp_dir("trustlsp-program-types");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        std::fs::write(
            src_dir.join("main.st"),
            r#"
PROGRAM Main
VAR
    RoomC : LREAL;
END_VAR
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        )
        .expect("write source");
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };
        let result = hmi_bindings_value_with_context(&context, Vec::new())
            .expect("compiled bindings response");
        let controllers = result["controlSymbols"]["controllers"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            controllers
                .iter()
                .find(|entry| entry["instance"] == "P1")
                .map(|entry| entry["type"].clone()),
            Some(json!("Main")),
            "a program instance is reported with its program type, not its instance name; controllers {controllers:?}"
        );
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_bindings_command_includes_only_numeric_plant_process_image_bindings() {
        let root = temp_dir("trustlsp-plant-signals");
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        std::fs::write(
            src_dir.join("main.st"),
            r#"
FUNCTION_BLOCK CustomRegulator
VAR Alpha : REAL := 2.0; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Command : LREAL;
    Feedback : REAL;
    Enabled : BOOL;
    Scratch : DINT;
    PID : CustomRegulator;
END_VAR
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Command AT %QL0 : LREAL;
    P1.Feedback AT %ID8 : REAL;
    P1.Enabled AT %QX16.0 : BOOL;
    P1.Scratch AT %MD20 : DINT;
END_VAR
END_CONFIGURATION
"#,
        )
        .expect("write source");

        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };

        let result = hmi_bindings_value_with_context(&context, Vec::new())
            .expect("compiled bindings response");
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(true));
        assert_eq!(
            result.get("command").and_then(Value::as_str),
            Some(HMI_BINDINGS_COMMAND)
        );
        let process_image = result.get("processImage").expect("process image catalog");
        assert_eq!(
            process_image.get("commands").and_then(Value::as_array),
            Some(&vec![json!({
                "symbol": "P1.Command",
                "address": "%QL0",
                "type": "LREAL"
            })])
        );
        assert_eq!(
            process_image.get("feedbacks").and_then(Value::as_array),
            Some(&vec![json!({
                "symbol": "P1.Feedback",
                "address": "%ID8",
                "type": "REAL"
            })])
        );

        assert_eq!(
            process_image.get("memory").and_then(Value::as_array),
            Some(&vec![json!({
                "symbol": "P1.Scratch",
                "address": "%MD20",
                "type": "DINT"
            })]),
            "compiled memory variables must be selectable in Control Studio"
        );
        assert_eq!(
            process_image.get("booleans").and_then(Value::as_array),
            Some(&vec![json!({
                "symbol": "P1.Enabled",
                "address": "%QX16.0",
                "type": "BOOL"
            })]),
            "a BOOL bit is listed apart from the plant candidates, so a declared state can name it"
        );

        let symbols = result["controlSymbols"]["symbols"]
            .as_array()
            .expect("compiled control symbols must be available");
        assert!(
            symbols.iter().any(|entry| entry["symbol"] == "P1.PID.Alpha"
                && entry["type"] == "REAL"
                && entry.get("address").is_none()),
            "unaddressed controller parameters must retain their compiled symbolic path"
        );
        let controllers = result["controlSymbols"]["controllers"]
            .as_array()
            .expect("compiled controller instances");
        assert!(
            controllers.iter().any(|entry| entry["instance"] == "P1.PID"
                && entry["type"] == "CustomRegulator"
                && entry["tasks"][0]["name"] == "MainTask"
                && entry["tasks"][0]["intervalMs"] == 100.0),
            "a misleading instance name must not replace the actual compiled controller type"
        );

        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    include!("hmi_alarm_texts_tests.rs");
    include!("hmi_unsaved_sources_tests.rs");
}
