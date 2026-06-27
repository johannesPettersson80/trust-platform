import * as vscode from "vscode";
import * as path from "path";
import { SfcEngine, SfcWorkspace } from "./sfcEngine";
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

type ExecutionMode = "simulation" | "hardware";

interface ExecutionState {
  workspace: SfcWorkspace;
  mode: ExecutionMode;
  isRunning: boolean;
  engine: SfcEngine;
  webviewPanel: vscode.WebviewPanel;
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
    <title>SFC Editor</title>
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
 * Custom Editor Provider for SFC JSON files
 * Provides a visual editor for .sfc.json files
 */
export class SfcEditorProvider implements vscode.CustomTextEditorProvider {
  private static readonly viewType = "trust-lsp.sfc.editor";
  private activeExecutions = new Map<string, ExecutionState>();
  private runtimeIoState = new Map<string, RuntimePanelIoState>();
  private readonly runtimeController = new RuntimeController({
    openPanel: () => vscode.commands.executeCommand("trust-lsp.debug.openIoPanel"),
    openSettings: () =>
      vscode.commands.executeCommand("trust-lsp.debug.openIoPanelSettings"),
  });

  public static register(context: vscode.ExtensionContext): vscode.Disposable {
    const provider = new SfcEditorProvider(context);
    const providerRegistration = vscode.window.registerCustomEditorProvider(
      SfcEditorProvider.viewType,
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
    const changeDocumentSubscription =
      vscode.workspace.onDidChangeTextDocument((e) => {
        if (e.document.uri.toString() === document.uri.toString()) {
          updateWebview();
          this.runtimeIoState.set(docId, this.buildIoStateFromDocument(document));
          this.postRuntimePanelIoState(docId, document, webviewPanel);
        }
      });

    // Handle messages from webview
    const messageSubscription = webviewPanel.webview.onDidReceiveMessage(
      async (message) => {
        console.log("[SFC] Received message from webview:", message.type);
        
        if (isRuntimePanelWebviewMessage(message)) {
          await this.handleRuntimePanelMessage(docId, document, message, webviewPanel);
          return;
        }

        if (isRuntimeWebviewMessage(message)) {
          await this.handleRuntimeMessage(docId, document, message, webviewPanel);
          return;
        }

        switch (message.type) {
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

          case "save":
            await this.saveDocument(document, message.content);
            return;

          case "error":
            vscode.window.showErrorMessage(
              message.error || "Could not open this SFC. Fix the file and try again."
            );
            return;

          case "validate":
            await this.validateSfc(document, webviewPanel);
            return;

          case "generateST":
            await this.generateST(
              document,
              webviewPanel,
              "content" in message && typeof message.content === "string"
                ? message.content
                : undefined
            );
            return;

          case "debugPause":
            await this.handleDebugPause(docId);
            return;

          case "debugResume":
            await this.handleDebugResume(docId);
            return;

          case "debugStepOver":
            await this.handleDebugStepOver(docId);
            return;

          case "toggleBreakpoint":
            if ("stepId" in message) {
              await this.handleToggleBreakpoint(docId, message.stepId);
            }
            return;
        }
      }
    );

    // Clean up when webview is disposed
    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
      messageSubscription.dispose();
      void this.stopExecution(docId, false);
      this.runtimeController.clear(docId);
      this.runtimeIoState.delete(docId);
    });

    // Initial update
    updateWebview();
  }

  /**
   * Get the HTML content for the webview
   */
  private getHtmlForWebview(webview: vscode.Webview): string {
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(path.join(this.context.extensionPath, "media", "sfcWebview.js"))
    );
    const styleUri = webview.asWebviewUri(
      vscode.Uri.file(path.join(this.context.extensionPath, "media", "sfcWebview.css"))
    );

    const cspSource = webview.cspSource;

