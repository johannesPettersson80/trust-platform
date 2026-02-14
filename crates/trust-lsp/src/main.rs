//! `trust-lsp` - Language Server Protocol implementation for IEC 61131-3 Structured Text.
//!
//! This is the main entry point for the ST language server.

mod config;
mod external_diagnostics;
mod handlers;
mod index_cache;
mod library_docs;
mod library_graph;
#[cfg(test)]
mod perf;
mod state;
mod telemetry;
#[cfg(test)]
mod test_support;

use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;

use crate::handlers::{
    HMI_BINDINGS_COMMAND, HMI_INIT_COMMAND, MOVE_NAMESPACE_COMMAND, PROJECT_INFO_COMMAND,
};
use crate::state::ServerState;
use crate::telemetry::TelemetryEvent;

/// The main language server struct.
pub struct StLanguageServer {
    /// LSP client for sending notifications.
    client: Client,
    /// Server state.
    state: Arc<ServerState>,
}

impl StLanguageServer {
    /// Creates a new language server instance.
    fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(ServerState::new()),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for StLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!("ST Language Server initializing");

        let progress_supported = params
            .capabilities
            .window
            .as_ref()
            .and_then(|window| window.work_done_progress)
            .unwrap_or(false);
        self.state.set_work_done_progress(progress_supported);
        let diagnostic_refresh_supported = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.diagnostic.as_ref())
            .and_then(|diagnostic| diagnostic.refresh_support)
            .unwrap_or(false);
        let diagnostic_pull_supported = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.diagnostic.as_ref())
            .is_some();
        let use_pull_diagnostics = diagnostic_pull_supported && diagnostic_refresh_supported;
        self.state
            .set_diagnostic_refresh_supported(diagnostic_refresh_supported);
        self.state
            .set_diagnostic_pull_supported(use_pull_diagnostics);
        let semantic_tokens_refresh_supported = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.semantic_tokens.as_ref())
            .and_then(|semantic_tokens| semantic_tokens.refresh_support)
            .unwrap_or(false);
        self.state
            .set_semantic_tokens_refresh_supported(semantic_tokens_refresh_supported);

        let mut workspace_folders = Vec::new();
        if let Some(folders) = params.workspace_folders {
            workspace_folders.extend(folders.into_iter().map(|folder| folder.uri));
        } else if let Some(root_uri) = params.root_uri {
            workspace_folders.push(root_uri);
        }

        if !workspace_folders.is_empty() {
            info!("Workspace folders: {:?}", workspace_folders);
            self.state.set_workspace_folders(workspace_folders);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Text document sync - incremental updates
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),

                // Hover support
                hover_provider: Some(HoverProviderCapability::Simple(true)),

                // Completion support
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        ":".to_string(),
                        "#".to_string(),
                    ]),
                    resolve_provider: Some(true),
                    ..Default::default()
                }),

                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        MOVE_NAMESPACE_COMMAND.to_string(),
                        PROJECT_INFO_COMMAND.to_string(),
                        HMI_INIT_COMMAND.to_string(),
                        HMI_BINDINGS_COMMAND.to_string(),
                    ],
                    ..Default::default()
                }),

                // Signature help
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    ..Default::default()
                }),

                // Go to definition
                definition_provider: Some(OneOf::Left(true)),

                // Go to declaration
                declaration_provider: Some(DeclarationCapability::Simple(true)),

                // Go to type definition
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),

                // Go to implementation
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),

                // Find references
                references_provider: Some(OneOf::Left(true)),

                // Document highlight
                document_highlight_provider: Some(OneOf::Left(true)),

                // Document symbols (outline)
                document_symbol_provider: Some(OneOf::Left(true)),

                // Workspace symbols
                workspace_symbol_provider: Some(OneOf::Left(true)),

                // Code actions
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),

                // Pull diagnostics (only when refresh is supported)
                diagnostic_provider: use_pull_diagnostics.then_some(
                    DiagnosticServerCapabilities::Options(DiagnosticOptions {
                        identifier: Some("trust-lsp".to_string()),
                        inter_file_dependencies: true,
                        workspace_diagnostics: true,
                        work_done_progress_options: Default::default(),
                    }),
                ),

                // Code lens
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),

                // Call hierarchy
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),

                // Rename
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),

                // Semantic tokens
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::KEYWORD,
                                    SemanticTokenType::TYPE,
                                    SemanticTokenType::VARIABLE,
                                    SemanticTokenType::PROPERTY,
                                    SemanticTokenType::METHOD,
                                    SemanticTokenType::FUNCTION,
                                    SemanticTokenType::PARAMETER,
                                    SemanticTokenType::NUMBER,
                                    SemanticTokenType::STRING,
                                    SemanticTokenType::COMMENT,
                                    SemanticTokenType::OPERATOR,
                                    SemanticTokenType::ENUM_MEMBER,
                                    SemanticTokenType::NAMESPACE,
                                ],
                                token_modifiers: vec![
                                    SemanticTokenModifier::DECLARATION,
                                    SemanticTokenModifier::DEFINITION,
                                    SemanticTokenModifier::READONLY,
                                    SemanticTokenModifier::STATIC,
                                    SemanticTokenModifier::MODIFICATION,
                                ],
                            },
                            full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                            range: Some(true),
                            ..Default::default()
                        },
                    ),
                ),

                // Document formatting
                document_formatting_provider: Some(OneOf::Left(true)),

                // Range formatting
                document_range_formatting_provider: Some(OneOf::Left(true)),

                // On-type formatting
                document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                    first_trigger_character: ";".to_string(),
                    more_trigger_character: Some(vec!["\n".to_string()]),
                }),

                // Folding ranges
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),

                // Selection ranges
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),

                // Linked editing ranges
                linked_editing_range_provider: Some(LinkedEditingRangeServerCapabilities::Simple(
                    true,
                )),

                // Document links
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: Default::default(),
                }),

                // Inlay hints
                inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                    InlayHintOptions {
                        resolve_provider: Some(false),
                        work_done_progress_options: Default::default(),
                    },
                ))),

                inline_value_provider: Some(OneOf::Right(InlineValueServerCapabilities::Options(
                    InlineValueOptions {
                        work_done_progress_options: Default::default(),
                    },
                ))),

                // Workspace file operations
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: None,
                    file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                        will_rename: Some(FileOperationRegistrationOptions {
                            filters: vec![
                                FileOperationFilter {
                                    scheme: Some("file".to_string()),
                                    pattern: FileOperationPattern {
                                        glob: "**/*.st".to_string(),
                                        matches: Some(FileOperationPatternKind::File),
                                        options: Some(FileOperationPatternOptions {
                                            ignore_case: Some(true),
                                        }),
                                    },
                                },
                                FileOperationFilter {
                                    scheme: Some("file".to_string()),
                                    pattern: FileOperationPattern {
                                        glob: "**/*.pou".to_string(),
                                        matches: Some(FileOperationPatternKind::File),
                                        options: Some(FileOperationPatternOptions {
                                            ignore_case: Some(true),
                                        }),
                                    },
                                },
                            ],
                        }),
                        did_rename: Some(FileOperationRegistrationOptions {
                            filters: vec![
                                FileOperationFilter {
                                    scheme: Some("file".to_string()),
                                    pattern: FileOperationPattern {
                                        glob: "**/*.st".to_string(),
                                        matches: Some(FileOperationPatternKind::File),
                                        options: Some(FileOperationPatternOptions {
                                            ignore_case: Some(true),
                                        }),
                                    },
                                },
                                FileOperationFilter {
                                    scheme: Some("file".to_string()),
                                    pattern: FileOperationPattern {
                                        glob: "**/*.pou".to_string(),
                                        matches: Some(FileOperationPatternKind::File),
                                        options: Some(FileOperationPatternOptions {
                                            ignore_case: Some(true),
                                        }),
                                    },
                                },
                            ],
                        }),
                        ..Default::default()
                    }),
                }),

                // Type hierarchy (keep experimental for clients that still read it)
                experimental: Some(json!({ "typeHierarchyProvider": true })),

                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "trust-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        info!("ST Language Server initialized");
        self.client
            .log_message(MessageType::INFO, "ST Language Server initialized!")
            .await;
        handlers::register_file_watchers(&self.client).await;
        handlers::register_type_hierarchy(&self.client).await;
        handlers::index_workspace_background_with_refresh(
            self.client.clone(),
            Arc::clone(&self.state),
        );
    }

    async fn shutdown(&self) -> Result<()> {
        info!("ST Language Server shutting down");
        self.state.flush_telemetry();
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        handlers::did_change_configuration(&self.state, params);
        handlers::refresh_diagnostics(&self.client, &self.state).await;
        handlers::refresh_semantic_tokens(&self.client, &self.state).await;
    }

    // =========================================================================
    // Document Synchronization
    // =========================================================================

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        handlers::did_open(&self.client, &self.state, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        handlers::did_change(&self.client, &self.state, params).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        handlers::did_save(&self.client, &self.state, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        handlers::did_close(&self.client, &self.state, params).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        handlers::did_change_watched_files(&self.client, &self.state, params).await;
    }

    async fn did_rename_files(&self, params: RenameFilesParams) {
        handlers::did_rename_files(&self.client, &self.state, params).await;
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::document_diagnostic(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Diagnostic, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn workspace_diagnostic(
        &self,
        params: WorkspaceDiagnosticParams,
    ) -> Result<WorkspaceDiagnosticReportResult> {
        let start = Instant::now();
        let result = self
            .state
            .run_background(async { handlers::workspace_diagnostic(&self.state, params) })
            .await;
        self.state
            .record_telemetry(TelemetryEvent::WorkspaceDiagnostic, start.elapsed(), None);
        Ok(result)
    }

    async fn will_rename_files(&self, params: RenameFilesParams) -> Result<Option<WorkspaceEdit>> {
        Ok(handlers::will_rename_files(&self.state, params))
    }

    // =========================================================================
    // Language Features
    // =========================================================================

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let start = Instant::now();
        let result = handlers::hover(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Hover, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::completion(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Completion, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        Ok(handlers::completion_resolve(&self.state, item))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let start = Instant::now();
        let result = handlers::signature_help(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::SignatureHelp, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let start = Instant::now();
        let result = handlers::goto_definition(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Definition, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let start = Instant::now();
        let result = handlers::goto_declaration(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Declaration, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn goto_type_definition(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> Result<Option<GotoTypeDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let start = Instant::now();
        let result = handlers::goto_type_definition(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::TypeDefinition, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let start = Instant::now();
        let result = handlers::goto_implementation(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Implementation, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let start = Instant::now();
        let result = self
            .state
            .run_background(handlers::references_with_progress(
                &self.client,
                &self.state,
                params,
            ))
            .await;
        self.state
            .record_telemetry(TelemetryEvent::References, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        Ok(handlers::document_highlight(&self.state, params))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::document_symbol(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::DocumentSymbol, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let start = Instant::now();
        let result = self
            .state
            .run_background(handlers::workspace_symbol_with_progress(
                &self.client,
                &self.state,
                params,
            ))
            .await;
        self.state
            .record_telemetry(TelemetryEvent::WorkspaceSymbol, start.elapsed(), None);
        Ok(result)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::code_action(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::CodeAction, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        Ok(handlers::execute_command(&self.client, &self.state, params).await)
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        Ok(handlers::code_lens(&self.state, params))
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        Ok(handlers::prepare_call_hierarchy(&self.state, params))
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        Ok(handlers::incoming_calls(&self.state, params))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        Ok(handlers::outgoing_calls(&self.state, params))
    }

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        Ok(handlers::prepare_type_hierarchy(&self.state, params))
    }

    async fn supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        Ok(handlers::type_hierarchy_supertypes(&self.state, params))
    }

    async fn subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        Ok(handlers::type_hierarchy_subtypes(&self.state, params))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::rename(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Rename, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::prepare_rename(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::PrepareRename, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::semantic_tokens_full(&self.state, params);
        self.state.record_telemetry(
            TelemetryEvent::SemanticTokensFull,
            start.elapsed(),
            Some(&uri),
        );
        Ok(result)
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::semantic_tokens_full_delta(&self.state, params);
        self.state.record_telemetry(
            TelemetryEvent::SemanticTokensDelta,
            start.elapsed(),
            Some(&uri),
        );
        Ok(result)
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        Ok(handlers::semantic_tokens_range(&self.state, params))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        Ok(handlers::folding_range(&self.state, params))
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        Ok(handlers::selection_range(&self.state, params))
    }

    async fn linked_editing_range(
        &self,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        Ok(handlers::linked_editing_range(&self.state, params))
    }

    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        Ok(handlers::document_link(&self.state, params))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        Ok(handlers::inlay_hint(&self.state, params))
    }

    async fn inline_value(&self, params: InlineValueParams) -> Result<Option<Vec<InlineValue>>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::inline_value(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::InlineValue, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::range_formatting(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::RangeFormatting, start.elapsed(), Some(&uri));
        Ok(result)
    }

    async fn on_type_formatting(
        &self,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(handlers::on_type_formatting(&self.state, params))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri.clone();
        let start = Instant::now();
        let result = handlers::formatting(&self.state, params);
        self.state
            .record_telemetry(TelemetryEvent::Formatting, start.elapsed(), Some(&uri));
        Ok(result)
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting ST Language Server");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(StLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
