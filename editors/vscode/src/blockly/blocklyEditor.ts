// The Blockly editor (specification 25 §11.4). The webview edits the workspace with the Blockly
// library and saves it; what the workspace means as Structured Text is the language server's:
// Validate asks `trust-lsp.visualValidate`, Generate ST asks `trust-lsp.visualCompanion` and, when
// the user agrees, writes the one companion and runtime entry beside the program.

import * as vscode from "vscode";
import * as path from "path";
import { requestCompanion, syncVisualCompanionFromUri } from "../visual/companionSt";
import { problemText, visualRequest, type VisualAnswer } from "../visual/visualCommands";

interface ValidateAnswer extends VisualAnswer {
  warnings?: string[];
  preview?: string;
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

/**
 * Provider for Blockly visual programming editor
 */
export class BlocklyEditorProvider implements vscode.CustomTextEditorProvider {
  private static readonly viewType = "trust-lsp.blockly.editor";

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

    // Handle messages from webview
    const messageSubscription = webviewPanel.webview.onDidReceiveMessage(
      (message) => {
        switch (message.type) {
          case "save":
            void this.saveDocument(document, message.content);
            return;

          case "ready":
            updateWebview();
            return;

          case "error":
            void vscode.window.showErrorMessage(
              message.error || "Could not open this Blockly program. Fix the file and try again."
            );
            return;

          case "openAsText":
            void vscode.commands.executeCommand(
              "vscode.openWith",
              document.uri,
              "default",
              { preview: false }
            );
            return;

          case "validate":
            void this.validateWorkspace(document, webviewPanel);
            return;

          case "generateCode":
            void this.generateCode(document, webviewPanel);
            return;
        }
      }
    );

    // Clean up when editor closes
    webviewPanel.onDidDispose(() => {
      changeDocumentSubscription.dispose();
      messageSubscription.dispose();
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
   * Generate ST: shows the companion the language server generates and, when the user agrees,
   * writes it and the runtime entry beside the program.
   */
  private async generateCode(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    const text = document.getText();
    const answer = await requestCompanion(document.uri, text);
    if (!answer.ok || !answer.companion) {
      void vscode.window.showErrorMessage(`Code generation failed: ${problemText(answer)}`);
      return;
    }
    const warnings = answer.warnings ?? [];
    if (warnings.length > 0) {
      void vscode.window.showWarningMessage(
        `Code generation completed with ${warnings.length} warnings`
      );
    }
    void webviewPanel.webview.postMessage({
      type: "codeGenerated",
      code: answer.companion.text,
      errors: warnings,
    });

    const saveCode = await vscode.window.showQuickPick(["Yes", "No"], {
      placeHolder: "Save generated ST code to file?",
    });
    if (saveCode === "Yes") {
      const companion = await syncVisualCompanionFromUri(document.uri, {
        force: true,
        showErrors: true,
        sourceText: text,
      });
      if (companion) {
        void vscode.window.showInformationMessage(
          `ST code saved to ${path.basename(companion.fsPath)}`
        );
      }
    }
  }

  private async validateWorkspace(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    const answer = await visualRequest<ValidateAnswer>("trust-lsp.visualValidate", {
      kind: "blockly",
      file: document.uri.fsPath,
      text: document.getText(),
    });
    if (!answer.ok) {
      void vscode.window.showErrorMessage(`Blockly validation failed: ${problemText(answer)}`);
      return;
    }
    const warnings = answer.warnings ?? [];
    void webviewPanel.webview.postMessage({
      type: "codeGenerated",
      code: answer.preview ?? "",
      errors: warnings,
    });
    if (warnings.length > 0) {
      void vscode.window.showWarningMessage(
        `Blockly validation completed with ${warnings.length} warnings.`
      );
    } else {
      void vscode.window.showInformationMessage("Blockly workspace validates.");
    }
  }
}
