import * as vscode from "vscode";
import * as path from "path";
import { StateMachineEngine } from "./stateMachineEngine";
import { RuntimeClient, getRuntimeConfig } from "./runtimeClient";
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

type ExecutionMode = "simulation" | "hardware";

interface SimulatorEntry {
  simulator: StateMachineEngine;
  timer?: NodeJS.Timeout;
  mode: ExecutionMode;
  runtimeClient?: RuntimeClient;
}

const IO_ADDRESS_PATTERN = /%[IQM][XWDLB]\d+(?:\.\d+)?/gi;

function parseRuntimeLiteral(value: string): boolean | number | string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  const boolSource =
    trimmed.startsWith("BOOL(") && trimmed.endsWith(")")
      ? trimmed.slice(5, -1).trim()
      : trimmed;
  const upperBool = boolSource.toUpperCase();
  if (upperBool === "TRUE" || upperBool === "1") {
    return true;
  }
  if (upperBool === "FALSE" || upperBool === "0") {
    return false;
  }

  const wrapperMatch = trimmed.match(/^[A-Za-z_][A-Za-z0-9_]*\((.*)\)$/);
  const numericSource = wrapperMatch ? wrapperMatch[1].trim() : trimmed;
  if (/^-?\d+(?:\.\d+)?$/.test(numericSource)) {
    return Number(numericSource);
  }

  return trimmed;
}

function isBitAddress(address: string): boolean {
  return /^%[IQM]X/i.test(address);
}

function classifyAddress(
  address: string
): "inputs" | "outputs" | "memory" | undefined {
  if (/^%IX/i.test(address)) {
    return "inputs";
  }
  if (/^%Q/i.test(address)) {
    return "outputs";
  }
  if (/^%M/i.test(address)) {
    return "memory";
  }
  return undefined;
}

function formatDisplayValue(address: string, value: unknown): string {
  if (isBitAddress(address)) {
    if (typeof value === "boolean") {
      return boolToDisplay(value);
    }
    if (typeof value === "number") {
      return boolToDisplay(value !== 0);
    }
    if (typeof value === "string") {
      const upper = value.trim().toUpperCase();
      if (upper === "TRUE" || upper === "1") {
        return boolToDisplay(true);
      }
      if (upper === "FALSE" || upper === "0") {
        return boolToDisplay(false);
      }
    }
    return boolToDisplay(false);
  }

  if (typeof value === "number" && Number.isFinite(value)) {
    return `INT(${value})`;
  }
  if (typeof value === "boolean") {
    return boolToDisplay(value);
  }
  if (typeof value === "string" && value.trim()) {
    return value.trim();
  }
  return "INT(0)";
}

