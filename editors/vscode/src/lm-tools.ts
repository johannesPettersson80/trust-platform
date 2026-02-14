import * as path from "path";
import * as crypto from "crypto";
import * as net from "net";
import { TextDecoder } from "util";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import { defaultRuntimeControlEndpoint } from "./runtimeDefaults";

type LmApi = {
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

type InvocationOptions<T> = {
  input: T;
};

type LspClientProvider = () => LanguageClient | undefined;

type LmToolResultCtor = new (parts: unknown[]) => unknown;
type LmTextPartCtor = new (value: string) => unknown;

const languageModelToolResultCtor = (
  vscode as unknown as { LanguageModelToolResult?: LmToolResultCtor }
).LanguageModelToolResult;

const languageModelTextPartCtor = (
  vscode as unknown as { LanguageModelTextPart?: LmTextPartCtor }
).LanguageModelTextPart;

const MAX_ITEMS = 200;

interface PositionParams {
  filePath: string;
  line: number;
  character: number;
}

interface DiagnosticsParams {
  filePath: string;
}

interface ReferencesParams extends PositionParams {
  includeDeclaration?: boolean;
}

interface CompletionParams extends PositionParams {
  triggerCharacter?: string;
}

interface WorkspaceSymbolsParams {
  query: string;
  limit?: number;
}

interface RenameParams extends PositionParams {
  newName: string;
}

interface RangeParams {
  filePath: string;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
}

interface RangePositionsParams {
  filePath: string;
  positions: Array<{ line: number; character: number }>;
}

interface SemanticTokensDeltaParams {
  filePath: string;
  previousResultId: string;
}

interface SemanticTokensRangeParams extends RangeParams {}

interface InlayHintsParams extends RangeParams {}

interface LinkedEditingParams extends PositionParams {}

interface DocumentLinksParams {
  filePath: string;
  resolve?: boolean;
}

interface CodeLensParams {
  filePath: string;
  resolve?: boolean;
}

interface OnTypeFormattingParams extends PositionParams {
  triggerCharacter: string;
}

interface LspRequestParams {
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

interface LspNotificationParams {
  method: string;
  params?: unknown;
}

interface WorkspaceFileRenameParams {
  oldPath: string;
  newPath: string;
  overwrite?: boolean;
  useWorkspaceEdit?: boolean;
}

interface SettingsToggleParams {
  key: string;
  value: unknown;
  scope?: "workspace" | "global" | "workspaceFolder";
  filePath?: string;
  timeoutMs?: number;
  forceRefresh?: boolean;
}

interface TelemetryReadParams {
  filePath?: string;
  limit?: number;
  tail?: boolean;
}

interface WorkspaceSymbolsTimedParams extends WorkspaceSymbolsParams {
  pathIncludes?: string[];
}

interface InlineValuesParams {
  frameId: number;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
  context?: Record<string, unknown>;
}

interface ProjectInfoParams {
  arguments?: unknown[];
}

interface HmiInitParams {
  style?: string;
}

interface HmiBindingsParams {
  rootPath?: string;
  filePath?: string;
}

interface HmiGetLayoutParams {
  rootPath?: string;
}

type HmiPatchOperation = {
  op: "add" | "remove" | "replace" | "move";
  path: string;
  from?: string;
  value?: unknown;
};

interface HmiApplyPatchParams {
  dry_run?: boolean;
  rootPath?: string;
  operations: HmiPatchOperation[];
}

interface HmiPlanIntentParams {
  rootPath?: string;
  dry_run?: boolean;
  summary?: string;
  goals?: string[];
  personas?: string[];
  kpis?: string[];
  priorities?: string[];
  constraints?: string[];
}

interface HmiValidateParams {
  rootPath?: string;
  dry_run?: boolean;
  prune?: boolean;
  retain_runs?: number;
}

interface HmiTraceCaptureParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  scenario?: string;
  ids?: string[];
  samples?: number;
  sample_interval_ms?: number;
}

interface HmiGenerateCandidatesParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  candidate_count?: number;
}

interface HmiPreviewSnapshotParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  candidate_id?: string;
  viewports?: string[];
}

type HmiJourneyAction = "read_values" | "wait" | "write";

interface HmiJourneyStepParams {
  action: HmiJourneyAction;
  ids?: string[];
  duration_ms?: number;
  widget_id?: string;
  value?: unknown;
  expect_error_code?: string;
}

interface HmiJourneyParams {
  id: string;
  title?: string;
  max_duration_ms?: number;
  steps?: HmiJourneyStepParams[];
}

interface HmiRunJourneyParams {
  rootPath?: string;
  dry_run?: boolean;
  run_id?: string;
  scenario?: string;
  journeys?: HmiJourneyParams[];
}

interface HmiExplainWidgetParams {
  rootPath?: string;
  widget_id?: string;
  path?: string;
}

interface FileReadParams {
  filePath: string;
  startLine?: number;
  startCharacter?: number;
  endLine?: number;
  endCharacter?: number;
}

interface FileWriteParams {
  filePath: string;
  text: string;
  save?: boolean;
}

interface ApplyEditsParams {
  filePath: string;
  edits: Array<{
    startLine: number;
    startCharacter: number;
    endLine: number;
    endCharacter: number;
    newText: string;
  }>;
  save?: boolean;
}

interface DebugStartParams {
  filePath?: string;
}

type EmptyParams = Record<string, never>;

function lmAvailable(): boolean {
  const lm = (vscode as unknown as { lm?: LmApi }).lm;
  return !!(lm && languageModelToolResultCtor && languageModelTextPartCtor);
}

function textResult(text: string): unknown {
  if (!languageModelToolResultCtor || !languageModelTextPartCtor) {
    return { text };
  }
  return new languageModelToolResultCtor([new languageModelTextPartCtor(text)]);
}

function errorResult(message: string): unknown {
  return textResult(`Error: ${message}`);
}

function clientUnavailableResult(): unknown {
  return errorResult("Language client is not available.");
}

function uriFromFilePath(filePath: string): vscode.Uri | undefined {
  const trimmed = filePath.trim();
  if (!trimmed) {
    return undefined;
  }
  if (trimmed.startsWith("file://")) {
    try {
      return vscode.Uri.parse(trimmed);
    } catch {
      return undefined;
    }
  }
  const hasScheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(trimmed);
  const isWindowsPath = /^[a-zA-Z]:[\\/]/.test(trimmed);
  if (hasScheme && !isWindowsPath) {
    try {
      return vscode.Uri.parse(trimmed);
    } catch {
      return undefined;
    }
  }
  if (!path.isAbsolute(trimmed)) {
    return undefined;
  }
  return vscode.Uri.file(trimmed);
}

function optionalUriFromFilePath(
  filePath: string | undefined,
): vscode.Uri | undefined {
  if (!filePath) {
    return undefined;
  }
  const trimmed = filePath.trim();
  if (!trimmed) {
    return undefined;
  }
  return uriFromFilePath(trimmed);
}

function isPathInside(base: string, target: string): boolean {
  const rel = path.relative(base, target);
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

function ensureWorkspaceUri(uri: vscode.Uri): string | undefined {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return "No workspace is open.";
  }
  const normalizedTarget = path.normalize(uri.fsPath);
  for (const folder of folders) {
    const normalizedBase = path.normalize(folder.uri.fsPath);
    if (isPathInside(normalizedBase, normalizedTarget)) {
      return undefined;
    }
  }
  return "filePath must be inside the current workspace.";
}

function resolveWorkspaceFolder(
  rootPath?: string,
): { folder?: vscode.WorkspaceFolder; error?: string } {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return { error: "No workspace is open." };
  }
  if (!rootPath || !rootPath.trim()) {
    const active = vscode.window.activeTextEditor?.document.uri;
    if (active) {
      const byActive = vscode.workspace.getWorkspaceFolder(active);
      if (byActive) {
        return { folder: byActive };
      }
    }
    return { folder: folders[0] };
  }

  const uri = uriFromFilePath(rootPath.trim());
  if (!uri) {
    return { error: "rootPath must be an absolute path or URI." };
  }
  const workspaceError = ensureWorkspaceUri(uri);
  if (workspaceError) {
    return { error: workspaceError };
  }
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  if (!folder) {
    return { error: "Unable to resolve workspace folder for rootPath." };
  }
  return { folder };
}

function decodeJsonPointerToken(token: string): string {
  return token.replace(/~1/g, "/").replace(/~0/g, "~");
}

function normalizeHmiTomlName(raw: string): string | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  const normalized = trimmed.replace(/^\/+/, "");
  if (
    normalized.includes("/") ||
    normalized.includes("\\") ||
    normalized.includes("..")
  ) {
    return undefined;
  }
  if (!/^[A-Za-z0-9._-]+\.toml$/.test(normalized)) {
    return undefined;
  }
  return normalized;
}

function hmiDescriptorFileFromPointer(pointer: string): string | undefined {
  if (!pointer.startsWith("/")) {
    return undefined;
  }
  const parts = pointer
    .split("/")
    .slice(1)
    .map(decodeJsonPointerToken);
  if (parts.length < 2 || parts[0] !== "files") {
    return undefined;
  }
  const file = normalizeHmiTomlName(parts[1] ?? "");
  if (!file) {
    return undefined;
  }
  if (parts.length === 2) {
    return file;
  }
  if (parts.length === 3 && parts[2] === "content") {
    return file;
  }
  return undefined;
}

type HmiLayoutFileEntry = { name: string; path: string; content: string };

type HmiLayoutSnapshot = {
  exists: boolean;
  rootPath: string;
  hmiPath: string;
  config: HmiLayoutFileEntry | null;
  pages: HmiLayoutFileEntry[];
  files: HmiLayoutFileEntry[];
  assets: string[];
};

type HmiBindingCatalogEntry = {
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

type HmiBindingCatalog = {
  entries: HmiBindingCatalogEntry[];
  byPath: Map<string, HmiBindingCatalogEntry>;
};

type HmiLockEntry = {
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

type HmiValidationCheck = {
  code: string;
  severity: "error" | "warning" | "info";
  message: string;
  file?: string;
  range?: string;
};

type HmiSchemaWidget = {
  id: string;
  path: string;
  label: string;
  data_type: string;
  writable: boolean;
  page: string;
  group: string;
};

type HmiSchemaResult = {
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

type HmiValuesResult = {
  connected: boolean;
  timestamp_ms: number;
  values: Record<string, { v: unknown; q: string; ts_ms: number }>;
};

type HmiCandidateStrategy = {
  id: string;
  grouping: "program" | "qualifier" | "path";
  density: "compact" | "balanced" | "spacious";
  widget_bias: "status_first" | "balanced" | "trend_first";
  alarm_emphasis: boolean;
};

type HmiCandidateMetrics = {
  readability: number;
  action_latency: number;
  alarm_salience: number;
  overall: number;
};

type HmiCandidate = {
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

type SnapshotViewport = "desktop" | "tablet" | "mobile";

type RuntimeControlRequestHandler = (
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params: unknown,
  token: vscode.CancellationToken,
  timeoutMs?: number,
) => Promise<unknown>;

type ParsedControlEndpoint =
  | { kind: "tcp"; host: string; port: number }
  | { kind: "unix"; path: string };

let controlRequestSeq = 1;

function hmiSeverityRank(severity: HmiValidationCheck["severity"]): number {
  if (severity === "error") {
    return 0;
  }
  if (severity === "warning") {
    return 1;
  }
  return 2;
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

function parseControlEndpoint(endpoint: string): ParsedControlEndpoint | undefined {
  if (endpoint.startsWith("tcp://")) {
    try {
      const url = new URL(endpoint);
      const port = Number(url.port);
      if (!url.hostname || !Number.isFinite(port) || port <= 0) {
        return undefined;
      }
      return { kind: "tcp", host: url.hostname, port };
    } catch {
      return undefined;
    }
  }
  if (endpoint.startsWith("unix://")) {
    if (process.platform === "win32") {
      return undefined;
    }
    const socketPath = endpoint.slice("unix://".length);
    if (!socketPath.trim()) {
      return undefined;
    }
    return { kind: "unix", path: socketPath };
  }
  return undefined;
}

function runtimeEndpointSettings(rootPath: string): {
  endpoint: string;
  authToken: string | undefined;
} {
  const config = vscode.workspace.getConfiguration(
    "trust-lsp",
    vscode.Uri.file(rootPath),
  );
  const endpointEnabled = config.get<boolean>("runtime.controlEndpointEnabled", true);
  const configured = endpointEnabled
    ? (config.get<string>("runtime.controlEndpoint") ?? "").trim()
    : "";
  const endpoint = configured || defaultRuntimeControlEndpoint();
  const auth = (config.get<string>("runtime.controlAuthToken") ?? "").trim();
  return {
    endpoint,
    authToken: auth.length > 0 ? auth : undefined,
  };
}

async function sendRuntimeControlRequest(
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params: unknown,
  token: vscode.CancellationToken,
  timeoutMs = 2000,
): Promise<unknown> {
  if (token.isCancellationRequested) {
    throw new Error("Cancelled.");
  }
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    throw new Error(`invalid control endpoint '${endpoint}'`);
  }
  const requestEnvelope = {
    id: controlRequestSeq++,
    type: requestType,
    params,
    auth: authToken,
  };
  return await new Promise<unknown>((resolve, reject) => {
    let settled = false;
    let buffer = "";
    const socket =
      parsed.kind === "tcp"
        ? net.createConnection({ host: parsed.host, port: parsed.port })
        : net.createConnection({ path: parsed.path });
    const disposables: vscode.Disposable[] = [];

    const finish = (callback: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      for (const disposable of disposables) {
        disposable.dispose();
      }
      callback();
    };

    socket.setTimeout(timeoutMs, () => {
      finish(() => reject(new Error("control request timeout")));
    });
    socket.once("error", (error) => {
      finish(() => reject(error));
    });
    socket.once("connect", () => {
      socket.write(`${JSON.stringify(requestEnvelope)}\n`);
    });
    socket.on("data", (chunk: Buffer | string) => {
      buffer += chunk.toString();
      let newlineIndex = buffer.indexOf("\n");
      while (newlineIndex !== -1) {
        const line = buffer.slice(0, newlineIndex).trim();
        buffer = buffer.slice(newlineIndex + 1);
        if (line.length > 0) {
          try {
            const parsedLine = JSON.parse(line) as {
              ok?: boolean;
              result?: unknown;
              error?: unknown;
              code?: unknown;
            };
            if (parsedLine.ok) {
              finish(() => resolve(parsedLine.result));
            } else {
              const code =
                typeof parsedLine.code === "string" && parsedLine.code.trim()
                  ? parsedLine.code.trim()
                  : undefined;
              const detail =
                typeof parsedLine.error === "string" && parsedLine.error.trim()
                  ? parsedLine.error.trim()
                  : "control request rejected";
              finish(() =>
                reject(new Error(code ? `${code}: ${detail}` : detail)),
              );
            }
            return;
          } catch (error) {
            finish(() => reject(error));
            return;
          }
        }
        newlineIndex = buffer.indexOf("\n");
      }
    });

    disposables.push(
      token.onCancellationRequested(() => {
        finish(() => reject(new Error("Cancelled.")));
      }),
    );
  });
}

let runtimeControlRequest: RuntimeControlRequestHandler =
  sendRuntimeControlRequest;

export function __testSetRuntimeControlRequestHandler(
  handler?: RuntimeControlRequestHandler,
): void {
  runtimeControlRequest = handler ?? sendRuntimeControlRequest;
}

async function requestRuntimeControl(
  rootPath: string,
  token: vscode.CancellationToken,
  requestType: string,
  params: unknown,
): Promise<unknown> {
  const settings = runtimeEndpointSettings(rootPath);
  return await runtimeControlRequest(
    settings.endpoint,
    settings.authToken,
    requestType,
    params,
    token,
  );
}

function parseHmiSchemaPayload(value: unknown): HmiSchemaResult | undefined {
  const record = asRecord(value);
  if (!record) {
    return undefined;
  }
  const widgetValues = Array.isArray(record.widgets) ? record.widgets : [];
  const pageValues = Array.isArray(record.pages) ? record.pages : [];

  const widgets: HmiSchemaWidget[] = [];
  for (const item of widgetValues) {
    const widget = asRecord(item);
    if (!widget || typeof widget.id !== "string") {
      continue;
    }
    widgets.push({
      id: widget.id,
      path: typeof widget.path === "string" ? widget.path : "",
      label: typeof widget.label === "string" ? widget.label : widget.id,
      data_type: typeof widget.data_type === "string" ? widget.data_type : "UNKNOWN",
      writable: widget.writable === true,
      page: typeof widget.page === "string" ? widget.page : "overview",
      group: typeof widget.group === "string" ? widget.group : "General",
    });
  }

  const pages: HmiSchemaResult["pages"] = [];
  for (const item of pageValues) {
    const page = asRecord(item);
    if (!page || typeof page.id !== "string") {
      continue;
    }
    const sectionsRaw = Array.isArray(page.sections) ? page.sections : [];
    const sections = sectionsRaw
      .map((entry) => {
        const section = asRecord(entry);
        if (!section || typeof section.title !== "string") {
          return undefined;
        }
        const widgetIds = Array.isArray(section.widget_ids)
          ? section.widget_ids
              .filter((id): id is string => typeof id === "string")
              .sort((left, right) => left.localeCompare(right))
          : [];
        const normalized: { title: string; span: number; widget_ids?: string[] } = {
          title: section.title,
          span:
            typeof section.span === "number" && Number.isFinite(section.span)
              ? section.span
              : 12,
        };
        if (widgetIds.length > 0) {
          normalized.widget_ids = widgetIds;
        }
        return normalized;
      })
      .filter(
        (entry): entry is { title: string; span: number; widget_ids?: string[] } =>
          !!entry,
      );
    pages.push({
      id: page.id,
      title: typeof page.title === "string" ? page.title : page.id,
      order:
        typeof page.order === "number" && Number.isFinite(page.order)
          ? page.order
          : 0,
      kind: typeof page.kind === "string" ? page.kind : undefined,
      sections: sections.length > 0 ? sections : undefined,
    });
  }

  return {
    version:
      typeof record.version === "number" && Number.isFinite(record.version)
        ? record.version
        : 1,
    mode: typeof record.mode === "string" ? record.mode : "read_only",
    read_only: record.read_only !== false,
    pages: pages.sort((left, right) =>
      left.order === right.order
        ? left.id.localeCompare(right.id)
        : left.order - right.order,
    ),
    widgets: widgets.sort((left, right) => left.id.localeCompare(right.id)),
  };
}

function parseHmiValuesPayload(value: unknown): HmiValuesResult | undefined {
  const record = asRecord(value);
  if (!record) {
    return undefined;
  }
  const rawValues = asRecord(record.values) ?? {};
  const values: HmiValuesResult["values"] = {};
  for (const [widgetId, rawEntry] of Object.entries(rawValues)) {
    const entry = asRecord(rawEntry);
    if (!entry) {
      continue;
    }
    const quality = typeof entry.q === "string" ? entry.q : "unknown";
    const ts =
      typeof entry.ts_ms === "number" && Number.isFinite(entry.ts_ms)
        ? entry.ts_ms
        : Date.now();
    values[widgetId] = {
      v: entry.v,
      q: quality,
      ts_ms: ts,
    };
  }
  return {
    connected: record.connected !== false,
    timestamp_ms:
      typeof record.timestamp_ms === "number" && Number.isFinite(record.timestamp_ms)
        ? record.timestamp_ms
        : Date.now(),
    values,
  };
}

function coerceInt(
  value: unknown,
  fallback: number,
  minimum: number,
  maximum: number,
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(minimum, Math.min(maximum, Math.trunc(value)));
}

async function sleepWithCancellation(
  durationMs: number,
  token: vscode.CancellationToken,
): Promise<boolean> {
  if (token.isCancellationRequested) {
    return false;
  }
  return await new Promise<boolean>((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) {
        return;
      }
      settled = true;
      disposable.dispose();
      resolve(true);
    }, Math.max(0, durationMs));
    const disposable = token.onCancellationRequested(() => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve(false);
    });
  });
}

