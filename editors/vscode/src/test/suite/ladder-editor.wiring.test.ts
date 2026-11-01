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
      {} as vscode.CancellationToken,
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
      ["edge_rung_1_0"],
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
      {} as vscode.CancellationToken,
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

  // Catches an editor that deletes, toggles or stores a field itself: the host forwards each edit
  // and the language server answers the program (the contact gone, its wires and implicit locals
  // with it), the toggled type, or why a field's value is not stored.
  test("deleting, toggling and a field go to the language server", async () => {
    const file = path.join(folder, "elements.ladder.json");
    fs.writeFileSync(file, JSON.stringify(PROGRAM, null, 2));
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    const provider = new LadderEditorProvider({ extensionPath: process.cwd() } as vscode.ExtensionContext);
    const panel = new FakeWebviewPanel();
    await provider.resolveCustomTextEditor(
      document,
      panel as unknown as vscode.WebviewPanel,
      {} as vscode.CancellationToken,
    );
    const webview = panel.webview;
    const edit = { programText: JSON.stringify(PROGRAM), rungIndex: 0 };

    webview.send({ type: "edit", requestId: 21, op: "removeElement", ...edit, elementIndex: 0 });
    const removed = await webview.next("editResult");
    assert.strictEqual(removed.requestId, 21);
    assert.strictEqual(removed.ok, true, JSON.stringify(removed.errors));
    const program = JSON.parse(String(removed.programText)) as {
      networks: Array<{ nodes: Array<{ id: string }> }>;
    };
    assert.deepStrictEqual(
      program.networks[0].nodes.map((node) => node.id),
      ["q1"],
    );

    webview.send({ type: "edit", requestId: 22, op: "toggleContactType", ...edit, elementIndex: 0 });
    const toggled = await webview.next("editResult");
    assert.strictEqual(toggled.contactType, "NC");

    webview.send({
      type: "edit",
      requestId: 23,
      op: "setField",
      ...edit,
      elementIndex: 1,
      field: "coilType",
      value: "PULSE",
    });
    const refused = await webview.next("editResult");
    assert.strictEqual(refused.requestId, 23);
    assert.strictEqual(refused.ok, false);
    assert.deepStrictEqual(refused.errors, [{ message: "coilType must be one of NORMAL, SET, RESET, NEGATED." }]);
    panel.dispose();
  });
});
