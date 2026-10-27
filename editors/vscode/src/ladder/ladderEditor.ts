// The Ladder editor (specification 16, specification 25 §11.4). The webview draws the program and
// handles the pointer; every rule about the file is the language server's (`trust-lsp.visual*`):
// how the file opens, the schema check on save and Validate, the edits stored in the program, and
// the Structured Text companion that Generate writes. This host forwards the webview's requests
// and shows the answers. A Ladder program runs through its companion and runtime entry; the
// editor has no runtime controls of its own (specification 17 §5).

import * as vscode from "vscode";
import * as path from "path";
import { syncVisualCompanionFromUri } from "../visual/companionSt";
import { problemText, visualRequest, type VisualAnswer } from "../visual/visualCommands";

interface ProgramAnswer extends VisualAnswer {
  programText?: string;
}

interface FormatAnswer extends VisualAnswer {
  text?: string;
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

/** The current time for a file stamp. */
function now(): string {
  return new Date().toISOString();
}

/**
 * Provider for the Ladder Logic visual editor.
 */
export class LadderEditorProvider implements vscode.CustomTextEditorProvider {
  private static readonly viewType = "trust-lsp.ladder.editor";

  public static register(context: vscode.ExtensionContext): vscode.Disposable {
    const provider = new LadderEditorProvider(context);
    return vscode.window.registerCustomEditorProvider(LadderEditorProvider.viewType, provider, {
      webviewOptions: {
        retainContextWhenHidden: true,
      },
    });
  }

  constructor(private readonly context: vscode.ExtensionContext) {}

  public async resolveCustomTextEditor(
    document: vscode.TextDocument,
    webviewPanel: vscode.WebviewPanel,
    _token: vscode.CancellationToken
  ): Promise<void> {
    webviewPanel.webview.options = {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.file(path.join(this.context.extensionPath, "media"))],
    };
    webviewPanel.webview.html = this.getHtmlForWebview(webviewPanel.webview);

    const messageSubscription = webviewPanel.webview.onDidReceiveMessage(async (message) => {
      switch (message?.type) {
        case "ready":
          await this.postProgram(document, webviewPanel);
          break;
        case "save":
          await this.saveProgram(document, message.program, "save");
          break;
        case "validate":
          await this.validateProgram(message.program);
          break;
        case "generateST":
          await this.generateCompanion(document, message.program);
          break;
        case "edit":
          await this.edit(message, webviewPanel);
          break;
        case "webviewBootError":
          console.error("[Ladder] webview boot error:", message.message);
          void vscode.window.showErrorMessage(
            `Ladder webview failed to initialize: ${String(message.message ?? "unknown error")}`
          );
          break;
      }
    });

    const changeDocumentSubscription = vscode.workspace.onDidChangeTextDocument((event) => {
      if (event.document.uri.toString() === document.uri.toString()) {
        void this.postProgram(document, webviewPanel);
      }
    });

    webviewPanel.onDidDispose(() => {
      messageSubscription.dispose();
      changeDocumentSubscription.dispose();
    });
  }

  /** Sends the program as the language server opens the file; a file it cannot read shows why. */
  private async postProgram(document: vscode.TextDocument, webviewPanel: vscode.WebviewPanel): Promise<void> {
    const answer = await visualRequest<ProgramAnswer>("trust-lsp.visualParse", {
      kind: "ladder",
      text: document.getText(),
      now: now(),
    });
    if (!answer.ok) {
      void vscode.window.showErrorMessage(`Failed to parse ladder program: ${problemText(answer)}`);
    }
    if (typeof answer.programText === "string") {
      void webviewPanel.webview.postMessage({
        type: "loadProgram",
        program: JSON.parse(answer.programText),
      });
    }
  }

  /** The file text the language server writes for `program`, or why it refuses it. */
  private async fileText(program: unknown): Promise<FormatAnswer> {
    return visualRequest<FormatAnswer>("trust-lsp.visualFormat", {
      kind: "ladder",
      programText: JSON.stringify(program ?? null),
      now: now(),
    });
  }

  /** Writes and saves `text` as the document's content when it differs. */
  private async writeDocument(document: vscode.TextDocument, text: string): Promise<void> {
    if (document.getText() !== text) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(document.uri, new vscode.Range(0, 0, document.lineCount, 0), text);
      await vscode.workspace.applyEdit(edit);
    }
    await document.save();
  }

  private async saveProgram(
    document: vscode.TextDocument,
    program: unknown,
    reason: "save" | "generate"
  ): Promise<string | undefined> {
    const answer = await this.fileText(program);
    if (!answer.ok || typeof answer.text !== "string") {
      const prefix =
        reason === "save" ? "Ladder save rejected by schema validation" : "Ladder ST generation failed";
      void vscode.window.showErrorMessage(`${prefix}: ${problemText(answer)}`);
      return undefined;
    }
    await this.writeDocument(document, answer.text);
    if (reason === "save") {
      void vscode.window.showInformationMessage("Ladder program saved");
    }
    return answer.text;
  }

  private async validateProgram(program: unknown): Promise<void> {
    const answer = await visualRequest("trust-lsp.visualValidate", {
      kind: "ladder",
      text: JSON.stringify(program ?? null),
    });
    if (answer.ok) {
      void vscode.window.showInformationMessage("Ladder program validates.");
    } else {
      void vscode.window.showErrorMessage(`Ladder validation failed: ${problemText(answer)}`);
    }
  }

  /** Saves the program, then writes its companion and runtime entry (specification 17 §3). */
  private async generateCompanion(document: vscode.TextDocument, program: unknown): Promise<void> {
    const text = await this.saveProgram(document, program, "generate");
    if (text === undefined) {
      return;
    }
    const companion = await syncVisualCompanionFromUri(document.uri, {
      force: true,
      showErrors: true,
      sourceText: text,
    });
    if (companion) {
      void vscode.window.showInformationMessage(`Generated ST companion: ${path.basename(companion.fsPath)}`);
    }
  }

  /** Forwards an edit request of the webview and posts the answer back with its request id. */
  private async edit(message: Record<string, unknown>, webviewPanel: vscode.WebviewPanel): Promise<void> {
    const { type: _type, requestId, ...edit } = message;
    const answer = await visualRequest("trust-lsp.visualEdit", { kind: "ladder", ...edit });
    void webviewPanel.webview.postMessage({ ...answer, type: "editResult", requestId });
  }

  private getHtmlForWebview(webview: vscode.Webview): string {
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(path.join(this.context.extensionPath, "media", "ladderWebview.js"))
    );
    const styleUri = webview.asWebviewUri(
      vscode.Uri.file(path.join(this.context.extensionPath, "media", "ladderWebview.css"))
    );
    return WEBVIEW_HTML_TEMPLATE.replace(/{{cspSource}}/g, webview.cspSource)
      .replace("{{webviewScript}}", scriptUri.toString())
      .replace("{{webviewStyle}}", styleUri.toString());
  }
}