function normalizeEvidenceRunId(value: string | undefined): string | undefined {
  if (!value) {
    return undefined;
  }
  const trimmed = value.trim();
  return /^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z$/.test(trimmed)
    ? trimmed
    : undefined;
}

function layoutDescriptorPages(files: HmiLayoutFileEntry[]): HmiLayoutFileEntry[] {
  return files.filter(
    (file) =>
      file.name !== "_config.toml" &&
      file.name !== "_intent.toml" &&
      !file.name.startsWith("_"),
  );
}

function parseQuotedArrayFromToml(content: string, key: string): string[] {
  const match = content.match(
    new RegExp(`^\\s*${key}\\s*=\\s*\\[(.*)\\]\\s*$`, "m"),
  );
  if (!match || typeof match[1] !== "string") {
    return [];
  }
  const values: string[] = [];
  for (const quoted of match[1].matchAll(/"([^"]+)"/g)) {
    const item = (quoted[1] ?? "").trim();
    if (item) {
      values.push(item);
    }
  }
  return normalizeStringList(values);
}

function normalizeErrorCode(value: string | undefined): string | undefined {
  if (!value) {
    return undefined;
  }
  const normalized = value.trim().toUpperCase().replace(/[^A-Z0-9_]/g, "_");
  return normalized.length > 0 ? normalized : undefined;
}

function extractErrorCode(message: string): string | undefined {
  const match = message.match(/^\s*([A-Z0-9_]{3,})\s*:/);
  if (!match || typeof match[1] !== "string") {
    return undefined;
  }
  return normalizeErrorCode(match[1]);
}

function errorCodeMatches(
  expected: string | undefined,
  code: string | undefined,
  detail: string,
): boolean {
  const normalizedExpected = normalizeErrorCode(expected);
  if (!normalizedExpected) {
    return false;
  }
  const normalizedCode = normalizeErrorCode(code);
  if (normalizedCode && normalizedCode === normalizedExpected) {
    return true;
  }
  return detail.toUpperCase().includes(normalizedExpected);
}

async function readHmiLayoutSnapshot(
  rootPath: string | undefined,
  token: vscode.CancellationToken,
): Promise<{ snapshot?: HmiLayoutSnapshot; error?: string }> {
  const resolved = resolveWorkspaceFolder(rootPath);
  if (resolved.error || !resolved.folder) {
    return { error: resolved.error ?? "Unable to resolve workspace folder." };
  }
  const hmiRoot = vscode.Uri.joinPath(resolved.folder.uri, "hmi");
  let entries: [string, vscode.FileType][];
  try {
    entries = await vscode.workspace.fs.readDirectory(hmiRoot);
  } catch {
    return {
      snapshot: {
        exists: false,
        rootPath: resolved.folder.uri.fsPath,
        hmiPath: hmiRoot.fsPath,
        config: null,
        pages: [],
        files: [],
        assets: [],
      },
    };
  }

  const tomlFiles = entries
    .filter(
      ([name, kind]) =>
        kind === vscode.FileType.File &&
        name.toLowerCase().endsWith(".toml"),
    )
    .map(([name]) => name)
    .sort((left, right) => left.localeCompare(right));
  const svgFiles = entries
    .filter(
      ([name, kind]) =>
        kind === vscode.FileType.File &&
        name.toLowerCase().endsWith(".svg"),
    )
    .map(([name]) => name)
    .sort((left, right) => left.localeCompare(right));

  const files: HmiLayoutFileEntry[] = [];
  for (const fileName of tomlFiles) {
    if (token.isCancellationRequested) {
      return { error: "Cancelled." };
    }
    const fileUri = vscode.Uri.joinPath(hmiRoot, fileName);
    const bytes = await vscode.workspace.fs.readFile(fileUri);
    files.push({
      name: fileName,
      path: path.posix.join("hmi", fileName),
      content: Buffer.from(bytes).toString("utf8"),
    });
  }

  const config = files.find((entry) => entry.name === "_config.toml") ?? null;
  const pages = files
    .filter((entry) => entry.name !== "_config.toml")
    .sort((left, right) => left.name.localeCompare(right.name));

  return {
    snapshot: {
      exists: true,
      rootPath: resolved.folder.uri.fsPath,
      hmiPath: hmiRoot.fsPath,
      config,
      pages,
      files,
      assets: svgFiles,
    },
  };
}

async function writeUtf8File(uri: vscode.Uri, text: string): Promise<void> {
  await vscode.workspace.fs.writeFile(uri, Buffer.from(text, "utf8"));
}

function normalizeStringList(values: string[] | undefined): string[] {
  if (!Array.isArray(values)) {
    return [];
  }
  return Array.from(
    new Set(
      values
        .map((value) => value.trim())
        .filter((value) => value.length > 0),
    ),
  ).sort((left, right) => left.localeCompare(right));
}

function tomlQuote(value: string): string {
  return `"${value
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n")
    .replace(/\t/g, "\\t")}"`;
}