    return WEBVIEW_HTML_TEMPLATE
      .replace(/{{cspSource}}/g, cspSource)
      .replace("{{webviewScript}}", scriptUri.toString())
      .replace("{{webviewStyle}}", styleUri.toString());
  }

  /**
   * Save document changes
   */
  private async saveDocument(
    document: vscode.TextDocument,
    content: string
  ): Promise<void> {
    try {
      // Validate JSON
      JSON.parse(content);

      const edit = new vscode.WorkspaceEdit();
      edit.replace(
        document.uri,
        new vscode.Range(0, 0, document.lineCount, 0),
        content
      );

      const success = await vscode.workspace.applyEdit(edit);
      if (success) {
        await document.save();
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      vscode.window.showErrorMessage(`Failed to save SFC: ${message}`);
    }
  }

  /**
   * Validate the SFC
   */
  private async validateSfc(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    try {
      const workspace: SfcWorkspace = JSON.parse(document.getText());
      const engine = new SfcEngine(workspace);
      const errors = engine.validate();

      await webviewPanel.webview.postMessage({
        type: "validationResult",
        errors: errors,
      });

      if (errors.length === 0) {
        vscode.window.showInformationMessage("SFC validation successful!");
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      vscode.window.showErrorMessage(`Validation failed: ${message}`);
    }
  }

  /**
   * Generate Structured Text from SFC
   */
  private async generateST(
    document: vscode.TextDocument,
    webviewPanel?: vscode.WebviewPanel,
    contentOverride?: string
  ): Promise<void> {
    try {
      const sourceText = contentOverride ?? document.getText();
      const workspace: SfcWorkspace = JSON.parse(sourceText);
      const engine = new SfcEngine(workspace);
      const stCode = engine.generateStructuredText();

      // Determine the companion .st file path
      const sfcUri = document.uri;
      const sfcPath = sfcUri.fsPath;
      const stPath = sfcPath.replace(/\.sfc\.json$/, ".st");
      const stUri = vscode.Uri.file(stPath);

      // Write the ST file
      await vscode.workspace.fs.writeFile(stUri, Buffer.from(stCode, "utf-8"));

      // Open the generated file
      const stDoc = await vscode.workspace.openTextDocument(stUri);
      await vscode.window.showTextDocument(stDoc, vscode.ViewColumn.Beside);

      if (webviewPanel) {
        await webviewPanel.webview.postMessage({
          type: "codeGenerated",
          code: stCode,
          errors: [],
        });
      }

      vscode.window.showInformationMessage(
        "Structured Text code generated successfully!"
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (webviewPanel) {
        await webviewPanel.webview.postMessage({
          type: "codeGenerated",
          errors: [message],
        });
      }
      vscode.window.showErrorMessage(`Code generation failed: ${message}`);
    }
  }

  /**
   * Handle runtime messages
   */
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
      startLocal: async () =>
        this.startExecution(docId, document, webviewPanel, "simulation"),
      startExternal: async () =>
        this.startExecution(docId, document, webviewPanel, "hardware"),
      stop: async () => this.stopExecution(docId, false),
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
      this.postRuntimePanelIoState(docId, document, webviewPanel);
    }
  }

  /**
   * Handle runtime panel messages
   */
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
      case "runtimeStart":
        {
          const current = this.runtimeController.ensureState(docId);
          await this.handleRuntimeMessage(
            docId,
            document,
            current.isExecuting ? runtimeMessage.stop() : runtimeMessage.start(),
            webviewPanel
          );
        }
        this.postRuntimePanelIoState(docId, document, webviewPanel);
        break;

      case "runtimeSetMode":
        await this.handleRuntimeMessage(
          docId,
          document,
          runtimeMessage.setMode(runtimePanelModeToUi(message.mode)),
          webviewPanel
        );
        break;

      case "saveSettings":
        if (message.payload) {
          await applyRuntimePanelSettings(document.uri, message.payload);
        }
        this.postRuntimePanelSettings(document.uri, webviewPanel);
        this.postRuntimeState(docId, document.uri, webviewPanel);
        webviewPanel.webview.postMessage({
          type: "status",
          payload: "Runtime settings saved.",
        });
        break;

      case "writeInput":
        if (!this.activeExecutions.get(docId)) {
          return;
        }
        // TODO: Implement write input
        this.postRuntimePanelIoState(docId, document, webviewPanel);
        break;

      case "forceInput":
        if (!this.activeExecutions.get(docId)) {
          return;
        }
        // TODO: Implement force input
        this.postRuntimePanelIoState(docId, document, webviewPanel);
        break;

      case "releaseInput":
        if (!this.activeExecutions.get(docId)) {
          return;
        }
        // TODO: Implement release input
        this.postRuntimePanelIoState(docId, document, webviewPanel);
        break;
    }
  }

  /**
   * Start execution
   */
  private async startExecution(
    docId: string,
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    modeOverride?: ExecutionMode
  ): Promise<void> {
    console.log("[SFC] Starting execution for document:", docId);
    
    // Stop existing execution
    await this.stopExecution(docId, false);

    const workspace: SfcWorkspace = JSON.parse(document.getText());
    const runtimeState = this.runtimeController.ensureState(docId);
    const mode = runtimeState.mode;
    // Mode is "local" or "external" (RuntimeUiMode)
    const executionMode: ExecutionMode =
      modeOverride ?? (mode === "external" ? "hardware" : "simulation");

    console.log(`[SFC] Execution mode: ${mode} → ${executionMode}`);

    if (executionMode === "simulation") {
      await this.startSimulation(docId, workspace, webviewPanel);
    } else {
      await this.startHardware(docId, workspace, document, webviewPanel);
    }

  }

  /**
   * Start simulation mode execution
   */
  private async startSimulation(
    docId: string,
    workspace: SfcWorkspace,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    console.log("[SFC] Starting simulation mode");

    const engine = new SfcEngine(workspace, "simulation");

    this.activeExecutions.set(docId, {
      workspace,
      mode: "simulation",
      isRunning: true,
      engine,
      webviewPanel,
    });

    await engine.start(100); // 100ms scan cycle

    // Send execution state updates periodically
    const updateInterval = setInterval(() => {
      const state = this.activeExecutions.get(docId);
      if (!state || !state.isRunning) {
        clearInterval(updateInterval);
        return;
      }

      const execState = engine.getExecutionState();
      webviewPanel.webview.postMessage({
        type: "executionState",
        state: {
          activeSteps: Array.from(execState.activeSteps),
          mode: "simulation",
          status: engine.getExecutionStatus(),
          breakpoints: engine.getBreakpoints(),
          currentStep: engine.getCurrentDebugStep(),
        },
      });

      this.postRuntimePanelIoState(docId, undefined, webviewPanel);
    }, 100);

    vscode.window.showInformationMessage("SFC simulation started");
  }

  /**
   * Start hardware mode execution
   */
  private async startHardware(
    docId: string,
    workspace: SfcWorkspace,
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    console.log("[SFC] Starting hardware mode");

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
    const config = await getRuntimeConfig(workspaceFolder);
    if (!config) {
      throw new Error("Could not get runtime configuration");
    }
    
    const runtimeClient = new RuntimeClient(config);
    await runtimeClient.connect();

    const engine = new SfcEngine(workspace, "hardware", runtimeClient);

    this.activeExecutions.set(docId, {
      workspace,
      mode: "hardware",
      isRunning: true,
      engine,
      webviewPanel,
    });

    await engine.start(100);

    // Send execution state updates
    const updateInterval = setInterval(() => {
      const state = this.activeExecutions.get(docId);
      if (!state || !state.isRunning) {
        clearInterval(updateInterval);
        return;
      }

      const execState = engine.getExecutionState();
      webviewPanel.webview.postMessage({
        type: "executionState",
        state: {
          activeSteps: Array.from(execState.activeSteps),
          mode: "hardware",
          status: engine.getExecutionStatus(),
          breakpoints: engine.getBreakpoints(),
          currentStep: engine.getCurrentDebugStep(),
        },
      });

      this.postRuntimePanelIoState(docId, document, webviewPanel);
    }, 100);

    vscode.window.showInformationMessage("SFC hardware execution started");
  }

  /**
   * Stop execution
   */
  private async stopExecution(docId: string, notify = true): Promise<void> {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    console.log("[SFC] Stopping execution");
    state.isRunning = false;

    await state.engine.stop();

    state.webviewPanel.webview.postMessage({ type: "executionStopped" });

    this.activeExecutions.delete(docId);

    if (notify) {
      vscode.window.showInformationMessage("SFC execution stopped");
    }
  }

  /**
   * Handle debug pause
   */
  private async handleDebugPause(docId: string): Promise<void> {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    state.engine.pause();
    console.log("[SFC Debug] Paused");
    
    // Send updated execution state
    this.sendExecutionStateUpdate(docId);
  }

  /**
   * Handle debug resume
   */
  private async handleDebugResume(docId: string): Promise<void> {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    state.engine.resume();
    console.log("[SFC Debug] Resumed");
    
    // Send updated execution state
    this.sendExecutionStateUpdate(docId);
  }

  /**
   * Handle debug step over
   */
  private async handleDebugStepOver(docId: string): Promise<void> {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    state.engine.stepOver();
    console.log("[SFC Debug] Step over");
    
    // Wait a bit for the step to execute, then send state
    setTimeout(() => {
      this.sendExecutionStateUpdate(docId);
    }, 150);
  }

  /**
   * Handle toggle breakpoint
   */
  private async handleToggleBreakpoint(docId: string, stepId: string): Promise<void> {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    state.engine.toggleBreakpoint(stepId);
    console.log(`[SFC Debug] Toggled breakpoint on step: ${stepId}`);
    
    // Send updated execution state
    this.sendExecutionStateUpdate(docId);
  }

  /**
   * Send execution state update to webview
   */
  private sendExecutionStateUpdate(docId: string): void {
    const state = this.activeExecutions.get(docId);
    if (!state) return;

    const execState = state.engine.getExecutionState();
    const executionStatus = state.engine.getExecutionStatus();
    const breakpoints = state.engine.getBreakpoints();
    const currentStep = state.engine.getCurrentDebugStep();

    state.webviewPanel.webview.postMessage({
      type: "executionState",
      state: {
        activeSteps: Array.from(execState.activeSteps),
        mode: state.mode,
        status: executionStatus,
        breakpoints,
        currentStep,
      },
    });
  }

  /**
   * Post runtime state to webview
   */
  private postRuntimeState(
    docId: string,
    uri: vscode.Uri,
    webviewPanel: vscode.WebviewPanel
  ): void {
    const runtimeState = this.runtimeController.ensureState(docId);

    webviewPanel.webview.postMessage(runtimeMessage.state(runtimeState));
    webviewPanel.webview.postMessage({
      type: "runtimeStatus",
      payload: runtimePanelStatusFromState(uri, runtimeState),
    });
  }

  /**
   * Post runtime panel settings
   */
  private postRuntimePanelSettings(
    uri: vscode.Uri,
    webviewPanel: vscode.WebviewPanel
  ): void {
    const settings = collectRuntimePanelSettings(uri);
    webviewPanel.webview.postMessage({
      type: "runtimePanel.settings",
      settings,
    });
  }

  /**
   * Build IO state from document/simulation
   */
  private buildIoStateFromDocument(document: vscode.TextDocument): RuntimePanelIoState {
    const state: RuntimePanelIoState = {
      inputs: [],
      outputs: [],
      memory: [],
    };

    try {
      const workspace: SfcWorkspace = JSON.parse(document.getText());
      
      // Extract I/O from variables
      if (workspace.variables) {
        for (const variable of workspace.variables) {
          if (variable.address) {
            const entry: RuntimePanelIoEntry = {
              name: variable.name,
              address: variable.address,
              value: "FALSE",
            };

            if (variable.address.startsWith("%I")) {
              state.inputs?.push(entry);
            } else if (variable.address.startsWith("%Q")) {
              state.outputs?.push(entry);
            } else if (variable.address.startsWith("%M")) {
              state.memory?.push(entry);
            }
          }
        }
      }
    } catch (error) {
      console.error("Failed to build IO state:", error);
    }

    return state;
  }

  /**
   * Post runtime panel IO state
   */
  private postRuntimePanelIoState(
    docId: string,
    document: vscode.TextDocument | undefined,
    webviewPanel: vscode.WebviewPanel
  ): void {
    const state = this.activeExecutions.get(docId);
    let ioState: RuntimePanelIoState;

    if (state && state.mode === "simulation") {
      // Get values from simulation engine
      ioState = this.buildSimulationIoState(state.engine);
    } else if (document) {
      // Get static values from document
      ioState = this.buildIoStateFromDocument(document);
    } else {
      ioState = { inputs: [], outputs: [], memory: [] };
    }

    this.runtimeIoState.set(docId, ioState);

    webviewPanel.webview.postMessage({
      type: "runtimePanel.ioState",
      ioState,
    });
  }

  /**
   * Build IO state from simulation engine
   */
  private buildSimulationIoState(engine: SfcEngine): RuntimePanelIoState {
    const state: RuntimePanelIoState = {
      inputs: [],
      outputs: [],
      memory: [],
    };

    const workspace = engine.getWorkspace();

    if (workspace.variables) {
      for (const variable of workspace.variables) {
        if (variable.address) {
          const value = engine.getVariable(variable.address);
          const displayValue = value !== undefined ? boolToDisplay(Boolean(value)) : "FALSE";

          const entry: RuntimePanelIoEntry = {
            name: variable.name,
            address: variable.address,
            value: displayValue,
          };

          if (variable.address.startsWith("%I")) {
            state.inputs?.push(entry);
          } else if (variable.address.startsWith("%Q")) {
            state.outputs?.push(entry);
          } else if (variable.address.startsWith("%M")) {
            state.memory?.push(entry);
          }
        }
      }
    }

    return state;
  }
}
