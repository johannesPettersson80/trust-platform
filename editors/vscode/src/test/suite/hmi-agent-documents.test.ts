import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { WorkspaceDocuments } from "../../hmiBuilder/agent/documents";

suite("HMI agent workspace snapshots", function () {
  this.timeout(20000);
  let root: string;
  let documents: WorkspaceDocuments;
  setup(async () => {
    root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-docs-"));
    await fs.mkdir(path.join(root, "hmi/pages"), { recursive: true });
    await fs.writeFile(path.join(root, "hmi/pages/overview.toml"), "saved");
    documents = new WorkspaceDocuments(vscode.Uri.file(root));
  });
  teardown(async () => {
    for (const document of vscode.workspace.textDocuments.filter((d) => d.uri.fsPath.startsWith(root + path.sep))) {
      if (document.isDirty) await document.save();
    }
    await fs.rm(root, { recursive: true, force: true });
  });
  test("uses dirty editor text and changes revision even before save", async () => {
    const before = await documents.read("hmi/pages/overview.toml");
    assert.strictEqual(before.text, "saved");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, "hmi/pages/overview.toml")));
    const edit = new vscode.WorkspaceEdit();
    edit.replace(doc.uri, new vscode.Range(0, 0, 0, 5), "manual edit");
    assert.ok(await vscode.workspace.applyEdit(edit));
    const after = await documents.read("hmi/pages/overview.toml");
    assert.strictEqual(after.text, "manual edit");
    assert.ok(after.dirty);
    assert.strictEqual(after.version, doc.version);
    assert.notStrictEqual(after.revision, before.revision);
    assert.strictEqual(await fs.readFile(doc.uri.fsPath, "utf8"), "saved");
  });
  test("missing and existing empty files have different revisions", async () => {
    const missing = await documents.read("hmi/pages/new.toml");
    assert.strictEqual(missing.exists, false);
    await fs.writeFile(path.join(root, "hmi/pages/new.toml"), "");
    const present = await documents.read("hmi/pages/new.toml");
    assert.strictEqual(present.exists, true);
    assert.notStrictEqual(present.revision, missing.revision);
  });
  test("rejects traversal, absolute paths and another URI authority", async () => {
    for (const file of [
      "../escape",
      "/tmp/outside",
      "hmi/../../escape",
      "file:///tmp/outside",
      "hmi\\..\\outside",
      "hmi//pages/a",
      "hmi/./pages/a",
    ]) {
      await assert.rejects(documents.read(file), /relative|path|workspace/i, file);
    }
    await assert.rejects(
      new WorkspaceDocuments(vscode.Uri.parse("vscode-remote://other/project")).read("hmi/a"),
      /unsupported/i,
    );
  });
  test("rejects symlink escape including a new file below an escaped directory", async function () {
    if (process.platform === "win32") this.skip();
    const outside = await fs.mkdtemp(path.join(os.tmpdir(), "trust-agent-outside-"));
    try {
      await fs.symlink(outside, path.join(root, "hmi/escaped"));
      await assert.rejects(documents.read("hmi/escaped/new.toml"), /workspace/i);
    } finally {
      await fs.rm(outside, { recursive: true, force: true });
    }
  });
  test("does not treat permission or invalid-path errors as a missing file", async () => {
    await assert.rejects(documents.read("hmi/pages/overview.toml/child"));
  });
  test("enforces per-file bytes and cancellation", async () => {
    await fs.writeFile(path.join(root, "hmi/pages/large.toml"), "é".repeat(600000));
    await assert.rejects(documents.read("hmi/pages/large.toml"), /1 MiB/);
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(documents.read("hmi/pages/overview.toml", controller.signal), /cancel/i);
  });
  test("a clean cached editor model does not resurrect a file deleted from disk", async () => {
    const file = "hmi/pages/overview.toml";
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, file)));
    assert.strictEqual(document.isDirty, false);
    await fs.unlink(path.join(root, file));
    const snapshot = await documents.read(file);
    assert.strictEqual(snapshot.exists, false, "deleted clean models are not existing source files");
    assert.strictEqual(snapshot.text, "");
    assert.strictEqual(snapshot.version, null);
  });
});
