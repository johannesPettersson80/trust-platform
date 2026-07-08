import * as vscode from "vscode";
import * as path from "path";
import type { LadderProgram } from "./ladderEngine.types";
import { LadderEngine } from "./ladderEngine";
import { RuntimeClient, getRuntimeConfig } from "../statechart/runtimeClient";
import { RuntimeController } from "../visual/runtime/runtimeController";
import {
  isRuntimeWebviewMessage,
  runtimeMessage,
  type RuntimeWebviewToExtensionMessage,
} from "../visual/runtime/runtimeMessages";
import {
  persistRuntimeUiMode,
  runtimeUiModeFromConfig,
} from "../visual/runtime/runtimeModeConfig";
import {
  applyRuntimePanelSettings,
  boolToDisplay,
  collectRuntimePanelSettings,
  isRuntimePanelWebviewMessage,
  runtimePanelModeToUi,
  runtimePanelStatusFromState,
  type RuntimePanelIoEntry,
  type RuntimePanelIoState,
  type RuntimePanelWebviewMessage,
} from "../visual/runtime/runtimePanelBridge";
import {
  forceVisualRuntimeIo,
  releaseVisualRuntimeIo,
  startVisualRuntime,
  stopVisualRuntime,
  writeVisualRuntimeIo,
} from "../visual/runtime/stRuntimeCommands";
import { syncVisualCompanionFromUri } from "../visual/companionSt";
import { validateLadderProgramValue } from "../visual/ladderToSt";
import { sanitizeIdentifier } from "../visual/stNaming";

type ExecutionMode = "simulation" | "hardware";

interface ExecutionState {
  program: LadderProgram;
  mode: ExecutionMode;
  isRunning: boolean;
  engine: LadderEngine;
  webviewPanel: vscode.WebviewPanel;
}

interface LadderExecutionSnapshot {
  inputs?: Record<string, boolean>;
  forcedInputs?: Record<string, boolean>;
  outputs?: Record<string, boolean>;
  markers?: Record<string, boolean>;
  memoryWords?: Record<string, number>;
  variableBooleans?: Record<string, boolean>;
  variableNumbers?: Record<string, number>;
}

interface DebugIoEntry {
  name?: unknown;
  address?: unknown;
  writeTarget?: unknown;
  value?: unknown;
  forced?: unknown;
}

interface DebugIoState {
  inputs?: unknown;
  outputs?: unknown;
  memory?: unknown;
}

const DEBUG_TYPE = "structured-text";
const DEBUG_EVENT_IO_STATE = "stIoState";
const LADDER_SOURCE_SUFFIX = ".ladder.json";
const RUNTIME_IO_CONFIRMATION_TIMEOUT_MS = 3000;

interface RuntimeIoOperationState {
  status: "pending" | "error";
  error?: string;
  timer?: NodeJS.Timeout;
}

function structuredTextSessionKey(session: vscode.DebugSession): string {
  return session.id ?? session.name;
}

