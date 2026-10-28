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
