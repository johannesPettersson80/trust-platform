import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

suite("HMI Builder host: inherited formats", function () {
  this.timeout(60000);
  test("saved and unsaved project formats reach the editor without editing the displayed page", async () => {
    await languageServerReady();
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-formats-"));
    fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
    fs.writeFileSync(path.join(root, "hmi", "screen.toml"), "format = 1\nsize = [1280, 720]\n");
    const page =
      'format = 1\ntitle = "Level"\n[[object]]\nid = "level"\ntype = "value"\nat = [0, 0]\nsize = [240, 96]\n[object.bind]\nvalue = "Main.Level"\n';
    const overview = path.join(root, "hmi", "pages", "overview.toml");
    const source = path.join(root, "hmi", "pages", "source.toml");
    fs.writeFileSync(overview, page);
    fs.writeFileSync(source, page.replace("[object.bind]", 'unit = "m"\ndecimals = 2\n[object.bind]'));
    const { panel, webview, document } = await openBuilder(overview);
    try {
      webview.send({ type: "ready" });
      const first = await until("the project catalogue", () => postedOfType(webview, "catalogue")[0]);
      assert.deepStrictEqual(first.formats?.["main.level"], { unit: "m", decimals: 2 });
      const doc = await vscode.workspace.openTextDocument(source);
      const offset = doc.getText().indexOf('unit = "m"');
      const edit = new vscode.WorkspaceEdit();
      edit.replace(
        doc.uri,
        new vscode.Range(doc.positionAt(offset), doc.positionAt(offset + 'unit = "m"'.length)),
        'unit = "cm"',
      );
      await vscode.workspace.applyEdit(edit);
      await until("the refreshed catalogue", () => postedOfType(webview, "catalogue").length > 1);
      const refreshed = postedOfType(webview, "catalogue");
      assert.strictEqual(refreshed[refreshed.length - 1]?.formats?.["main.level"]?.unit, "cm");
      assert.strictEqual(document.getText(), page);
      assert.strictEqual(document.isDirty, false);
      assert.ok(fs.readFileSync(source, "utf8").includes('unit = "m"'), "the source edit remains unsaved");
    } finally {
      panel.dispose();
    }
  });
});
