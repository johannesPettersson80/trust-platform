// Responsibility: focused LM tools module with a single concern.
import { TextDecoder } from "util";
import * as vscode from "vscode";
import { MAX_ITEMS, type LspClientProvider } from "./types";
import { carriesCredential, credentialNamesOf, MODEL_CREDENTIAL_REFUSAL } from "../../lspCredentials";

export async function ensureDocument(uri: vscode.Uri): Promise<vscode.TextDocument> {
  const open = openDocumentIfLoaded(uri);
  if (open) {
    return open;
  }
  return vscode.workspace.openTextDocument(uri);
}

export function openDocumentIfLoaded(
  uri: vscode.Uri,
): vscode.TextDocument | undefined {
  return vscode.workspace.textDocuments.find(
    (doc) => doc.uri.toString() === uri.toString(),
  );
}

export function fullDocumentRange(doc: vscode.TextDocument): vscode.Range {
  if (doc.lineCount === 0) {
    return new vscode.Range(
      new vscode.Position(0, 0),
      new vscode.Position(0, 0),
    );
  }
  const lastLine = doc.lineCount - 1;
  return new vscode.Range(
    new vscode.Position(0, 0),
    doc.lineAt(lastLine).range.end,
  );
}

export function resolveRange(
  doc: vscode.TextDocument,
  startLine: number,
  startCharacter: number,
  endLine: number,
  endCharacter: number,
): vscode.Range {
  const start = resolvePosition(doc, startLine, startCharacter);
  const end = resolvePosition(doc, endLine, endCharacter);
  return start.isBefore(end)
    ? new vscode.Range(start, end)
    : new vscode.Range(end, start);
}

export function resolvePosition(
  doc: vscode.TextDocument,
  line: number,
  character: number,
): vscode.Position {
  const safeLine = Math.max(0, Math.min(line, doc.lineCount - 1));
  const safeChar = Math.max(
    0,
    Math.min(character, doc.lineAt(safeLine).text.length),
  );
  return new vscode.Position(safeLine, safeChar);
}

export async function waitForDiagnostics(
  uri: vscode.Uri,
  token: vscode.CancellationToken,
  timeoutMs = 1000,
): Promise<boolean> {
  if (token.isCancellationRequested) {
    return false;
  }
  return new Promise<boolean>((resolve) => {
    let settled = false;
    const finish = (value: boolean) => {
      if (settled) {
        return;
      }
      settled = true;
      disposable.dispose();
      resolve(value);
    };
    const timer = setTimeout(() => finish(false), timeoutMs);
    const disposable = vscode.languages.onDidChangeDiagnostics((event) => {
      if (event.uris.some((changed) => changed.toString() === uri.toString())) {
        clearTimeout(timer);
        finish(true);
      }
    });
    token.onCancellationRequested(() => {
      clearTimeout(timer);
      finish(false);
    });
  });
}

export function resolveLspPosition(position: vscode.Position): {
  line: number;
  character: number;
} {
  return { line: position.line, character: position.character };
}

export function resolveLspRange(range: vscode.Range): {
  start: { line: number; character: number };
  end: { line: number; character: number };
} {
  return { start: resolveLspPosition(range.start), end: resolveLspPosition(range.end) };
}

export async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  timeoutMessage: string,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(timeoutMessage)), timeoutMs);
    promise
      .then((value) => {
        clearTimeout(timer);
        resolve(value);
      })
      .catch((err) => {
        clearTimeout(timer);
        reject(err);
      });
  });
}