const WEBVIEW_HTML_TEMPLATE = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; img-src {{cspSource}} data: https:; style-src {{cspSource}} 'unsafe-inline'; script-src {{cspSource}} 'unsafe-eval';"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Ladder Logic Editor</title>
    <link rel="stylesheet" href="{{webviewStyle}}" />
    <style>
      * {
        box-sizing: border-box;
        margin: 0;
        padding: 0;
      }

      html,
      body,
      #root {
        width: 100%;
        height: 100%;
        overflow: hidden;
        font-family: var(
          --vscode-font-family,
          -apple-system,
          BlinkMacSystemFont,
          "Segoe UI",
          Roboto,
          Oxygen,
          Ubuntu,
          Cantarell,
          sans-serif
        );
        background-color: var(--trust-canvas, var(--vscode-editor-background, #1e1e1e));
        color: var(--trust-text, var(--vscode-editor-foreground, var(--vscode-foreground, #cccccc)));
      }
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script src="{{webviewScript}}"></script>
  </body>
</html>
`;

function createEmptyProgram(name = "New Ladder Program"): LadderProgram {
  return {
    schemaVersion: 2,
    networks: [],
    variables: [],
    metadata: {
      name,
      description: "Ladder logic program",
      created: new Date().toISOString(),
      modified: new Date().toISOString(),
    },
  };
}

function tryValidateSchemaV2Program(value: unknown): LadderProgram | undefined {
  try {
    return validateLadderProgramValue(value);
  } catch {
    return undefined;
  }
}

function parseBooleanInput(value: string): boolean | undefined {
  const text = value.trim().toUpperCase();
  if (!text) {
    return undefined;
  }
  const normalized =
    text.startsWith("BOOL(") && text.endsWith(")")
      ? text.slice(5, -1).trim()
      : text;
  if (normalized === "TRUE" || normalized === "1") {
    return true;
  }
  if (normalized === "FALSE" || normalized === "0") {
    return false;
  }
  return undefined;
}

function isDirectRuntimeIoTarget(value: string): boolean {
  const normalized = value.trim().toUpperCase();
  return (
    normalized.startsWith("%IX") ||
    normalized.startsWith("%QX") ||
    normalized.startsWith("%MX") ||
    normalized.startsWith("%MW")
  );
}

function canonicalRuntimeIoTarget(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }
  return isDirectRuntimeIoTarget(trimmed) ? trimmed.toUpperCase() : trimmed;
}

function runtimeIoEntryKey(value: string): string {
  return canonicalRuntimeIoTarget(value).toUpperCase();
}

function valueForRuntimeKey<T>(
  record: Record<string, T>,
  ...candidates: Array<string | undefined>
): T | undefined {
  for (const candidate of candidates) {
    const token = candidate?.trim();
    if (!token) {
      continue;
    }
    if (token in record) {
      return record[token];
    }
    const upper = token.toUpperCase();
    if (upper in record) {
      return record[upper];
    }
    const lower = token.toLowerCase();
    if (lower in record) {
      return record[lower];
    }
  }
  return undefined;
}

function toBooleanRecord(value: unknown): Record<string, boolean> {
  if (!value || typeof value !== "object") {
    return {};
  }
  const record: Record<string, boolean> = {};
  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (typeof entry === "boolean") {
      record[key] = entry;
    } else if (typeof entry === "number") {
      record[key] = entry !== 0;
    } else if (typeof entry === "string") {
      const normalized = entry.trim().toLowerCase();
      if (normalized === "true" || normalized === "1") {
        record[key] = true;
      } else if (normalized === "false" || normalized === "0") {
        record[key] = false;
      }
    }
  }
  return record;
}

function toNumberRecord(value: unknown): Record<string, number> {
  if (!value || typeof value !== "object") {
    return {};
  }
  const record: Record<string, number> = {};
  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (typeof entry === "number" && Number.isFinite(entry)) {
      record[key] = entry;
    } else if (typeof entry === "string") {
      const parsed = Number(entry);
      if (Number.isFinite(parsed)) {
        record[key] = parsed;
      }
    }
  }
  return record;
}

/**
 * Provider for Ladder Logic visual programming editor
 */
export class LadderEditorProvider implements vscode.CustomTextEditorProvider {
  private static readonly viewType = "trust-lsp.ladder.editor";
  private activeExecutions = new Map<string, ExecutionState>();
  private latestPrograms = new Map<string, LadderProgram>();
  private runtimeIoState = new Map<string, RuntimePanelIoState>();
  private runtimeIoOperations = new Map<string, Map<string, RuntimeIoOperationState>>();
  private structuredTextSessions = new Map<string, vscode.DebugSession>();
  private editorSessions = new Map<
    string,
    {
      document: vscode.TextDocument;
      webviewPanel: vscode.WebviewPanel;
    }
  >();
  private readonly runtimeController = new RuntimeController({
    openPanel: () => vscode.commands.executeCommand("trust-lsp.debug.openIoPanel"),
    openSettings: () =>
      vscode.commands.executeCommand("trust-lsp.debug.openIoPanelSettings"),
  });

  public static register(context: vscode.ExtensionContext): vscode.Disposable {
    const provider = new LadderEditorProvider(context);
    const providerRegistration = vscode.window.registerCustomEditorProvider(
      LadderEditorProvider.viewType,
      provider,
      {
        webviewOptions: {
          retainContextWhenHidden: true,
        },
      }
    );
    return providerRegistration;
  }

  constructor(private readonly context: vscode.ExtensionContext) {
    this.registerDebugIoBridge();
  }

  /**
   * Called when a custom editor is opened
   */
  public async resolveCustomTextEditor(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken
  ): Promise<void> {
    const docId = document.uri.toString();
    this.editorSessions.set(docId, { document, webviewPanel });
    console.log("[Ladder] resolveCustomTextEditor called for:", docId);

    // Setup webview
    webviewPanel.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.file(path.join(this.context.extensionPath, "media")),
      ],
    };

    // Set initial HTML content
    const html = this.getHtmlForWebview(webviewPanel.webview);
    console.log("[Ladder] Setting webview HTML, length:", html.length);
    webviewPanel.webview.html = html;

    // Handle messages from webview
    webviewPanel.webview.onDidReceiveMessage(async (message) => {
      console.log("[Ladder] Received message from webview:", message.type);

      if (isRuntimePanelWebviewMessage(message)) {
        await this.handleRuntimePanelMessage(docId, document, message, webviewPanel);
        return;
      }

      if (isRuntimeWebviewMessage(message)) {
        await this.handleRuntimeMessage(docId, document, message, webviewPanel);
        return;
      }

      switch (message.type) {
        case "save": {
          let validatedProgram: LadderProgram;
          try {
            validatedProgram = validateLadderProgramValue(message.program);
          } catch (error) {
            const details = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(
              `Ladder save rejected by schema validation: ${details}`
            );
            break;
          }
          const persisted = await this.saveProgram(document, validatedProgram);
          this.latestPrograms.set(docId, persisted);
          break;
        }

        case "validate": {
          try {
            validateLadderProgramValue(message.program);
            void vscode.window.showInformationMessage("Ladder program validates.");
          } catch (error) {
            const details = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(
              `Ladder validation failed: ${details}`
            );
          }
          break;
        }

        case "generateST": {
          let validatedProgram: LadderProgram;
          try {
            validatedProgram = validateLadderProgramValue(message.program);
          } catch (error) {
            const details = error instanceof Error ? error.message : String(error);
            void vscode.window.showErrorMessage(
              `Ladder ST generation failed: ${details}`
            );
            break;
          }
          const persisted = await this.saveProgram(document, validatedProgram, {
            notify: false,
          });
          this.latestPrograms.set(docId, persisted);
          const companion = await syncVisualCompanionFromUri(document.uri, {
            force: true,
            showErrors: true,
            sourceText: `${JSON.stringify(persisted, null, 2)}\n`,
          });
          if (companion) {
            void vscode.window.showInformationMessage(
              `Generated ST companion: ${path.basename(companion.fsPath)}`
            );
          }
          break;
        }

        case "programState":
          {
            const validatedProgram = tryValidateSchemaV2Program(message.program);
            if (validatedProgram) {
              this.latestPrograms.set(docId, validatedProgram);
            }
          }
          break;

        case "webviewBootError":
          console.error("[Ladder] webview boot error:", message.message);
          void vscode.window.showErrorMessage(
            `Ladder webview failed to initialize: ${String(
              message.message ?? "unknown error"
            )}`
          );
          break;

        case "stop":
          await this.handleRuntimeMessage(
            docId,
            document,
            runtimeMessage.stop(),
            webviewPanel
          );
          break;

        case "openRuntimePanel":
          await this.handleRuntimeMessage(
            docId,
            document,
            runtimeMessage.openPanel(),
            webviewPanel
          );
          break;

        case "ready": {
          const program = this.loadProgram(document);
          this.latestPrograms.set(docId, program);
          this.runtimeIoState.set(
            docId,
            this.buildRuntimePanelIoState(docId, document)
          );
          this.runtimeController.ensureState(docId);
          this.runtimeController.setMode(
            docId,
            runtimeUiModeFromConfig(document.uri)
          );
          webviewPanel.webview.postMessage({
            type: "loadProgram",
            program,
          });
          this.postRuntimeState(docId, document.uri, webviewPanel);
          this.postRuntimePanelIoState(docId, webviewPanel, document);
          this.postRuntimePanelSettings(document.uri, webviewPanel);
          break;
        }
      }
    });

    // Handle document changes
    const changeDocumentSubscription = vscode.workspace.onDidChangeTextDocument(
      (e) => {
        if (e.document.uri.toString() === document.uri.toString()) {
          const program = this.loadProgram(document);
          this.latestPrograms.set(docId, program);
          this.runtimeIoState.set(
            docId,
            this.buildRuntimePanelIoState(docId, document)
          );
          webviewPanel.webview.postMessage({
            type: "loadProgram",
            program,
          });
          this.postRuntimePanelIoState(docId, webviewPanel, document);
        }
      }
    );

    // Cleanup on panel dispose
    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
      void this.stopExecution(docId);
      this.activeExecutions.delete(docId);
      this.latestPrograms.delete(docId);
      this.runtimeIoState.delete(docId);
      this.clearRuntimeIoOperations(docId);
      this.runtimeController.clear(docId);
      this.editorSessions.delete(docId);
    });
  }

  private registerDebugIoBridge(): void {
    this.context.subscriptions.push(
      vscode.debug.onDidReceiveDebugSessionCustomEvent((event) => {
        if (
          event.session.type !== DEBUG_TYPE ||
          event.event !== DEBUG_EVENT_IO_STATE
        ) {
          return;
        }
        const normalized = this.normalizeDebugIoState(event.body);
        if (!normalized) {
          return;
        }

        for (const [docId, session] of this.editorSessions) {
          const runtime = this.runtimeController.ensureState(docId);
          if (!runtime.isExecuting) {
            continue;
          }
          this.clearConfirmedRuntimeIoOperations(docId, normalized);
          const merged = this.mergeRuntimeIoState(
            this.ensureRuntimeIoState(docId, session.document),
            normalized
          );
          this.runtimeIoState.set(docId, merged);
          session.webviewPanel.webview.postMessage({
            type: "ioState",
            payload: merged,
          });
        }
      })
    );

    this.context.subscriptions.push(
      vscode.debug.onDidStartDebugSession((session) => {
        if (session.type !== DEBUG_TYPE) {
          return;
        }
        this.structuredTextSessions.set(structuredTextSessionKey(session), session);
        void this.requestStructuredTextIoState();
      })
    );

    this.context.subscriptions.push(
      vscode.debug.onDidTerminateDebugSession((session) => {
        if (session.type !== DEBUG_TYPE) {
          return;
        }
        this.structuredTextSessions.delete(structuredTextSessionKey(session));
      })
    );

    this.context.subscriptions.push(
      vscode.debug.onDidChangeActiveDebugSession((session) => {
        if (!session || session.type !== DEBUG_TYPE) {
          return;
        }
        this.structuredTextSessions.set(structuredTextSessionKey(session), session);
        void this.requestStructuredTextIoState();
      })
    );
  }

  private async requestStructuredTextIoState(): Promise<void> {
    const active = vscode.debug.activeDebugSession;
    const session =
      active && active.type === DEBUG_TYPE
        ? active
        : this.structuredTextSessions.values().next().value;
    if (!session) {
      return;
    }
    try {
      await session.customRequest(DEBUG_EVENT_IO_STATE);
    } catch {
      // Ignore transient request errors while sessions start/stop.
    }
  }

  private runtimeIoOperationStateForDoc(
    docId: string,
    create = false
  ): Map<string, RuntimeIoOperationState> | undefined {
    const existing = this.runtimeIoOperations.get(docId);
    if (existing || !create) {
      return existing;
    }
    const created = new Map<string, RuntimeIoOperationState>();
    this.runtimeIoOperations.set(docId, created);
    return created;
  }

  private clearRuntimeIoOperationTimer(state: RuntimeIoOperationState): void {
    if (!state.timer) {
      return;
    }
    clearTimeout(state.timer);
    delete state.timer;
  }

  private clearRuntimeIoOperations(docId: string): void {
    const operations = this.runtimeIoOperationStateForDoc(docId);
    if (!operations) {
      return;
    }
    for (const state of operations.values()) {
      this.clearRuntimeIoOperationTimer(state);
    }
    this.runtimeIoOperations.delete(docId);
  }

  private setPendingRuntimeIoOperation(docId: string, targetRaw: string): void {
    const key = runtimeIoEntryKey(targetRaw);
    if (!key) {
      return;
    }
    const operations = this.runtimeIoOperationStateForDoc(docId, true)!;
    const existing = operations.get(key);
    if (existing) {
      this.clearRuntimeIoOperationTimer(existing);
    }

    const state: RuntimeIoOperationState = { status: "pending" };
    state.timer = setTimeout(() => {
      const current = this.runtimeIoOperationStateForDoc(docId);
      const operation = current?.get(key);
      if (!operation || operation.status !== "pending") {
        return;
      }
      this.setRuntimeIoOperationError(
        docId,
        targetRaw,
        `No runtime confirmation received for ${targetRaw}.`
      );
      const session = this.editorSessions.get(docId);
      if (session) {
        this.postRuntimePanelIoState(docId, session.webviewPanel, session.document);
      }
    }, RUNTIME_IO_CONFIRMATION_TIMEOUT_MS);

    operations.set(key, state);
  }

  private setRuntimeIoOperationError(
    docId: string,
    targetRaw: string,
    error: string
  ): void {
    const key = runtimeIoEntryKey(targetRaw);
    if (!key) {
      return;
    }
    const operations = this.runtimeIoOperationStateForDoc(docId, true)!;
    const existing = operations.get(key);
    if (existing) {
      this.clearRuntimeIoOperationTimer(existing);
    }
    operations.set(key, {
      status: "error",
      error,
    });
  }

  private clearRuntimeIoOperation(docId: string, targetRaw: string): void {
    const key = runtimeIoEntryKey(targetRaw);
    if (!key) {
      return;
    }
    const operations = this.runtimeIoOperationStateForDoc(docId);
    if (!operations) {
      return;
    }
    const existing = operations.get(key);
    if (!existing) {
      return;
    }
    this.clearRuntimeIoOperationTimer(existing);
    operations.delete(key);
    if (operations.size === 0) {
      this.runtimeIoOperations.delete(docId);
    }
  }

  private clearConfirmedRuntimeIoOperations(
    docId: string,
    update: RuntimePanelIoState
  ): void {
    const operations = this.runtimeIoOperationStateForDoc(docId);
    if (!operations || operations.size === 0) {
      return;
    }
    for (const bucket of [update.inputs, update.outputs, update.memory]) {
      for (const entry of bucket) {
        this.clearRuntimeIoOperation(docId, entry.writeTarget ?? entry.address);
      }
    }
  }

  private applyRuntimeIoOperationSuccess(
    docId: string,
    document: vscode.TextDocument,
    targetRaw: string,
    operation: "write" | "force" | "release",
    boolValue?: boolean
  ): void {
    const normalizedTarget = canonicalRuntimeIoTarget(targetRaw);
    if (!normalizedTarget) {
      return;
    }
    if (isDirectRuntimeIoTarget(normalizedTarget)) {
      return;
    }

    this.clearRuntimeIoOperation(docId, normalizedTarget);
    if (operation === "release") {
      this.releaseRuntimeIoEntry(docId, document, normalizedTarget);
      return;
    }

    const value = boolToDisplay(boolValue ?? false);
    const forced = operation === "force" ? true : undefined;
    this.upsertRuntimeIoEntry(docId, document, normalizedTarget, value, forced);
  }

  private annotateRuntimeIoStateWithOperations(
    docId: string,
    source: RuntimePanelIoState
  ): RuntimePanelIoState {
    const operations = this.runtimeIoOperationStateForDoc(docId);
    if (!operations || operations.size === 0) {
      return source;
    }
    const decorateEntry = (entry: RuntimePanelIoEntry): RuntimePanelIoEntry => {
      const key = runtimeIoEntryKey(entry.writeTarget ?? entry.address);
      const operation = operations.get(key);
      if (!operation) {
        return {
          ...entry,
          operationStatus: undefined,
          operationError: undefined,
        };
      }
      return {
        ...entry,
        operationStatus: operation.status,
        operationError: operation.error,
      };
    };

    return {
      inputs: source.inputs.map(decorateEntry),
      outputs: source.outputs.map(decorateEntry),
      memory: source.memory.map(decorateEntry),
    };
  }

  private postRuntimeState(
    docId: string,
    resource: vscode.Uri,
    webviewPanel: vscode.WebviewPanel
  ): void {
    const state = this.runtimeController.ensureState(docId);
    webviewPanel.webview.postMessage(runtimeMessage.state(state));
    webviewPanel.webview.postMessage({
      type: "runtimeStatus",
      payload: runtimePanelStatusFromState(resource, state),
    });
  }

  private currentProgramForDoc(
    docId: string,
    document?: vscode.TextDocument
  ): LadderProgram {
    if (this.latestPrograms.has(docId)) {
      return this.latestPrograms.get(docId)!;
    }
    if (document) {
      return this.loadProgram(document);
    }
    return createEmptyProgram();
  }

  private runtimeFbInstanceNameForDocument(
    document?: vscode.TextDocument
  ): string | undefined {
    if (!document) {
      return undefined;
    }
    const sourceName = path.basename(document.uri.fsPath);
    const lower = sourceName.toLowerCase();
    const baseName = lower.endsWith(LADDER_SOURCE_SUFFIX)
      ? sourceName.slice(0, -LADDER_SOURCE_SUFFIX.length)
      : sourceName.replace(/\.json$/i, "");
    const baseId = sanitizeIdentifier(baseName, "Visual");
    return sanitizeIdentifier(`fb_${baseId}`, "fb_visual");
  }

  private normalizeDebugIoEntry(value: unknown): RuntimePanelIoEntry | undefined {
    if (!value || typeof value !== "object") {
      return undefined;
    }
    const entry = value as DebugIoEntry;
    if (typeof entry.address !== "string") {
      return undefined;
    }

    const address = canonicalRuntimeIoTarget(entry.address);
    if (!address) {
      return undefined;
    }

    const writeTarget =
      typeof entry.writeTarget === "string"
        ? canonicalRuntimeIoTarget(entry.writeTarget)
        : undefined;

    let displayValue = "";
    if (typeof entry.value === "string") {
      displayValue = entry.value;
    } else if (typeof entry.value === "boolean") {
      displayValue = boolToDisplay(entry.value);
    } else if (typeof entry.value === "number") {
      displayValue =
        address.startsWith("%IX") ||
        address.startsWith("%QX") ||
        address.startsWith("%MX")
          ? boolToDisplay(entry.value !== 0)
          : `INT(${Math.trunc(entry.value)})`;
    } else if (entry.value !== undefined && entry.value !== null) {
      displayValue = String(entry.value);
    }

    return {
      address,
      writeTarget,
      name:
        typeof entry.name === "string" && entry.name.trim().length > 0
          ? entry.name.trim()
          : address,
      value: displayValue,
      forced: typeof entry.forced === "boolean" ? entry.forced : undefined,
    };
  }

  private normalizeDebugIoState(value: unknown): RuntimePanelIoState | undefined {
    if (!value || typeof value !== "object") {
      return undefined;
    }

    const state = value as DebugIoState;
    const normalizeBucket = (bucket: unknown): RuntimePanelIoEntry[] => {
      if (!Array.isArray(bucket)) {
        return [];
      }
      return bucket
        .map((entry) => this.normalizeDebugIoEntry(entry))
        .filter((entry): entry is RuntimePanelIoEntry => !!entry);
    };

    return {
      inputs: normalizeBucket(state.inputs),
      outputs: normalizeBucket(state.outputs),
      memory: normalizeBucket(state.memory),
    };
  }

  private mergeRuntimeIoState(
    base: RuntimePanelIoState,
    update: RuntimePanelIoState
  ): RuntimePanelIoState {
    const clone = (bucket: RuntimePanelIoEntry[]): RuntimePanelIoEntry[] =>
      bucket.map((entry) => ({ ...entry }));

    const merged: RuntimePanelIoState = {
      inputs: clone(base.inputs),
      outputs: clone(base.outputs),
      memory: clone(base.memory),
    };

    const mergeBucket = (
      target: RuntimePanelIoEntry[],
      source: RuntimePanelIoEntry[]
    ): void => {
      for (const entry of source) {
        const sourceKey = runtimeIoEntryKey(entry.writeTarget ?? entry.address);
        const index = target.findIndex(
          (candidate) => {
            const writeKey = runtimeIoEntryKey(candidate.writeTarget ?? "");
            const addressKey = runtimeIoEntryKey(candidate.address);
            return writeKey === sourceKey || addressKey === sourceKey;
          }
        );
        if (index >= 0) {
          target[index] = {
            ...target[index],
            value: entry.value,
            forced: entry.forced ?? target[index].forced,
            name: target[index].name || entry.name,
            writeTarget: entry.writeTarget ?? target[index].writeTarget,
          };
          continue;
        }
        target.push({
          ...entry,
          address: canonicalRuntimeIoTarget(entry.address),
          writeTarget: entry.writeTarget
            ? canonicalRuntimeIoTarget(entry.writeTarget)
            : undefined,
        });
      }
      target.sort((left, right) => left.address.localeCompare(right.address));
    };

    mergeBucket(merged.inputs, update.inputs);
    mergeBucket(merged.outputs, update.outputs);
    mergeBucket(merged.memory, update.memory);
    return merged;
  }

  private ensureRuntimeIoState(
    docId: string,
    document?: vscode.TextDocument
  ): RuntimePanelIoState {
    const existing = this.runtimeIoState.get(docId);
    if (existing) {
      return existing;
    }
    const created = this.buildRuntimePanelIoState(docId, document);
    this.runtimeIoState.set(docId, created);
    return created;
  }

  private collectProgramIoAddresses(
    program: LadderProgram,
    document?: vscode.TextDocument
  ): {
    inputs: Map<string, RuntimePanelIoEntry>;
    outputs: Map<string, RuntimePanelIoEntry>;
    memory: Map<string, RuntimePanelIoEntry>;
  } {
    const inputs = new Map<string, RuntimePanelIoEntry>();
    const outputs = new Map<string, RuntimePanelIoEntry>();
    const memory = new Map<string, RuntimePanelIoEntry>();

    const normalizeAddress = (value?: string): string | undefined => {
      const token = value?.trim();
      if (!token) {
        return undefined;
      }
      const upper = token.toUpperCase();
      if (
        upper.startsWith("%IX") ||
        upper.startsWith("%QX") ||
        upper.startsWith("%MX") ||
        upper.startsWith("%MW")
      ) {
        return upper;
      }
      return undefined;
    };

    const parseQualifiedSymbol = (
      value: string
    ): { scope?: "local" | "global"; name: string } => {
      const trimmed = value.trim();
      const upper = trimmed.toUpperCase();
      if (upper.startsWith("LOCAL::")) {
        return { scope: "local", name: trimmed.slice(7).trim() };
      }
      if (upper.startsWith("L::")) {
        return { scope: "local", name: trimmed.slice(3).trim() };
      }
      if (upper.startsWith("GLOBAL::")) {
        return { scope: "global", name: trimmed.slice(8).trim() };
      }
      if (upper.startsWith("G::")) {
        return { scope: "global", name: trimmed.slice(3).trim() };
      }
      if (upper.startsWith("LOCAL.")) {
        return { scope: "local", name: trimmed.slice(6).trim() };
      }
      if (upper.startsWith("GLOBAL.")) {
        return { scope: "global", name: trimmed.slice(7).trim() };
      }
      return { name: trimmed };
    };

    const symbolKey = (value: string): string => value.trim().toUpperCase();
    const toScope = (scope: unknown): "local" | "global" =>
      scope === "local" ? "local" : "global";

    const localSymbols = new Set<string>();
    const globalSymbols = new Set<string>();
    for (const variable of program.variables) {
      const key = symbolKey(variable.name ?? "");
      if (!key) {
        continue;
      }
      if (toScope(variable.scope) === "local") {
        localSymbols.add(key);
      } else {
        globalSymbols.add(key);
      }
    }

    const isShadowedSymbol = (key: string): boolean =>
      localSymbols.has(key) && globalSymbols.has(key);

    const symbolReferenceForVariable = (
      variable: LadderProgram["variables"][number]
    ): string => {
      const name = variable.name?.trim() ?? "";
      if (!name) {
        return "";
      }
      const key = symbolKey(name);
      if (!isShadowedSymbol(key)) {
        return name;
      }
      return `${toScope(variable.scope).toUpperCase()}::${name}`;
    };

    const runtimeFbInstanceName = this.runtimeFbInstanceNameForDocument(document);

    const variableLookup = new Map<
      string,
      {
        name: string;
        declaredRef: string;
        scope: "local" | "global";
        address?: string;
        type: LadderProgram["variables"][number]["type"];
      }
    >();
    const addressAlias = new Map<string, string>();
    const boolVariables = new Set<string>();
    const numericVariables = new Set<string>();
    const inputVariables = new Set<string>();
    const outputVariables = new Set<string>();
    const memoryVariables = new Set<string>();

    const registerVariableCategory = (
      reference: string,
      variable: LadderProgram["variables"][number]
    ) => {
      if (!reference) {
        return;
      }
      const key = symbolKey(reference);
      const normalizedAddress = normalizeAddress(variable.address);
      if (variable.type === "BOOL") {
        boolVariables.add(key);
        if (normalizedAddress?.startsWith("%IX")) {
          inputVariables.add(key);
        } else if (normalizedAddress?.startsWith("%QX")) {
          outputVariables.add(key);
        } else {
          memoryVariables.add(key);
        }
      } else {
        numericVariables.add(key);
        memoryVariables.add(key);
      }
    };

    for (const variable of program.variables) {
      const declaredRef = symbolReferenceForVariable(variable);
      const key = symbolKey(declaredRef || variable.name || "");
      if (!key) {
        continue;
      }
      const normalizedAddress = normalizeAddress(variable.address);
      const name = variable.name.trim();
      const scope = toScope(variable.scope);
      variableLookup.set(key, {
        name,
        declaredRef: declaredRef || variable.name.trim(),
        scope,
        address: normalizedAddress,
        type: variable.type,
      });
      registerVariableCategory(declaredRef || variable.name.trim(), variable);
      if (normalizedAddress && declaredRef) {
        addressAlias.set(normalizedAddress, declaredRef);
      }
    }

    const classifyAddress = (
      address: string
    ): "inputs" | "outputs" | "memory" => {
      if (address.startsWith("%IX")) {
        return "inputs";
      }
      if (address.startsWith("%QX")) {
        return "outputs";
      }
      return "memory";
    };

    const classifySymbol = (
      reference: string,
      preferred?: "inputs" | "outputs" | "memory"
    ): "inputs" | "outputs" | "memory" => {
      const parsed = parseQualifiedSymbol(reference);
      const key = symbolKey(
        parsed.scope ? `${parsed.scope.toUpperCase()}::${parsed.name}` : parsed.name
      );
      if (inputVariables.has(key)) {
        return "inputs";
      }
      if (outputVariables.has(key)) {
        return "outputs";
      }
      if (memoryVariables.has(key)) {
        return "memory";
      }
      if (boolVariables.has(key)) {
        return preferred ?? "memory";
      }
      if (numericVariables.has(key)) {
        return "memory";
      }
      return "memory";
    };

    const runtimeKey = (value: string): string => runtimeIoEntryKey(value);

    const addEntry = (
      bucket: Map<string, RuntimePanelIoEntry>,
      target: string,
      name?: string,
      writeTargetRaw?: string
    ) => {
      const address = canonicalRuntimeIoTarget(target);
      const writeTarget = canonicalRuntimeIoTarget(writeTargetRaw ?? address);
      if (!address || !writeTarget) {
        return;
      }
      const key = runtimeKey(writeTarget);
      if (bucket.has(key)) {
        const existing = bucket.get(key);
        if (existing && !existing.name && name?.trim()) {
          existing.name = name.trim();
        }
        return;
      }
      bucket.set(key, {
        address,
        writeTarget,
        name: name?.trim() || address,
        value: "",
      });
    };

    const addReference = (
      raw: unknown,
      preferred?: "inputs" | "outputs" | "memory"
    ) => {
      if (typeof raw !== "string") {
        return;
      }
      const token = raw.trim();
      if (!token) {
        return;
      }

      const normalizedAddress = normalizeAddress(token);
      if (normalizedAddress) {
        const bucketName = classifyAddress(normalizedAddress);
        const label = addressAlias.get(normalizedAddress) ?? normalizedAddress;
        const bucket =
          bucketName === "inputs"
            ? inputs
            : bucketName === "outputs"
              ? outputs
              : memory;
        addEntry(bucket, normalizedAddress, label, normalizedAddress);
        return;
      }

      const parsed = parseQualifiedSymbol(token);
      let explicitScopeRef = parsed.scope
        ? `${parsed.scope.toUpperCase()}::${parsed.name}`
        : parsed.name;
      const key = symbolKey(explicitScopeRef);
      const lookupKey = key || symbolKey(token);
      let info = variableLookup.get(lookupKey);
      if (info) {
        explicitScopeRef = info.declaredRef;
      }
      if (!info && !parsed.scope) {
        const localRef = `LOCAL::${parsed.name}`;
        const localInfo = variableLookup.get(symbolKey(localRef));
        if (localInfo) {
          info = localInfo;
          explicitScopeRef = localInfo.declaredRef;
        } else {
          const globalRef = `GLOBAL::${parsed.name}`;
          const globalInfo = variableLookup.get(symbolKey(globalRef));
          if (globalInfo) {
            info = globalInfo;
            explicitScopeRef = globalInfo.declaredRef;
          }
        }
      }
      const target = info?.address ?? explicitScopeRef;
      const name = info?.declaredRef ?? explicitScopeRef;
      let writeTarget = info?.address;
      if (!writeTarget) {
        if (info) {
          writeTarget =
            info.scope === "local" && runtimeFbInstanceName
              ? `${runtimeFbInstanceName}.${info.name}`
              : info.name;
        } else if (parsed.scope === "local" && runtimeFbInstanceName) {
          writeTarget = `${runtimeFbInstanceName}.${parsed.name}`;
        } else if (parsed.scope === "global") {
          writeTarget = parsed.name;
        } else {
          writeTarget = explicitScopeRef;
        }
      }
      const bucketName = classifySymbol(explicitScopeRef, preferred);
      const bucket =
        bucketName === "inputs"
          ? inputs
          : bucketName === "outputs"
            ? outputs
            : memory;
      addEntry(bucket, target, name, writeTarget);
    };

    for (const variable of program.variables) {
      const declaredRef = symbolReferenceForVariable(variable);
      if (!declaredRef) {
        continue;
      }
      addReference(declaredRef);
    }

    for (const network of program.networks) {
      for (const node of network.nodes) {
        if ("variable" in node) {
          const preferred = node.type === "contact" ? "inputs" : "outputs";
          addReference(node.variable, preferred);
        }
        if ("input" in node) {
          addReference(node.input, "inputs");
        }
        if ("qOutput" in node) {
          addReference(node.qOutput, "outputs");
        }
        if ("etOutput" in node) {
          addReference(node.etOutput, "memory");
        }
        if ("cvOutput" in node) {
          addReference(node.cvOutput, "memory");
        }
        if ("left" in node) {
          addReference(node.left, "memory");
        }
        if ("right" in node) {
          addReference(node.right, "memory");
        }
        if ("output" in node) {
          addReference(node.output, "memory");
        }
      }
    }

    return { inputs, outputs, memory };
  }

  private buildRuntimePanelIoState(
    docId: string,
    document?: vscode.TextDocument
  ): RuntimePanelIoState {
    const program = this.currentProgramForDoc(docId, document);
    const addresses = this.collectProgramIoAddresses(program, document);
    const execution = this.activeExecutions.get(docId);
    const snapshot = execution?.engine.getExecutionState() as
      | LadderExecutionSnapshot
      | undefined;

    const inputs = toBooleanRecord(snapshot?.inputs);
    const forcedInputs = toBooleanRecord(snapshot?.forcedInputs);
    const outputs = toBooleanRecord(snapshot?.outputs);
    const markers = toBooleanRecord(snapshot?.markers);
    const memoryWords = toNumberRecord(snapshot?.memoryWords);
    const variableBooleans = toBooleanRecord(snapshot?.variableBooleans);
    const variableNumbers = toNumberRecord(snapshot?.variableNumbers);

    const ensureEntry = (
      bucket: Map<string, RuntimePanelIoEntry>,
      target: string,
      writeTargetRaw?: string
    ) => {
      const address = canonicalRuntimeIoTarget(target);
      const writeTarget = canonicalRuntimeIoTarget(writeTargetRaw ?? address);
      if (!address || !writeTarget) {
        return;
      }
      const key = runtimeIoEntryKey(writeTarget);
      if (bucket.has(key)) {
        return;
      }
      bucket.set(key, {
        address,
        writeTarget,
        name: address,
        value: "",
      });
    };

    const hasEntry = (
      bucket: Map<string, RuntimePanelIoEntry>,
      target: string
    ): boolean => {
      const key = runtimeIoEntryKey(target);
      if (!key) {
        return false;
      }
      if (bucket.has(key)) {
        return true;
      }
      for (const entry of bucket.values()) {
        if (
          runtimeIoEntryKey(entry.address) === key ||
          runtimeIoEntryKey(entry.name ?? "") === key
        ) {
          return true;
        }
      }
      return false;
    };

    for (const target of Object.keys(inputs)) {
      ensureEntry(addresses.inputs, target);
    }
    for (const target of Object.keys(outputs)) {
      ensureEntry(addresses.outputs, target);
    }
    for (const target of Object.keys(markers)) {
      ensureEntry(addresses.memory, target);
    }
    for (const target of Object.keys(memoryWords)) {
      ensureEntry(addresses.memory, target);
    }
    for (const target of Object.keys(variableBooleans)) {
      if (
        !hasEntry(addresses.inputs, target) &&
        !hasEntry(addresses.outputs, target) &&
        !hasEntry(addresses.memory, target)
      ) {
        ensureEntry(addresses.memory, target);
      }
    }
    for (const target of Object.keys(variableNumbers)) {
      if (!hasEntry(addresses.memory, target)) {
        ensureEntry(addresses.memory, target);
      }
    }

    const sortedEntries = (
      source: Map<string, RuntimePanelIoEntry>
    ): RuntimePanelIoEntry[] =>
      Array.from(source.values()).sort((left, right) =>
        (left.address || "").localeCompare(right.address || "")
      );

    const inputEntries = sortedEntries(addresses.inputs).map((entry) => ({
      ...entry,
      value: boolToDisplay(
        valueForRuntimeKey(
          inputs,
          entry.address,
          entry.name,
          entry.writeTarget
        ) ??
          valueForRuntimeKey(
            variableBooleans,
            entry.address,
            entry.name,
            entry.writeTarget
          ) ??
          false
      ),
      forced:
        valueForRuntimeKey(
          forcedInputs,
          entry.address,
          entry.name,
          entry.writeTarget
        ) ?? false,
    }));

    const outputEntries = sortedEntries(addresses.outputs).map((entry) => ({
      ...entry,
      value: boolToDisplay(
        valueForRuntimeKey(
          outputs,
          entry.address,
          entry.name,
          entry.writeTarget
        ) ??
          valueForRuntimeKey(
            variableBooleans,
            entry.address,
            entry.name,
            entry.writeTarget
          ) ??
          false
      ),
    }));

    const memoryEntries = sortedEntries(addresses.memory).map((entry) => {
      if (entry.address.startsWith("%MW")) {
        return {
          ...entry,
          value: `INT(${
            valueForRuntimeKey(
              memoryWords,
              entry.address,
              entry.name,
              entry.writeTarget
            ) ??
            valueForRuntimeKey(
              variableNumbers,
              entry.address,
              entry.name,
              entry.writeTarget
            ) ??
            0
          })`,
        };
      }

      if (entry.address.startsWith("%MX")) {
        return {
          ...entry,
          value: boolToDisplay(
            valueForRuntimeKey(
              markers,
              entry.address,
              entry.name,
              entry.writeTarget
            ) ??
              valueForRuntimeKey(
                variableBooleans,
                entry.address,
                entry.name,
                entry.writeTarget
              ) ??
              false
          ),
        };
      }

      const numericValue = valueForRuntimeKey(
        variableNumbers,
        entry.address,
        entry.name,
        entry.writeTarget
      );
      if (numericValue !== undefined) {
        return {
          ...entry,
          value: `INT(${numericValue})`,
        };
      }

      return {
        ...entry,
        value: boolToDisplay(
          valueForRuntimeKey(
            variableBooleans,
            entry.address,
            entry.name,
            entry.writeTarget
          ) ?? false
        ),
      };
    });

    return {
      inputs: inputEntries,
      outputs: outputEntries,
      memory: memoryEntries,
    };
  }

  private postRuntimePanelIoState(
    docId: string,
    webviewPanel: vscode.WebviewPanel,
    document?: vscode.TextDocument
  ): void {
    const ioState = this.ensureRuntimeIoState(docId, document);
    webviewPanel.webview.postMessage({
      type: "ioState",
      payload: this.annotateRuntimeIoStateWithOperations(docId, ioState),
    });
  }

  private postRuntimePanelSettings(
    resource: vscode.Uri,
    webviewPanel: vscode.WebviewPanel
  ): void {
    webviewPanel.webview.postMessage({
      type: "settings",
      payload: collectRuntimePanelSettings(resource),
    });
  }

  private upsertRuntimeIoEntry(
    docId: string,
    document: vscode.TextDocument,
    targetRaw: string,
    value: string,
    forced?: boolean
  ): RuntimePanelIoState {
    const normalizedTarget = canonicalRuntimeIoTarget(targetRaw);
    if (!normalizedTarget) {
      return this.ensureRuntimeIoState(docId, document);
    }
    const targetKey = runtimeIoEntryKey(normalizedTarget);
    const ioState = this.ensureRuntimeIoState(docId, document);
    const bucketName = normalizedTarget.startsWith("%IX")
      ? "inputs"
      : normalizedTarget.startsWith("%QX")
        ? "outputs"
        : "memory";
    const bucket = ioState[bucketName];
    let entry = bucket.find((candidate) => {
      const writeKey = runtimeIoEntryKey(candidate.writeTarget ?? "");
      const addressKey = runtimeIoEntryKey(candidate.address);
      return writeKey === targetKey || addressKey === targetKey;
    });
    if (!entry) {
      entry = {
        address: normalizedTarget,
        writeTarget: normalizedTarget,
        name: normalizedTarget,
        value,
      };
      bucket.push(entry);
      bucket.sort((left, right) => left.address.localeCompare(right.address));
    } else {
      entry.value = value;
      entry.writeTarget = entry.writeTarget || normalizedTarget;
    }
    if (forced !== undefined) {
      entry.forced = forced;
    }
    this.runtimeIoState.set(docId, ioState);
    return ioState;
  }

  private releaseRuntimeIoEntry(
    docId: string,
    document: vscode.TextDocument,
    targetRaw: string
  ): RuntimePanelIoState {
    const normalizedTarget = canonicalRuntimeIoTarget(targetRaw);
    if (!normalizedTarget) {
      return this.ensureRuntimeIoState(docId, document);
    }
    const targetKey = runtimeIoEntryKey(normalizedTarget);
    const ioState = this.ensureRuntimeIoState(docId, document);
    for (const bucket of [ioState.inputs, ioState.outputs, ioState.memory]) {
      const entry = bucket.find(
        (candidate) => {
          const writeKey = runtimeIoEntryKey(candidate.writeTarget ?? "");
          const addressKey = runtimeIoEntryKey(candidate.address);
          return writeKey === targetKey || addressKey === targetKey;
        }
      );
      if (entry) {
        entry.forced = false;
      }
    }
    this.runtimeIoState.set(docId, ioState);
    return ioState;
  }

  private ensureRuntimeIoEntry(
    docId: string,
    document: vscode.TextDocument,
    targetRaw: string
  ): RuntimePanelIoState {
    const normalizedTarget = canonicalRuntimeIoTarget(targetRaw);
    if (!normalizedTarget) {
      return this.ensureRuntimeIoState(docId, document);
    }
    const targetKey = runtimeIoEntryKey(normalizedTarget);
    const ioState = this.ensureRuntimeIoState(docId, document);
    const bucketName = normalizedTarget.startsWith("%IX")
      ? "inputs"
      : normalizedTarget.startsWith("%QX")
        ? "outputs"
        : "memory";
    const bucket = ioState[bucketName];
    const existing = bucket.find((candidate) => {
      const writeKey = runtimeIoEntryKey(candidate.writeTarget ?? "");
      const addressKey = runtimeIoEntryKey(candidate.address);
      return writeKey === targetKey || addressKey === targetKey;
    });
    if (existing) {
      return ioState;
    }
    bucket.push({
      address: normalizedTarget,
      writeTarget: normalizedTarget,
      name: normalizedTarget,
      value: "",
    });
    bucket.sort((left, right) => left.address.localeCompare(right.address));
    this.runtimeIoState.set(docId, ioState);
    return ioState;
  }

  private async handleRuntimePanelMessage(
    docId: string,
    document: vscode.TextDocument,
    message: RuntimePanelWebviewMessage,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    switch (message.type) {
      case "webviewReady":
        this.runtimeController.ensureState(docId);
        this.clearRuntimeIoOperations(docId);
        this.runtimeIoState.set(
          docId,
          this.buildRuntimePanelIoState(docId, document)
        );
        this.postRuntimeState(docId, document.uri, webviewPanel);
        this.postRuntimePanelSettings(document.uri, webviewPanel);
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        return;
      case "requestSettings":
        this.postRuntimePanelSettings(document.uri, webviewPanel);
        return;
      case "saveSettings":
        await applyRuntimePanelSettings(document.uri, message.payload ?? {});
        this.postRuntimePanelSettings(document.uri, webviewPanel);
        this.postRuntimeState(docId, document.uri, webviewPanel);
        webviewPanel.webview.postMessage({
          type: "status",
          payload: "Runtime settings saved.",
        });
        return;
      case "runtimeSetMode":
        await this.handleRuntimeMessage(
          docId,
          document,
          runtimeMessage.setMode(runtimePanelModeToUi(message.mode)),
          webviewPanel
        );
        return;
      case "runtimeStart": {
        const current = this.runtimeController.ensureState(docId);
        await this.handleRuntimeMessage(
          docId,
          document,
          current.isExecuting ? runtimeMessage.stop() : runtimeMessage.start(),
          webviewPanel
        );
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        void this.requestStructuredTextIoState();
        return;
      }
      case "writeInput": {
        const target = canonicalRuntimeIoTarget(message.address ?? "");
        if (!target) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Missing I/O address.",
          });
          return;
        }
        const parsed = parseBooleanInput(message.value ?? "");
        if (parsed === undefined) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Input writes require TRUE/FALSE.",
          });
          return;
        }
        this.ensureRuntimeIoEntry(docId, document, target);
        this.setPendingRuntimeIoOperation(docId, target);
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        try {
          await writeVisualRuntimeIo(target, parsed ? "TRUE" : "FALSE");
        } catch (error) {
          const details = error instanceof Error ? error.message : String(error);
          this.setRuntimeIoOperationError(docId, target, details);
          webviewPanel.webview.postMessage({
            type: "status",
            payload: `I/O write failed: ${details}`,
          });
          this.postRuntimePanelIoState(docId, webviewPanel, document);
          return;
        }
        webviewPanel.webview.postMessage({
          type: "status",
          payload: `Input write queued for ${target}.`,
        });
        this.applyRuntimeIoOperationSuccess(
          docId,
          document,
          target,
          "write",
          parsed
        );
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        void this.requestStructuredTextIoState();
        return;
      }
      case "forceInput": {
        const target = canonicalRuntimeIoTarget(message.address ?? "");
        if (!target) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Missing I/O address.",
          });
          return;
        }
        const parsed = parseBooleanInput(message.value ?? "");
        if (parsed === undefined) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Input force requires TRUE/FALSE.",
          });
          return;
        }
        this.ensureRuntimeIoEntry(docId, document, target);
        this.setPendingRuntimeIoOperation(docId, target);
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        try {
          await forceVisualRuntimeIo(target, parsed ? "TRUE" : "FALSE");
        } catch (error) {
          const details = error instanceof Error ? error.message : String(error);
          this.setRuntimeIoOperationError(docId, target, details);
          webviewPanel.webview.postMessage({
            type: "status",
            payload: `I/O force failed: ${details}`,
          });
          this.postRuntimePanelIoState(docId, webviewPanel, document);
          return;
        }
        webviewPanel.webview.postMessage({
          type: "status",
          payload: `Input force active at ${target}.`,
        });
        this.applyRuntimeIoOperationSuccess(
          docId,
          document,
          target,
          "force",
          parsed
        );
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        void this.requestStructuredTextIoState();
        return;
      }
      case "releaseInput": {
        const target = canonicalRuntimeIoTarget(message.address ?? "");
        if (!target) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Missing I/O address.",
          });
          return;
        }
        this.ensureRuntimeIoEntry(docId, document, target);
        this.setPendingRuntimeIoOperation(docId, target);
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        try {
          await releaseVisualRuntimeIo(target);
        } catch (error) {
          const details = error instanceof Error ? error.message : String(error);
          this.setRuntimeIoOperationError(docId, target, details);
          webviewPanel.webview.postMessage({
            type: "status",
            payload: `I/O release failed: ${details}`,
          });
          this.postRuntimePanelIoState(docId, webviewPanel, document);
          return;
        }
        webviewPanel.webview.postMessage({
          type: "status",
          payload: `Input force released at ${target}.`,
        });
        this.applyRuntimeIoOperationSuccess(
          docId,
          document,
          target,
          "release"
        );
        this.postRuntimePanelIoState(docId, webviewPanel, document);
        void this.requestStructuredTextIoState();
        return;
      }
    }
  }

  private async handleRuntimeMessage(
    docId: string,
    document: vscode.TextDocument,
    message: RuntimeWebviewToExtensionMessage,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    if (message.type === "runtime.setMode") {
      this.runtimeController.setMode(docId, message.mode);
      await persistRuntimeUiMode(document.uri, message.mode);
      this.postRuntimeState(docId, document.uri, webviewPanel);
      return;
    }

    if (message.type === "runtime.openPanel") {
      await this.runtimeController.openRuntimePanel();
      this.postRuntimeState(docId, document.uri, webviewPanel);
      return;
    }

    if (message.type === "runtime.openSettings") {
      await this.runtimeController.openRuntimeSettings();
      this.postRuntimeState(docId, document.uri, webviewPanel);
      return;
    }

    const adapter = {
      startLocal: async () => startVisualRuntime(document.uri, "local"),
      startExternal: async () => startVisualRuntime(document.uri, "external"),
      stop: async () => stopVisualRuntime(),
    };

    try {
      if (message.type === "runtime.start") {
        await this.syncLatestProgramForRuntime(docId, document);
        await this.runtimeController.start(docId, adapter);
      }
      if (message.type === "runtime.stop") {
        await this.runtimeController.stop(docId, adapter);
        this.clearRuntimeIoOperations(docId);
      }
    } catch (error) {
      const details = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(details);
      webviewPanel.webview.postMessage(runtimeMessage.error(details));
    } finally {
      this.postRuntimeState(docId, document.uri, webviewPanel);
      void this.requestStructuredTextIoState();
    }
  }

  private normalizeProgramForPersistence(
    program: LadderProgram,
    touchModified: boolean
  ): LadderProgram {
    return {
      ...program,
      schemaVersion: 2,
      networks: [...program.networks].sort((left, right) => left.order - right.order),
      metadata: {
        ...program.metadata,
        modified: touchModified
          ? new Date().toISOString()
          : program.metadata.modified ?? new Date().toISOString(),
      },
    };
  }

  private async replaceDocumentContent(
    document: vscode.TextDocument,
    text: string
  ): Promise<boolean> {
    if (document.getText() === text) {
      return false;
    }
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), text);
    await vscode.workspace.applyEdit(edit);
    return true;
  }

  private async syncLatestProgramForRuntime(
    docId: string,
    document: vscode.TextDocument
  ): Promise<void> {
    const latest = this.latestPrograms.get(docId);
    if (!latest) {
      return;
    }
    const normalized = this.normalizeProgramForPersistence(latest, false);
    const json = `${JSON.stringify(normalized, null, 2)}\n`;
    const changed = await this.replaceDocumentContent(document, json);
    if (changed) {
      await document.save();
      this.latestPrograms.set(docId, normalized);
      this.runtimeIoState.set(docId, this.buildRuntimePanelIoState(docId, document));
    }
  }

  /**
   * Load ladder program from document
   */
  private loadProgram(document: vscode.TextDocument): LadderProgram {
    const text = document.getText();
    if (!text.trim()) {
      // Return empty program if document is empty
      return createEmptyProgram();
    }

    try {
      const parsed = JSON.parse(text);
      const validated = validateLadderProgramValue(parsed);

      return {
        ...validated,
        metadata: {
          name: validated.metadata.name ?? "Ladder Program",
          description: validated.metadata.description ?? "Ladder logic program",
          created: validated.metadata.created,
          modified: validated.metadata.modified,
        },
      };
    } catch (error) {
      vscode.window.showErrorMessage(`Failed to parse ladder program: ${error}`);
      return createEmptyProgram("Invalid Ladder Program");
    }
  }

  /**
   * Save ladder program to document
   */
  private async saveProgram(
    document: vscode.TextDocument,
    program: LadderProgram,
    options: { notify?: boolean } = {}
  ): Promise<LadderProgram> {
    const normalized = this.normalizeProgramForPersistence(program, true);
    const json = `${JSON.stringify(normalized, null, 2)}\n`;
    await this.replaceDocumentContent(document, json);
    await document.save();
    if (options.notify !== false) {
      vscode.window.showInformationMessage("Ladder program saved");
    }
    return normalized;
  }

  /**
   * Run in simulation mode
   */
  private async runSimulation(docId: string, program: LadderProgram, webviewPanel: vscode.WebviewPanel): Promise<void> {
    console.log("[Ladder] Starting simulation mode");
    
    // Create ladder engine
    const engine = new LadderEngine(program, "simulation", {
      scanCycleMs: 100, // 100ms scan cycle
    });

    // Set up state change callback to send updates to webview
    engine.setStateChangeCallback((state) => {
      webviewPanel.webview.postMessage({
        type: "stateUpdate",
        state,
      });
      this.postRuntimePanelIoState(docId, webviewPanel);
    });

    this.activeExecutions.set(docId, {
      program,
      mode: "simulation",
      isRunning: true,
      engine,
      webviewPanel,
    });

    // Start execution
    await engine.start();

    // Notify webview
    webviewPanel.webview.postMessage({ type: "executionStarted", mode: "simulation" });
    this.postRuntimePanelIoState(docId, webviewPanel);
    
    vscode.window.showInformationMessage("🚀 Ladder simulation started (100ms scan cycle)");
  }

  /**
   * Run in hardware mode
   */
  private async runHardware(docId: string, program: LadderProgram, webviewPanel: vscode.WebviewPanel): Promise<void> {
    console.log("[Ladder] Starting hardware mode");
    // Get runtime configuration
    const config = await getRuntimeConfig();
    if (!config) {
      throw new Error(
        "No runtime configuration found. Please configure trust-runtime connection."
      );
    }

    // Create and connect runtime client
    const runtimeClient = new RuntimeClient(config);
    await runtimeClient.connect();

    // Create ladder engine with hardware mode
    const engine = new LadderEngine(program, "hardware", {
      scanCycleMs: 100,
      runtimeClient,
    });

    // Set up state change callback
    engine.setStateChangeCallback((state) => {
      webviewPanel.webview.postMessage({
        type: "stateUpdate",
        state,
      });
      this.postRuntimePanelIoState(docId, webviewPanel);
    });

    this.activeExecutions.set(docId, {
      program,
      mode: "hardware",
      isRunning: true,
      engine,
      webviewPanel,
    });

    // Start execution
    await engine.start();

    // Notify webview
    webviewPanel.webview.postMessage({ type: "executionStarted", mode: "hardware" });
    this.postRuntimePanelIoState(docId, webviewPanel);

    vscode.window.showInformationMessage("🔧 Ladder hardware execution started");
  }

  /**
   * Stop execution
   */
  private async stopExecution(docId: string, notify = true): Promise<void> {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    console.log("[Ladder] Stopping execution");
    state.isRunning = false;

    // Stop engine and cleanup
    await state.engine.cleanup();
    
    // Notify webview
    state.webviewPanel.webview.postMessage({ type: "executionStopped" });
    this.postRuntimePanelIoState(docId, state.webviewPanel);
    
    this.activeExecutions.delete(docId);
    
    if (notify) {
      vscode.window.showInformationMessage("⏹️ Ladder execution stopped");
    }
  }

  /**
   * Get HTML content for webview
   */
  private getHtmlForWebview(webview: vscode.Webview): string {
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "media", "ladderWebview.js")
      )
    );

    const styleUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "media", "ladderWebview.css")
      )
    );

    const cspSource = webview.cspSource;

    return WEBVIEW_HTML_TEMPLATE.replace(/{{cspSource}}/g, cspSource)
      .replace("{{webviewScript}}", scriptUri.toString())
      .replace("{{webviewStyle}}", styleUri.toString());
  }
}
