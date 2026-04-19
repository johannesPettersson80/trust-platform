import * as vscode from "vscode";

import {
  lmAvailable,
  type LmApi,
  type LspClientProvider,
} from "./lm-tools/shared";
import {
  STCodeActionsTool,
  STCodeLensTool,
  STCallHierarchyIncomingTool,
  STCallHierarchyOutgoingTool,
  STCallHierarchyPrepareTool,
  STCompletionTool,
  STDeclarationTool,
  STDefinitionTool,
  STDiagnosticsTool,
  STDocumentLinksTool,
  STDocumentSymbolsTool,
  STFormatTool,
  STHoverTool,
  STImplementationTool,
  STInlayHintsTool,
  STInlineValuesTool,
  STLinkedEditingTool,
  STLspNotificationTool,
  STLspRequestTool,
  STOnTypeFormattingTool,
  STProjectInfoTool,
  STReferencesTool,
  STRenameTool,
  STSelectionRangeTool,
  STSemanticTokensDeltaTool,
  STSemanticTokensFullTool,
  STSemanticTokensRangeTool,
  STSettingsUpdateTool,
  STSignatureHelpTool,
  STTelemetryReadTool,
  STTypeDefinitionTool,
  STTypeHierarchyPrepareTool,
  STTypeHierarchySubtypesTool,
  STTypeHierarchySupertypesTool,
  STWorkspaceRenameFileTool,
  STWorkspaceSymbolsTimedTool,
  STWorkspaceSymbolsTool,
} from "./lm-tools/lspTools";
import {
  STHmiApplyPatchTool,
  STHmiExplainWidgetTool,
  STHmiGenerateCandidatesTool,
  STHmiGetBindingsTool,
  STHmiGetLayoutTool,
  STHmiInitTool,
  STHmiPlanIntentTool,
  STHmiPreviewSnapshotTool,
  STHmiRunJourneyTool,
  STHmiTraceCaptureTool,
  STHmiValidateTool,
} from "./lm-tools/hmiTools";
import {
  STApplyEditsTool,
  STFileReadTool,
  STFileWriteTool,
  STReadRangeTool,
} from "./lm-tools/fileTools";
import {
  STDebugAttachTool,
  STDebugEnsureConfigurationTool,
  STDebugOpenIoPanelTool,
  STDebugReloadTool,
  STDebugStartTool,
} from "./lm-tools/debugTools";

export { __testSetRuntimeControlRequestHandler } from "./lm-tools/shared";
export * from "./lm-tools/hmiTools";

