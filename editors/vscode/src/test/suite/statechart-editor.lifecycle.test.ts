import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { StateChartEditorProvider } from "../../statechart/stateChartEditor";

// Specification 25 §11.4: the Statechart editor draws the graph the language server reads from
// the file (`trust-lsp.visualParse`) and saves the graph as the file the language server writes
// (`trust-lsp.visualFormat`). These tests prove that wiring and the editor's lifecycle; the rules
// themselves are tested in crates/trust-visual.

class FakeWebview {
  public options: vscode.WebviewOptions = {};
  public html = "";
  public readonly cspSource = "vscode-webview://statechart-test";
  public readonly posted: Array<Record<string, unknown>> = [];
  public readonly messageListeners: Array<(message: unknown) => void> = [];

  asWebviewUri(localResource: vscode.Uri): vscode.Uri {
    return localResource;
  }

  postMessage(message: unknown): Thenable<boolean> {
    this.posted.push(message as Record<string, unknown>);
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

  send(message: unknown): void {
    for (const listener of [...this.messageListeners]) {
      listener(message);
    }
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

async function waitFor(reached: () => boolean, what: string): Promise<void> {
  for (let attempt = 0; attempt < 200 && !reached(); attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.ok(reached(), `timed out waiting for ${what}`);
}

function openProvider(): StateChartEditorProvider {
  return new StateChartEditorProvider({
    extensionPath: process.cwd(),
  } as vscode.ExtensionContext);
}

suite("StateChart editor lifecycle", function () {
  this.timeout(60000);

  test("draws the graph the language server reads from the file", async () => {
    const document = {
      uri: vscode.Uri.parse("untitled:/statechart-lifecycle.statechart.json"),
      getText: () => '{"id":"lifecycle","initial":"Idle","states":{"Idle":{"on":{"GO":"Run"}},"Run":{"type":"final"}}}',
      lineCount: 1,
    } as unknown as vscode.TextDocument;
    const panel = new FakeWebviewPanel();
    await openProvider().resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken,
    );

    await waitFor(
      () => panel.webview.posted.some((message) => message.type === "update" && message.graph),
      "the graph",
    );
    const update = panel.webview.posted.find((message) => message.graph) as {
      graph: {
        states: Array<{ id: string; type: string }>;
        transitions: Array<{ source: string; target: string; event: string }>;
      };
    };
    assert.deepStrictEqual(
      update.graph.states.map((state) => `${state.id} ${state.type}`),
      ["state_Idle initial", "state_Run final"],
    );
    assert.deepStrictEqual(
      update.graph.transitions.map((edge) => `${edge.source} -${edge.event}-> ${edge.target}`),
      ["state_Idle -GO-> state_Run"],
    );
    panel.fireDidDispose();
  });

  test("saves the edited graph as the file the language server writes", async () => {
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-statechart-"));
    const file = path.join(folder, "door.statechart.json");
    fs.writeFileSync(file, '{"id":"door","states":{"Idle":{}}}');
    try {
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const panel = new FakeWebviewPanel();
      await openProvider().resolveCustomTextEditor(
        document,
        panel as unknown as vscode.WebviewPanel,
        {} as vscode.CancellationToken,
      );
      panel.webview.send({
        type: "save",
        graph: {
          states: [
            { id: "state_Idle", label: "Idle", type: "initial", entry: [], exit: [] },
            { id: "state_Open", label: "Open", type: "normal", entry: ["lamp"], exit: [] },
          ],
          transitions: [{ id: "e1", source: "state_Idle", target: "state_Open", event: "OPEN" }],
          actionMappings: {},
        },
      });
      // The save keeps the chart's own id (`door`); it used to become `stateMachine`.
      const expected = [
        "{",
        '  "id": "door",',
        '  "initial": "Idle",',
        '  "states": {',
        '    "Idle": {',
        '      "on": {',
        '        "OPEN": {',
        '          "target": "Open"',
        "        }",
        "      }",
        "    },",
        '    "Open": {',
        '      "entry": [',
        '        "lamp"',
        "      ]",
        "    }",
        "  },",
        '  "actionMappings": {}',
        "}",
      ].join("\n");
      await waitFor(() => document.getText() === expected, "the written file");
      panel.fireDidDispose();
    } finally {
      fs.rmSync(folder, { recursive: true, force: true });
    }
  });

  // Catches a panel that converts the typed timer or mapping value itself: the host forwards the
  // text and hands back the language server's value with the request id, or why it refuses it.
  test("reads a typed timer and mapping value through the language server", async () => {
    const document = {
      uri: vscode.Uri.parse("untitled:/statechart-values.statechart.json"),
      getText: () => '{"id":"values","states":{"Idle":{}}}',
      lineCount: 1,
    } as unknown as vscode.TextDocument;
    const panel = new FakeWebviewPanel();
    await openProvider().resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken,
    );
    const answerTo = async (requestId: number): Promise<Record<string, unknown>> => {
      await waitFor(
        () => panel.webview.posted.some((message) => message.type === "valueResult" && message.requestId === requestId),
        `the answer to request ${requestId}`,
      );
      return panel.webview.posted.find(
        (message) => message.type === "valueResult" && message.requestId === requestId,
      ) as Record<string, unknown>;
    };

    panel.webview.send({ type: "readValue", requestId: 1, field: "afterDelay", text: "1500" });
    const delay = await answerTo(1);
    assert.strictEqual(delay.ok, true, JSON.stringify(delay.errors));
    assert.strictEqual(delay.after, 1500);

    panel.webview.send({ type: "readValue", requestId: 2, field: "afterDelay", text: "-5" });
    const refused = await answerTo(2);
    assert.strictEqual(refused.ok, false);
    assert.deepStrictEqual(refused.errors, [
      { message: "Auto-transition timer must be a number of milliseconds from 0, or empty for none." },
    ]);

    panel.webview.send({ type: "readValue", requestId: 3, field: "mappingValue", text: "true" });
    assert.strictEqual((await answerTo(3)).value, true);
    panel.webview.send({ type: "readValue", requestId: 4, field: "mappingValue", text: "42" });
    assert.strictEqual((await answerTo(4)).value, 42);
    panel.fireDidDispose();
  });

  // Catches a toolbar that names a new state itself: the host forwards the graph and the state's
  // type, and hands back the language server's state with the request id, or why it adds none.
  test("asks the language server for a new state", async () => {
    const document = {
      uri: vscode.Uri.parse("untitled:/statechart-add.statechart.json"),
      getText: () => '{"id":"add","initial":"State1","states":{"State1":{}}}',
      lineCount: 1,
    } as unknown as vscode.TextDocument;
    const panel = new FakeWebviewPanel();
    await openProvider().resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken,
    );
    const graph = {
      states: [{ id: "state_State1", label: "State1", type: "initial" }],
      transitions: [],
      actionMappings: {},
    };
    const answerTo = async (requestId: number): Promise<Record<string, unknown>> => {
      await waitFor(
        () => panel.webview.posted.some((message) => message.type === "stateAdded" && message.requestId === requestId),
        `the answer to request ${requestId}`,
      );
      return panel.webview.posted.find(
        (message) => message.type === "stateAdded" && message.requestId === requestId,
      ) as Record<string, unknown>;
    };

    panel.webview.send({ type: "addState", requestId: 1, graph, stateType: "normal" });
    const added = await answerTo(1);
    assert.strictEqual(added.ok, true, JSON.stringify(added.errors));
    assert.deepStrictEqual(added.state, {
      id: "state_State2",
      label: "State2",
      type: "normal",
      entry: [],
      exit: [],
    });

    panel.webview.send({ type: "addState", requestId: 2, graph, stateType: "initial" });
    const refused = await answerTo(2);
    assert.strictEqual(refused.ok, false);
    panel.fireDidDispose();
  });

  test("stops listening to the webview when the panel is closed", async () => {
    const document = {
      uri: vscode.Uri.parse("untitled:/statechart-closed.statechart.json"),
      getText: () => '{"id":"closed","states":{"Idle":{}}}',
      lineCount: 1,
    } as unknown as vscode.TextDocument;
    const panel = new FakeWebviewPanel();
    await openProvider().resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken,
    );
    assert.strictEqual(panel.webview.messageListeners.length, 1);
    panel.fireDidDispose();
    assert.strictEqual(panel.webview.messageListeners.length, 0);
  });
});
