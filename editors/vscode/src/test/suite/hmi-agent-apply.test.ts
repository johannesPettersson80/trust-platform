import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { applyEditorFiles } from "../../hmiBuilder/agent/apply";

// Specification 43, AT-09: the editor transaction writes the files the authoring preflight prepared, in one
// WorkspaceEdit. The preflight's conflict and receipt rules are proved by the Rust tests.

suite("HMI agent editor transactions", function () {
  this.timeout(30000);
  let root: string;
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-apply-"));
    await fs.mkdir(path.join(root, "hmi/pages"), { recursive: true });
    await fs.writeFile(path.join(root, "hmi/pages/a.toml"), "original");
  });
  teardown(async () => {
    for (const doc of vscode.workspace.textDocuments.filter((d) => d.uri.fsPath.startsWith(root + path.sep)))
      if (doc.isDirty) await doc.save();
    await fs.rm(root, { recursive: true, force: true });
  });
  test("updates through the editor without implicit save and supports Undo", async () => {
    const file = "hmi/pages/a.toml";
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    await vscode.window.showTextDocument(doc);
    const outcome = await applyEditorFiles(
      vscode.Uri.file(root),
      [{ file, text: "agent change", create: false }],
      false,
      new AbortController().signal,
    );
    assert.strictEqual(outcome.applied, true);
    assert.strictEqual(doc.getText(), "agent change");
    assert.ok(doc.isDirty);
    assert.strictEqual(await fs.readFile(doc.uri.fsPath, "utf8"), "original");
    await vscode.commands.executeCommand("undo");
    assert.strictEqual(doc.getText(), "original");
  });
  test("creates a new page and explicit save persists its actual content", async () => {
    const outcome = await applyEditorFiles(
      vscode.Uri.file(root),
      [{ file: "hmi/pages/new.toml", text: "new page", create: true }],
      true,
      new AbortController().signal,
    );
    assert.strictEqual(outcome.applied, true);
    assert.deepStrictEqual(outcome.saved, ["hmi/pages/new.toml"]);
    assert.strictEqual(await fs.readFile(path.join(root, "hmi/pages/new.toml"), "utf8"), "new page");
  });
  test("a cancelled transaction writes nothing", async () => {
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(
      applyEditorFiles(
        vscode.Uri.file(root),
        [{ file: "hmi/pages/a.toml", text: "agent", create: false }],
        false,
        controller.signal,
      ),
      /CANCELLED/,
    );
    assert.strictEqual(await fs.readFile(path.join(root, "hmi/pages/a.toml"), "utf8"), "original");
  });
});