export function registerLanguageModelTools(
  context: vscode.ExtensionContext,
  options?: { getClient?: LspClientProvider },
): void {
  if (!lmAvailable()) {
    return;
  }
  const lm = (vscode as unknown as { lm?: LmApi }).lm;
  if (!lm) {
    return;
  }
  const getClient = options?.getClient;
  context.subscriptions.push(
    lm.registerTool("trust_lsp_request", new STLspRequestTool(getClient)),
    lm.registerTool("trust_lsp_notify", new STLspNotificationTool(getClient)),
    lm.registerTool("trust_get_hover", new STHoverTool()),
    lm.registerTool(
      "trust_get_semantic_tokens_full",
      new STSemanticTokensFullTool(getClient),
    ),
    lm.registerTool(
      "trust_get_semantic_tokens_delta",
      new STSemanticTokensDeltaTool(getClient),
    ),
    lm.registerTool(
      "trust_get_semantic_tokens_range",
      new STSemanticTokensRangeTool(getClient),
    ),
    lm.registerTool("trust_get_inlay_hints", new STInlayHintsTool(getClient)),
    lm.registerTool(
      "trust_get_linked_editing",
      new STLinkedEditingTool(getClient),
    ),
    lm.registerTool(
      "trust_get_document_links",
      new STDocumentLinksTool(getClient),
    ),
    lm.registerTool("trust_get_code_lens", new STCodeLensTool(getClient)),
    lm.registerTool(
      "trust_get_selection_ranges",
      new STSelectionRangeTool(getClient),
    ),
    lm.registerTool(
      "trust_get_on_type_formatting_edits",
      new STOnTypeFormattingTool(getClient),
    ),
    lm.registerTool(
      "trust_call_hierarchy_prepare",
      new STCallHierarchyPrepareTool(getClient),
    ),
    lm.registerTool(
      "trust_call_hierarchy_incoming",
      new STCallHierarchyIncomingTool(getClient),
    ),
    lm.registerTool(
      "trust_call_hierarchy_outgoing",
      new STCallHierarchyOutgoingTool(getClient),
    ),
    lm.registerTool(
      "trust_type_hierarchy_prepare",
      new STTypeHierarchyPrepareTool(getClient),
    ),
    lm.registerTool(
      "trust_type_hierarchy_supertypes",
      new STTypeHierarchySupertypesTool(getClient),
    ),
    lm.registerTool(
      "trust_type_hierarchy_subtypes",
      new STTypeHierarchySubtypesTool(getClient),
    ),
    lm.registerTool("trust_file_read", new STFileReadTool()),
    lm.registerTool("trust_read_range", new STReadRangeTool()),
    lm.registerTool("trust_file_write", new STFileWriteTool()),
    lm.registerTool("trust_apply_edits", new STApplyEditsTool()),
    lm.registerTool("trust_get_diagnostics", new STDiagnosticsTool()),
    lm.registerTool("trust_get_definition", new STDefinitionTool()),
    lm.registerTool("trust_get_declaration", new STDeclarationTool()),
    lm.registerTool("trust_get_type_definition", new STTypeDefinitionTool()),
    lm.registerTool("trust_get_implementation", new STImplementationTool()),
    lm.registerTool("trust_get_references", new STReferencesTool()),
    lm.registerTool("trust_get_completions", new STCompletionTool()),
    lm.registerTool("trust_get_signature_help", new STSignatureHelpTool()),
    lm.registerTool("trust_get_document_symbols", new STDocumentSymbolsTool()),
    lm.registerTool("trust_get_workspace_symbols", new STWorkspaceSymbolsTool()),
    lm.registerTool(
      "trust_get_workspace_symbols_timed",
      new STWorkspaceSymbolsTimedTool(),
    ),
    lm.registerTool("trust_get_rename_edits", new STRenameTool()),
    lm.registerTool("trust_get_formatting_edits", new STFormatTool()),
    lm.registerTool("trust_get_code_actions", new STCodeActionsTool(getClient)),
    lm.registerTool("trust_get_project_info", new STProjectInfoTool(getClient)),
    lm.registerTool("trust_hmi_get_bindings", new STHmiGetBindingsTool(getClient)),
    lm.registerTool("trust_hmi_get_layout", new STHmiGetLayoutTool()),
    lm.registerTool("trust_hmi_apply_patch", new STHmiApplyPatchTool()),
    lm.registerTool("trust_hmi_plan_intent", new STHmiPlanIntentTool()),
    lm.registerTool("trust_hmi_trace_capture", new STHmiTraceCaptureTool()),
    lm.registerTool(
      "trust_hmi_generate_candidates",
      new STHmiGenerateCandidatesTool(getClient),
    ),
    lm.registerTool("trust_hmi_validate", new STHmiValidateTool(getClient)),
    lm.registerTool(
      "trust_hmi_preview_snapshot",
      new STHmiPreviewSnapshotTool(getClient),
    ),
    lm.registerTool("trust_hmi_run_journey", new STHmiRunJourneyTool()),
    lm.registerTool(
      "trust_hmi_explain_widget",
      new STHmiExplainWidgetTool(getClient),
    ),
    lm.registerTool("trust_hmi_init", new STHmiInitTool(getClient)),
    lm.registerTool("trust_workspace_rename_file", new STWorkspaceRenameFileTool()),
    lm.registerTool("trust_update_settings", new STSettingsUpdateTool(getClient)),
    lm.registerTool("trust_read_telemetry", new STTelemetryReadTool()),
    lm.registerTool("trust_get_inline_values", new STInlineValuesTool()),
    lm.registerTool("trust_debug_start", new STDebugStartTool()),
    lm.registerTool("trust_debug_attach", new STDebugAttachTool()),
    lm.registerTool("trust_debug_reload", new STDebugReloadTool()),
    lm.registerTool("trust_debug_open_io_panel", new STDebugOpenIoPanelTool()),
    lm.registerTool(
      "trust_debug_ensure_configuration",
      new STDebugEnsureConfigurationTool(),
    ),
  );
}
