import * as assert from "assert";
import * as vscode from "vscode";
import { BlocklyEditorProvider } from "../../blockly/blocklyEditor";

// Specification 25 §11.4: what a Blockly workspace means as Structured Text is the language
// server's. Validate shows the function block and warnings `trust-lsp.visualValidate` answers;
// Generate ST shows the companion `trust-lsp.visualCompanion` answers and asks before writing it.
// These tests prove that wiring; the lowering is tested in crates/trust-visual.

const WORKSPACE = JSON.stringify({
  blocks: {
    languageVersion: 0,
    blocks: [
      { id: "a", type: "foo" },
      {
        id: "b",
        type: "variables_set",
        fields: { VAR: "lamp" },
        inputs: { VALUE: { block: { id: "c", type: "logic_boolean", fields: { BOOL: "TRUE" } } } },
      },
    ],
  },
  variables: [{ id: "v", name: "lamp", type: "BOOL" }],
});

class FakeWebview {
  public options: vscode.WebviewOptions = {};
  public html = "";
  public readonly cspSource = "vscode-webview://blockly-test";
  public readonly posted: Array<Record<string, unknown>> = [];
  private readonly listeners: Array<(message: unknown) => void> = [];

  asWebviewUri(localResource: vscode.Uri): vscode.Uri {
    return localResource;
  }

  postMessage(message: unknown): Thenable<boolean> {
    this.posted.push(message as Record<string, unknown>);
    return Promise.resolve(true);
  }

  onDidReceiveMessage(listener: (message: unknown) => void): vscode.Disposable {
    this.listeners.push(listener);
    return new vscode.Disposable(() => {
      const index = this.listeners.indexOf(listener);
      if (index >= 0) {
        this.listeners.splice(index, 1);
      }
    });
  }

  send(message: unknown): void {
    for (const listener of [...this.listeners]) {
      listener(message);
    }
  }
}

class FakeWebviewPanel {
  public readonly webview = new FakeWebview();
  private readonly disposeListeners: Array<() => void> = [];

  onDidDispose(listener: () => void): vscode.Disposable {
    this.disposeListeners.push(listener);
    return new vscode.Disposable(() => undefined);
  }

  dispose(): void {
    for (const listener of [...this.disposeListeners]) {
      listener();
    }
  }
}

async function generatedCode(panel: FakeWebviewPanel): Promise<{ code: string; errors: string[] }> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const message = panel.webview.posted.find((posted) => posted.type === "codeGenerated");
    if (message) {
      return message as unknown as { code: string; errors: string[] };
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.fail("timed out waiting for the generated code");
}

async function openEditor(): Promise<FakeWebviewPanel> {
  const provider = new BlocklyEditorProvider({ extensionPath: process.cwd() } as vscode.ExtensionContext);
  const document = {
    uri: vscode.Uri.parse("untitled:/blockly-wiring.blockly.json"),
    getText: () => WORKSPACE,
    lineCount: 1,
  } as unknown as vscode.TextDocument;
  const panel = new FakeWebviewPanel();
  await provider.resolveCustomTextEditor(
    document,
    panel as unknown as vscode.WebviewPanel,
    {} as vscode.CancellationToken
  );
  return panel;
}

suite("Blockly editor", function () {
  this.timeout(60000);

  test("Validate shows the function block and warnings of the language server", async () => {
    const panel = await openEditor();
    try {
      panel.webview.send({ type: "validate" });
      const generated = await generatedCode(panel);
      assert.ok(generated.code.startsWith("(*\n  Blockly conversion warnings (1):"), generated.code);
      assert.ok(generated.code.includes("FUNCTION_BLOCK FB_blockly_wiring_BLOCKLY"), generated.code);
      assert.ok(generated.code.includes("lamp := TRUE;"), generated.code);
      assert.deepStrictEqual(generated.errors, ["Unknown block type: foo"]);
    } finally {
      panel.dispose();
    }
  });

  test("Generate ST shows the companion and writes nothing when the user declines", async () => {
    const showQuickPick = vscode.window.showQuickPick;
    let offered = 0;
    (vscode.window as any).showQuickPick = async () => {
      offered += 1;
      return "No";
    };
    const panel = await openEditor();
    try {
      panel.webview.send({ type: "generateCode" });
      const generated = await generatedCode(panel);
      assert.ok(
        generated.code.startsWith(
          "(*\n  Auto-generated from blockly source: blockly-wiring.blockly.json\n"
        ),
        generated.code
      );
      for (let attempt = 0; attempt < 50 && offered === 0; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      assert.strictEqual(offered, 1, "Generate ST asks whether to save the companion");
    } finally {
      (vscode.window as any).showQuickPick = showQuickPick;
      panel.dispose();
    }
  });
});
