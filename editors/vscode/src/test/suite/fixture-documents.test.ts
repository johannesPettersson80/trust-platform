import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { disposeFixtureProject } from "./fixtureDocuments";

// The clean-up of a generated test project that the HMI suites share (fixtureDocuments.ts). Its flakes failed a
// first attempt of the VS Code job on two release pull requests: W18 (1 November 2026) and G225 (19 November 2026).

suite("Test fixture clean-up", function () {
  this.timeout(30000);

  test("the clean-up opens no editor for a file the test deleted, and closes the editors of the files that exist", async () => {
    // G225: the HMI agent's apply creates a page and its Undo deletes it, while VS Code still lists the page's
    // document as open. Catches a clean-up that shows that document to close it: VS Code then opens an editor for a
    // file that is gone, or fails at random with "Could NOT open editor for …" when it has already dropped the model.
    // Also catches a clean-up that leaves the editor of a file that exists open, or the folder on disk.
    const root = await fs.mkdtemp(path.join(os.tmpdir(), "trust-fixture-cleanup-"));
    const kept = path.join(root, "hmi", "pages", "kept.toml");
    const gone = path.join(root, "hmi", "pages", "gone.toml");
    await fs.mkdir(path.dirname(kept), { recursive: true });
    await fs.writeFile(kept, "kept = true\n");
    await fs.writeFile(gone, "gone = true\n");
    const goneDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(gone));
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(vscode.Uri.file(kept)), {
      preview: false,
    });
    await fs.rm(gone);
    assert.strictEqual(goneDocument.isClosed, false, "VS Code still lists the deleted file's document as open");
    const shown: string[] = [];
    const watch = vscode.window.onDidChangeVisibleTextEditors((editors) =>
      shown.push(...editors.map((editor) => editor.document.uri.fsPath)),
    );
    try {
      await disposeFixtureProject(root);
    } finally {
      watch.dispose();
    }
    assert.ok(!shown.includes(gone), `the clean-up showed the deleted file: ${JSON.stringify(shown)}`);
    const open = vscode.window.tabGroups.all
      .flatMap((group) => group.tabs)
      .filter((tab) => tab.input instanceof vscode.TabInputText && tab.input.uri.fsPath.startsWith(root + path.sep));
    assert.deepStrictEqual(
      open.map((tab) => tab.label),
      [],
      "no editor of the project stays open",
    );
    await assert.rejects(fs.stat(root), "the project's folder is deleted");
  });
});
