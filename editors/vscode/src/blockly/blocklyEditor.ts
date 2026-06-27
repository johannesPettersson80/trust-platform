import * as vscode from "vscode";
import * as path from "path";
import { BlocklyEngine, BlocklyWorkspace } from "./blocklyEngine";
import { BlocklyInterpreter } from "./blocklyInterpreter";
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

interface ExecutionState {
  workspaceData: BlocklyWorkspace;
  generatedCode: string;
  interpreter?: BlocklyInterpreter;
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
      content="default-src 'none'; img-src {{cspSource}} data: https:; style-src {{cspSource}} 'unsafe-inline'; script-src {{cspSource}} 'unsafe-eval';"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Blockly Editor</title>
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
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script src="{{webviewScript}}"></script>
  </body>
</html>
`;

/**
 * Provider for Blockly visual programming editor
 */
export class BlocklyEditorProvider implements vscode.CustomTextEditorProvider {
  private static readonly viewType = "trust-lsp.blockly.editor";
  private activeExecutions = new Map<string, ExecutionState>();
  private runtimeIoState = new Map<string, RuntimePanelIoState>();
  private readonly runtimeController = new RuntimeController({
    openPanel: () => vscode.commands.executeCommand("trust-lsp.debug.openIoPanel"),
    openSettings: () =>
      vscode.commands.executeCommand("trust-lsp.debug.openIoPanelSettings"),
  });

  public static register(context: vscode.ExtensionContext): vscode.Disposable {
    const provider = new BlocklyEditorProvider(context);
    const providerRegistration = vscode.window.registerCustomEditorProvider(
      BlocklyEditorProvider.viewType,
      provider,
      {
        webviewOptions: {
          retainContextWhenHidden: true,
        },
      }
    );
    return providerRegistration;
  }

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

    // Handle messages from webview
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
              message.error || "Could not open this Blockly program. Fix the file and try again."
            );
            return;

          case "generateCode":
            void this.generateCode(document, webviewPanel);
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

          case "executeBlock":
            void this.executeBlock(docId, message.blockId, webviewPanel);
            return;
        }
      }
    );

    // Clean up when editor closes
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

    const toEntry = (address: string): RuntimePanelIoEntry => ({
      address,
      name: address,
      value: formatDisplayValue(address, isBitAddress(address) ? false : 0),
      forced: false,
    });

    return {
      inputs: Array.from(inputs).sort().map(toEntry),
      outputs: Array.from(outputs).sort().map(toEntry),
      memory: Array.from(memory).sort().map(toEntry),
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
        address,
        name: address,
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
        path.join(this.context.extensionPath, "media", "blocklyWebview.js")
      )
    );

    const cssUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "media", "blocklyWebview.css")
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
   * Generate ST code from Blockly workspace
   */
  private async generateCode(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel
  ) {
    try {
      const workspaceData: BlocklyWorkspace = JSON.parse(document.getText());
      const engine = new BlocklyEngine();
      const result = engine.generateCode(workspaceData);

      if (result.errors.length > 0) {
        vscode.window.showWarningMessage(
          `Code generation completed with ${result.errors.length} warnings`
        );
        console.warn("Generation warnings:", result.errors);
      }

      // Send generated code to webview
      void webviewPanel.webview.postMessage({
        type: "codeGenerated",
        code: result.structuredText,
        variables: Array.from(result.variables.entries()),
        errors: result.errors,
      });

      // Optionally save to .st file
      const saveCode = await vscode.window.showQuickPick(
        ["Yes", "No"],
        { placeHolder: "Save generated ST code to file?" }
      );

      if (saveCode === "Yes") {
        const stFileName = document.uri.fsPath.replace(/\.blockly\.json$/, ".st");
        const stFileUri = vscode.Uri.file(stFileName);
        await vscode.workspace.fs.writeFile(
          stFileUri,
          Buffer.from(result.structuredText, "utf8")
        );
        vscode.window.showInformationMessage(`ST code saved to ${path.basename(stFileName)}`);
      }

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      vscode.window.showErrorMessage(`Code generation failed: ${errorMessage}`);
    }
  }

  /**
   * Start execution of the Blockly program
   */
  private async startExecution(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    mode: ExecutionMode
  ) {
    const docId = document.uri.toString();
    // Stop any existing execution
    await this.stopExecution(docId, false);

    const workspaceData: BlocklyWorkspace = JSON.parse(document.getText());
    const engine = new BlocklyEngine();
    const result = engine.generateCode(workspaceData);

    let runtimeClient: RuntimeClient | undefined;

    // Hardware mode: connect to trust-runtime
    if (mode === "hardware") {
      const config = getRuntimeConfig();

      runtimeClient = new RuntimeClient(config);
      await runtimeClient.connect();
      console.log(
        "Blockly: Connected to trust-runtime via Unix socket:",
        config.controlEndpoint
      );

      // Ensure runtime is in RUN mode (not paused)
      await runtimeClient.resume();
      console.log("Blockly: Runtime resumed (ready for I/O commands)");

      vscode.window.showInformationMessage(
        "✅ Connected to trust-runtime for hardware execution"
      );
    }

    // Create and start interpreter
    const interpreter = new BlocklyInterpreter(workspaceData, mode, runtimeClient);

    // Store execution state
    this.activeExecutions.set(docId, {
      workspaceData,
      generatedCode: result.structuredText,
      mode,
      runtimeClient,
      interpreter,
    });
    this.runtimeIoState.set(docId, this.buildIoStateFromDocument(document));
    this.postRuntimePanelIoState(docId, document, webviewPanel);

    // Send execution started message
    void webviewPanel.webview.postMessage({
      type: "executionStarted",
      mode,
      code: result.structuredText,
    });

    // Start execution
    void this.executeBlocksWithHighlight(workspaceData, interpreter, webviewPanel);

    vscode.window.showInformationMessage(
      `Blockly program execution started in ${mode} mode`
    );
  }

  /**
   * Execute blocks with visual highlighting in webview
   */
  private async executeBlocksWithHighlight(
    workspace: BlocklyWorkspace,
    interpreter: BlocklyInterpreter,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    // Hook into interpreter to send block execution events
    const originalExecute = interpreter['executeBlock'].bind(interpreter);
    
    interpreter['executeBlock'] = async (blockDef: any) => {
      console.log(`[blocklyEditor] Highlighting block: ${blockDef.id}`);
      
      // Highlight block in webview (non-blocking)
      void webviewPanel.webview.postMessage({
        type: "highlightBlock",
        blockId: blockDef.id,
      });

      // Execute the block immediately
      const result = await originalExecute(blockDef);

      // Unhighlight immediately (non-blocking)
      void webviewPanel.webview.postMessage({
        type: "unhighlightBlock",
        blockId: blockDef.id,
      });

      return result;
    };

    // Start execution
    await interpreter.start();
  }

  /**
   * Stop execution of the Blockly program
   */
  private async stopExecution(docId: string, showMessage = true) {
    const execution = this.activeExecutions.get(docId);
    
    if (!execution) {
      return;
    }

    // Stop interpreter
    if (execution.interpreter) {
      execution.interpreter.stop();
    }

    // Disconnect from runtime if connected
    if (execution.runtimeClient) {
      execution.runtimeClient.disconnect();
    }

    this.activeExecutions.delete(docId);

    if (showMessage) {
      vscode.window.showInformationMessage("Blockly program execution stopped");
    }
  }

  /**
   * Execute a specific block (for debugging/step mode)
   */
  private async executeBlock(
    docId: string,
    blockId: string,
    webviewPanel: vscode.WebviewPanel
  ) {
    const execution = this.activeExecutions.get(docId);
    
    if (!execution) {
      vscode.window.showErrorMessage("No active execution");
      return;
    }

    // Send block execution result to webview
    void webviewPanel.webview.postMessage({
      type: "blockExecuted",
      blockId,
    });
  }
}
