// The SFC editor's host (specification 25 §11.4, specification 17 §5). Every rule about an SFC
// file is the language server's (`trust-lsp.visual*`): the canvas graph of the file, the file of a
// graph, the structure checks, the canvas edits and the Structured Text companion. This host
// forwards the webview's requests there, writes the answers into the document and the companion
// files, and shows the outcome. Running an SFC is the shared Run in the truST sidebar, through its
// generated companion and runtime entry; the editor has no runtime of its own.

import * as vscode from "vscode";
import * as path from "path";
import {
  openCompanion,
  requestCompanion,
  syncVisualCompanionFromUri,
} from "../visual/companionSt";
import {
  problemText,
  visualRequest,
  type VisualAnswer,
  type VisualProblem,
} from "../visual/visualCommands";
/** The canvas graph the webview holds (`sfc/webview/types.ts` `SfcGraph`); the host only passes it on. */
type SfcGraph = unknown;

/** A canvas edit request of the webview (`sfc/webview/types.ts` `SfcEditRequest`). */
interface SfcEditRequest {
  type: "edit";
  requestId: number;
  graph: SfcGraph;
  edit: Record<string, unknown>;
}

/** The messages of the SFC webview (`sfc/webview/types.ts` `SfcWebviewToExtensionMessage`). */
type SfcWebviewToExtensionMessage =
  | { type: "ready" }
  | { type: "openAsText" }
  | { type: "save" | "validate" | "previewST" | "generateST"; graph: SfcGraph }
  | SfcEditRequest;

interface GraphAnswer extends VisualAnswer {
  graphText?: string;
}

interface TextAnswer extends VisualAnswer {
  text?: string;
}

