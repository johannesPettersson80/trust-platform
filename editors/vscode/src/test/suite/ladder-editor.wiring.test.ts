// Wiring of the Ladder editor's host to the language server (specification 25 §11.4). The Ladder
// rules are tested in Rust (`crates/trust-visual/src/ladder`); this suite proves that the host
// forwards each webview request to the `trust-lsp.visual*` commands and hands the answer back to
// the webview or writes it into the document.

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { LadderEditorProvider } from "../../ladder/ladderEditor";

type Message = { type: string; [key: string]: unknown };

class FakeWebview {
  public options: vscode.WebviewOptions = {};
  public html = "";
  public readonly cspSource = "vscode-webview://ladder-test";
  private readonly listeners: Array<(message: unknown) => void> = [];
  private readonly posted: Message[] = [];
  private readonly waiting: Array<{ type: string; resolve: (message: Message) => void }> = [];

  asWebviewUri(localResource: vscode.Uri): vscode.Uri {
    return localResource;
  }

  postMessage(message: Message): Thenable<boolean> {
    const waiter = this.waiting.findIndex((entry) => entry.type === message.type);
    if (waiter >= 0) {
      const [entry] = this.waiting.splice(waiter, 1);
      entry.resolve(message);
    } else {
      this.posted.push(message);
    }
    return Promise.resolve(true);
  }

  onDidReceiveMessage(listener: (message: unknown) => void): vscode.Disposable {
    this.listeners.push(listener);
    return new vscode.Disposable(() => {
      this.listeners.splice(this.listeners.indexOf(listener), 1);
    });
  }

  /** Delivers a message from the (fake) webview to the host. */
  send(message: Message): void {
    for (const listener of [...this.listeners]) {
      listener(message);
    }
  }