function renderIntentToml(params: HmiPlanIntentParams): string {
  const summary = (params.summary ?? "").trim();
  const goals = normalizeStringList(params.goals);
  const personas = normalizeStringList(params.personas);
  const kpis = normalizeStringList(params.kpis);
  const priorities = normalizeStringList(params.priorities);
  const constraints = normalizeStringList(params.constraints);
  const lines: string[] = [];
  lines.push("version = 1");
  lines.push("");
  lines.push("[intent]");
  lines.push(
    `summary = ${tomlQuote(summary || "Operator-focused HMI intent plan")}`,
  );
  lines.push(
    `personas = [${personas.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `goals = [${goals.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `kpis = [${kpis.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `priorities = [${priorities.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `constraints = [${constraints.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push("");
  lines.push("[workflow]");
  lines.push("requires_validation = true");
  lines.push("requires_evidence = true");
  lines.push("requires_journey = true");
  return `${lines.join("\n")}\n`;
}

type HmiLayoutBindingRef = { file: string; path: string };

function extractLayoutBindingRefs(
  files: HmiLayoutFileEntry[],
): HmiLayoutBindingRef[] {
  const refs: HmiLayoutBindingRef[] = [];
  const pattern = /^\s*(bind|source)\s*=\s*"([^"]+)"/;
  for (const file of files) {
    for (const line of file.content.split(/\r?\n/)) {
      const match = line.match(pattern);
      if (!match) {
        continue;
      }
      const bindPath = (match[2] ?? "").trim();
      if (!bindPath) {
        continue;
      }
      refs.push({ file: file.name, path: bindPath });
    }
  }
  refs.sort((left, right) =>
    left.path === right.path
      ? left.file.localeCompare(right.file)
      : left.path.localeCompare(right.path),
  );
  return refs;
}

function parseWritePolicyFromConfigToml(configContent: string | undefined): {
  enabled: boolean;
  allow: string[];
} {
  if (!configContent) {
    return { enabled: false, allow: [] };
  }
  let inWriteSection = false;
  let collectingAllow = false;
  let enabled = false;
  const allow = new Set<string>();
  for (const rawLine of configContent.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith("#")) {
      continue;
    }
    const section = line.match(/^\[([^\]]+)\]$/);
    if (section) {
      inWriteSection = section[1].trim().toLowerCase() === "write";
      collectingAllow = false;
      continue;
    }
    if (!inWriteSection) {
      continue;
    }
    const enabledMatch = line.match(/^enabled\s*=\s*(true|false)\s*$/i);
    if (enabledMatch) {
      enabled = enabledMatch[1].toLowerCase() === "true";
      continue;
    }
    if (collectingAllow || /^allow\s*=/.test(line)) {
      collectingAllow = true;
      for (const quoted of line.matchAll(/"([^"]+)"/g)) {
        const value = (quoted[1] ?? "").trim();
        if (value) {
          allow.add(value);
        }
      }
      if (line.includes("]")) {
        collectingAllow = false;
      }
    }
  }
  return {
    enabled,
    allow: Array.from(allow.values()).sort((left, right) =>
      left.localeCompare(right),
    ),
  };
}

function stableComponent(value: string): string {
  const source = value.trim().toLowerCase();
  if (!source) {
    return "x";
  }
  let out = "";
  let previousDash = false;
  for (const char of source) {
    const code = char.charCodeAt(0);
    const isAlphaNum =
      (code >= 48 && code <= 57) ||
      (code >= 97 && code <= 122);
    if (isAlphaNum) {
      out += char;
      previousDash = false;
      continue;
    }
    if (!previousDash) {
      out += "-";
      previousDash = true;
    }
  }
  const normalized = out.replace(/^-+/, "").replace(/-+$/, "");
  return normalized || "x";
}

function canonicalWidgetIdFromPath(pathValue: string): string {
  const trimmed = pathValue.trim();
  if (trimmed.toLowerCase().startsWith("global.")) {
    const name = trimmed.slice("global.".length);
    return `resource/resource/global/${stableComponent(name)}`;
  }
  const parts = trimmed.split(".");
  if (parts.length >= 2) {
    const program = parts[0];
    const field = parts.slice(1).join(".");
    return `resource/resource/program/${stableComponent(program)}/field/${stableComponent(field)}`;
  }
  return `resource/resource/path/${stableComponent(trimmed)}`;
}

function normalizeHmiBindingsCatalog(response: unknown): HmiBindingCatalog {
  const byPath = new Map<string, HmiBindingCatalogEntry>();
  const entries: HmiBindingCatalogEntry[] = [];
  const payload =
    response && typeof response === "object"
      ? (response as Record<string, unknown>)
      : {};
  const programs = Array.isArray(payload.programs) ? payload.programs : [];
  const globals = Array.isArray(payload.globals) ? payload.globals : [];
  for (const program of programs) {
    if (!program || typeof program !== "object") {
      continue;
    }
    const variables = Array.isArray((program as { variables?: unknown }).variables)
      ? ((program as { variables?: unknown[] }).variables ?? [])
      : [];
    for (const variable of variables) {
      if (!variable || typeof variable !== "object") {
        continue;
      }
      const record = variable as Record<string, unknown>;
      const pathValue = typeof record.path === "string" ? record.path.trim() : "";
      if (!pathValue) {
        continue;
      }
      const entry: HmiBindingCatalogEntry = {
        id: canonicalWidgetIdFromPath(pathValue),
        path: pathValue,
        dataType:
          typeof record.type === "string"
            ? record.type
            : typeof record.data_type === "string"
              ? record.data_type
              : "UNKNOWN",
        qualifier:
          typeof record.qualifier === "string" ? record.qualifier : "UNKNOWN",
        writable: record.writable === true,
        unit: typeof record.unit === "string" ? record.unit : null,
        min: Number.isFinite(record.min) ? Number(record.min) : null,
        max: Number.isFinite(record.max) ? Number(record.max) : null,
        enumValues: normalizeStringList(
          Array.isArray(record.enum_values)
            ? (record.enum_values.filter((value) => typeof value === "string") as string[])
            : [],
        ),
      };
      byPath.set(pathValue, entry);
      entries.push(entry);
    }
  }

  for (const variable of globals) {
    if (!variable || typeof variable !== "object") {
      continue;
    }
    const record = variable as Record<string, unknown>;
    const pathValue = typeof record.path === "string" ? record.path.trim() : "";
    if (!pathValue) {
      continue;
    }
    const entry: HmiBindingCatalogEntry = {
      id: canonicalWidgetIdFromPath(pathValue),
      path: pathValue,
      dataType:
        typeof record.type === "string"
          ? record.type
          : typeof record.data_type === "string"
            ? record.data_type
            : "UNKNOWN",
      qualifier:
        typeof record.qualifier === "string" ? record.qualifier : "UNKNOWN",
      writable: record.writable === true,
      unit: typeof record.unit === "string" ? record.unit : null,
      min: Number.isFinite(record.min) ? Number(record.min) : null,
      max: Number.isFinite(record.max) ? Number(record.max) : null,
      enumValues: normalizeStringList(
        Array.isArray(record.enum_values)
          ? (record.enum_values.filter((value) => typeof value === "string") as string[])
          : [],
      ),
    };
    byPath.set(pathValue, entry);
    entries.push(entry);
  }

  entries.sort((left, right) =>
    left.path === right.path
      ? left.id.localeCompare(right.id)
      : left.path.localeCompare(right.path),
  );

  return { entries, byPath };
}

function stableSortDeep(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((entry) => stableSortDeep(entry));
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  const source = value as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  for (const key of Object.keys(source).sort((left, right) =>
    left.localeCompare(right),
  )) {
    out[key] = stableSortDeep(source[key]);
  }
  return out;
}

function stableJsonString(value: unknown): string {
  return JSON.stringify(stableSortDeep(value), null, 2);
}

function bindingFingerprint(entry: Omit<HmiLockEntry, "binding_fingerprint">): string {
  return crypto
    .createHash("sha256")
    .update(stableJsonString(entry))
    .digest("hex")
    .slice(0, 16);
}

function buildHmiLockEntries(
  layoutRefs: HmiLayoutBindingRef[],
  catalog: HmiBindingCatalog,
): { entries: HmiLockEntry[]; unknownPaths: string[] } {
  const filesByPath = new Map<string, Set<string>>();
  for (const ref of layoutRefs) {
    const files = filesByPath.get(ref.path) ?? new Set<string>();
    files.add(ref.file);
    filesByPath.set(ref.path, files);
  }
  const layoutPaths = Array.from(filesByPath.keys()).sort((left, right) =>
    left.localeCompare(right),
  );
  const targetPaths =
    layoutPaths.length > 0
      ? layoutPaths
      : Array.from(catalog.byPath.keys()).sort((left, right) =>
          left.localeCompare(right),
        );
  const unknownPaths: string[] = [];
  const entries: HmiLockEntry[] = [];
  for (const pathValue of targetPaths) {
    const match = catalog.byPath.get(pathValue);
    if (!match) {
      unknownPaths.push(pathValue);
    }
    const base: Omit<HmiLockEntry, "binding_fingerprint"> = {
      id: match?.id ?? canonicalWidgetIdFromPath(pathValue),
      path: pathValue,
      data_type: match?.dataType ?? "UNKNOWN",
      qualifier: match?.qualifier ?? "UNKNOWN",
      writable: match?.writable ?? false,
      constraints: {
        unit: match?.unit ?? null,
        min: match?.min ?? null,
        max: match?.max ?? null,
        enum_values: match?.enumValues ?? [],
      },
      files: Array.from(filesByPath.get(pathValue) ?? []).sort((left, right) =>
        left.localeCompare(right),
      ),
    };
    entries.push({
      ...base,
      binding_fingerprint: bindingFingerprint(base),
    });
  }
  entries.sort((left, right) =>
    left.id === right.id
      ? left.path.localeCompare(right.path)
      : left.id.localeCompare(right.id),
  );
  unknownPaths.sort((left, right) => left.localeCompare(right));
  return { entries, unknownPaths };
}

const HMI_CANDIDATE_STRATEGIES: readonly HmiCandidateStrategy[] = [
  {
    id: "balanced",
    grouping: "program",
    density: "balanced",
    widget_bias: "balanced",
    alarm_emphasis: true,
  },
  {
    id: "alarm_first",
    grouping: "qualifier",
    density: "balanced",
    widget_bias: "status_first",
    alarm_emphasis: true,
  },
  {
    id: "compact",
    grouping: "program",
    density: "compact",
    widget_bias: "status_first",
    alarm_emphasis: false,
  },
  {
    id: "trend_first",
    grouping: "path",
    density: "spacious",
    widget_bias: "trend_first",
    alarm_emphasis: false,
  },
];

function metric(value: number): number {
  return Math.round(Math.max(0, Math.min(100, value)) * 100) / 100;
}

function strategyGroupKey(
  bindPath: string,
  catalog: HmiBindingCatalog,
  strategy: HmiCandidateStrategy,
): string {
  if (strategy.grouping === "qualifier") {
    const qualifier = catalog.byPath.get(bindPath)?.qualifier ?? "UNQUALIFIED";
    return qualifier.trim() || "UNQUALIFIED";
  }
  if (strategy.grouping === "path") {
    const root = bindPath.split(".")[0] ?? "Path";
    return root.trim() || "Path";
  }
  const program = bindPath.split(".")[0] ?? "Program";
  return program.trim() || "Program";
}

function buildCandidatePreview(
  bindPaths: string[],
  catalog: HmiBindingCatalog,
  strategy: HmiCandidateStrategy,
): HmiCandidate["preview"] {
  const sectionsByTitle = new Map<string, string[]>();
  for (const bindPath of bindPaths) {
    const sectionTitle = strategyGroupKey(bindPath, catalog, strategy);
    const widgetId = catalog.byPath.get(bindPath)?.id ?? canonicalWidgetIdFromPath(bindPath);
    const section = sectionsByTitle.get(sectionTitle) ?? [];
    section.push(widgetId);
    sectionsByTitle.set(sectionTitle, section);
  }
  const sections = Array.from(sectionsByTitle.entries())
    .map(([title, widgetIds]) => ({
      title,
      widget_ids: Array.from(new Set(widgetIds.values())).sort((left, right) =>
        left.localeCompare(right),
      ),
    }))
    .sort((left, right) => left.title.localeCompare(right.title));
  return {
    title: `Candidate ${strategy.id.replace(/_/g, " ")}`,
    sections,
  };
}

function intentPriorityWeights(intentContent: string | undefined): {
  readability: number;
  action_latency: number;
  alarm_salience: number;
} {
  const priorities = intentContent
    ? parseQuotedArrayFromToml(intentContent, "priorities")
    : [];
  let readability = 1;
  let actionLatency = 1;
  let alarmSalience = 1;
  for (const priority of priorities) {
    const normalized = priority.toLowerCase();
    if (
      normalized.includes("readability") ||
      normalized.includes("clarity") ||
      normalized.includes("usability")
    ) {
      readability += 1.5;
    }
    if (
      normalized.includes("latency") ||
      normalized.includes("response") ||
      normalized.includes("speed")
    ) {
      actionLatency += 1.5;
    }
    if (normalized.includes("alarm") || normalized.includes("safety")) {
      alarmSalience += 1.5;
    }
  }
  const total = readability + actionLatency + alarmSalience;
  return {
    readability: readability / total,
    action_latency: actionLatency / total,
    alarm_salience: alarmSalience / total,
  };
}

function generateCandidateMetrics(
  bindPaths: string[],
  catalog: HmiBindingCatalog,
  strategy: HmiCandidateStrategy,
  sectionCount: number,
  weights: {
    readability: number;
    action_latency: number;
    alarm_salience: number;
  },
): HmiCandidateMetrics {
  const bindCount = Math.max(1, bindPaths.length);
  const boolCount = bindPaths.filter((bindPath) => {
    const dataType = (catalog.byPath.get(bindPath)?.dataType ?? "").toUpperCase();
    return dataType === "BOOL";
  }).length;
  const boolRatio = boolCount / bindCount;
  const densityPenalty =
    strategy.density === "compact"
      ? 16
      : strategy.density === "balanced"
        ? 10
        : 6;
  const readability = metric(
    100 -
      densityPenalty -
      Math.max(0, bindCount - 8) * 1.2 -
      sectionCount * 2 +
      (strategy.widget_bias === "trend_first" ? -4 : 2),
  );
  const actionLatency = metric(
    100 -
      sectionCount * 4 -
      (strategy.density === "spacious"
        ? 14
        : strategy.density === "balanced"
          ? 10
          : 6) +
      (strategy.widget_bias === "status_first" ? 8 : 2),
  );
  const alarmSalience = metric(
    60 +
      (strategy.alarm_emphasis ? 25 : 8) +
      boolRatio * 15 -
      (strategy.density === "compact" ? 5 : 0),
  );
  const overall = metric(
    readability * weights.readability +
      actionLatency * weights.action_latency +
      alarmSalience * weights.alarm_salience,
  );
  return {
    readability,
    action_latency: actionLatency,
    alarm_salience: alarmSalience,
    overall,
  };
}

function generateHmiCandidates(
  layoutRefs: HmiLayoutBindingRef[],
  catalog: HmiBindingCatalog,
  intentContent: string | undefined,
  candidateCount: number,
): HmiCandidate[] {
  const uniqueBindPaths = Array.from(
    new Set(
      (layoutRefs.length > 0
        ? layoutRefs.map((ref) => ref.path)
        : catalog.entries.map((entry) => entry.path)
      ).filter((pathValue) => pathValue.trim().length > 0),
    ),
  ).sort((left, right) => left.localeCompare(right));
  const limit = Math.max(
    1,
    Math.min(HMI_CANDIDATE_STRATEGIES.length, Math.trunc(candidateCount)),
  );
  const weights = intentPriorityWeights(intentContent);
  const candidates = HMI_CANDIDATE_STRATEGIES.slice(0, limit).map((strategy) => {
    const preview = buildCandidatePreview(uniqueBindPaths, catalog, strategy);
    const metrics = generateCandidateMetrics(
      uniqueBindPaths,
      catalog,
      strategy,
      preview.sections.length,
      weights,
    );
    return {
      id: `candidate-${strategy.id}`,
      rank: 0,
      strategy,
      metrics,
      summary: {
        bindings: uniqueBindPaths.length,
        sections: preview.sections.length,
      },
      preview,
    } as HmiCandidate;
  });
  candidates.sort((left, right) => {
    if (left.metrics.overall !== right.metrics.overall) {
      return right.metrics.overall - left.metrics.overall;
    }
    return left.id.localeCompare(right.id);
  });
  return candidates.map((candidate, index) => ({
    ...candidate,
    rank: index + 1,
  }));
}

function normalizeTraceIds(
  ids: string[] | undefined,
  schema: HmiSchemaResult,
): string[] {
  const explicit = normalizeStringList(ids);
  if (explicit.length > 0) {
    return explicit;
  }
  return schema.widgets
    .map((widget) => widget.id)
    .sort((left, right) => left.localeCompare(right))
    .slice(0, 10);
}

function normalizeScenario(value: string | undefined): string {
  const trimmed = value?.trim();
  if (!trimmed) {
    return "normal";
  }
  return stableComponent(trimmed);
}

function normalizeSnapshotViewports(values: string[] | undefined): SnapshotViewport[] {
  const valid = new Set(
    normalizeStringList(values)
      .map((value) => value.toLowerCase())
      .filter((value) => value === "desktop" || value === "tablet" || value === "mobile"),
  );
  const order: SnapshotViewport[] = ["desktop", "tablet", "mobile"];
  if (valid.size === 0) {
    return order;
  }
  return order.filter((name) => valid.has(name));
}

function viewportSize(viewport: SnapshotViewport): { width: number; height: number } {
  if (viewport === "mobile") {
    return { width: 390, height: 844 };
  }
  if (viewport === "tablet") {
    return { width: 1024, height: 768 };
  }
  return { width: 1440, height: 900 };
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function renderSnapshotSvg(
  viewport: SnapshotViewport,
  candidate: HmiCandidate,
): string {
  const size = viewportSize(viewport);
  const padding = 24;
  const contentWidth = size.width - padding * 2;
  const titleY = 46;
  const rows = Math.max(1, Math.min(candidate.preview.sections.length, 8));
  const rowHeight = Math.max(56, Math.floor((size.height - 120) / rows));
  const lines: string[] = [];
  lines.push(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size.width}" height="${size.height}" viewBox="0 0 ${size.width} ${size.height}">`,
  );
  lines.push(`<rect x="0" y="0" width="${size.width}" height="${size.height}" fill="#0f172a" />`);
  lines.push(
    `<text x="${padding}" y="${titleY}" fill="#e2e8f0" font-family="Menlo, monospace" font-size="20">${escapeXml(candidate.preview.title)} (${viewport})</text>`,
  );
  candidate.preview.sections.slice(0, 8).forEach((section, index) => {
    const y = 72 + index * rowHeight;
    lines.push(
      `<rect x="${padding}" y="${y}" width="${contentWidth}" height="${rowHeight - 8}" rx="8" fill="#1e293b" stroke="#334155" />`,
    );
    lines.push(
      `<text x="${padding + 12}" y="${y + 26}" fill="#f8fafc" font-family="Menlo, monospace" font-size="14">${escapeXml(section.title)}</text>`,
    );
    lines.push(
      `<text x="${padding + 12}" y="${y + 46}" fill="#94a3b8" font-family="Menlo, monospace" font-size="12">widgets: ${section.widget_ids.length}</text>`,
    );
  });
  lines.push("</svg>");
  return `${lines.join("\n")}\n`;
}

function hashContent(value: string): string {
  return crypto.createHash("sha256").update(value).digest("hex").slice(0, 16);
}

function evidenceRunId(date: Date): string {
  const year = date.getUTCFullYear().toString().padStart(4, "0");
  const month = (date.getUTCMonth() + 1).toString().padStart(2, "0");
  const day = date.getUTCDate().toString().padStart(2, "0");
  const hours = date.getUTCHours().toString().padStart(2, "0");
  const minutes = date.getUTCMinutes().toString().padStart(2, "0");
  const seconds = date.getUTCSeconds().toString().padStart(2, "0");
  return `${year}-${month}-${day}T${hours}-${minutes}-${seconds}Z`;
}

async function pruneEvidenceRuns(
  hmiRoot: vscode.Uri,
  retainRuns: number,
): Promise<string[]> {
  const evidenceRoot = vscode.Uri.joinPath(hmiRoot, "_evidence");
  let entries: [string, vscode.FileType][];
  try {
    entries = await vscode.workspace.fs.readDirectory(evidenceRoot);
  } catch {
    return [];
  }
  const dirs = entries
    .filter(([, kind]) => kind === vscode.FileType.Directory)
    .map(([name]) => name)
    .sort((left, right) => left.localeCompare(right));
  const limit = Math.max(1, Math.trunc(retainRuns));
  if (dirs.length <= limit) {
    return [];
  }
  const removable = dirs.slice(0, dirs.length - limit);
  for (const name of removable) {
    await vscode.workspace.fs.delete(vscode.Uri.joinPath(evidenceRoot, name), {
      recursive: true,
      useTrash: false,
    });
  }
  return removable;
}

async function collectHmiDiagnosticsForFiles(
  rootPath: string,
  files: HmiLayoutFileEntry[],
  token: vscode.CancellationToken,
): Promise<HmiValidationCheck[]> {
  const checks: HmiValidationCheck[] = [];
  for (const file of files) {
    if (token.isCancellationRequested) {
      break;
    }
    const uri = vscode.Uri.joinPath(vscode.Uri.file(rootPath), file.path);
    try {
      await ensureDocument(uri);
    } catch {
      continue;
    }
    const diagnostics = vscode.languages.getDiagnostics(uri);
    for (const diagnostic of diagnostics) {
      const severity: HmiValidationCheck["severity"] =
        diagnostic.severity === vscode.DiagnosticSeverity.Error
          ? "error"
          : diagnostic.severity === vscode.DiagnosticSeverity.Warning
            ? "warning"
            : "info";
      const code =
        typeof diagnostic.code === "string" || typeof diagnostic.code === "number"
          ? String(diagnostic.code)
          : typeof diagnostic.code?.value === "string" ||
              typeof diagnostic.code?.value === "number"
            ? String(diagnostic.code.value)
            : "HMI_VALIDATE_DIAGNOSTIC";
      checks.push({
        code,
        severity,
        message: diagnostic.message,
        file: file.path,
        range: formatRange(diagnostic.range),
      });
    }
  }
  checks.sort((left, right) => {
    const rank = hmiSeverityRank(left.severity) - hmiSeverityRank(right.severity);
    if (rank !== 0) {
      return rank;
    }
    if ((left.file ?? "") !== (right.file ?? "")) {
      return (left.file ?? "").localeCompare(right.file ?? "");
    }
    if (left.code !== right.code) {
      return left.code.localeCompare(right.code);
    }
    return left.message.localeCompare(right.message);
  });
  return checks;
}

async function ensureDocument(uri: vscode.Uri): Promise<vscode.TextDocument> {
  const open = openDocumentIfLoaded(uri);
  if (open) {
    return open;
  }
  return vscode.workspace.openTextDocument(uri);
}

function openDocumentIfLoaded(
  uri: vscode.Uri,
): vscode.TextDocument | undefined {
  return vscode.workspace.textDocuments.find(
    (doc) => doc.uri.toString() === uri.toString(),
  );
}

function fullDocumentRange(doc: vscode.TextDocument): vscode.Range {
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

function resolveRange(
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

function resolvePosition(
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

async function waitForDiagnostics(
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

function resolveLspPosition(position: vscode.Position): {
  line: number;
  character: number;
} {
  return { line: position.line, character: position.character };
}

function resolveLspRange(range: vscode.Range): {
  start: { line: number; character: number };
  end: { line: number; character: number };
} {
  return { start: resolveLspPosition(range.start), end: resolveLspPosition(range.end) };
}

async function withTimeout<T>(
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

function makeProgressToken(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function sendLspRequest(
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

function diagnosticsPayload(diagnostics: vscode.Diagnostic[]): {
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

function toLspDiagnostic(diagnostic: vscode.Diagnostic): {
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

function diagnosticsForRange(
  uri: vscode.Uri,
  range: vscode.Range,
): vscode.Diagnostic[] {
  return vscode
    .languages
    .getDiagnostics(uri)
    .filter((diag) => !!diag.range.intersection(range));
}

function renderMarkup(value: unknown): string {
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

function formatRange(range: vscode.Range): string {
  return `${range.start.line + 1}:${range.start.character + 1}-${
    range.end.line + 1
  }:${range.end.character + 1}`;
}

function formatUriString(uri: string): string {
  try {
    return vscode.Uri.parse(uri).fsPath;
  } catch {
    return uri;
  }
}

function formatLspRange(range: {
  start: { line: number; character: number };
  end: { line: number; character: number };
}): string {
  return `${range.start.line + 1}:${range.start.character + 1}-${
    range.end.line + 1
  }:${range.end.character + 1}`;
}

function formatLspLocation(location: { uri: string; range: any }): string {
  return `${formatUriString(location.uri)}:${location.range.start.line + 1}:${
    location.range.start.character + 1
  }`;
}

function formatLocationLike(
  location: vscode.Location | vscode.LocationLink,
): string {
  const uri = "uri" in location ? location.uri : location.targetUri;
  const range = "range" in location ? location.range : location.targetRange;
  return `${uri.fsPath}:${range.start.line + 1}:${range.start.character + 1}`;
}

function truncateItems<T>(
  items: T[],
  limit = MAX_ITEMS,
): { items: T[]; truncated: boolean } {
  if (items.length <= limit) {
    return { items, truncated: false };
  }
  return { items: items.slice(0, limit), truncated: true };
}

function completionLabel(label: vscode.CompletionItem["label"]): string {
  return typeof label === "string" ? label : label.label;
}

function completionDocumentation(
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

function completionInsertText(
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

function symbolKindName(
  kind: vscode.SymbolKind | undefined,
): string | undefined {
  if (typeof kind !== "number") {
    return undefined;
  }
  return vscode.SymbolKind[kind];
}

function completionKindName(
  kind: vscode.CompletionItemKind | undefined,
): string | undefined {
  if (typeof kind !== "number") {
    return undefined;
  }
  return vscode.CompletionItemKind[kind];
}

type InlayHintLabelValue = string | vscode.InlayHintLabelPart[];

function inlayHintLabel(label: InlayHintLabelValue): string {
  if (typeof label === "string") {
    return label;
  }
  return label.map((part: vscode.InlayHintLabelPart) => part.value).join("");
}

function documentSymbolsToList(
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

function workspaceEditEntries(
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

function summarizeWorkspaceEdit(edit: vscode.WorkspaceEdit): {
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

function summarizeLspTextEdits(
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

function summarizeSemanticTokens(result: unknown): unknown {
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

export class STHoverTool {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
        "vscode.executeHoverProvider",
        uri,
        position,
      );
      if (!hovers || hovers.length === 0) {
        return textResult("No hover information available at this position.");
      }
      const content = hovers
        .flatMap((hover) => hover.contents)
        .map((item) => renderMarkup(item))
        .filter((item) => item.length > 0)
        .join("\n\n");
      return textResult(
        content || "No hover information available at this position.",
      );
    } catch (error) {
      return errorResult(`Failed to get hover info: ${String(error)}`);
    }
  }
}

export class STDiagnosticsTool {
  async invoke(
    options: InvocationOptions<DiagnosticsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const alreadyOpen = vscode.workspace.textDocuments.some(
      (doc) => doc.uri.toString() === uri.toString(),
    );
    try {
      await ensureDocument(uri);
      if (!alreadyOpen) {
        await waitForDiagnostics(uri, token);
      }
      const diagnostics = vscode.languages.getDiagnostics(uri);
      if (diagnostics.length === 0) {
        return textResult("No diagnostics (errors or warnings) found.");
      }
      return textResult(
        JSON.stringify(diagnosticsPayload(diagnostics), null, 2),
      );
    } catch (error) {
      return errorResult(`Failed to get diagnostics: ${String(error)}`);
    }
  }
}

export class STDefinitionTool {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const definitions = await vscode.commands.executeCommand<
        vscode.Location[] | vscode.LocationLink[]
      >("vscode.executeDefinitionProvider", uri, position);
      if (!definitions || definitions.length === 0) {
        return textResult("No definition found.");
      }
      const locations = definitions.map(formatLocationLike);
      return textResult(JSON.stringify({ locations }, null, 2));
    } catch (error) {
      return errorResult(`Failed to find definition: ${String(error)}`);
    }
  }
}

export class STDeclarationTool {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const locations = await vscode.commands.executeCommand<
        vscode.Location[] | vscode.LocationLink[]
      >("vscode.executeDeclarationProvider", uri, position);
      if (!locations || locations.length === 0) {
        return textResult("No declaration found.");
      }
      const formatted = locations.map(formatLocationLike);
      return textResult(JSON.stringify({ locations: formatted }, null, 2));
    } catch (error) {
      return errorResult(`Failed to find declaration: ${String(error)}`);
    }
  }
}

export class STTypeDefinitionTool {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const locations = await vscode.commands.executeCommand<
        vscode.Location[] | vscode.LocationLink[]
      >("vscode.executeTypeDefinitionProvider", uri, position);
      if (!locations || locations.length === 0) {
        return textResult("No type definition found.");
      }
      const formatted = locations.map(formatLocationLike);
      return textResult(JSON.stringify({ locations: formatted }, null, 2));
    } catch (error) {
      return errorResult(`Failed to find type definition: ${String(error)}`);
    }
  }
}

export class STImplementationTool {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const implementations = await vscode.commands.executeCommand<
        vscode.Location[] | vscode.LocationLink[]
      >("vscode.executeImplementationProvider", uri, position);
      if (!implementations || implementations.length === 0) {
        return textResult("No implementations found.");
      }
      const locations = implementations.map(formatLocationLike);
      return textResult(JSON.stringify({ locations }, null, 2));
    } catch (error) {
      return errorResult(`Failed to find implementations: ${String(error)}`);
    }
  }
}

export class STReferencesTool {
  async invoke(
    options: InvocationOptions<ReferencesParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character, includeDeclaration } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const references = await vscode.commands.executeCommand<
        vscode.Location[]
      >("vscode.executeReferenceProvider", uri, position, {
        includeDeclaration: includeDeclaration ?? true,
      });
      if (!references || references.length === 0) {
        return textResult("No references found.");
      }
      const locations = references.map((ref) => formatLocationLike(ref));
      return textResult(JSON.stringify({ locations }, null, 2));
    } catch (error) {
      return errorResult(`Failed to find references: ${String(error)}`);
    }
  }
}

export class STCompletionTool {
  async invoke(
    options: InvocationOptions<CompletionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character, triggerCharacter } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const completions = await vscode.commands.executeCommand<
        vscode.CompletionList | vscode.CompletionItem[]
      >(
        "vscode.executeCompletionItemProvider",
        uri,
        position,
        triggerCharacter,
      );
      if (!completions) {
        return textResult("No completion items returned.");
      }
      const items = Array.isArray(completions)
        ? completions
        : completions.items;
      const { items: trimmed, truncated } = truncateItems(items);
      const payload = trimmed.map((item) => ({
        label: completionLabel(item.label),
        kind: completionKindName(item.kind),
        detail: item.detail || undefined,
        documentation: completionDocumentation(item.documentation),
        insertText: completionInsertText(item.insertText),
      }));
      return textResult(JSON.stringify({ items: payload, truncated }, null, 2));
    } catch (error) {
      return errorResult(`Failed to get completions: ${String(error)}`);
    }
  }
}

export class STSignatureHelpTool {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const signatureHelp =
        await vscode.commands.executeCommand<vscode.SignatureHelp>(
          "vscode.executeSignatureHelpProvider",
          uri,
          position,
        );
      if (!signatureHelp || signatureHelp.signatures.length === 0) {
        return textResult("No signature help available.");
      }
      const payload = signatureHelp.signatures.map((sig, index) => ({
        label: sig.label,
        documentation: sig.documentation
          ? renderMarkup(sig.documentation as unknown)
          : undefined,
        parameters: sig.parameters?.map((param) => ({
          label: param.label,
          documentation: param.documentation
            ? renderMarkup(param.documentation as unknown)
            : undefined,
        })),
        isActiveSignature: index === signatureHelp.activeSignature,
      }));
      return textResult(
        JSON.stringify(
          {
            activeSignature: signatureHelp.activeSignature ?? 0,
            activeParameter: signatureHelp.activeParameter ?? 0,
            signatures: payload,
          },
          null,
          2,
        ),
      );
    } catch (error) {
      return errorResult(`Failed to get signature help: ${String(error)}`);
    }
  }
}

export class STDocumentSymbolsTool {
  async invoke(
    options: InvocationOptions<DiagnosticsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      await ensureDocument(uri);
      const symbols = await vscode.commands.executeCommand<
        vscode.DocumentSymbol[] | vscode.SymbolInformation[]
      >("vscode.executeDocumentSymbolProvider", uri);
      if (!symbols || symbols.length === 0) {
        return textResult("No document symbols found.");
      }
      if (symbols.length > 0 && "location" in symbols[0]) {
        const infoSymbols = symbols as vscode.SymbolInformation[];
        const { items, truncated } = truncateItems(infoSymbols);
        const payload = items.map((symbol) => ({
          name: symbol.name,
          kind: symbolKindName(symbol.kind),
          containerName: symbol.containerName || undefined,
          location: formatLocationLike(symbol.location),
        }));
        return textResult(
          JSON.stringify({ symbols: payload, truncated }, null, 2),
        );
      }
      const docSymbols = symbols as vscode.DocumentSymbol[];
      const flattened = documentSymbolsToList(docSymbols);
      const { items, truncated } = truncateItems(flattened);
      return textResult(JSON.stringify({ symbols: items, truncated }, null, 2));
    } catch (error) {
      return errorResult(`Failed to get document symbols: ${String(error)}`);
    }
  }
}

export class STWorkspaceSymbolsTool {
  async invoke(
    options: InvocationOptions<WorkspaceSymbolsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { query, limit } = options.input;
    if (!query.trim()) {
      return errorResult("query must be a non-empty string.");
    }
    try {
      const symbols = await vscode.commands.executeCommand<
        vscode.SymbolInformation[]
      >("vscode.executeWorkspaceSymbolProvider", query);
      if (!symbols || symbols.length === 0) {
        return textResult("No workspace symbols found.");
      }
      const { items, truncated } = truncateItems(symbols, limit ?? MAX_ITEMS);
      const payload = items.map((symbol) => ({
        name: symbol.name,
        kind: symbolKindName(symbol.kind),
        containerName: symbol.containerName || undefined,
        location: formatLocationLike(symbol.location),
      }));
      return textResult(
        JSON.stringify({ symbols: payload, truncated }, null, 2),
      );
    } catch (error) {
      return errorResult(`Failed to get workspace symbols: ${String(error)}`);
    }
  }
}

export class STRenameTool {
  async invoke(
    options: InvocationOptions<RenameParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character, newName } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    if (!newName.trim()) {
      return errorResult("newName must be a non-empty string.");
    }
    try {
      const doc = await ensureDocument(uri);
      const position = resolvePosition(doc, line, character);
      const edit = await vscode.commands.executeCommand<vscode.WorkspaceEdit>(
        "vscode.executeDocumentRenameProvider",
        uri,
        position,
        newName,
      );
      if (!edit) {
        return textResult("No rename edits returned.");
      }
      const summary = summarizeWorkspaceEdit(edit);
      return textResult(JSON.stringify({ edit: summary }, null, 2));
    } catch (error) {
      return errorResult(`Failed to rename symbol: ${String(error)}`);
    }
  }
}

export class STFormatTool {
  async invoke(
    options: InvocationOptions<DiagnosticsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      await ensureDocument(uri);
      const editorConfig = vscode.workspace.getConfiguration("editor", uri);
      const formattingOptions: vscode.FormattingOptions = {
        insertSpaces: editorConfig.get<boolean>("insertSpaces", true),
        tabSize: editorConfig.get<number>("tabSize", 2),
      };
      const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
        "vscode.executeFormatDocumentProvider",
        uri,
        formattingOptions,
      );
      if (!edits || edits.length === 0) {
        return textResult("No formatting edits returned.");
      }
      const edit = summarizeWorkspaceEdit({
        set: () => {},
        insert: () => {},
        delete: () => {},
        replace: () => {},
        entries: () => [[uri, edits]],
        size: edits.length,
        has: () => true,
      } as unknown as vscode.WorkspaceEdit);
      return textResult(JSON.stringify({ edit }, null, 2));
    } catch (error) {
      return errorResult(`Failed to format document: ${String(error)}`);
    }
  }
}

