import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { openBuilder, postedOfType } from "./hmiBuilderHost";
import { liveBridge, liveGate, liveOperation, watchHmiEdits } from "../../hmiBuilder/editor/liveOperatorRuntime";

/** Waits up to five seconds for `check` to hold. */
async function until(check: () => boolean, what: string): Promise<void> {
  const deadline = Date.now() + 5_000;
  while (!check()) {
    assert.ok(Date.now() < deadline, what);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

/** Drops the unsaved edits of the open documents at `paths` and closes them. */
async function discardEdits(paths: string[]): Promise<void> {
  const uris = new Set(paths.map((file) => vscode.Uri.file(file).toString()));
  for (const document of vscode.workspace.textDocuments.filter((open) => uris.has(open.uri.toString()))) {
    await vscode.window.showTextDocument(document);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  }
}

suite("HMI Builder host: named Live operation", function () {
  this.timeout(15000);
  test("the editor routes its operating connection and closes it with its panel", async () => {
    // Catches an edit of the HMI that leaves operation running, and an edit of another project file that ends it: the
    // language server names the HMI's files, and the editor watches them (specification 39, section 11 item 6).
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-live-"));
    fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
    fs.mkdirSync(path.join(root, "src"));
    const file = path.join(root, "hmi", "pages", "overview.toml");
    const program = path.join(root, "src", "main.st");
    fs.writeFileSync(file, 'format = 1\ntitle = "Cell"\n');
    fs.writeFileSync(program, "PROGRAM Main\nEND_PROGRAM\n");
    const requests: string[] = [];
    const { panel, webview } = await openBuilder(file, undefined, undefined, undefined, {
      unsaved: () => ({ root, unsaved: [] }),
      bundle: async () => ({ ok: true, version: "v1" }),
      runTarget: async () => ({ label: "Cell", simulator: false }),
      liveGate,
      liveOperation,
      watchEdits: (edited) => watchHmiEdits(root, edited),
      session: {
        bridge: liveBridge,
        target: async () => ({ url: "http://plc:8080", endpoint: "tcp://plc:9000", label: "Cell" }),
        request: async (_target, _method, route) => {
          requests.push(route);
          return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
        },
      },
    });
    try {
      webview.send({ type: "operateOpen", id: "one", page: "overview" });
      await until(() => postedOfType(webview, "operateReady").length > 0, "the operating view opens");
      assert.deepStrictEqual(
        postedOfType(webview, "operateReady"),
        [{ type: "operateReady", id: "one", target: "Cell" }],
        "the provider connects the Live operating view",
      );
      const source = new vscode.WorkspaceEdit();
      source.insert(vscode.Uri.file(program), new vscode.Position(0, 0), "// changed\n");
      await vscode.workspace.applyEdit(source);
      await new Promise((resolve) => setTimeout(resolve, 300));
      assert.deepStrictEqual(postedOfType(webview, "operateEnded"), [], "an edit of the program is no HMI change");
      const edit = new vscode.WorkspaceEdit();
      edit.insert(vscode.Uri.file(file), new vscode.Position(0, 0), "# changed\n");
      await vscode.workspace.applyEdit(edit);
      await until(() => postedOfType(webview, "operateEnded").length > 0, "editing the HMI ends operation");
      assert.ok(
        postedOfType(webview, "operateEnded").some(
          (message) => message.reason === "The HMI changed. Save and deploy it before operating again.",
        ),
        "editing the HMI ends its operating connection immediately, with the language server's reason",
      );
      panel.dispose();
      webview.send({ type: "operateRequest", id: "one", request: 1, method: "POST", path: "/api/hmi/login", body: {} });
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.deepStrictEqual(requests, ["/api/hmi/page?name=overview"]);
    } finally {
      panel.dispose();
      // Nothing of this test stays in the language server for the tests after it, where its `PROGRAM Main` would
      // collide with each project's `Main`. A closed document stays in the server's project while its file exists
      // (specification 14, 7.2.1), so the folder goes first; the edited documents then close without saving.
      fs.rmSync(root, { recursive: true, force: true });
      await discardEdits([program, file]);
    }
    await until(
      () => !vscode.workspace.textDocuments.some((open) => open.uri.fsPath === program),
      "the edited program closes, so that the language server drops it",
    );
  });
});