const WEBVIEW_HTML_TEMPLATE = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; img-src {{cspSource}} data:; style-src {{cspSource}} 'unsafe-inline'; script-src {{cspSource}};"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>StateChart Editor</title>
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
        background-color: var(--vscode-editor-background, #1e1e1e);
        color: var(--vscode-editor-foreground, var(--vscode-foreground, #cccccc));
      }

      .react-flow__controls button {
        background-color: var(--vscode-button-background, #0e639c);
        color: var(--vscode-button-foreground, #ffffff);
        border-color: var(--vscode-button-border, var(--vscode-panel-border, #2b2b2b));
      }

      .react-flow__controls button:hover {
        background-color: var(--vscode-button-hoverBackground, #1177bb);
      }
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script src="{{webviewScript}}"></script>
  </body>
</html>`;

/**
 * Custom Editor Provider for StateChart JSON files
 * Provides a visual editor for .statechart.json files
 */
export class StateChartEditorProvider
  implements vscode.CustomTextEditorProvider
{
  private simulators: Map<string, SimulatorEntry> = new Map();
  private runtimeIoState = new Map<string, RuntimePanelIoState>();
  private readonly runtimeController = new RuntimeController({
    openPanel: () => vscode.commands.executeCommand("trust-lsp.debug.openIoPanel"),
    openSettings: () =>
      vscode.commands.executeCommand("trust-lsp.debug.openIoPanelSettings"),
  });
  public static register(context: vscode.ExtensionContext): vscode.Disposable {
    const provider = new StateChartEditorProvider(context);
    const providerRegistration = vscode.window.registerCustomEditorProvider(
      StateChartEditorProvider.viewType,
      provider,
      {
        webviewOptions: {
          retainContextWhenHidden: true,
        },
        supportsMultipleEditorsPerDocument: false,
      }
    );
    return providerRegistration;
  }

  private static readonly viewType = "trust-lsp.statechartEditor";

  constructor(private readonly context: vscode.ExtensionContext) {}

  /**
   * Called when a custom editor is opened
   */
  public async resolveCustomTextEditor(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken
  ): Promise<void> {
    const docId = document.uri.toString();

    // Setup webview
    webviewPanel.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.file(path.join(this.context.extensionPath, "media")),
      ],
    };

    webviewPanel.webview.html = this.getHtmlForWebview(webviewPanel.webview);

    // Helper function to update webview
    function updateWebview() {
      void webviewPanel.webview.postMessage({
        type: "update",
        content: document.getText(),
      });
    }

    // Hook up event handlers
    const changeDocumentSubscription = vscode.workspace.onDidChangeTextDocument(
      (e) => {
        if (e.document.uri.toString() === document.uri.toString()) {
          updateWebview();
          this.runtimeIoState.set(docId, this.buildIoStateFromDocument(document));
          this.postRuntimePanelIoState(docId, document, webviewPanel);
        }
      }
    );

    // Make sure we get rid of the listener when our editor is closed
    const messageSubscription = webviewPanel.webview.onDidReceiveMessage(
      (message) => {
        if (isRuntimePanelWebviewMessage(message)) {
          void this.handleRuntimePanelMessage(docId, document, message, webviewPanel);
          return;
        }

        if (isRuntimeWebviewMessage(message)) {
          void this.handleRuntimeMessage(docId, document, message, webviewPanel);
          return;
        }

        switch (message.type) {
          case "save":
            void this.saveDocument(document, message.content);
            return;

          case "ready":
            updateWebview();
            this.runtimeController.ensureState(docId);
            this.runtimeController.setMode(
              docId,
              runtimeUiModeFromConfig(document.uri)
            );
            this.postRuntimeState(docId, document.uri, webviewPanel);
            this.postRuntimePanelSettings(document.uri, webviewPanel);
            this.postRuntimePanelIoState(docId, document, webviewPanel);
            return;

          case "error":
            void vscode.window.showErrorMessage(
              `StateChart Editor Error: ${message.error}`
            );
            return;

          // Backward compatibility for older webviews.
          case "startExecution":
            void this.handleRuntimeMessage(
              docId,
              document,
              runtimeMessage.setMode(
                message.mode === "hardware" ? "external" : "local"
              ),
              webviewPanel
            ).then(() =>
              this.handleRuntimeMessage(
                docId,
                document,
                runtimeMessage.start(),
                webviewPanel
              )
            );
            return;

          case "stopExecution":
            void this.handleRuntimeMessage(
              docId,
              document,
              runtimeMessage.stop(),
              webviewPanel
            );
            return;

          case "sendEvent":
            void this.sendEvent(docId, message.event, webviewPanel);
            return;
        }
      }
    );

    // Make sure we get rid of listeners and running execution when editor closes.
    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
      messageSubscription.dispose();
      void this.stopExecution(docId, false);
      this.runtimeController.clear(docId);
      this.runtimeIoState.delete(docId);
    });

    // Send initial content
    updateWebview();
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

  private buildIoStateFromDocument(
    document: vscode.TextDocument
  ): RuntimePanelIoState {
    const inputs = new Set<string>();
    const outputs = new Set<string>();
    const memory = new Set<string>();
    const text = document.getText();
    for (const match of text.matchAll(IO_ADDRESS_PATTERN)) {
      const address = match[0].toUpperCase();
      const bucket = classifyAddress(address);
      if (bucket === "inputs") {
        inputs.add(address);
      } else if (bucket === "outputs") {
        outputs.add(address);
      } else if (bucket === "memory") {
        memory.add(address);
      }
    }

    const mapAddress = (address: string): RuntimePanelIoEntry => ({
      address,
      name: address,
      value: formatDisplayValue(address, isBitAddress(address) ? false : 0),
      forced: false,
    });

    return {
      inputs: Array.from(inputs).sort().map(mapAddress),
      outputs: Array.from(outputs).sort().map(mapAddress),
      memory: Array.from(memory).sort().map(mapAddress),
    };
  }

  private ensureRuntimeIoState(
    docId: string,
    document: vscode.TextDocument
  ): RuntimePanelIoState {
    const existing = this.runtimeIoState.get(docId);
    if (existing) {
      return existing;
    }
    const created = this.buildIoStateFromDocument(document);
    this.runtimeIoState.set(docId, created);
    return created;
  }

  private postRuntimePanelIoState(
    docId: string,
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel
  ): void {
    webviewPanel.webview.postMessage({
      type: "ioState",
      payload: this.ensureRuntimeIoState(docId, document),
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
    addressRaw: string,
    value: unknown,
    forced?: boolean
  ): RuntimePanelIoState {
    const ioState = this.ensureRuntimeIoState(docId, document);
    const address = addressRaw.trim().toUpperCase();
    const bucket = classifyAddress(address) ?? "memory";
    const entries = ioState[bucket];
    let entry = entries.find((candidate) => candidate.address === address);
    if (!entry) {
      entry = {
        name: address,
        address,
        value: formatDisplayValue(address, value),
      };
      entries.push(entry);
      entries.sort((left, right) => left.address.localeCompare(right.address));
    } else {
      entry.value = formatDisplayValue(address, value);
    }
    if (forced !== undefined) {
      entry.forced = forced;
    }
    return ioState;
  }

  private releaseRuntimeIoEntry(
    docId: string,
    document: vscode.TextDocument,
    addressRaw: string
  ): RuntimePanelIoState {
    const ioState = this.ensureRuntimeIoState(docId, document);
    const address = addressRaw.trim().toUpperCase();
    for (const bucket of [ioState.inputs, ioState.outputs, ioState.memory]) {
      const entry = bucket.find((candidate) => candidate.address === address);
      if (entry) {
        entry.forced = false;
      }
    }
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
        this.postRuntimeState(docId, document.uri, webviewPanel);
        this.postRuntimePanelSettings(document.uri, webviewPanel);
        this.postRuntimePanelIoState(docId, document, webviewPanel);
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
        this.postRuntimePanelIoState(docId, document, webviewPanel);
        return;
      }
      case "writeInput":
      case "forceInput": {
        const address = (message.address ?? "").trim().toUpperCase();
        if (!address) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Missing I/O address.",
          });
          return;
        }
        const value = parseRuntimeLiteral(message.value ?? "");
        this.upsertRuntimeIoEntry(
          docId,
          document,
          address,
          value,
          message.type === "forceInput" ? true : undefined
        );

        try {
          if (message.type === "forceInput") {
            await forceVisualRuntimeIo(address, message.value ?? "");
          } else {
            await writeVisualRuntimeIo(address, message.value ?? "");
          }
        } catch (error) {
          const details = error instanceof Error ? error.message : String(error);
          webviewPanel.webview.postMessage({
            type: "status",
            payload: `I/O update failed: ${details}`,
          });
          this.postRuntimePanelIoState(docId, document, webviewPanel);
          return;
        }

        webviewPanel.webview.postMessage({
          type: "status",
          payload:
            message.type === "forceInput"
              ? `I/O force active at ${address}.`
              : `I/O write queued for ${address}.`,
        });
        this.postRuntimePanelIoState(docId, document, webviewPanel);
        return;
      }
      case "releaseInput": {
        const address = (message.address ?? "").trim().toUpperCase();
        if (!address) {
          webviewPanel.webview.postMessage({
            type: "status",
            payload: "Missing I/O address.",
          });
          return;
        }

        this.releaseRuntimeIoEntry(docId, document, address);
        try {
          await releaseVisualRuntimeIo(address);
        } catch (error) {
          const details = error instanceof Error ? error.message : String(error);
          webviewPanel.webview.postMessage({
            type: "status",
            payload: `I/O release failed: ${details}`,
          });
          this.postRuntimePanelIoState(docId, document, webviewPanel);
          return;
        }
        webviewPanel.webview.postMessage({
          type: "status",
          payload: `I/O force released at ${address}.`,
        });
        this.postRuntimePanelIoState(docId, document, webviewPanel);
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
        await this.runtimeController.start(docId, adapter);
      }
      if (message.type === "runtime.stop") {
        await this.runtimeController.stop(docId, adapter);
      }
    } catch (error) {
      const details = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(details);
      webviewPanel.webview.postMessage(runtimeMessage.error(details));
    } finally {
      this.postRuntimeState(docId, document.uri, webviewPanel);
    }
  }

  /**
   * Get the HTML content for the webview
   */
  private getHtmlForWebview(webview: vscode.Webview): string {
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "media", "stateChartWebview.js")
      )
    );

    const cssUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "media", "stateChartWebview.css")
      )
    );

    let html = WEBVIEW_HTML_TEMPLATE;

    // Replace placeholders
    html = html.replace(/{{cspSource}}/g, webview.cspSource);
    html = html.replace(/{{webviewScript}}/g, scriptUri.toString());
    html = html.replace(/{{webviewStyle}}/g, cssUri.toString());

    return html;
  }

  /**
   * Update the document with new content from webview
   */
  private updateTextDocument(document: vscode.TextDocument, content: string) {
    const edit = new vscode.WorkspaceEdit();

    // Replace entire document
    edit.replace(
      document.uri,
      new vscode.Range(0, 0, document.lineCount, 0),
      content
    );

    return vscode.workspace.applyEdit(edit);
  }

  private async saveDocument(
    document: vscode.TextDocument,
    content: string
  ): Promise<void> {
    await this.updateTextDocument(document, content);
    await document.save();
  }

  /**
   * Start execution of the state machine
   */
  private async startExecution(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    mode: ExecutionMode
  ) {
    const docId = document.uri.toString();
    // Stop any existing execution
    await this.stopExecution(docId, false);

    let runtimeClient: RuntimeClient | undefined;

    // Hardware mode: connect to trust-runtime
    if (mode === "hardware") {
      const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
      const config = await getRuntimeConfig(workspaceFolder);

      if (!config) {
        throw new Error(
          "Hardware mode requires trust-runtime configuration. Set 'trust-lsp.runtime.controlEndpoint' in settings."
        );
      }

      runtimeClient = new RuntimeClient(config);
      await runtimeClient.connect();
      vscode.window.showInformationMessage(
        `✅ Connected to trust-runtime: ${config.controlEndpoint}`
      );
    }

    // Create new simulator
    const content = document.getText();
    const simulator = new StateMachineEngine(content, mode, runtimeClient);
    await simulator.initialize();

    // Send initial state
    const executionState = simulator.getExecutionState();
    webviewPanel.webview.postMessage({
      type: "executionState",
      state: executionState,
    });

    // Update state every 100ms (in case of auto-transitions or context changes)
    const timer = setInterval(() => {
      const state = simulator.getExecutionState();
      webviewPanel.webview.postMessage({
        type: "executionState",
        state,
      });
    }, 100);

    this.simulators.set(docId, { simulator, timer, mode, runtimeClient });
    this.runtimeIoState.set(docId, this.buildIoStateFromDocument(document));
    this.postRuntimePanelIoState(docId, document, webviewPanel);

    const modeText = mode === "simulation" ? "🖥️  Simulation" : "🔌 Hardware";
    vscode.window.showInformationMessage(`${modeText} execution started`);
  }

  /**
   * Stop execution of the state machine
   */
  private async stopExecution(docId: string, notify = true): Promise<void> {
    const entry = this.simulators.get(docId);
    if (!entry) {
      return;
    }

    if (entry.timer) {
      clearInterval(entry.timer);
    }

    try {
      // Cleanup forced I/O addresses.
      await entry.simulator.cleanup();
    } catch (error) {
      console.error("[StateChart] Failed to cleanup execution entry", error);
    } finally {
      // Disconnect from runtime if connected.
      if (entry.runtimeClient) {
        entry.runtimeClient.disconnect();
      }

      this.simulators.delete(docId);
    }

    if (notify) {
      const modeText = entry.mode === "simulation" ? "Simulation" : "Hardware";
      void vscode.window.showInformationMessage(`${modeText} execution stopped`);
    }
  }

  /**
   * Send an event to the running state machine
   */
  private async sendEvent(
    docId: string,
    event: string,
    webviewPanel: vscode.WebviewPanel
  ) {
    const entry = this.simulators.get(docId);
    if (!entry) {
      vscode.window.showWarningMessage("State machine is not running");
      return;
    }

    const success = await entry.simulator.sendEvent(event);
    if (success) {
      // Send updated state immediately
      const executionState = entry.simulator.getExecutionState();
      webviewPanel.webview.postMessage({
        type: "executionState",
        state: executionState,
      });
    } else {
      vscode.window.showWarningMessage(
        `Event "${event}" not available in current state`
      );
    }
  }
}