interface ValidateAnswer extends VisualAnswer {
  issues?: unknown[];
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
        background-color: var(--trust-canvas, var(--vscode-editor-background, #1e1e1e));
        color: var(--trust-text, var(--vscode-editor-foreground, var(--vscode-foreground, #cccccc)));
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
 * The file text of a canvas graph, or why there is none. Saved over `previousText`, the file keeps
 * that file's name and metadata.
 */
async function formatGraph(graph: SfcGraph, previousText?: string): Promise<TextAnswer> {
  return visualRequest<TextAnswer>("trust-lsp.visualFormat", {
    kind: "sfc",
    graphText: JSON.stringify(graph),
    previousText,
    now: new Date().toISOString(),
  });
}

/**
 * Custom Editor Provider for SFC JSON files
 * Provides a visual editor for .sfc.json files
 */
export class SfcEditorProvider implements vscode.CustomTextEditorProvider {
  private static readonly viewType = "trust-lsp.sfc.editor";

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
    webviewPanel.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.file(path.join(this.context.extensionPath, "media")),
      ],
    };

    webviewPanel.webview.html = this.getHtmlForWebview(webviewPanel.webview);

    // The canvas graph of the document; only the answer to the latest request is shown.
    let latestUpdate = 0;
    const updateWebview = async (): Promise<void> => {
      const update = ++latestUpdate;
      const answer = await visualRequest<GraphAnswer>("trust-lsp.visualParse", {
        kind: "sfc",
        text: document.getText(),
      });
      if (update !== latestUpdate) {
        return;
      }
      if (answer.ok && typeof answer.graphText === "string") {
        void webviewPanel.webview.postMessage({
          type: "update",
          graph: JSON.parse(answer.graphText) as SfcGraph,
        });
        return;
      }
      const detail = problemText(answer);
      void webviewPanel.webview.postMessage({ type: "update", error: detail });
      void vscode.window.showErrorMessage(`Could not open this SFC: ${detail}`);
    };

    const changeDocumentSubscription =
      vscode.workspace.onDidChangeTextDocument((e) => {
        if (e.document.uri.toString() === document.uri.toString()) {
          void updateWebview();
        }
      });

    const messageSubscription = webviewPanel.webview.onDidReceiveMessage(
      async (message: SfcWebviewToExtensionMessage) => {
        switch (message.type) {
          case "ready":
            await updateWebview();
            return;

          case "save":
            await this.saveDocument(document, message.graph);
            return;

          case "openAsText":
            await vscode.commands.executeCommand(
              "vscode.openWith",
              document.uri,
              "default",
              { preview: false }
            );
            return;

          case "validate":
            await this.validateSfc(message.graph, webviewPanel);
            return;

          case "previewST":
            await this.showCompanion(document, message.graph, webviewPanel, false);
            return;

          case "generateST":
            await this.showCompanion(document, message.graph, webviewPanel, true);
            return;

          case "edit":
            await this.edit(message, webviewPanel);
            return;
        }
      }
    );

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

  /** Writes the file of the canvas graph into the document and saves it. */
  private async saveDocument(document: vscode.TextDocument, graph: SfcGraph): Promise<void> {
    const answer = await formatGraph(graph, document.getText());
    if (!answer.ok || typeof answer.text !== "string") {
      void vscode.window.showErrorMessage(`Failed to save SFC: ${problemText(answer)}`);
      return;
    }
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), answer.text);
    if (await vscode.workspace.applyEdit(edit)) {
      await document.save();
    }
  }

  /** Checks the chart on the canvas and shows the findings. */
  private async validateSfc(graph: SfcGraph, webviewPanel: vscode.WebviewPanel): Promise<void> {
    const file = await formatGraph(graph);
    if (!file.ok || typeof file.text !== "string") {
      void vscode.window.showErrorMessage(`Validation failed: ${problemText(file)}`);
      return;
    }
    const answer = await visualRequest<ValidateAnswer>("trust-lsp.visualValidate", {
      kind: "sfc",
      text: file.text,
    });
    const issues = answer.issues ?? [];
    await webviewPanel.webview.postMessage({ type: "validationResult", errors: issues });
    if (answer.ok) {
      void vscode.window.showInformationMessage("SFC validation successful!");
    } else if (issues.length > 0) {
      void vscode.window.showWarningMessage(
        `SFC validation found ${issues.length} problem${issues.length === 1 ? "" : "s"}: ${problemText(answer)}`
      );
    } else {
      void vscode.window.showErrorMessage(`Validation failed: ${problemText(answer)}`);
    }
  }

  /**
   * Shows the Structured Text companion of the chart on the canvas in the code panel. With
   * `write`, it also writes the companion and runtime entry beside the file and opens the
   * companion beside the editor (Generate ST); without, it only previews (Preview ST).
   */
  private async showCompanion(
    document: vscode.TextDocument,
    graph: SfcGraph,
    webviewPanel: vscode.WebviewPanel,
    write: boolean
  ): Promise<void> {
    const fail = async (answer: VisualAnswer): Promise<void> => {
      const errors = (answer.errors ?? []).map((problem: VisualProblem) => problem.message);
      await webviewPanel.webview.postMessage({ type: "codeGenerated", errors });
      void vscode.window.showErrorMessage(`Code generation failed: ${problemText(answer)}`);
    };
    const file = await formatGraph(graph);
    if (!file.ok || typeof file.text !== "string") {
      await fail(file);
      return;
    }
    const answer = await requestCompanion(document.uri, file.text);
    if (!answer.ok || !answer.companion) {
      await fail(answer);
      return;
    }
    if (write) {
      const companionUri = await syncVisualCompanionFromUri(document.uri, {
        force: true,
        showErrors: true,
        sourceText: file.text,
      });
      if (companionUri) {
        await openCompanion(companionUri, { beside: true });
        void vscode.window.showInformationMessage(
          `Generated ST companion: ${path.basename(companionUri.fsPath)}`
        );
      }
    }
    await webviewPanel.webview.postMessage({
      type: "codeGenerated",
      code: answer.companion.text,
      errors: answer.warnings ?? [],
    });
  }

  /** Forwards a canvas edit to the language server and answers the webview. */
  private async edit(message: SfcEditRequest, webviewPanel: vscode.WebviewPanel): Promise<void> {
    const { requestId, graph, edit } = message;
    const answer = await visualRequest("trust-lsp.visualEdit", {
      ...edit,
      kind: "sfc",
      graphText: JSON.stringify(graph),
    });
    await webviewPanel.webview.postMessage({ type: "editResult", requestId, answer });
  }
}
