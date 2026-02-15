import * as vscode from "vscode";
import * as path from "path";
import { StateMachineEngine } from "./stateMachineEngine";
import { RuntimeClient, getRuntimeConfig } from "./runtimeClient";

type ExecutionMode = "simulation" | "hardware";

interface SimulatorEntry {
  simulator: StateMachineEngine;
  timer?: NodeJS.Timeout;
  mode: ExecutionMode;
  runtimeClient?: RuntimeClient;
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
        background-color: var(--vscode-editor-background);
        color: var(--vscode-editor-foreground);
      }

      .react-flow__controls button {
        background-color: var(--vscode-button-background);
        color: var(--vscode-button-foreground);
        border-color: var(--vscode-button-border);
      }

      .react-flow__controls button:hover {
        background-color: var(--vscode-button-hoverBackground);
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
        }
      }
    );

    // Make sure we get rid of the listener when our editor is closed
    const messageSubscription = webviewPanel.webview.onDidReceiveMessage((message) => {
      switch (message.type) {
        case "save":
          this.updateTextDocument(document, message.content);
          return;

        case "ready":
          // Send initial content when webview is ready
          updateWebview();
          return;

        case "error":
          void vscode.window.showErrorMessage(
            `StateChart Editor Error: ${message.error}`
          );
          return;

        case "startExecution":
          void this.startExecution(
            document,
            webviewPanel,
            message.mode || "simulation"
          );
          return;

        case "stopExecution":
          void this.stopExecution(docId);
          void webviewPanel.webview.postMessage({ type: "executionStopped" });
          return;

        case "sendEvent":
          void this.sendEvent(docId, message.event, webviewPanel);
          return;
      }
    });

    // Make sure we get rid of listeners and running execution when editor closes.
    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
      messageSubscription.dispose();
      void this.stopExecution(docId, false);
    });

    // Send initial content
    updateWebview();
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

  /**
   * Start execution of the state machine
   */
  private async startExecution(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    mode: ExecutionMode
  ) {
    const docId = document.uri.toString();
    
    try {
      // Stop any existing execution
      await this.stopExecution(docId);

      let runtimeClient: RuntimeClient | undefined;

      // Hardware mode: connect to trust-runtime
      if (mode === "hardware") {
        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
        const config = await getRuntimeConfig(workspaceFolder);
        
        if (!config) {
          vscode.window.showErrorMessage(
            "Hardware mode requires trust-runtime configuration. Set 'trust-lsp.runtime.controlEndpoint' in settings."
          );
          return;
        }

        runtimeClient = new RuntimeClient(config);
        
        try {
          await runtimeClient.connect();
          vscode.window.showInformationMessage(
            `✅ Connected to trust-runtime: ${config.controlEndpoint}`
          );
        } catch (error) {
          vscode.window.showErrorMessage(
            `❌ Failed to connect to trust-runtime: ${error}. Make sure the runtime is running.`
          );
          return;
        }
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
      
      const modeText = mode === "simulation" ? "🖥️  Simulation" : "🔌 Hardware";
      vscode.window.showInformationMessage(`${modeText} execution started`);
    } catch (error) {
      vscode.window.showErrorMessage(
        `Failed to start execution: ${error}`
      );
    }
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