class LspToolBase {
  constructor(protected readonly getClient?: LspClientProvider) {}

  protected async request(
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
    return sendLspRequest(this.getClient, method, params, token, options);
  }
}

export class STCodeActionsTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<RangeParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, startLine, startCharacter, endLine, endCharacter } =
      options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    try {
      const doc = await ensureDocument(uri);
      const range = resolveRange(
        doc,
        startLine,
        startCharacter,
        endLine,
        endCharacter,
      );
      const diagnostics = diagnosticsForRange(uri, range).map(toLspDiagnostic);
      const params = {
        textDocument: { uri: uri.toString() },
        range: resolveLspRange(range),
        context: {
          diagnostics,
          triggerKind: 1,
        },
      };
      const result = await this.request("textDocument/codeAction", params, token);
      if ("error" in result) {
        return errorResult(result.error);
      }
      const actions = Array.isArray(result.response) ? result.response : [];
      if (actions.length === 0) {
        return textResult("No code actions found.");
      }
      const payload = actions.map((action) => {
        const isCodeAction =
          action &&
          typeof action === "object" &&
          ("edit" in action ||
            "kind" in action ||
            "isPreferred" in action ||
            "disabled" in action);
        if (!isCodeAction) {
          const cmd = action as vscode.Command;
          return {
            title: cmd.title,
            command: cmd.command,
            arguments: cmd.arguments,
          };
        }
        const codeAction = action as vscode.CodeAction;
        const kind =
          typeof codeAction.kind === "string"
            ? codeAction.kind
            : codeAction.kind?.value;
        return {
          title: codeAction.title,
          kind,
          isPreferred: codeAction.isPreferred ?? false,
          command: codeAction.command?.command,
          arguments: codeAction.command?.arguments,
        };
      });
      return textResult(JSON.stringify({ actions: payload }, null, 2));
    } catch (error) {
      return errorResult(`Failed to get code actions: ${String(error)}`);
    }
  }
}

export class STLspRequestTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<LspRequestParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const {
      method,
      params,
      requestTimeoutMs,
      captureNotifications,
      notificationTimeoutMs,
      captureProgress,
      capturePartialResults,
      workDoneToken,
      partialResultToken,
    } = options.input;
    if (!method || !method.trim()) {
      return errorResult("method must be a non-empty string.");
    }
    const notificationSet = new Set(captureNotifications ?? []);
    let nextParams = params;
    let usedWorkDoneToken = workDoneToken;
    let usedPartialResultToken = partialResultToken;
    const wantsProgress =
      captureProgress ||
      capturePartialResults ||
      !!workDoneToken ||
      !!partialResultToken;
    if (wantsProgress) {
      if (!params || typeof params !== "object" || Array.isArray(params)) {
        return errorResult(
          "params must be an object when progress/partial tokens are requested.",
        );
      }
      const paramRecord = { ...(params as Record<string, unknown>) };
      if (typeof paramRecord.workDoneToken === "string") {
        usedWorkDoneToken = paramRecord.workDoneToken;
      }
      if (typeof paramRecord.partialResultToken === "string") {
        usedPartialResultToken = paramRecord.partialResultToken;
      }
      if (!usedWorkDoneToken && captureProgress) {
        usedWorkDoneToken = makeProgressToken("trustlsp-work");
      }
      if (!usedPartialResultToken && capturePartialResults) {
        usedPartialResultToken = makeProgressToken("trustlsp-partial");
      }
      if (usedWorkDoneToken && paramRecord.workDoneToken === undefined) {
        paramRecord.workDoneToken = usedWorkDoneToken;
      }
      if (usedPartialResultToken && paramRecord.partialResultToken === undefined) {
        paramRecord.partialResultToken = usedPartialResultToken;
      }
      notificationSet.add("$/progress");
      nextParams = paramRecord;
    }
    const result = await this.request(method, nextParams, token, {
      requestTimeoutMs,
      captureNotifications: Array.from(notificationSet),
      notificationTimeoutMs,
    });
    if ("error" in result) {
      return errorResult(result.error);
    }
    return textResult(
      JSON.stringify(
        {
          response: result.response,
          notifications: result.notifications,
          workDoneToken: usedWorkDoneToken ?? undefined,
          partialResultToken: usedPartialResultToken ?? undefined,
        },
        null,
        2,
      ),
    );
  }
}

export class STLspNotificationTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<LspNotificationParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { method, params } = options.input;
    if (!method || !method.trim()) {
      return errorResult("method must be a non-empty string.");
    }
    const client = this.getClient?.();
    if (!client) {
      return clientUnavailableResult();
    }
    try {
      await client.sendNotification(method, params);
      return textResult("Notification sent.");
    } catch (error) {
      return errorResult(`Failed to send notification: ${String(error)}`);
    }
  }
}

export class STSemanticTokensFullTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<DiagnosticsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const uri = uriFromFilePath(options.input.filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    await ensureDocument(uri);
    const params = { textDocument: { uri: uri.toString() } };
    const result = await this.request(
      "textDocument/semanticTokens/full",
      params,
      token,
    );
    if ("error" in result) {
      return errorResult(result.error);
    }
    return textResult(JSON.stringify(summarizeSemanticTokens(result.response), null, 2));
  }
}

export class STSemanticTokensDeltaTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<SemanticTokensDeltaParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, previousResultId } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    if (!previousResultId.trim()) {
      return errorResult("previousResultId must be a non-empty string.");
    }
    await ensureDocument(uri);
    const params = {
      textDocument: { uri: uri.toString() },
      previousResultId,
    };
    const result = await this.request(
      "textDocument/semanticTokens/full/delta",
      params,
      token,
    );
    if ("error" in result) {
      return errorResult(result.error);
    }
    return textResult(JSON.stringify(summarizeSemanticTokens(result.response), null, 2));
  }
}

