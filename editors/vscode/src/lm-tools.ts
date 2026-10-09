import { registerAgentConnection } from "./hmiBuilder/agent/connect";
import { HmiAgentTool } from "./hmiBuilder/agent/tools";
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
  registerAgentConnection(context, getClient);
  const authoringEnvironment = { storageRoot: context.globalStorageUri.fsPath + "/hmi-drafts", fontFile: context.asAbsolutePath("media/fonts/InterVariable.woff2") };
  context.subscriptions.push(
    lm.registerTool("hmi_apply_draft", new HmiAgentTool("hmi_apply_draft", getClient, authoringEnvironment)),
    lm.registerTool("hmi_preview_states", new HmiAgentTool("hmi_preview_states", getClient, authoringEnvironment)),
    lm.registerTool("hmi_render_draft", new HmiAgentTool("hmi_render_draft", getClient, authoringEnvironment)),
    lm.registerTool("hmi_validate_draft", new HmiAgentTool("hmi_validate_draft", getClient, authoringEnvironment)),
    lm.registerTool("hmi_edit_draft", new HmiAgentTool("hmi_edit_draft", getClient, authoringEnvironment)),
    lm.registerTool("hmi_compose_page", new HmiAgentTool("hmi_compose_page", getClient, authoringEnvironment)),
    lm.registerTool("hmi_get_context", new HmiAgentTool("hmi_get_context", getClient, authoringEnvironment)),
    lm.registerTool("hmi_browse_library", new HmiAgentTool("hmi_browse_library", getClient, authoringEnvironment)),
    lm.registerTool("hmi_inspect_page", new HmiAgentTool("hmi_inspect_page", getClient, authoringEnvironment)),
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
