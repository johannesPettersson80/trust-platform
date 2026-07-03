// Responsibility: focused LM tools module with a single concern.
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

export type LmApi = {
  registerTool: <T>(
    name: string,
    tool: {
      invoke: (
        options: InvocationOptions<T>,
        token: vscode.CancellationToken,
      ) => Promise<unknown> | unknown;
    },
  ) => vscode.Disposable;
};

export type InvocationOptions<T> = {
  input: T;
};

export type LspClientProvider = () => LanguageClient | undefined;

export type LmToolResultCtor = new (parts: unknown[]) => unknown;
export type LmTextPartCtor = new (value: string) => unknown;

export const languageModelToolResultCtor = (
  vscode as unknown as { LanguageModelToolResult?: LmToolResultCtor }
).LanguageModelToolResult;

export const languageModelTextPartCtor = (
  vscode as unknown as { LanguageModelTextPart?: LmTextPartCtor }
).LanguageModelTextPart;

export const MAX_ITEMS = 200;

export interface PositionParams {
  filePath: string;
  line: number;
  character: number;
}

export interface DiagnosticsParams {
  filePath: string;
}

export interface ReferencesParams extends PositionParams {
  includeDeclaration?: boolean;
}

export interface CompletionParams extends PositionParams {
  triggerCharacter?: string;
}

export interface WorkspaceSymbolsParams {
  query: string;
  limit?: number;
}

export interface RenameParams extends PositionParams {
  newName: string;
}

export interface RangeParams {
  filePath: string;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
}

export interface RangePositionsParams {
  filePath: string;
  positions: Array<{ line: number; character: number }>;
}

export interface SemanticTokensDeltaParams {
  filePath: string;
  previousResultId: string;
}

export interface SemanticTokensRangeParams extends RangeParams {}

export interface InlayHintsParams extends RangeParams {}

export interface LinkedEditingParams extends PositionParams {}

export interface DocumentLinksParams {
  filePath: string;
  resolve?: boolean;
}

export interface CodeLensParams {
  filePath: string;
  resolve?: boolean;
}

export interface OnTypeFormattingParams extends PositionParams {
  triggerCharacter: string;
}

export interface LspRequestParams {
  method: string;
  params?: unknown;
  requestTimeoutMs?: number;
  captureNotifications?: string[];
  notificationTimeoutMs?: number;
  captureProgress?: boolean;
  capturePartialResults?: boolean;
  workDoneToken?: string;
  partialResultToken?: string;
}

export interface LspNotificationParams {
  method: string;
  params?: unknown;
}

export interface WorkspaceFileRenameParams {
  oldPath: string;
  newPath: string;
  overwrite?: boolean;
  useWorkspaceEdit?: boolean;
}

export interface SettingsToggleParams {
  key: string;
  value: unknown;
  scope?: "workspace" | "global" | "workspaceFolder";
  filePath?: string;
  timeoutMs?: number;
  forceRefresh?: boolean;
}

export interface TelemetryReadParams {
  filePath?: string;
  limit?: number;
  tail?: boolean;
}

export interface WorkspaceSymbolsTimedParams extends WorkspaceSymbolsParams {
  pathIncludes?: string[];
}

export interface InlineValuesParams {
  frameId: number;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
  context?: Record<string, unknown>;
}

export interface ProjectInfoParams {
  arguments?: unknown[];
}

export interface HmiInitParams {
  style?: string;
}

export interface HmiBindingsParams {
  rootPath?: string;
  filePath?: string;
}

export interface HmiGetLayoutParams {
  rootPath?: string;
}

export type HmiPatchOperation = {
  op: "add" | "remove" | "replace" | "move";
  path: string;
  from?: string;
  value?: unknown;
};

export interface HmiApplyPatchParams {
  dry_run?: boolean;
  rootPath?: string;
  operations: HmiPatchOperation[];
}