  /** The next message of `type` the host posts. */
  next(type: string): Promise<Message> {
    const index = this.posted.findIndex((message) => message.type === type);
    if (index >= 0) {
      return Promise.resolve(this.posted.splice(index, 1)[0]);
    }
    return new Promise((resolve) => this.waiting.push({ type, resolve }));
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

const PROGRAM = {
  schemaVersion: 2,
  networks: [
    {
      id: "rung_1",
      order: 0,
      nodes: [
        { id: "c1", type: "contact", contactType: "NO", variable: "Start", position: { x: 100, y: 100 } },
        { id: "q1", type: "coil", coilType: "NORMAL", variable: "%QX0.0", position: { x: 260, y: 100 } },
      ],
      edges: [],
      layout: { y: 100 },
    },
  ],
  variables: [{ name: "Start", type: "BOOL", address: "%IX0.0" }],
  metadata: { name: "line", description: "wiring" },
};

/** The parts of a program the test reads back. */
interface ProgramShape {
  networks: Array<{ nodes: unknown[]; edges: Array<{ id: string }> }>;
}

async function serverReady(): Promise<void> {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    if ((await vscode.commands.getCommands(true)).includes("trust-lsp.visualEdit")) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.fail("the language server never offered trust-lsp.visualEdit");
}

suite("Ladder editor wiring", function () {
  this.timeout(60000);
  let folder: string;

  suiteSetup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await serverReady();
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-ladder-"));
  });

  suiteTeardown(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  // Catches a host that loads, edits or saves the program itself instead of asking the language
  // server: the program opens as the server reads it, an edit comes back with its request id, and
  // Save writes the text the server formats.
  test("the Ladder editor opens, edits and saves through the language server", async () => {
    const file = path.join(folder, "line.ladder.json");
    fs.writeFileSync(file, JSON.stringify(PROGRAM, null, 2));
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    const provider = new LadderEditorProvider({ extensionPath: process.cwd() } as vscode.ExtensionContext);
    const panel = new FakeWebviewPanel();
    await provider.resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken
    );
    const webview = panel.webview;

    webview.send({ type: "ready" });
    const loaded = (await webview.next("loadProgram")).program as ProgramShape;
    assert.strictEqual(loaded.networks[0].nodes.length, 2);

    webview.send({ type: "edit", requestId: 7, op: "autoRoute", programText: JSON.stringify(loaded) });
    const routed = await webview.next("editResult");
    assert.strictEqual(routed.requestId, 7);
    assert.strictEqual(routed.ok, true, JSON.stringify(routed.errors));
    const program = JSON.parse(String(routed.programText)) as ProgramShape;
    assert.deepStrictEqual(
      program.networks[0].edges.map((edge) => edge.id),
      ["edge_rung_1_0"]
    );

    webview.send({ type: "save", program });
    for (let attempt = 0; attempt < 150 && !fs.readFileSync(file, "utf8").includes("edge_rung_1_0"); attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    const saved = JSON.parse(fs.readFileSync(file, "utf8"));
    assert.strictEqual(saved.networks[0].edges[0].id, "edge_rung_1_0");
    assert.strictEqual(typeof saved.metadata.modified, "string");
    panel.dispose();
  });

  // G28: the webview's sandbox ignores window.prompt and window.alert, so Find and replace asks for
  // the symbol and its replacement in VS Code's input boxes, hands them to the webview, and shows the
  // outcome the webview sends; Escape on the replacement asks nothing more. Catches a Find and replace
  // that never reaches the edit and an outcome nobody sees.
  test("Find and replace asks in VS Code's input boxes and shows the outcome", async () => {
    const file = path.join(folder, "replace.ladder.json");
    fs.writeFileSync(file, JSON.stringify(PROGRAM, null, 2));
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    const provider = new LadderEditorProvider({ extensionPath: process.cwd() } as vscode.ExtensionContext);
    const panel = new FakeWebviewPanel();
    await provider.resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken
    );
    const webview = panel.webview;
    const window = vscode.window as unknown as {
      showInputBox: typeof vscode.window.showInputBox;
      showInformationMessage: typeof vscode.window.showInformationMessage;
    };
    const original = {
      showInputBox: window.showInputBox,
      showInformationMessage: window.showInformationMessage,
    };
    const answers: Array<string | undefined> = ["Start", "Run", "Start", undefined];
    const prompts: string[] = [];
    const shown: string[] = [];
    window.showInputBox = (async (options?: vscode.InputBoxOptions) => {
      prompts.push(options?.prompt ?? "");
      return answers.shift();
    }) as typeof vscode.window.showInputBox;
    window.showInformationMessage = (async (message: string) => {
      shown.push(message);
      return undefined;
    }) as typeof vscode.window.showInformationMessage;
    try {
      webview.send({ type: "findReplace" });
      const input = await webview.next("findReplaceInput");
      assert.deepStrictEqual([input.find, input.replace], ["Start", "Run"]);
      assert.deepStrictEqual(prompts, ["Find symbol/address", "Replace 'Start' with"]);

      webview.send({ type: "notify", level: "info", text: "Updated 1 occurrence(s)." });
      assert.deepStrictEqual(shown, ["Updated 1 occurrence(s)."]);

      webview.send({ type: "findReplace" });
      for (let attempt = 0; attempt < 50 && prompts.length < 4; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20));
      }
      assert.strictEqual(prompts.length, 4, "both boxes asked");
      const nothing = await Promise.race([
        webview.next("findReplaceInput").then(() => "sent"),
        new Promise((resolve) => setTimeout(() => resolve("nothing"), 300)),
      ]);
      assert.strictEqual(nothing, "nothing", "Escape on the replacement sends nothing");
    } finally {
      Object.assign(window, original);
      panel.dispose();
    }
  });

  // Catches a properties panel whose number fields convert the typed text themselves: the host
  // forwards the text, the language server stores the number it reads (130 snaps to 140) or
  // answers why it stores nothing.
  test("a typed number goes to the language server and comes back stored or refused", async () => {
    const file = path.join(folder, "fields.ladder.json");
    fs.writeFileSync(file, JSON.stringify(PROGRAM, null, 2));
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    const provider = new LadderEditorProvider({ extensionPath: process.cwd() } as vscode.ExtensionContext);
    const panel = new FakeWebviewPanel();
    await provider.resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken
    );
    const webview = panel.webview;
    const field = { op: "setNumber", programText: JSON.stringify(PROGRAM), rungIndex: 0, elementIndex: 0, field: "x" };

    webview.send({ type: "edit", requestId: 11, ...field, text: "130" });
    const stored = await webview.next("editResult");
    assert.strictEqual(stored.requestId, 11);
    assert.strictEqual(stored.ok, true, JSON.stringify(stored.errors));
    assert.strictEqual(stored.value, 140);
    const program = JSON.parse(String(stored.programText)) as {
      networks: Array<{ nodes: Array<{ position: { x: number; y: number } }> }>;
    };
    assert.deepStrictEqual(program.networks[0].nodes[0].position, { x: 140, y: 100 });

    webview.send({ type: "edit", requestId: 12, ...field, text: "" });
    const refused = await webview.next("editResult");
    assert.strictEqual(refused.requestId, 12);
    assert.strictEqual(refused.ok, false);
    assert.deepStrictEqual(refused.errors, [{ message: "Position X must be a number." }]);
    panel.dispose();
  });
});
