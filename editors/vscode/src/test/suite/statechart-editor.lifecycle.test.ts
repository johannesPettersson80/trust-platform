import * as assert from "assert";
import * as vscode from "vscode";
import { StateChartEditorProvider } from "../../statechart/stateChartEditor";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

class FakeWebview {
  public options: vscode.WebviewOptions = {};
  public html = "";
  public readonly cspSource = "vscode-webview://statechart-test";
  private readonly messageListeners: Array<(message: unknown) => void> = [];

  asWebviewUri(localResource: vscode.Uri): vscode.Uri {
    return localResource;
  }

  postMessage(_message: unknown): Thenable<boolean> {
    return Promise.resolve(true);
  }

  onDidReceiveMessage(listener: (message: unknown) => void): vscode.Disposable {
    this.messageListeners.push(listener);
    return new vscode.Disposable(() => {
      const index = this.messageListeners.indexOf(listener);
      if (index >= 0) {
        this.messageListeners.splice(index, 1);
      }
    });
  }
}

class FakeWebviewPanel {
  public readonly webview = new FakeWebview();
  private readonly disposeListeners: Array<() => void> = [];

  onDidDispose(listener: () => void): vscode.Disposable {
    this.disposeListeners.push(listener);
    return new vscode.Disposable(() => {
      const index = this.disposeListeners.indexOf(listener);
      if (index >= 0) {
        this.disposeListeners.splice(index, 1);
      }
    });
  }

  fireDidDispose(): void {
    for (const listener of [...this.disposeListeners]) {
      listener();
    }
  }
}

suite("StateChart editor lifecycle", function () {
  this.timeout(10000);

  test("disposes running execution session when panel is closed", async () => {
    const provider = new StateChartEditorProvider({
      extensionPath: process.cwd(),
    } as vscode.ExtensionContext);

    const document = {
      uri: vscode.Uri.parse("untitled:/statechart-lifecycle.statechart.json"),
      getText: () => '{"id":"lifecycle","initial":"Idle","states":{"Idle":{}}}',
      lineCount: 1,
    } as unknown as vscode.TextDocument;

    const panel = new FakeWebviewPanel();
    const calls: string[] = [];
    const timer = setInterval(() => {
      // no-op
    }, 5000);
    (provider as any).simulators.set(document.uri.toString(), {
      simulator: {
        cleanup: async () => {
          calls.push("cleanup");
        },
      },
      timer,
      mode: "hardware",
      runtimeClient: {
        disconnect: () => {
          calls.push("disconnect");
        },
      },
    });

    await provider.resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken
    );

    panel.fireDidDispose();
    await delay(25);

    assert.deepStrictEqual(calls, ["cleanup", "disconnect"]);
    assert.strictEqual((provider as any).simulators.has(document.uri.toString()), false);
  });
});
