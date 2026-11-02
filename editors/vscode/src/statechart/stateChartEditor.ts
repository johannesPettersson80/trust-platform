// The Statechart editor (specification 25 §11.4). The language server reads a `.statechart.json`
// file into the editor graph (`trust-lsp.visualParse`), writes a graph back as the file
// (`trust-lsp.visualFormat`), validates it (`trust-lsp.visualValidate`), generates its
// Structured Text companion (`trust-lsp.visualCompanion`) and reads the timer and mapping values
// typed into the panels (`trust-lsp.visualEdit`); this provider moves those answers between the
// document and the webview, which draws the graph.

import * as vscode from "vscode";
import * as path from "path";
import { syncVisualCompanionFromUri } from "../visual/companionSt";
import { problemText, visualRequest, type VisualAnswer } from "../visual/visualCommands";

interface GraphAnswer extends VisualAnswer {
  graphText?: string;
}

interface FormatAnswer extends VisualAnswer {
  text?: string;
}

interface ActionReportAnswer extends VisualAnswer {
  unmapped?: string[];
  unused?: string[];
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
        background-color: var(--trust-canvas, var(--vscode-editor-background, #1e1e1e));
        color: var(--trust-text, var(--vscode-editor-foreground, var(--vscode-foreground, #cccccc)));
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
export class StateChartEditorProvider implements vscode.CustomTextEditorProvider {
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
      },
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
    _token: vscode.CancellationToken,
  ): Promise<void> {
    webviewPanel.webview.options = {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.file(path.join(this.context.extensionPath, "media"))],
    };

    webviewPanel.webview.html = this.getHtmlForWebview(webviewPanel.webview);

    // Sends the graph of the document as it is now; an answer to an older text is dropped.
    let latestUpdate = 0;
    const updateWebview = async (): Promise<void> => {
      const update = ++latestUpdate;
      const answer = await visualRequest<GraphAnswer>("trust-lsp.visualParse", {
        kind: "statechart",
        text: document.getText(),
      });
      if (update !== latestUpdate) {
        return;
      }
      if (answer.ok && typeof answer.graphText === "string") {
        void webviewPanel.webview.postMessage({
          type: "update",
          graph: JSON.parse(answer.graphText),
        });
        return;
      }
      const detail = problemText(answer);
      void webviewPanel.webview.postMessage({ type: "update", error: detail });
      void vscode.window.showErrorMessage(`Could not open this statechart: ${detail}`);
    };

    const changeDocumentSubscription = vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.uri.toString() === document.uri.toString()) {
        void updateWebview();
      }
    });

    const messageSubscription = webviewPanel.webview.onDidReceiveMessage((message) => {
      switch (message.type) {
        case "save":
          void this.saveGraph(document, message.graph);
          return;

        case "validate":
          void this.validateGraph(document, message.graph);
          return;

        case "generateST":
          void this.generateStructuredTextCompanion(document, message.graph);
          return;

        case "ready":
          void updateWebview();
          return;

        case "actionReport":
          void this.postActionReport(webviewPanel, message.requestId, message.graph);
          return;

        case "readValue":
          void this.postValue(webviewPanel, message);
          return;

        case "addState":
          void this.postAddedState(webviewPanel, message);
          return;

        case "openAsText":
          void vscode.commands.executeCommand("vscode.openWith", document.uri, "default", { preview: false });
          return;
      }
    });

    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
      messageSubscription.dispose();
    });

    void updateWebview();
  }

  /**
   * Get the HTML content for the webview
   */
  private getHtmlForWebview(webview: vscode.Webview): string {
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(path.join(this.context.extensionPath, "media", "stateChartWebview.js")),
    );

    const cssUri = webview.asWebviewUri(
      vscode.Uri.file(path.join(this.context.extensionPath, "media", "stateChartWebview.css")),
    );

    let html = WEBVIEW_HTML_TEMPLATE;

    // Replace placeholders
    html = html.replace(/{{cspSource}}/g, webview.cspSource);
    html = html.replace(/{{webviewScript}}/g, scriptUri.toString());
    html = html.replace(/{{webviewStyle}}/g, cssUri.toString());

    return html;
  }

  /** Answers which actions the webview's graph names without a mapping and which it maps unused. */
  private async postActionReport(webviewPanel: vscode.WebviewPanel, requestId: unknown, graph: unknown): Promise<void> {
    const answer = await visualRequest<ActionReportAnswer>("trust-lsp.visualEdit", {
      kind: "statechart",
      op: "actionReport",
      graphText: JSON.stringify(graph ?? null),
    });
    void webviewPanel.webview.postMessage({
      type: "actionReport",
      requestId,
      unmapped: answer.ok ? (answer.unmapped ?? []) : [],
      unused: answer.ok ? (answer.unused ?? []) : [],
    });
  }

  /**
   * Answers the value the language server reads from text typed into a panel field: the
   * auto-transition timer (`afterDelay`) or a mapping value (`mappingValue`).
   */
  private async postValue(
    webviewPanel: vscode.WebviewPanel,
    message: { requestId?: unknown; field?: unknown; text?: unknown },
  ): Promise<void> {
    const answer = await visualRequest("trust-lsp.visualEdit", {
      kind: "statechart",
      op: message.field,
      text: message.text,
    });
    void webviewPanel.webview.postMessage({
      ...answer,
      type: "valueResult",
      requestId: message.requestId,
    });
  }

  /** Answers the new state the language server gives the webview's graph (`addState`). */
  private async postAddedState(
    webviewPanel: vscode.WebviewPanel,
    message: { requestId?: unknown; graph?: unknown; stateType?: unknown },
  ): Promise<void> {
    const answer = await visualRequest("trust-lsp.visualEdit", {
      kind: "statechart",
      op: "addState",
      graphText: JSON.stringify(message.graph ?? null),
      stateType: message.stateType,
    });
    void webviewPanel.webview.postMessage({
      ...answer,
      type: "stateAdded",
      requestId: message.requestId,
    });
  }

  /** The file text of the webview's graph, as the language server writes it. */
  private async formatGraph(graph: unknown, previousText: string): Promise<string | undefined> {
    // Saved over `previousText`, the file keeps that file's id.
    const answer = await visualRequest<FormatAnswer>("trust-lsp.visualFormat", {
      kind: "statechart",
      graphText: JSON.stringify(graph ?? null),
      previousText,
    });
    if (answer.ok && typeof answer.text === "string") {
      return answer.text;
    }
    void vscode.window.showErrorMessage(`Could not write this statechart: ${problemText(answer)}`);
    return undefined;
  }

  /**
   * Update the document with new content from webview
   */
  private updateTextDocument(document: vscode.TextDocument, content: string) {
    const edit = new vscode.WorkspaceEdit();

    // Replace entire document
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), content);

    return vscode.workspace.applyEdit(edit);
  }

  private async saveDocument(document: vscode.TextDocument, content: string): Promise<void> {
    await this.updateTextDocument(document, content);
    await document.save();
  }

  private async saveGraph(document: vscode.TextDocument, graph: unknown): Promise<void> {
    const text = await this.formatGraph(graph, document.getText());
    if (text !== undefined) {
      await this.saveDocument(document, text);
    }
  }

  private async validateGraph(document: vscode.TextDocument, graph: unknown): Promise<void> {
    const text = await this.formatGraph(graph, document.getText());
    if (text === undefined) {
      return;
    }
    const answer = await visualRequest("trust-lsp.visualValidate", {
      kind: "statechart",
      file: document.uri.fsPath,
      text,
    });
    if (answer.ok) {
      void vscode.window.showInformationMessage("Statechart validates.");
    } else {
      void vscode.window.showErrorMessage(`Statechart validation failed: ${problemText(answer)}`);
    }
  }

  private async generateStructuredTextCompanion(document: vscode.TextDocument, graph: unknown): Promise<void> {
    const text = await this.formatGraph(graph, document.getText());
    if (text === undefined) {
      return;
    }
    await this.saveDocument(document, text);
    const companion = await syncVisualCompanionFromUri(document.uri, {
      force: true,
      showErrors: true,
      sourceText: text,
    });
    if (companion) {
      void vscode.window.showInformationMessage(`Generated ST companion: ${path.basename(companion.fsPath)}`);
    }
  }
}