export class STSemanticTokensRangeTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<SemanticTokensRangeParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, startLine, startCharacter, endLine, endCharacter } =
      options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const doc = await ensureDocument(uri);
    const range = resolveRange(
      doc,
      startLine,
      startCharacter,
      endLine,
      endCharacter,
    );
    const params = {
      textDocument: { uri: uri.toString() },
      range: resolveLspRange(range),
    };
    const result = await this.request(
      "textDocument/semanticTokens/range",
      params,
      token,
    );
    if ("error" in result) {
      return errorResult(result.error);
    }
    return textResult(JSON.stringify(summarizeSemanticTokens(result.response), null, 2));
  }
}

export class STInlayHintsTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<InlayHintsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, startLine, startCharacter, endLine, endCharacter } =
      options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const doc = await ensureDocument(uri);
    const range = resolveRange(
      doc,
      startLine,
      startCharacter,
      endLine,
      endCharacter,
    );
    const params = { textDocument: { uri: uri.toString() }, range: resolveLspRange(range) };
    const result = await this.request("textDocument/inlayHint", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const hints = Array.isArray(result.response) ? result.response : [];
    const { items, truncated } = truncateItems(hints);
    const payload = items.map((hint) => ({
      position: hint.position
        ? `${hint.position.line + 1}:${hint.position.character + 1}`
        : undefined,
      label: hint.label ? inlayHintLabel(hint.label) : "",
      kind:
        typeof hint.kind === "number" ? vscode.InlayHintKind[hint.kind] : undefined,
      tooltip: hint.tooltip ? renderMarkup(hint.tooltip) : undefined,
      paddingLeft: hint.paddingLeft ?? false,
      paddingRight: hint.paddingRight ?? false,
    }));
    return textResult(JSON.stringify({ hints: payload, truncated }, null, 2));
  }
}

export class STLinkedEditingTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<LinkedEditingParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const doc = await ensureDocument(uri);
    const position = resolvePosition(doc, line, character);
    const params = {
      textDocument: { uri: uri.toString() },
      position: resolveLspPosition(position),
    };
    const result = await this.request("textDocument/linkedEditingRange", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const linked = result.response as { ranges?: any[]; wordPattern?: string } | null;
    if (!linked || !Array.isArray(linked.ranges)) {
      return textResult("No linked editing ranges returned.");
    }
    const ranges = linked.ranges.map((range) => formatLspRange(range));
    return textResult(
      JSON.stringify({ ranges, wordPattern: linked.wordPattern }, null, 2),
    );
  }
}

export class STDocumentLinksTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<DocumentLinksParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, resolve } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    await ensureDocument(uri);
    const params = { textDocument: { uri: uri.toString() } };
    const result = await this.request("textDocument/documentLink", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const links = Array.isArray(result.response) ? result.response : [];
    const { items, truncated } = truncateItems(links);
    const payload: Array<{
      range: string;
      target?: string;
      tooltip?: string;
    }> = [];
    for (const link of items) {
      let resolvedLink = link;
      if (resolve) {
        const resolved = await this.request("documentLink/resolve", link, token);
        if (!("error" in resolved)) {
          resolvedLink = resolved.response;
        }
      }
      payload.push({
        range: resolvedLink.range ? formatLspRange(resolvedLink.range) : "",
        target: resolvedLink.target ? formatUriString(resolvedLink.target) : undefined,
        tooltip: resolvedLink.tooltip ? renderMarkup(resolvedLink.tooltip) : undefined,
      });
    }
    return textResult(JSON.stringify({ links: payload, truncated }, null, 2));
  }
}

export class STCodeLensTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<CodeLensParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, resolve } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    await ensureDocument(uri);
    const params = { textDocument: { uri: uri.toString() } };
    const result = await this.request("textDocument/codeLens", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const lenses = Array.isArray(result.response) ? result.response : [];
    const { items, truncated } = truncateItems(lenses);
    const payload = [];
    for (const lens of items) {
      let resolvedLens = lens;
      if (resolve) {
        const resolved = await this.request("codeLens/resolve", lens, token);
        if (!("error" in resolved)) {
          resolvedLens = resolved.response;
        }
      }
      payload.push({
        range: resolvedLens.range ? formatLspRange(resolvedLens.range) : "",
        command: resolvedLens.command?.title ?? undefined,
      });
    }
    return textResult(JSON.stringify({ lenses: payload, truncated }, null, 2));
  }
}

export class STSelectionRangeTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<RangePositionsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, positions } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    if (!Array.isArray(positions) || positions.length === 0) {
      return errorResult("positions must be a non-empty array.");
    }
    const doc = await ensureDocument(uri);
    const lspPositions = positions.map((pos) =>
      resolveLspPosition(resolvePosition(doc, pos.line, pos.character)),
    );
    const params = { textDocument: { uri: uri.toString() }, positions: lspPositions };
    const result = await this.request("textDocument/selectionRange", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const ranges = Array.isArray(result.response) ? result.response : [];
    const payload = ranges.map((range) => {
      const chain: string[] = [];
      let current = range;
      while (current) {
        chain.push(formatLspRange(current.range));
        current = current.parent;
      }
      return chain;
    });
    return textResult(JSON.stringify({ ranges: payload }, null, 2));
  }
}

export class STOnTypeFormattingTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<OnTypeFormattingParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character, triggerCharacter } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    if (!triggerCharacter) {
      return errorResult("triggerCharacter must be provided.");
    }
    const doc = await ensureDocument(uri);
    const position = resolvePosition(doc, line, character);
    const editorConfig = vscode.workspace.getConfiguration("editor", uri);
    const formattingOptions = {
      insertSpaces: editorConfig.get<boolean>("insertSpaces", true),
      tabSize: editorConfig.get<number>("tabSize", 2),
    };
    const params = {
      textDocument: { uri: uri.toString() },
      position: resolveLspPosition(position),
      ch: triggerCharacter,
      options: formattingOptions,
    };
    const result = await this.request("textDocument/onTypeFormatting", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const edits = Array.isArray(result.response) ? result.response : [];
    const summary = summarizeLspTextEdits(edits);
    return textResult(JSON.stringify(summary, null, 2));
  }
}

export class STCallHierarchyPrepareTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const doc = await ensureDocument(uri);
    const position = resolvePosition(doc, line, character);
    const params = {
      textDocument: { uri: uri.toString() },
      position: resolveLspPosition(position),
    };
    const result = await this.request("textDocument/prepareCallHierarchy", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const items = Array.isArray(result.response) ? result.response : [];
    const payload = items.map((item) => ({
      name: item.name,
      kind: item.kind ? vscode.SymbolKind[item.kind] : undefined,
      uri: formatUriString(item.uri),
      range: item.range ? formatLspRange(item.range) : undefined,
      selectionRange: item.selectionRange
        ? formatLspRange(item.selectionRange)
        : undefined,
      detail: item.detail ?? undefined,
      item,
    }));
    return textResult(JSON.stringify({ items: payload }, null, 2));
  }
}

export class STCallHierarchyIncomingTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<{ item: unknown }>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { item } = options.input;
    if (!item) {
      return errorResult("item is required.");
    }
    const result = await this.request("callHierarchy/incomingCalls", { item }, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const calls = Array.isArray(result.response) ? result.response : [];
    const payload = calls.map((call) => ({
      from: call.from?.name,
      fromUri: call.from?.uri ? formatUriString(call.from.uri) : undefined,
      fromRange: call.from?.range ? formatLspRange(call.from.range) : undefined,
      fromRanges: Array.isArray(call.fromRanges)
        ? call.fromRanges.map(
            (range: { start: { line: number; character: number }; end: { line: number; character: number } }) =>
              formatLspRange(range),
          )
        : [],
      call,
    }));
    return textResult(JSON.stringify({ calls: payload }, null, 2));
  }
}

export class STCallHierarchyOutgoingTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<{ item: unknown }>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { item } = options.input;
    if (!item) {
      return errorResult("item is required.");
    }
    const result = await this.request("callHierarchy/outgoingCalls", { item }, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const calls = Array.isArray(result.response) ? result.response : [];
    const payload = calls.map((call) => ({
      to: call.to?.name,
      toUri: call.to?.uri ? formatUriString(call.to.uri) : undefined,
      toRange: call.to?.range ? formatLspRange(call.to.range) : undefined,
      fromRanges: Array.isArray(call.fromRanges)
        ? call.fromRanges.map(
            (range: { start: { line: number; character: number }; end: { line: number; character: number } }) =>
              formatLspRange(range),
          )
        : [],
      call,
    }));
    return textResult(JSON.stringify({ calls: payload }, null, 2));
  }
}

export class STTypeHierarchyPrepareTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<PositionParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, line, character } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const doc = await ensureDocument(uri);
    const position = resolvePosition(doc, line, character);
    const params = {
      textDocument: { uri: uri.toString() },
      position: resolveLspPosition(position),
    };
    const result = await this.request("textDocument/prepareTypeHierarchy", params, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const items = Array.isArray(result.response) ? result.response : [];
    const payload = items.map((item) => ({
      name: item.name,
      kind: item.kind ? vscode.SymbolKind[item.kind] : undefined,
      uri: formatUriString(item.uri),
      range: item.range ? formatLspRange(item.range) : undefined,
      selectionRange: item.selectionRange
        ? formatLspRange(item.selectionRange)
        : undefined,
      detail: item.detail ?? undefined,
      item,
    }));
    return textResult(JSON.stringify({ items: payload }, null, 2));
  }
}

export class STTypeHierarchySupertypesTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<{ item: unknown }>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { item } = options.input;
    if (!item) {
      return errorResult("item is required.");
    }
    const result = await this.request("typeHierarchy/supertypes", { item }, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const items = Array.isArray(result.response) ? result.response : [];
    const payload = items.map((entry) => ({
      name: entry.name,
      kind: entry.kind ? vscode.SymbolKind[entry.kind] : undefined,
      uri: formatUriString(entry.uri),
      range: entry.range ? formatLspRange(entry.range) : undefined,
      selectionRange: entry.selectionRange
        ? formatLspRange(entry.selectionRange)
        : undefined,
      detail: entry.detail ?? undefined,
      item: entry,
    }));
    return textResult(JSON.stringify({ items: payload }, null, 2));
  }
}

export class STTypeHierarchySubtypesTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<{ item: unknown }>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { item } = options.input;
    if (!item) {
      return errorResult("item is required.");
    }
    const result = await this.request("typeHierarchy/subtypes", { item }, token);
    if ("error" in result) {
      return errorResult(result.error);
    }
    const items = Array.isArray(result.response) ? result.response : [];
    const payload = items.map((entry) => ({
      name: entry.name,
      kind: entry.kind ? vscode.SymbolKind[entry.kind] : undefined,
      uri: formatUriString(entry.uri),
      range: entry.range ? formatLspRange(entry.range) : undefined,
      selectionRange: entry.selectionRange
        ? formatLspRange(entry.selectionRange)
        : undefined,
      detail: entry.detail ?? undefined,
      item: entry,
    }));
    return textResult(JSON.stringify({ items: payload }, null, 2));
  }
}

export class STWorkspaceRenameFileTool {
  async invoke(
    options: InvocationOptions<WorkspaceFileRenameParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { oldPath, newPath, overwrite, useWorkspaceEdit } = options.input;
    const oldUri = uriFromFilePath(oldPath);
    const newUri = uriFromFilePath(newPath);
    if (!oldUri || !newUri) {
      return errorResult("oldPath/newPath must be absolute paths.");
    }
    const oldWorkspaceError = ensureWorkspaceUri(oldUri);
    if (oldWorkspaceError) {
      return errorResult(oldWorkspaceError);
    }
    const newWorkspaceError = ensureWorkspaceUri(newUri);
    if (newWorkspaceError) {
      return errorResult(newWorkspaceError);
    }
    try {
      if (useWorkspaceEdit ?? true) {
        const edit = new vscode.WorkspaceEdit();
        edit.renameFile(oldUri, newUri, { overwrite: overwrite ?? false });
        await vscode.workspace.applyEdit(edit);
      } else {
        await vscode.workspace.fs.rename(oldUri, newUri, {
          overwrite: overwrite ?? false,
        });
      }
      return textResult("File rename/move applied.");
    } catch (error) {
      return errorResult(`Failed to rename/move file: ${String(error)}`);
    }
  }
}

export class STSettingsUpdateTool extends LspToolBase {
  constructor(getClient?: LspClientProvider) {
    super(getClient);
  }

  async invoke(
    options: InvocationOptions<SettingsToggleParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { key, value, scope, filePath, timeoutMs, forceRefresh } = options.input;
    if (!key.trim()) {
      return errorResult("key must be a non-empty string.");
    }
    const split = key.split(".");
    const section = split.length > 1 ? split[0] : "trust-lsp";
    const setting = split.length > 1 ? split.slice(1).join(".") : key;
    const target =
      scope === "global"
        ? vscode.ConfigurationTarget.Global
        : scope === "workspaceFolder"
          ? vscode.ConfigurationTarget.WorkspaceFolder
          : vscode.ConfigurationTarget.Workspace;
    let configurationTarget: vscode.ConfigurationTarget | undefined = target;
    let scopeUri: vscode.Uri | undefined;
    if (target === vscode.ConfigurationTarget.WorkspaceFolder) {
      if (!filePath) {
        return errorResult("filePath is required for workspaceFolder scope.");
      }
      scopeUri = uriFromFilePath(filePath);
      if (!scopeUri) {
        return errorResult("filePath must be an absolute path or URI.");
      }
    }
    try {
      const config = vscode.workspace.getConfiguration(section, scopeUri);
      await config.update(setting, value, configurationTarget);
      const effectiveTimeoutMs =
        typeof timeoutMs === "number" ? timeoutMs : forceRefresh ? 3000 : 1000;
      let targetUri: vscode.Uri | undefined;
      if (filePath) {
        targetUri = uriFromFilePath(filePath);
        if (!targetUri) {
          return errorResult("filePath must be an absolute path or URI.");
        }
      } else {
        targetUri = vscode.window.activeTextEditor?.document.uri;
      }
      if (!targetUri) {
        return textResult(
          JSON.stringify(
            {
              setting: `${section}.${setting}`,
              diagnosticsRefreshed: false,
              reason: "No active document available for diagnostics refresh.",
            },
            null,
            2,
          ),
        );
      }
      await ensureDocument(targetUri);
      let diagnosticsRefreshed = false;
      let pullDiagnostics: unknown | undefined;
      let refreshError: string | undefined;
      if (forceRefresh) {
        const result = await this.request(
          "textDocument/diagnostic",
          { textDocument: { uri: targetUri.toString() } },
          token,
          { requestTimeoutMs: effectiveTimeoutMs },
        );
        if ("error" in result) {
          refreshError = result.error;
        } else {
          pullDiagnostics = result.response;
          diagnosticsRefreshed = true;
        }
      }
      const waited = await waitForDiagnostics(
        targetUri,
        token,
        effectiveTimeoutMs,
      );
      diagnosticsRefreshed = diagnosticsRefreshed || waited;
      const diagnostics = vscode.languages.getDiagnostics(targetUri);
      return textResult(
        JSON.stringify(
          {
            setting: `${section}.${setting}`,
            diagnosticsRefreshed,
            diagnostics: diagnosticsPayload(diagnostics).diagnostics,
            pullDiagnostics: pullDiagnostics ?? undefined,
            refreshError: refreshError ?? undefined,
          },
          null,
          2,
        ),
      );
    } catch (error) {
      return errorResult(`Failed to update setting: ${String(error)}`);
    }
  }
}

export class STTelemetryReadTool {
  async invoke(
    options: InvocationOptions<TelemetryReadParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, limit, tail } = options.input;
    const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
    let uri: vscode.Uri | undefined = undefined;
    if (filePath) {
      uri = uriFromFilePath(filePath);
      if (!uri) {
        return errorResult("filePath must be an absolute path or URI.");
      }
    } else {
      for (const folder of workspaceFolders) {
        const candidate = vscode.Uri.joinPath(
          folder.uri,
          ".trust-lsp",
          "telemetry.jsonl",
        );
        try {
          await vscode.workspace.fs.stat(candidate);
          uri = candidate;
          break;
        } catch {
          continue;
        }
      }
    }
    if (!uri) {
      return errorResult("Telemetry file not found.");
    }
    const workspaceError = ensureWorkspaceUri(uri);
    if (workspaceError) {
      return errorResult(workspaceError);
    }
    try {
      const bytes = await vscode.workspace.fs.readFile(uri);
      const text = new TextDecoder().decode(bytes);
      const lines = text.split(/\r?\n/).filter((line) => line.trim().length > 0);
      const maxItems = limit ?? 100;
      const slice = tail ? lines.slice(-maxItems) : lines.slice(0, maxItems);
      const items = slice.map((line) => {
        try {
          return JSON.parse(line);
        } catch {
          return { parseError: true, line };
        }
      });
      return textResult(
        JSON.stringify(
          {
            filePath: uri.fsPath,
            totalLines: lines.length,
            items,
            truncated: lines.length > slice.length,
          },
          null,
          2,
        ),
      );
    } catch (error) {
      return errorResult(`Failed to read telemetry: ${String(error)}`);
    }
  }
}