export interface HmiPlanIntentParams {
  rootPath?: string;
  dry_run?: boolean;
  summary?: string;
  goals?: string[];
  personas?: string[];
  kpis?: string[];
  priorities?: string[];
  constraints?: string[];
}

export interface HmiValidateParams {
  rootPath?: string;
  dry_run?: boolean;
  prune?: boolean;
  retain_runs?: number;
}

export interface HmiTraceCaptureParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  scenario?: string;
  ids?: string[];
  samples?: number;
  sample_interval_ms?: number;
}

export interface HmiGenerateCandidatesParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  candidate_count?: number;
}

export interface HmiPreviewSnapshotParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  candidate_id?: string;
  viewports?: string[];
}

export type HmiJourneyAction = "read_values" | "wait" | "write";

export interface HmiJourneyStepParams {
  action: HmiJourneyAction;
  ids?: string[];
  duration_ms?: number;
  widget_id?: string;
  value?: unknown;
  expect_error_code?: string;
}

export interface HmiJourneyParams {
  id: string;
  title?: string;
  max_duration_ms?: number;
  steps?: HmiJourneyStepParams[];
}

export interface HmiRunJourneyParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  scenario?: string;
  journeys?: HmiJourneyParams[];
}

export interface HmiExplainWidgetParams {
  rootPath?: string;
  widget_id?: string;
  path?: string;
}

export type HmiLayoutFileEntry = { name: string; path: string; content: string };

export type HmiLayoutSnapshot = {
  exists: boolean;
  rootPath: string;
  hmiPath: string;
  config: HmiLayoutFileEntry | null;
  pages: HmiLayoutFileEntry[];
  files: HmiLayoutFileEntry[];
  assets: string[];
};

export type HmiBindingCatalogEntry = {
  id: string;
  path: string;
  dataType: string;
  qualifier: string;
  writable: boolean;
  unit: string | null;
  min: number | null;
  max: number | null;
  enumValues: string[];
};

export type HmiBindingCatalog = {
  entries: HmiBindingCatalogEntry[];
  byPath: Map<string, HmiBindingCatalogEntry>;
};

export type HmiLockEntry = {
  id: string;
  path: string;
  data_type: string;
  qualifier: string;
  writable: boolean;
  constraints: {
    unit: string | null;
    min: number | null;
    max: number | null;
    enum_values: string[];
  };
  files: string[];
  binding_fingerprint: string;
};

export type HmiValidationCheck = {
  code: string;
  severity: "error" | "warning" | "info";
  message: string;
  file?: string;
  range?: string;
};

export type HmiSchemaWidget = {
  id: string;
  path: string;
  label: string;
  data_type: string;
  writable: boolean;
  page: string;
  group: string;
};

export type HmiSchemaResult = {
  version: number;
  mode: string;
  read_only: boolean;
  pages: Array<{
    id: string;
    title: string;
    order: number;
    kind?: string;
    sections?: Array<{ title: string; span: number; widget_ids?: string[] }>;
  }>;
  widgets: HmiSchemaWidget[];
};

export type HmiValuesResult = {
  connected: boolean;
  timestamp_ms: number;
  values: Record<string, { v: unknown; q: string; ts_ms: number }>;
};

export type HmiCandidateStrategy = {
  id: string;
  grouping: "program" | "qualifier" | "path";
  density: "compact" | "balanced" | "spacious";
  widget_bias: "status_first" | "balanced" | "trend_first";
  alarm_emphasis: boolean;
};

export type HmiCandidateMetrics = {
  readability: number;
  action_latency: number;
  alarm_salience: number;
  overall: number;
};

export type HmiCandidate = {
  id: string;
  rank: number;
  strategy: HmiCandidateStrategy;
  metrics: HmiCandidateMetrics;
  summary: {
    bindings: number;
    sections: number;
  };
  preview: {
    title: string;
    sections: Array<{
      title: string;
      widget_ids: string[];
    }>;
  };
};

export type SnapshotViewport = "desktop" | "tablet" | "mobile";