export function makeProgressToken(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

export async function sendLspRequest(
  getClient: LspClientProvider | undefined,
  method: string,
  params: unknown,
  token: vscode.CancellationToken,
  options?: {
    requestTimeoutMs?: number;
    captureNotifications?: string[];
    notificationTimeoutMs?: number;
  },
): Promise<
  | { response: unknown; notifications: Array<{ method: string; params: unknown }> }
  | { error: string }
> {
  if (token.isCancellationRequested) {
    return { error: "Cancelled." };
  }
  const client = getClient?.();
  if (!client) {
    return { error: "Language client is not available." };
  }
  // A language-model tool never sends or receives a credential (specification 25 §6.4): such a request goes to the
  // language server around the client's middleware, and its answer would reach the model.
  if (method === "workspace/executeCommand") {
    const { command, arguments: args } = (params ?? {}) as { command?: unknown; arguments?: unknown };
    const names = credentialNamesOf(client.initializeResult?.capabilities.experimental);
    if (names && typeof command === "string" && carriesCredential(command, Array.isArray(args) ? args : [], names)) {
      return { error: MODEL_CREDENTIAL_REFUSAL };
    }
  }
  const notifications: Array<{ method: string; params: unknown }> = [];
  const disposables: vscode.Disposable[] = [];
  if (options?.captureNotifications) {
    for (const name of options.captureNotifications) {
      disposables.push(
        client.onNotification(name, (payload) => {
          notifications.push({ method: name, params: payload });
        }),
      );
    }
  }
  try {
    const requestPromise = client.sendRequest(method, params);
    const response =
      typeof options?.requestTimeoutMs === "number"
        ? await withTimeout(
            requestPromise,
            options.requestTimeoutMs,
            `LSP request timed out after ${options.requestTimeoutMs}ms.`,
          )
        : await requestPromise;
    if (options?.notificationTimeoutMs) {
      await new Promise<void>((resolve) =>
        setTimeout(resolve, options.notificationTimeoutMs),
      );
    }
    return { response, notifications };
  } catch (error) {
    return { error: String(error) };
  } finally {
    for (const disposable of disposables) {
      disposable.dispose();
    }
  }
}

export function diagnosticsPayload(diagnostics: vscode.Diagnostic[]): {
  diagnostics: Array<{
    severity: string;
    message: string;
    range: string;
    source?: string;
    code?: string | number;
  }>;
} {
  return {
    diagnostics: diagnostics.map((diag) => ({
      severity: vscode.DiagnosticSeverity[diag.severity],
      message: diag.message,
      range: formatRange(diag.range),
      source: diag.source ?? undefined,
      code:
        typeof diag.code === "string" || typeof diag.code === "number"
          ? diag.code
          : diag.code?.value,
    })),
  };
}

export function toLspDiagnostic(diagnostic: vscode.Diagnostic): {
  range: { start: { line: number; character: number }; end: { line: number; character: number } };
  severity?: number;
  code?: string | number;
  source?: string;
  message: string;
  relatedInformation?: Array<{
    location: { uri: string; range: { start: { line: number; character: number }; end: { line: number; character: number } } };
    message: string;
  }>;
  tags?: number[];
} {
  const code =
    typeof diagnostic.code === "string" || typeof diagnostic.code === "number"
      ? diagnostic.code
      : diagnostic.code?.value;
  const relatedInformation = diagnostic.relatedInformation?.map((info) => ({
    location: {
      uri: info.location.uri.toString(),
      range: resolveLspRange(info.location.range),
    },
    message: info.message,
  }));
  return {
    range: resolveLspRange(diagnostic.range),
    severity: diagnostic.severity,
    code: code ?? undefined,
    source: diagnostic.source ?? undefined,
    message: diagnostic.message,
    relatedInformation: relatedInformation?.length ? relatedInformation : undefined,
    tags: diagnostic.tags?.length ? diagnostic.tags : undefined,
  };
}

export function diagnosticsForRange(
  uri: vscode.Uri,
  range: vscode.Range,
): vscode.Diagnostic[] {
  return vscode
    .languages
    .getDiagnostics(uri)
    .filter((diag) => !!diag.range.intersection(range));
}

export function renderMarkup(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (value && typeof value === "object") {
    const asRecord = value as Record<string, unknown>;
    if (typeof asRecord.value === "string") {
      return asRecord.value;
    }
    if (
      typeof asRecord.language === "string" &&
      typeof asRecord.value === "string"
    ) {
      return asRecord.value;
    }
  }
  return value ? String(value) : "";
}

export function formatRange(range: vscode.Range): string {
  return `${range.start.line + 1}:${range.start.character + 1}-${
    range.end.line + 1
  }:${range.end.character + 1}`;
}

export function formatUriString(uri: string): string {
  try {
    return vscode.Uri.parse(uri).fsPath;
  } catch {
    return uri;
  }
}

export function formatLspRange(range: {
  start: { line: number; character: number };
  end: { line: number; character: number };
}): string {
  return `${range.start.line + 1}:${range.start.character + 1}-${
    range.end.line + 1
  }:${range.end.character + 1}`;
}

export function formatLspLocation(location: { uri: string; range: any }): string {
  return `${formatUriString(location.uri)}:${location.range.start.line + 1}:${
    location.range.start.character + 1
  }`;
}

export function formatLocationLike(
  location: vscode.Location | vscode.LocationLink,
): string {
  const uri = "uri" in location ? location.uri : location.targetUri;
  const range = "range" in location ? location.range : location.targetRange;
  return `${uri.fsPath}:${range.start.line + 1}:${range.start.character + 1}`;
}

export function truncateItems<T>(
  items: T[],
  limit = MAX_ITEMS,
): { items: T[]; truncated: boolean } {
  if (items.length <= limit) {
    return { items, truncated: false };
  }
  return { items: items.slice(0, limit), truncated: true };
}

export function completionLabel(label: vscode.CompletionItem["label"]): string {
  return typeof label === "string" ? label : label.label;
}

export function completionDocumentation(
  documentation: vscode.CompletionItem["documentation"],
): string | undefined {
  if (!documentation) {
    return undefined;
  }
  if (typeof documentation === "string") {
    return documentation;
  }
  return renderMarkup(documentation);
}

export function completionInsertText(
  insertText: vscode.CompletionItem["insertText"],
): string | undefined {
  if (!insertText) {
    return undefined;
  }
  if (typeof insertText === "string") {
    return insertText;
  }
  return insertText.value;
}

export function symbolKindName(
  kind: vscode.SymbolKind | undefined,
): string | undefined {
  if (typeof kind !== "number") {
    return undefined;
  }
  return vscode.SymbolKind[kind];
}

export function completionKindName(
  kind: vscode.CompletionItemKind | undefined,
): string | undefined {
  if (typeof kind !== "number") {
    return undefined;
  }
  return vscode.CompletionItemKind[kind];
}

export type InlayHintLabelValue = string | vscode.InlayHintLabelPart[];

export function inlayHintLabel(label: InlayHintLabelValue): string {
  if (typeof label === "string") {
    return label;
  }
  return label.map((part: vscode.InlayHintLabelPart) => part.value).join("");
}

export function documentSymbolsToList(
  symbols: vscode.DocumentSymbol[],
  prefix = "",
): Array<{
  name: string;
  kind?: string;
  detail?: string;
  range: string;
  selectionRange: string;
  path: string;
}> {
  const items: Array<{
    name: string;
    kind?: string;
    detail?: string;
    range: string;
    selectionRange: string;
    path: string;
  }> = [];
  for (const symbol of symbols) {
    const pathSegment = prefix ? `${prefix}.${symbol.name}` : symbol.name;
    items.push({
      name: symbol.name,
      kind: symbolKindName(symbol.kind),
      detail: symbol.detail || undefined,
      range: formatRange(symbol.range),
      selectionRange: formatRange(symbol.selectionRange),
      path: pathSegment,
    });
    if (symbol.children && symbol.children.length > 0) {
      items.push(...documentSymbolsToList(symbol.children, pathSegment));
    }
  }
  return items;
}

export function workspaceEditEntries(
  edit: vscode.WorkspaceEdit,
): Array<{ uri: vscode.Uri; edits: vscode.TextEdit[] }> {
  const anyEdit = edit as unknown as {
    entries?: () => Array<[vscode.Uri, vscode.TextEdit[]]>;
    changes?: Record<string, vscode.TextEdit[]>;
    documentChanges?: Array<any>;
  };
  if (typeof anyEdit.entries === "function") {
    return anyEdit.entries().map(([uri, edits]) => ({ uri, edits }));
  }
  if (anyEdit.changes) {
    return Object.entries(anyEdit.changes).map(([uri, edits]) => ({
      uri: vscode.Uri.parse(uri),
      edits,
    }));
  }
  if (Array.isArray(anyEdit.documentChanges)) {
    const entries: Array<{ uri: vscode.Uri; edits: vscode.TextEdit[] }> = [];
    for (const change of anyEdit.documentChanges) {
      if ("edits" in change && "textDocument" in change) {
        entries.push({ uri: change.textDocument.uri, edits: change.edits });
      }
    }
    return entries;
  }
  return [];
}

export function summarizeWorkspaceEdit(edit: vscode.WorkspaceEdit): {
  files: Array<{
    filePath: string;
    edits: Array<{
      range: string;
      newTextPreview: string;
    }>;
  }>;
  truncated: boolean;
} {
  const entries = workspaceEditEntries(edit);
  const flattened: Array<{
    filePath: string;
    range: string;
    newText: string;
  }> = [];
  for (const entry of entries) {
    for (const textEdit of entry.edits) {
      flattened.push({
        filePath: entry.uri.fsPath,
        range: formatRange(textEdit.range),
        newText: textEdit.newText,
      });
    }
  }
  const { items, truncated } = truncateItems(flattened);
  const grouped = new Map<
    string,
    Array<{ range: string; newTextPreview: string }>
  >();
  for (const item of items) {
    const preview =
      item.newText.length > 120
        ? `${item.newText.slice(0, 117)}...`
        : item.newText;
    const edits = grouped.get(item.filePath) ?? [];
    edits.push({ range: item.range, newTextPreview: preview });
    grouped.set(item.filePath, edits);
  }
  return {
    files: Array.from(grouped.entries()).map(([filePath, edits]) => ({
      filePath,
      edits,
    })),
    truncated,
  };
}

export function summarizeLspTextEdits(
  edits: Array<{ range: { start: any; end: any }; newText: string }>,
): { edits: Array<{ range: string; newTextPreview: string }>; truncated: boolean } {
  const { items, truncated } = truncateItems(edits);
  const summarized = items.map((edit) => ({
    range: formatLspRange(edit.range),
    newTextPreview:
      edit.newText.length > 120
        ? `${edit.newText.slice(0, 117)}...`
        : edit.newText,
  }));
  return { edits: summarized, truncated };
}

export function summarizeSemanticTokens(result: unknown): unknown {
  if (!result || typeof result !== "object") {
    return result;
  }
  const record = result as {
    resultId?: string;
    data?: number[];
    edits?: Array<{ start: number; deleteCount: number; data?: number[] }>;
  };
  if (Array.isArray(record.data)) {
    return {
      resultId: record.resultId ?? undefined,
      dataLength: record.data.length,
      data: record.data,
    };
  }
  if (Array.isArray(record.edits)) {
    return {
      resultId: record.resultId ?? undefined,
      edits: record.edits.map((edit) => ({
        start: edit.start,
        deleteCount: edit.deleteCount,
        dataLength: Array.isArray(edit.data) ? edit.data.length : 0,
        data: Array.isArray(edit.data) ? edit.data : [],
      })),
    };
  }
  return result;
}