export class STInlineValuesTool {
  async invoke(
    options: InvocationOptions<InlineValuesParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const {
      frameId,
      startLine,
      startCharacter,
      endLine,
      endCharacter,
      context,
    } = options.input;
    if (!Number.isInteger(frameId)) {
      return errorResult("frameId must be an integer.");
    }
    const session = vscode.debug.activeDebugSession;
    if (!session) {
      return errorResult("No active debug session.");
    }
    const range = {
      start: { line: startLine + 1, column: startCharacter + 1 },
      end: { line: endLine + 1, column: endCharacter + 1 },
    };
    try {
      const inlineValues = await session.customRequest("inlineValues", {
        frameId,
        range,
        context,
      });
      return textResult(
        JSON.stringify({ inlineValues, session: session.name }, null, 2),
      );
    } catch (error) {
      return errorResult(`Failed to fetch inline values: ${String(error)}`);
    }
  }
}

export class STProjectInfoTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<ProjectInfoParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const args = Array.isArray(options.input.arguments)
      ? options.input.arguments
      : [];
    const result = await this.request(
      "workspace/executeCommand",
      { command: "trust-lsp.projectInfo", arguments: args },
      token,
    );
    if ("error" in result) {
      return errorResult(result.error);
    }
    return textResult(
      JSON.stringify(
        {
          command: "trust-lsp.projectInfo",
          result: result.response,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiGetBindingsTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiBindingsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }

    const args: Record<string, unknown> = {};
    if (options.input.rootPath && options.input.rootPath.trim()) {
      const uri = uriFromFilePath(options.input.rootPath.trim());
      if (!uri) {
        return errorResult("rootPath must be an absolute path or URI.");
      }
      const workspaceError = ensureWorkspaceUri(uri);
      if (workspaceError) {
        return errorResult(workspaceError);
      }
      args.root_uri = uri.toString();
    }
    if (options.input.filePath && options.input.filePath.trim()) {
      const uri = uriFromFilePath(options.input.filePath.trim());
      if (!uri) {
        return errorResult("filePath must be an absolute path or URI.");
      }
      const workspaceError = ensureWorkspaceUri(uri);
      if (workspaceError) {
        return errorResult(workspaceError);
      }
      args.text_document = { uri: uri.toString() };
    }

    const result = await this.request(
      "workspace/executeCommand",
      {
        command: "trust-lsp.hmiBindings",
        arguments: Object.keys(args).length > 0 ? [args] : [],
      },
      token,
    );
    if ("error" in result) {
      return errorResult(result.error);
    }
    const response = result.response as { ok?: boolean; error?: unknown } | null;
    if (
      response &&
      typeof response === "object" &&
      response.ok === false
    ) {
      const message =
        typeof response.error === "string"
          ? response.error
          : "trust-lsp.hmiBindings failed.";
      return errorResult(message);
    }
    return textResult(
      JSON.stringify(
        {
          command: "trust-lsp.hmiBindings",
          result: result.response,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiGetLayoutTool {
  async invoke(
    options: InvocationOptions<HmiGetLayoutParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot) {
      return errorResult("Unable to read HMI layout.");
    }
    return textResult(
      JSON.stringify(
        snapshot.exists
          ? snapshot
          : {
              exists: false,
              rootPath: snapshot.rootPath,
              hmiPath: snapshot.hmiPath,
            },
        null,
        2,
      ),
    );
  }
}

export class STHmiApplyPatchTool {
  async invoke(
    options: InvocationOptions<HmiApplyPatchParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    if (!Array.isArray(options.input.operations) || options.input.operations.length === 0) {
      return errorResult("operations must be a non-empty array.");
    }

    const resolved = resolveWorkspaceFolder(options.input.rootPath);
    if (resolved.error || !resolved.folder) {
      return errorResult(resolved.error ?? "Unable to resolve workspace folder.");
    }
    const dryRun = options.input.dry_run === true;
    const hmiRoot = vscode.Uri.joinPath(resolved.folder.uri, "hmi");

    const currentFiles = new Map<string, string>();
    try {
      const entries = await vscode.workspace.fs.readDirectory(hmiRoot);
      for (const [name, kind] of entries) {
        if (kind !== vscode.FileType.File || !name.toLowerCase().endsWith(".toml")) {
          continue;
        }
        const content = await vscode.workspace.fs.readFile(vscode.Uri.joinPath(hmiRoot, name));
        currentFiles.set(name, Buffer.from(content).toString("utf8"));
      }
    } catch {
      // hmi/ may not exist yet; this is valid for patch application.
    }

    const nextFiles = new Map(currentFiles);
    const conflicts: Array<{
      code: string;
      index: number;
      path?: string;
      message: string;
    }> = [];

    for (const [index, operation] of options.input.operations.entries()) {
      if (token.isCancellationRequested) {
        return textResult("Cancelled.");
      }
      const op = operation?.op;
      if (!op || !["add", "remove", "replace", "move"].includes(op)) {
        conflicts.push({
          code: "HMI_PATCH_INVALID_OP",
          index,
          message: "operation.op must be one of add/remove/replace/move",
        });
        continue;
      }
      const target = hmiDescriptorFileFromPointer(String(operation.path ?? ""));
      if (!target) {
        conflicts.push({
          code: "HMI_PATCH_INVALID_PATH",
          index,
          path: String(operation.path ?? ""),
          message: "path must target /files/<name>.toml or /files/<name>.toml/content",
        });
        continue;
      }

      if (op === "add" || op === "replace") {
        if (typeof operation.value !== "string") {
          conflicts.push({
            code: "HMI_PATCH_TYPE_MISMATCH",
            index,
            path: String(operation.path ?? ""),
            message: "add/replace requires a string value containing TOML content",
          });
          continue;
        }
        if (op === "add" && nextFiles.has(target)) {
          conflicts.push({
            code: "HMI_PATCH_CONFLICT_EXISTS",
            index,
            path: String(operation.path ?? ""),
            message: `target file '${target}' already exists`,
          });
          continue;
        }
        if (op === "replace" && !nextFiles.has(target)) {
          conflicts.push({
            code: "HMI_PATCH_NOT_FOUND",
            index,
            path: String(operation.path ?? ""),
            message: `target file '${target}' does not exist`,
          });
          continue;
        }
        nextFiles.set(target, operation.value);
        continue;
      }

      if (op === "remove") {
        if (!nextFiles.has(target)) {
          conflicts.push({
            code: "HMI_PATCH_NOT_FOUND",
            index,
            path: String(operation.path ?? ""),
            message: `target file '${target}' does not exist`,
          });
          continue;
        }
        nextFiles.delete(target);
        continue;
      }

      const from = hmiDescriptorFileFromPointer(String(operation.from ?? ""));
      if (!from) {
        conflicts.push({
          code: "HMI_PATCH_INVALID_FROM",
          index,
          path: String(operation.from ?? ""),
          message: "move requires a valid from pointer",
        });
        continue;
      }
      const sourceText = nextFiles.get(from);
      if (sourceText === undefined) {
        conflicts.push({
          code: "HMI_PATCH_NOT_FOUND",
          index,
          path: String(operation.from ?? ""),
          message: `source file '${from}' does not exist`,
        });
        continue;
      }
      if (nextFiles.has(target)) {
        conflicts.push({
          code: "HMI_PATCH_CONFLICT_EXISTS",
          index,
          path: String(operation.path ?? ""),
          message: `target file '${target}' already exists`,
        });
        continue;
      }
      nextFiles.delete(from);
      nextFiles.set(target, sourceText);
    }

    const changedFiles: Array<{ file: string; action: "add" | "replace" | "remove" }> = [];
    const names = new Set<string>([...currentFiles.keys(), ...nextFiles.keys()]);
    for (const name of Array.from(names.values()).sort((left, right) => left.localeCompare(right))) {
      const before = currentFiles.get(name);
      const after = nextFiles.get(name);
      if (before === undefined && after !== undefined) {
        changedFiles.push({ file: path.posix.join("hmi", name), action: "add" });
      } else if (before !== undefined && after === undefined) {
        changedFiles.push({ file: path.posix.join("hmi", name), action: "remove" });
      } else if (before !== undefined && after !== undefined && before !== after) {
        changedFiles.push({ file: path.posix.join("hmi", name), action: "replace" });
      }
    }

    if (dryRun || conflicts.length > 0) {
      return textResult(
        JSON.stringify(
          {
            ok: conflicts.length === 0,
            dry_run: dryRun,
            rootPath: resolved.folder.uri.fsPath,
            conflicts,
            changes: changedFiles,
          },
          null,
          2,
        ),
      );
    }

    await vscode.workspace.fs.createDirectory(hmiRoot);
    for (const change of changedFiles) {
      if (token.isCancellationRequested) {
        return textResult("Cancelled.");
      }
      const fileName = change.file.slice("hmi/".length);
      const fileUri = vscode.Uri.joinPath(hmiRoot, fileName);
      if (change.action === "remove") {
        try {
          await vscode.workspace.fs.delete(fileUri, { useTrash: false });
        } catch {
          // Ignore missing files during remove reconciliation.
        }
        continue;
      }
      const text = nextFiles.get(fileName) ?? "";
      await vscode.workspace.fs.writeFile(fileUri, Buffer.from(text, "utf8"));
    }

    try {
      await vscode.commands.executeCommand("trust-lsp.hmi.refreshFromDescriptor");
    } catch {
      // Optional refresh command; ignore failures.
    }

    return textResult(
      JSON.stringify(
        {
          ok: true,
          dry_run: false,
          rootPath: resolved.folder.uri.fsPath,
          conflicts: [],
          changes: changedFiles,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiPlanIntentTool {
  async invoke(
    options: InvocationOptions<HmiPlanIntentParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot) {
      return errorResult("Unable to resolve HMI workspace.");
    }
    const dryRun = options.input.dry_run === true;
    const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
    const intentUri = vscode.Uri.joinPath(hmiRoot, "_intent.toml");
    const content = renderIntentToml(options.input);

    let previous = "";
    let existed = false;
    try {
      const bytes = await vscode.workspace.fs.readFile(intentUri);
      previous = Buffer.from(bytes).toString("utf8");
      existed = true;
    } catch {
      existed = false;
    }

    const changed = previous !== content;
    if (!dryRun && changed) {
      await vscode.workspace.fs.createDirectory(hmiRoot);
      await writeUtf8File(intentUri, content);
    }

    return textResult(
      JSON.stringify(
        {
          ok: true,
          dry_run: dryRun,
          rootPath: snapshot.rootPath,
          intentPath: path.posix.join("hmi", "_intent.toml"),
          existed,
          changed,
          content,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiValidateTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiValidateParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot) {
      return errorResult("Unable to resolve HMI workspace.");
    }
    if (!snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }

    const dryRun = options.input.dry_run === true;
    const prune = options.input.prune === true;
    const retainRuns = Number.isInteger(options.input.retain_runs)
      ? Math.max(1, Number(options.input.retain_runs))
      : 10;
    const checks: HmiValidationCheck[] = [];
    const layoutRefs = extractLayoutBindingRefs(snapshot.pages);
    if (layoutRefs.length === 0) {
      checks.push({
        code: "HMI_VALIDATE_LAYOUT_NO_BINDS",
        severity: "warning",
        message: "No bind/source entries were found in hmi page files.",
      });
    }

    const writePolicy = parseWritePolicyFromConfigToml(snapshot.config?.content);
    if (writePolicy.enabled && writePolicy.allow.length === 0) {
      checks.push({
        code: "HMI_VALIDATE_WRITE_ALLOWLIST_EMPTY",
        severity: "error",
        file: "hmi/_config.toml",
        message:
          "[write].enabled is true but no allowlist entries were found in hmi/_config.toml.",
      });
    }
    for (const target of writePolicy.allow) {
      if (!target.startsWith("resource/")) {
        checks.push({
          code: "HMI_VALIDATE_WRITE_ALLOW_NON_CANONICAL",
          severity: "warning",
          file: "hmi/_config.toml",
          message: `Write allowlist target '${target}' is not canonical (expected resource/... identifier).`,
        });
      }
    }

    const pollMs = vscode.workspace
      .getConfiguration("trust-lsp", vscode.Uri.file(snapshot.rootPath))
      .get<number>("hmi.pollIntervalMs", 500);
    if (pollMs < 50) {
      checks.push({
        code: "HMI_VALIDATE_POLL_INTERVAL_TOO_LOW",
        severity: "warning",
        message: `Configured poll interval (${pollMs}ms) is below the recommended lower bound (50ms).`,
      });
    } else if (pollMs > 1000) {
      checks.push({
        code: "HMI_VALIDATE_POLL_INTERVAL_TOO_HIGH",
        severity: "warning",
        message: `Configured poll interval (${pollMs}ms) exceeds the recommended upper bound (1000ms).`,
      });
    }

    let catalog = normalizeHmiBindingsCatalog({});
    let catalogAvailable = false;
    const bindingsRequest = await this.request(
      "workspace/executeCommand",
      {
        command: "trust-lsp.hmiBindings",
        arguments: [{ root_uri: vscode.Uri.file(snapshot.rootPath).toString() }],
      },
      token,
    );
    if ("error" in bindingsRequest) {
      checks.push({
        code: "HMI_VALIDATE_BINDINGS_UNAVAILABLE",
        severity: "warning",
        message: `Unable to load binding catalog from trust-lsp.hmiBindings: ${bindingsRequest.error}`,
      });
    } else {
      const payload =
        bindingsRequest.response && typeof bindingsRequest.response === "object"
          ? (bindingsRequest.response as Record<string, unknown>)
          : {};
      if (payload.ok === false) {
        checks.push({
          code: "HMI_VALIDATE_BINDINGS_UNAVAILABLE",
          severity: "warning",
          message: `trust-lsp.hmiBindings failed: ${String(payload.error ?? "unknown error")}`,
        });
      } else {
        catalog = normalizeHmiBindingsCatalog(payload);
        catalogAvailable = true;
      }
    }

    const lock = buildHmiLockEntries(layoutRefs, catalog);
    for (const unknownPath of lock.unknownPaths) {
      checks.push({
        code: "HMI_VALIDATE_UNKNOWN_BIND_PATH",
        severity: catalogAvailable ? "error" : "warning",
        message: `Binding path '${unknownPath}' is not present in the current binding catalog.`,
      });
    }

    const diagnosticChecks = await collectHmiDiagnosticsForFiles(
      snapshot.rootPath,
      snapshot.files,
      token,
    );
    checks.push(...diagnosticChecks);
    checks.sort((left, right) => {
      const severity = hmiSeverityRank(left.severity) - hmiSeverityRank(right.severity);
      if (severity !== 0) {
        return severity;
      }
      if ((left.file ?? "") !== (right.file ?? "")) {
        return (left.file ?? "").localeCompare(right.file ?? "");
      }
      if (left.code !== right.code) {
        return left.code.localeCompare(right.code);
      }
      return left.message.localeCompare(right.message);
    });

    const errors = checks.filter((check) => check.severity === "error").length;
    const warnings = checks.filter((check) => check.severity === "warning").length;
    const infos = checks.filter((check) => check.severity === "info").length;
    const ok = errors === 0;

    const lockDocument = {
      version: 1,
      widgets: lock.entries,
    };
    const lockContent = `${stableJsonString(lockDocument)}\n`;
    const generatedAt = new Date();
    const validationDocument = {
      version: 1,
      generated_at: generatedAt.toISOString(),
      ok,
      root_path: snapshot.rootPath,
      hmi_path: snapshot.hmiPath,
      counts: {
        errors,
        warnings,
        infos,
      },
      checks,
    };
    const journeysDocument = {
      version: 1,
      generated_at: generatedAt.toISOString(),
      journeys: [],
      note: "No journey scenarios executed in validate-only run.",
    };

    const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
    const lockUri = vscode.Uri.joinPath(hmiRoot, "_lock.json");
    let evidencePath: string | null = null;
    let prunedRuns: string[] = [];
    if (!dryRun) {
      await vscode.workspace.fs.createDirectory(hmiRoot);
      await writeUtf8File(lockUri, lockContent);

      const runId = evidenceRunId(generatedAt);
      const evidenceRoot = vscode.Uri.joinPath(hmiRoot, "_evidence");
      const runRoot = vscode.Uri.joinPath(evidenceRoot, runId);
      await vscode.workspace.fs.createDirectory(runRoot);
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, "validation.json"),
        `${stableJsonString(validationDocument)}\n`,
      );
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, "journeys.json"),
        `${stableJsonString(journeysDocument)}\n`,
      );
      evidencePath = path.posix.join("hmi", "_evidence", runId);
      if (prune) {
        prunedRuns = await pruneEvidenceRuns(hmiRoot, retainRuns);
      }
    }

    return textResult(
      JSON.stringify(
        {
          ok,
          dry_run: dryRun,
          prune,
          retain_runs: retainRuns,
          rootPath: snapshot.rootPath,
          lockPath: path.posix.join("hmi", "_lock.json"),
          evidencePath,
          prunedRuns,
          checks,
          counts: { errors, warnings, infos },
          widgetCount: lock.entries.length,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiTraceCaptureTool {
  async invoke(
    options: InvocationOptions<HmiTraceCaptureParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot || !snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }
    let schemaPayload: HmiSchemaResult;
    try {
      const schemaRaw = await requestRuntimeControl(
        snapshot.rootPath,
        token,
        "hmi.schema.get",
        undefined,
      );
      const parsed = parseHmiSchemaPayload(schemaRaw);
      if (!parsed) {
        return errorResult("runtime returned an invalid hmi.schema.get payload.");
      }
      schemaPayload = parsed;
    } catch (error) {
      return errorResult(`Failed to capture trace schema: ${String(error)}`);
    }

    const dryRun = options.input.dry_run === true;
    const scenario = normalizeScenario(options.input.scenario);
    const sampleCount = coerceInt(options.input.samples, 4, 1, 50);
    const sampleIntervalMs = coerceInt(options.input.sample_interval_ms, 200, 10, 5000);
    const ids = normalizeTraceIds(options.input.ids, schemaPayload);
    if (ids.length === 0) {
      return errorResult("No widget IDs available for trace capture.");
    }

    const startedAt = new Date();
    const samples: Array<{
      index: number;
      timestamp_ms: number;
      connected: boolean;
      values: Record<string, unknown>;
      qualities: Record<string, string>;
      error?: string;
    }> = [];
    for (let index = 0; index < sampleCount; index += 1) {
      if (token.isCancellationRequested) {
        return textResult("Cancelled.");
      }
      try {
        const valuesRaw = await requestRuntimeControl(
          snapshot.rootPath,
          token,
          "hmi.values.get",
          { ids },
        );
        const parsedValues = parseHmiValuesPayload(valuesRaw);
        if (!parsedValues) {
          throw new Error("runtime returned an invalid hmi.values.get payload.");
        }
        const valueSnapshot: Record<string, unknown> = {};
        const qualitySnapshot: Record<string, string> = {};
        for (const widgetId of ids) {
          const entry = parsedValues.values[widgetId];
          if (!entry) {
            continue;
          }
          valueSnapshot[widgetId] = entry.v;
          qualitySnapshot[widgetId] = entry.q;
        }
        samples.push({
          index: index + 1,
          timestamp_ms: parsedValues.timestamp_ms,
          connected: parsedValues.connected,
          values: valueSnapshot,
          qualities: qualitySnapshot,
        });
      } catch (error) {
        samples.push({
          index: index + 1,
          timestamp_ms: Date.now(),
          connected: false,
          values: {},
          qualities: {},
          error: String(error),
        });
      }

      if (index + 1 < sampleCount) {
        const slept = await sleepWithCancellation(sampleIntervalMs, token);
        if (!slept) {
          return textResult("Cancelled.");
        }
      }
    }

    const runId = normalizeEvidenceRunId(options.input.run_id) ?? evidenceRunId(startedAt);
    const traceDocument = {
      version: 1,
      generated_at: startedAt.toISOString(),
      scenario,
      ids,
      sample_interval_ms: sampleIntervalMs,
      samples,
    };

    let evidencePath: string | null = null;
    let tracePath: string | null = null;
    if (!dryRun) {
      const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
      const runRoot = vscode.Uri.joinPath(hmiRoot, "_evidence", runId);
      await vscode.workspace.fs.createDirectory(runRoot);
      const fileName = `trace-${scenario}.json`;
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, fileName),
        `${stableJsonString(traceDocument)}\n`,
      );
      evidencePath = path.posix.join("hmi", "_evidence", runId);
      tracePath = path.posix.join(evidencePath, fileName);
    }

    return textResult(
      JSON.stringify(
        {
          ok: true,
          dry_run: dryRun,
          rootPath: snapshot.rootPath,
          scenario,
          run_id: runId,
          evidencePath,
          tracePath,
          counts: {
            requested_samples: sampleCount,
            captured_samples: samples.length,
            error_samples: samples.filter((sample) => !!sample.error).length,
          },
          ids,
          samples,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiGenerateCandidatesTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiGenerateCandidatesParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot || !snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }

    const descriptorPages = layoutDescriptorPages(snapshot.files);
    const refs = extractLayoutBindingRefs(descriptorPages);
    let catalog = normalizeHmiBindingsCatalog({});
    let catalogAvailable = false;
    let catalogError: string | undefined;
    const bindingsRequest = await this.request(
      "workspace/executeCommand",
      {
        command: "trust-lsp.hmiBindings",
        arguments: [{ root_uri: vscode.Uri.file(snapshot.rootPath).toString() }],
      },
      token,
    );
    if ("error" in bindingsRequest) {
      catalogError = bindingsRequest.error;
    } else {
      const payload =
        bindingsRequest.response && typeof bindingsRequest.response === "object"
          ? (bindingsRequest.response as Record<string, unknown>)
          : {};
      if (payload.ok === false) {
        catalogError = String(payload.error ?? "unknown error");
      } else {
        catalog = normalizeHmiBindingsCatalog(payload);
        catalogAvailable = true;
      }
    }

    const intentContent = snapshot.files.find((file) => file.name === "_intent.toml")?.content;
    const candidateCount = coerceInt(
      options.input.candidate_count,
      3,
      1,
      HMI_CANDIDATE_STRATEGIES.length,
    );
    const candidates = generateHmiCandidates(
      refs,
      catalog,
      intentContent,
      candidateCount,
    );
    const generatedAt = new Date();
    const runId = normalizeEvidenceRunId(options.input.run_id) ?? evidenceRunId(generatedAt);
    const dryRun = options.input.dry_run === true;
    const candidateDocument = {
      version: 1,
      generated_at: generatedAt.toISOString(),
      intent_priorities: intentContent
        ? parseQuotedArrayFromToml(intentContent, "priorities")
        : [],
      candidates,
    };

    let evidencePath: string | null = null;
    let candidatesPath: string | null = null;
    if (!dryRun) {
      const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
      const runRoot = vscode.Uri.joinPath(hmiRoot, "_evidence", runId);
      await vscode.workspace.fs.createDirectory(runRoot);
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, "candidates.json"),
        `${stableJsonString(candidateDocument)}\n`,
      );
      evidencePath = path.posix.join("hmi", "_evidence", runId);
      candidatesPath = path.posix.join(evidencePath, "candidates.json");
    }

    return textResult(
      JSON.stringify(
        {
          ok: true,
          dry_run: dryRun,
          rootPath: snapshot.rootPath,
          run_id: runId,
          evidencePath,
          candidatesPath,
          catalogAvailable,
          catalogError,
          candidates,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiPreviewSnapshotTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiPreviewSnapshotParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot || !snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }

    const dryRun = options.input.dry_run === true;
    const requestedRunId = normalizeEvidenceRunId(options.input.run_id);
    let runId = requestedRunId;
    let candidates: HmiCandidate[] = [];
    if (requestedRunId) {
      const candidatesUri = vscode.Uri.joinPath(
        vscode.Uri.file(snapshot.hmiPath),
        "_evidence",
        requestedRunId,
        "candidates.json",
      );
      try {
        const bytes = await vscode.workspace.fs.readFile(candidatesUri);
        const payload = JSON.parse(Buffer.from(bytes).toString("utf8")) as {
          candidates?: unknown[];
        };
        if (Array.isArray(payload.candidates)) {
          candidates = payload.candidates
            .map((entry) => {
              const record = asRecord(entry);
              if (!record || typeof record.id !== "string") {
                return undefined;
              }
              const strategy = asRecord(record.strategy);
              const metrics = asRecord(record.metrics);
              const preview = asRecord(record.preview);
              if (!strategy || !metrics || !preview || !Array.isArray(preview.sections)) {
                return undefined;
              }
              return {
                id: record.id,
                rank:
                  typeof record.rank === "number" && Number.isFinite(record.rank)
                    ? record.rank
                    : 0,
                strategy: {
                  id: typeof strategy.id === "string" ? strategy.id : "loaded",
                  grouping:
                    strategy.grouping === "qualifier" ||
                    strategy.grouping === "path"
                      ? strategy.grouping
                      : "program",
                  density:
                    strategy.density === "compact" || strategy.density === "spacious"
                      ? strategy.density
                      : "balanced",
                  widget_bias:
                    strategy.widget_bias === "status_first" ||
                    strategy.widget_bias === "trend_first"
                      ? strategy.widget_bias
                      : "balanced",
                  alarm_emphasis: strategy.alarm_emphasis === true,
                },
                metrics: {
                  readability:
                    typeof metrics.readability === "number" ? metrics.readability : 0,
                  action_latency:
                    typeof metrics.action_latency === "number"
                      ? metrics.action_latency
                      : 0,
                  alarm_salience:
                    typeof metrics.alarm_salience === "number"
                      ? metrics.alarm_salience
                      : 0,
                  overall: typeof metrics.overall === "number" ? metrics.overall : 0,
                },
                summary: {
                  bindings:
                    typeof asRecord(record.summary)?.bindings === "number"
                      ? (asRecord(record.summary)?.bindings as number)
                      : 0,
                  sections:
                    typeof asRecord(record.summary)?.sections === "number"
                      ? (asRecord(record.summary)?.sections as number)
                      : 0,
                },
                preview: {
                  title: typeof preview.title === "string" ? preview.title : "Candidate",
                  sections: preview.sections
                    .map((section) => {
                      const sectionRecord = asRecord(section);
                      if (!sectionRecord || typeof sectionRecord.title !== "string") {
                        return undefined;
                      }
                      const widgetIds = Array.isArray(sectionRecord.widget_ids)
                        ? sectionRecord.widget_ids.filter(
                            (value): value is string => typeof value === "string",
                          )
                        : [];
                      return {
                        title: sectionRecord.title,
                        widget_ids: widgetIds.sort((left, right) =>
                          left.localeCompare(right),
                        ),
                      };
                    })
                    .filter(
                      (
                        section,
                      ): section is { title: string; widget_ids: string[] } => !!section,
                    ),
                },
              } as HmiCandidate;
            })
            .filter((entry): entry is HmiCandidate => !!entry)
            .sort((left, right) => left.rank - right.rank);
        }
      } catch {
        candidates = [];
      }
    }

    if (candidates.length === 0) {
      const descriptorPages = layoutDescriptorPages(snapshot.files);
      const refs = extractLayoutBindingRefs(descriptorPages);
      let catalog = normalizeHmiBindingsCatalog({});
      const bindingsRequest = await this.request(
        "workspace/executeCommand",
        {
          command: "trust-lsp.hmiBindings",
          arguments: [{ root_uri: vscode.Uri.file(snapshot.rootPath).toString() }],
        },
        token,
      );
      if (!("error" in bindingsRequest)) {
        const payload =
          bindingsRequest.response && typeof bindingsRequest.response === "object"
            ? (bindingsRequest.response as Record<string, unknown>)
            : {};
        if (payload.ok !== false) {
          catalog = normalizeHmiBindingsCatalog(payload);
        }
      }
      const intentContent = snapshot.files.find((file) => file.name === "_intent.toml")?.content;
      candidates = generateHmiCandidates(refs, catalog, intentContent, 3);
    }

    if (candidates.length === 0) {
      return errorResult("No candidate layouts are available for snapshot rendering.");
    }
    const selectedCandidate =
      (options.input.candidate_id
        ? candidates.find((candidate) => candidate.id === options.input.candidate_id)
        : undefined) ?? candidates[0];
    const viewports = normalizeSnapshotViewports(options.input.viewports);
    const generatedAt = new Date();
    runId = runId ?? evidenceRunId(generatedAt);

    const snapshots = viewports.map((viewport) => {
      const svg = renderSnapshotSvg(viewport, selectedCandidate);
      return {
        viewport,
        fileName: `${viewport}-overview.svg`,
        content: svg,
        hash: hashContent(svg),
        bytes: Buffer.byteLength(svg, "utf8"),
      };
    });

    let evidencePath: string | null = null;
    const files: Array<{ viewport: SnapshotViewport; path: string; hash: string; bytes: number }> = [];
    if (!dryRun) {
      const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
      const screenshotRoot = vscode.Uri.joinPath(
        hmiRoot,
        "_evidence",
        runId,
        "screenshots",
      );
      await vscode.workspace.fs.createDirectory(screenshotRoot);
      for (const snapshotEntry of snapshots) {
        await writeUtf8File(
          vscode.Uri.joinPath(screenshotRoot, snapshotEntry.fileName),
          snapshotEntry.content,
        );
        files.push({
          viewport: snapshotEntry.viewport,
          path: path.posix.join(
            "hmi",
            "_evidence",
            runId,
            "screenshots",
            snapshotEntry.fileName,
          ),
          hash: snapshotEntry.hash,
          bytes: snapshotEntry.bytes,
        });
      }
      evidencePath = path.posix.join("hmi", "_evidence", runId);
    } else {
      for (const snapshotEntry of snapshots) {
        files.push({
          viewport: snapshotEntry.viewport,
          path: path.posix.join(
            "hmi",
            "_evidence",
            runId,
            "screenshots",
            snapshotEntry.fileName,
          ),
          hash: snapshotEntry.hash,
          bytes: snapshotEntry.bytes,
        });
      }
    }

    return textResult(
      JSON.stringify(
        {
          ok: true,
          dry_run: dryRun,
          rootPath: snapshot.rootPath,
          run_id: runId,
          evidencePath,
          candidate_id: selectedCandidate.id,
          files,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiRunJourneyTool {
  async invoke(
    options: InvocationOptions<HmiRunJourneyParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot || !snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }

    let schema: HmiSchemaResult;
    try {
      const schemaRaw = await requestRuntimeControl(
        snapshot.rootPath,
        token,
        "hmi.schema.get",
        undefined,
      );
      const parsedSchema = parseHmiSchemaPayload(schemaRaw);
      if (!parsedSchema) {
        return errorResult("runtime returned an invalid hmi.schema.get payload.");
      }
      schema = parsedSchema;
    } catch (error) {
      return errorResult(`Failed to execute journey schema load: ${String(error)}`);
    }

    const defaultIds = normalizeTraceIds(undefined, schema).slice(0, 5);
    const schemaWidgetById = new Map(
      schema.widgets.map((widget) => [widget.id, widget] as const),
    );
    const writePolicy = parseWritePolicyFromConfigToml(snapshot.config?.content);
    const writeAllow = new Set(writePolicy.allow);

    const localWriteGuard = (
      widgetId: string,
    ): { code: string; message: string } | undefined => {
      if (schema.read_only) {
        return {
          code: "HMI_JOURNEY_WRITE_READ_ONLY",
          message: "hmi.write is disabled because runtime schema is read-only.",
        };
      }
      if (!writePolicy.enabled) {
        return {
          code: "HMI_JOURNEY_WRITE_DISABLED",
          message: "hmi.write is disabled in hmi/_config.toml.",
        };
      }
      if (writeAllow.size === 0) {
        return {
          code: "HMI_JOURNEY_WRITE_ALLOWLIST_EMPTY",
          message: "hmi.write allowlist is empty in hmi/_config.toml.",
        };
      }
      const widgetPath = schemaWidgetById.get(widgetId)?.path;
      const allowlisted = writeAllow.has(widgetId) || (widgetPath ? writeAllow.has(widgetPath) : false);
      if (!allowlisted) {
        return {
          code: "HMI_JOURNEY_WRITE_NOT_ALLOWLISTED",
          message: `write target '${widgetId}' is not in tool-side allowlist checks`,
        };
      }
      return undefined;
    };

    const requestedJourneys = Array.isArray(options.input.journeys)
      ? options.input.journeys
      : [];
    const journeys = requestedJourneys
      .map((journey, index) => {
        const id = typeof journey.id === "string" && journey.id.trim()
          ? stableComponent(journey.id)
          : `journey-${index + 1}`;
        const title =
          typeof journey.title === "string" && journey.title.trim()
            ? journey.title.trim()
            : `Journey ${index + 1}`;
        const maxDurationMs = coerceInt(journey.max_duration_ms, 60000, 100, 300000);
        const steps =
          Array.isArray(journey.steps) && journey.steps.length > 0
            ? journey.steps
            : [{ action: "read_values" as const, ids: defaultIds }];
        return {
          id,
          title,
          max_duration_ms: maxDurationMs,
          steps,
        };
      })
      .filter((journey) => journey.steps.length > 0);
    if (journeys.length === 0) {
      journeys.push({
        id: "default",
        title: "Default value fetch journey",
        max_duration_ms: 60000,
        steps: [{ action: "read_values", ids: defaultIds }],
      });
    }

    const dryRun = options.input.dry_run === true;
    const scenario = normalizeScenario(options.input.scenario);
    const generatedAt = new Date();
    const runId = normalizeEvidenceRunId(options.input.run_id) ?? evidenceRunId(generatedAt);
    const results: Array<{
      id: string;
      title: string;
      status: "passed" | "failed";
      duration_ms: number;
      api_actions: number;
      steps: Array<{
        index: number;
        action: HmiJourneyAction;
        status: "passed" | "failed";
        duration_ms: number;
        code?: string;
        detail?: string;
      }>;
    }> = [];
    for (const journey of journeys) {
      if (token.isCancellationRequested) {
        return textResult("Cancelled.");
      }
      const stepResults: Array<{
        index: number;
        action: HmiJourneyAction;
        status: "passed" | "failed";
        duration_ms: number;
        code?: string;
        detail?: string;
      }> = [];
      let apiActions = 0;
      let failed = false;
      const started = Date.now();
      for (const [stepIndex, step] of journey.steps.entries()) {
        const action = step.action;
        const stepStarted = Date.now();
        let status: "passed" | "failed" = "passed";
        let code: string | undefined;
        let detail: string | undefined;
        if (action === "wait") {
          const durationMs = coerceInt(step.duration_ms, 150, 10, 10000);
          const slept = await sleepWithCancellation(durationMs, token);
          if (!slept) {
            return textResult("Cancelled.");
          }
        } else if (action === "read_values") {
          apiActions += 1;
          const ids = normalizeStringList(step.ids);
          const requestIds = ids.length > 0 ? ids : defaultIds;
          try {
            const valuesRaw = await requestRuntimeControl(
              snapshot.rootPath,
              token,
              "hmi.values.get",
              { ids: requestIds },
            );
            const parsedValues = parseHmiValuesPayload(valuesRaw);
            if (!parsedValues) {
              throw new Error("runtime returned an invalid hmi.values.get payload.");
            }
            detail = `values=${Object.keys(parsedValues.values).length}`;
          } catch (error) {
            status = "failed";
            code = "HMI_JOURNEY_READ_VALUES_FAILED";
            detail = String(error);
          }
        } else if (action === "write") {
          const widgetId =
            typeof step.widget_id === "string" ? step.widget_id.trim() : "";
          const expectedErrorCode = normalizeErrorCode(step.expect_error_code);
          if (!widgetId) {
            status = "failed";
            code = "HMI_JOURNEY_WRITE_MISSING_TARGET";
            detail = "write step requires widget_id";
          } else {
            const blocked = localWriteGuard(widgetId);
            if (blocked) {
              code = blocked.code;
              detail = blocked.message;
              status = errorCodeMatches(expectedErrorCode, code, detail)
                ? "passed"
                : "failed";
            } else {
              apiActions += 1;
              try {
                await requestRuntimeControl(
                  snapshot.rootPath,
                  token,
                  "hmi.write",
                  { id: widgetId, value: step.value },
                );
                if (expectedErrorCode) {
                  status = "failed";
                  code = "HMI_JOURNEY_EXPECTED_ERROR_MISSING";
                  detail = `expected error code '${expectedErrorCode}' but write succeeded`;
                }
              } catch (error) {
                const message = String(error);
                const runtimeCode = extractErrorCode(message);
                code = runtimeCode ?? "HMI_JOURNEY_WRITE_FAILED";
                if (errorCodeMatches(expectedErrorCode, code, message)) {
                  status = "passed";
                  detail = message;
                } else {
                  status = "failed";
                  detail = message;
                }
              }
            }
          }
        }
        const durationMs = Date.now() - stepStarted;
        if (status === "failed") {
          failed = true;
        }
        stepResults.push({
          index: stepIndex + 1,
          action,
          status,
          duration_ms: durationMs,
          code,
          detail,
        });
      }
      const durationMs = Date.now() - started;
      if (durationMs > journey.max_duration_ms) {
        failed = true;
      }
      results.push({
        id: journey.id,
        title: journey.title,
        status: failed ? "failed" : "passed",
        duration_ms: durationMs,
        api_actions: apiActions,
        steps: stepResults,
      });
    }

    const ok = results.every((journey) => journey.status === "passed");
    const journeysDocument = {
      version: 1,
      generated_at: generatedAt.toISOString(),
      scenario,
      ok,
      journeys: results,
    };

    let evidencePath: string | null = null;
    let journeysPath: string | null = null;
    if (!dryRun) {
      const hmiRoot = vscode.Uri.file(snapshot.hmiPath);
      const runRoot = vscode.Uri.joinPath(hmiRoot, "_evidence", runId);
      await vscode.workspace.fs.createDirectory(runRoot);
      await writeUtf8File(
        vscode.Uri.joinPath(runRoot, "journeys.json"),
        `${stableJsonString(journeysDocument)}\n`,
      );
      evidencePath = path.posix.join("hmi", "_evidence", runId);
      journeysPath = path.posix.join(evidencePath, "journeys.json");
    }

    return textResult(
      JSON.stringify(
        {
          ok,
          dry_run: dryRun,
          rootPath: snapshot.rootPath,
          run_id: runId,
          scenario,
          evidencePath,
          journeysPath,
          journeys: results,
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiExplainWidgetTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiExplainWidgetParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const layout = await readHmiLayoutSnapshot(options.input.rootPath, token);
    if (layout.error) {
      if (layout.error === "Cancelled.") {
        return textResult("Cancelled.");
      }
      return errorResult(layout.error);
    }
    const snapshot = layout.snapshot;
    if (!snapshot || !snapshot.exists) {
      return errorResult("hmi/ directory does not exist.");
    }

    const descriptorPages = layoutDescriptorPages(snapshot.files);
    const refs = extractLayoutBindingRefs(descriptorPages);
    let catalog = normalizeHmiBindingsCatalog({});
    let catalogAvailable = false;
    let catalogError: string | undefined;
    const bindingsRequest = await this.request(
      "workspace/executeCommand",
      {
        command: "trust-lsp.hmiBindings",
        arguments: [{ root_uri: vscode.Uri.file(snapshot.rootPath).toString() }],
      },
      token,
    );
    if ("error" in bindingsRequest) {
      catalogError = bindingsRequest.error;
    } else {
      const payload =
        bindingsRequest.response && typeof bindingsRequest.response === "object"
          ? (bindingsRequest.response as Record<string, unknown>)
          : {};
      if (payload.ok === false) {
        catalogError = String(payload.error ?? "unknown error");
      } else {
        catalog = normalizeHmiBindingsCatalog(payload);
        catalogAvailable = true;
      }
    }

    const lock = buildHmiLockEntries(refs, catalog);
    const requestedId = options.input.widget_id?.trim();
    const requestedPath = options.input.path?.trim();
    const selected =
      (requestedId
        ? lock.entries.find((entry) => entry.id === requestedId)
        : undefined) ??
      (requestedPath
        ? lock.entries.find((entry) => entry.path === requestedPath)
        : undefined) ??
      lock.entries[0];
    if (!selected) {
      return errorResult("No widget/binding metadata available for explanation.");
    }

    const writePolicy = parseWritePolicyFromConfigToml(snapshot.config?.content);
    const allowlisted =
      writePolicy.allow.includes(selected.id) || writePolicy.allow.includes(selected.path);
    const bindingCatalogEntry = catalog.byPath.get(selected.path);

    return textResult(
      JSON.stringify(
        {
          ok: true,
          rootPath: snapshot.rootPath,
          requested: {
            widget_id: requestedId ?? null,
            path: requestedPath ?? null,
          },
          widget: selected,
          provenance: {
            canonical_id: selected.id,
            symbol_path: selected.path,
            type: selected.data_type,
            qualifier: selected.qualifier,
            writable: selected.writable,
            write_policy: {
              enabled: writePolicy.enabled,
              allowlisted,
              allow: writePolicy.allow,
            },
            alarm_policy: {
              min: selected.constraints.min,
              max: selected.constraints.max,
              unit: selected.constraints.unit,
            },
            source_files: selected.files.map((file) => path.posix.join("hmi", file)),
            contract_endpoints: ["hmi.schema.get", "hmi.values.get", "hmi.write"],
            binding_catalog: {
              available: catalogAvailable,
              error: catalogError,
              match: bindingCatalogEntry
                ? {
                    id: bindingCatalogEntry.id,
                    path: bindingCatalogEntry.path,
                    dataType: bindingCatalogEntry.dataType,
                    qualifier: bindingCatalogEntry.qualifier,
                    writable: bindingCatalogEntry.writable,
                  }
                : null,
            },
          },
        },
        null,
        2,
      ),
    );
  }
}

export class STHmiInitTool extends LspToolBase {
  async invoke(
    options: InvocationOptions<HmiInitParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const rawStyle =
      typeof options.input.style === "string" ? options.input.style.trim() : "";
    const args =
      rawStyle.length > 0
        ? [{ style: rawStyle.toLowerCase() }]
        : [];
    const result = await this.request(
      "workspace/executeCommand",
      { command: "trust-lsp.hmiInit", arguments: args },
      token,
    );
    if ("error" in result) {
      return errorResult(result.error);
    }
    const response = result.response as { ok?: boolean; error?: unknown } | null;
    if (
      response &&
      typeof response === "object" &&
      response.ok === false
    ) {
      const message =
        typeof response.error === "string"
          ? response.error
          : "trust-lsp.hmiInit failed.";
      return errorResult(message);
    }
    return textResult(
      JSON.stringify(
        {
          command: "trust-lsp.hmiInit",
          result: result.response,
        },
        null,
        2,
      ),
    );
  }
}

export class STWorkspaceSymbolsTimedTool {
  async invoke(
    options: InvocationOptions<WorkspaceSymbolsTimedParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { query, limit, pathIncludes } = options.input;
    if (!query.trim()) {
      return errorResult("query must be a non-empty string.");
    }
    const start = Date.now();
    try {
      const symbols = await vscode.commands.executeCommand<
        vscode.SymbolInformation[]
      >("vscode.executeWorkspaceSymbolProvider", query);
      const durationMs = Date.now() - start;
      if (!symbols || symbols.length === 0) {
        return textResult(
          JSON.stringify({ durationMs, symbols: [] }, null, 2),
        );
      }
      let filtered = symbols;
      if (Array.isArray(pathIncludes) && pathIncludes.length > 0) {
        filtered = symbols.filter((symbol) =>
          pathIncludes.some((part) =>
            formatLocationLike(symbol.location).includes(part),
          ),
        );
      }
      const { items, truncated } = truncateItems(filtered, limit ?? MAX_ITEMS);
      const payload = items.map((symbol) => ({
        name: symbol.name,
        kind: symbolKindName(symbol.kind),
        containerName: symbol.containerName || undefined,
        location: formatLocationLike(symbol.location),
      }));
      return textResult(
        JSON.stringify({ durationMs, symbols: payload, truncated }, null, 2),
      );
    } catch (error) {
      return errorResult(`Failed to get workspace symbols: ${String(error)}`);
    }
  }
}

export class STFileReadTool {
  async invoke(
    options: InvocationOptions<FileReadParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, startLine, startCharacter, endLine, endCharacter } =
      options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const workspaceError = ensureWorkspaceUri(uri);
    if (workspaceError) {
      return errorResult(workspaceError);
    }
    const hasRangePart =
      startLine !== undefined ||
      startCharacter !== undefined ||
      endLine !== undefined ||
      endCharacter !== undefined;
    const hasFullRange =
      startLine !== undefined &&
      startCharacter !== undefined &&
      endLine !== undefined &&
      endCharacter !== undefined;
    if (hasRangePart && !hasFullRange) {
      return errorResult(
        "Provide all of startLine/startCharacter/endLine/endCharacter for range reads.",
      );
    }
    try {
      const doc = await ensureDocument(uri);
      if (
        startLine !== undefined &&
        startCharacter !== undefined &&
        endLine !== undefined &&
        endCharacter !== undefined
      ) {
        const range = resolveRange(
          doc,
          startLine,
          startCharacter,
          endLine,
          endCharacter,
        );
        const text = doc.getText(range);
        return textResult(
          JSON.stringify(
            {
              filePath: uri.fsPath,
              range: formatRange(range),
              text,
            },
            null,
            2,
          ),
        );
      }
      const text = doc.getText();
      return textResult(
        JSON.stringify({ filePath: uri.fsPath, text }, null, 2),
      );
    } catch (error) {
      return errorResult(`Failed to read file: ${String(error)}`);
    }
  }
}

export class STReadRangeTool {
  async invoke(
    options: InvocationOptions<RangeParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, startLine, startCharacter, endLine, endCharacter } =
      options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const workspaceError = ensureWorkspaceUri(uri);
    if (workspaceError) {
      return errorResult(workspaceError);
    }
    try {
      const doc = await ensureDocument(uri);
      const range = resolveRange(
        doc,
        startLine,
        startCharacter,
        endLine,
        endCharacter,
      );
      const text = doc.getText(range);
      return textResult(
        JSON.stringify(
          {
            filePath: uri.fsPath,
            range: formatRange(range),
            text,
          },
          null,
          2,
        ),
      );
    } catch (error) {
      return errorResult(`Failed to read range: ${String(error)}`);
    }
  }
}

export class STFileWriteTool {
  async invoke(
    options: InvocationOptions<FileWriteParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, text, save } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const workspaceError = ensureWorkspaceUri(uri);
    if (workspaceError) {
      return errorResult(workspaceError);
    }
    try {
      const openDoc = openDocumentIfLoaded(uri);
      if (openDoc) {
        const edit = new vscode.WorkspaceEdit();
        edit.replace(uri, fullDocumentRange(openDoc), text);
        await vscode.workspace.applyEdit(edit);
        if (save) {
          await openDoc.save();
        }
        return textResult("File updated.");
      }
      const encoder = new TextEncoder();
      await vscode.workspace.fs.writeFile(uri, encoder.encode(text));
      if (save) {
        const doc = await ensureDocument(uri);
        await doc.save();
      }
      return textResult("File written.");
    } catch (error) {
      return errorResult(`Failed to write file: ${String(error)}`);
    }
  }
}

export class STApplyEditsTool {
  async invoke(
    options: InvocationOptions<ApplyEditsParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const { filePath, edits, save } = options.input;
    const uri = uriFromFilePath(filePath);
    if (!uri) {
      return errorResult("filePath must be an absolute path or URI.");
    }
    const workspaceError = ensureWorkspaceUri(uri);
    if (workspaceError) {
      return errorResult(workspaceError);
    }
    if (!Array.isArray(edits) || edits.length === 0) {
      return errorResult("edits must be a non-empty array.");
    }
    try {
      const doc = await ensureDocument(uri);
      const workspaceEdit = new vscode.WorkspaceEdit();
      for (const edit of edits) {
        const range = resolveRange(
          doc,
          edit.startLine,
          edit.startCharacter,
          edit.endLine,
          edit.endCharacter,
        );
        workspaceEdit.replace(uri, range, edit.newText);
      }
      await vscode.workspace.applyEdit(workspaceEdit);
      if (save) {
        await doc.save();
      }
      return textResult("Edits applied.");
    } catch (error) {
      return errorResult(`Failed to apply edits: ${String(error)}`);
    }
  }
}

export class STDebugStartTool {
  async invoke(
    options: InvocationOptions<DebugStartParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const uri = optionalUriFromFilePath(options.input.filePath);
    if (options.input.filePath && !uri) {
      return errorResult("filePath must be an absolute path or URI when provided.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.start", uri);
      return textResult("Debug start requested.");
    } catch (error) {
      return errorResult(`Failed to start debugging: ${String(error)}`);
    }
  }
}

export class STDebugAttachTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.attach");
      return textResult("Debug attach requested.");
    } catch (error) {
      return errorResult(`Failed to attach debugger: ${String(error)}`);
    }
  }
}

export class STDebugReloadTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.reload");
      return textResult("Debug reload requested.");
    } catch (error) {
      return errorResult(`Failed to reload debugger: ${String(error)}`);
    }
  }
}

export class STDebugOpenIoPanelTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
      return textResult("Opened I/O panel.");
    } catch (error) {
      return errorResult(`Failed to open I/O panel: ${String(error)}`);
    }
  }
}

export class STDebugEnsureConfigurationTool {
  async invoke(
    _options: InvocationOptions<EmptyParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    try {
      await vscode.commands.executeCommand("trust-lsp.debug.ensureConfiguration");
      return textResult("Ensure configuration requested.");
    } catch (error) {
      return errorResult(`Failed to ensure configuration: ${String(error)}`);
    }
  }
}

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
