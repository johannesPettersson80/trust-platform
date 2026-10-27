// Wiring of the SFC editor's host to the language server (specification 25 §11.4). The SFC rules
// are tested in Rust (`crates/trust-visual/src/sfc`); this suite proves that the host forwards each
// webview request to the `trust-lsp.visual*` commands and hands the answers back or writes them.

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { SfcEditorProvider } from "../../sfc/sfcEditor";

type Message = { type: string; [key: string]: unknown };

class FakeWebview {
  public options: vscode.WebviewOptions = {};
  public html = "";
  public readonly cspSource = "vscode-webview://sfc-test";
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

const CHART = {
  name: "Line",
  steps: [
    { id: "init", name: "Init", initial: true, x: 250, y: 50, actions: [] },
    {
      id: "a",
      name: "A",
      x: 200,
      y: 250,
      actions: [{ id: "act", name: "Lamp", qualifier: "N", body: "%QX0.0 := TRUE;" }],
    },
    { id: "b", name: "B", x: 300, y: 250, actions: [] },
  ],
  transitions: [
    { id: "t0", name: "Go", condition: "TRUE", sourceStepId: "init", targetStepId: "sp" },
    { id: "t1", name: "ToA", condition: "TRUE", sourceStepId: "sp", targetStepId: "a" },
  ],
  parallelSplits: [{ id: "sp", name: "Split", x: 250, y: 150, branchIds: ["a"] }],
  variables: [],
};

async function waitFor(condition: () => boolean, timeoutMs = 15000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!condition()) {
    if (Date.now() > deadline) {
      throw new Error("timed out waiting for the condition");
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

async function openEditor(file: string): Promise<FakeWebviewPanel> {
  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  const provider = new SfcEditorProvider({
    extensionPath: process.cwd(),
  } as vscode.ExtensionContext);
  const panel = new FakeWebviewPanel();
  await provider.resolveCustomTextEditor(
    document,
    panel as unknown as vscode.WebviewPanel,
    {} as vscode.CancellationToken
  );
  return panel;
}

suite("SFC editor wiring", function () {
  this.timeout(60000);
  let folder: string;

  suiteSetup(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-sfc-"));
  });

  suiteTeardown(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  test("the SFC editor reads, edits, checks, previews, generates and saves through the language server", async () => {
    const file = path.join(folder, "line.sfc.json");
    fs.writeFileSync(file, JSON.stringify(CHART, null, 2));
    const panel = await openEditor(file);
    const webview = panel.webview;

    // Open: the language server's canvas graph, with the split branch on its handle.
    const update = await webview.next("update");
    const graph = update.graph as {
      nodes: Array<{ id: string }>;
      edges: Array<{ id: string; sourceHandle?: string }>;
    };
    assert.deepStrictEqual(
      graph.nodes.map((node) => node.id),
      ["init", "a", "b", "sp"]
    );
    assert.strictEqual(graph.edges.find((edge) => edge.id === "t1")?.sourceHandle, "branch-out-0");

    // Edit: the split's free second branch may connect to B.
    webview.send({
      type: "edit",
      requestId: 7,
      graph,
      edit: {
        op: "connect",
        connection: { source: "sp", target: "b", sourceHandle: "branch-out-1", targetHandle: "step-target-top" },
        fallback: { sourceHandle: null, targetHandle: null },
        stamp: "99",
      },
    });
    const edited = await webview.next("editResult");
    const answer = edited.answer as { ok: boolean; edge?: { id: string } };
    assert.strictEqual(edited.requestId, 7);
    assert.strictEqual(answer.ok, true, JSON.stringify(answer));
    assert.strictEqual(answer.edge?.id, "trans_99");

    // Validate: the split has one branch.
    webview.send({ type: "validate", graph });
    const validation = await webview.next("validationResult");
    const issues = validation.errors as Array<{ id: string }>;
    assert.ok(
      issues.some((issue) => issue.id === "split_branch_count_sp"),
      JSON.stringify(issues)
    );

    // A chart Validate refuses has no companion: Preview ST shows Validate's finding.
    // (An action named On would be refused too: ON is a keyword of Structured Text.)
    webview.send({ type: "previewST", graph });
    const refused = await webview.next("codeGenerated");
    assert.strictEqual(refused.code, undefined);
    assert.ok(
      (refused.errors as string[]).some((error) => /at least 2 branches/.test(error)),
      JSON.stringify(refused.errors)
    );

    // With the second branch the edit answered, the chart is valid.
    const twoBranches = { ...graph, edges: [...graph.edges, answer.edge as { id: string }] };

    // Preview ST shows the companion without writing it.
    const companionFile = path.join(folder, "line.st");
    webview.send({ type: "previewST", graph: twoBranches });
    const preview = await webview.next("codeGenerated");
    assert.match(String(preview.code), /FUNCTION_BLOCK FB_line_SFC/);
    assert.match(String(preview.code), /SFC_T1 := Init_active;/);
    assert.match(String(preview.code), /ld_io_QX0_0 := TRUE;/);
    assert.strictEqual(fs.existsSync(companionFile), false, "Preview ST must not write the companion");

    // Generate ST writes the companion and the runtime entry beside the file.
    webview.send({ type: "generateST", graph: twoBranches });
    const generated = await webview.next("codeGenerated");
    assert.match(String(generated.code), /FUNCTION_BLOCK FB_line_SFC/);
    assert.match(fs.readFileSync(companionFile, "utf8"), /END_FUNCTION_BLOCK/);
    assert.match(
      fs.readFileSync(path.join(folder, "line.visual.runtime.st"), "utf8"),
      /ld_io_QX0_0 AT %QX0\.0 : BOOL;/
    );

    // Save writes the file the language server formats; it keeps the chart's own name and
    // stamps the save time.
    webview.send({ type: "save", graph: twoBranches });
    await waitFor(() => fs.readFileSync(file, "utf8").includes('"modified"'));
    const saved = JSON.parse(fs.readFileSync(file, "utf8"));
    assert.strictEqual(saved.name, "Line");
    assert.deepStrictEqual(saved.parallelSplits[0].branchIds, ["a", "b"]);
    assert.strictEqual(saved.metadata.version, "1.0");
    assert.strictEqual(typeof saved.metadata.modified, "string");

    panel.dispose();
  });

  test("a file that is no SFC opens as a recovery card", async () => {
    const file = path.join(folder, "broken.sfc.json");
    fs.writeFileSync(file, "{ \"steps\": ");
    const panel = await openEditor(file);
    const update = await panel.webview.next("update");
    assert.strictEqual(update.graph, undefined);
    assert.match(String(update.error), /Invalid JSON/);
    panel.dispose();
  });
});
