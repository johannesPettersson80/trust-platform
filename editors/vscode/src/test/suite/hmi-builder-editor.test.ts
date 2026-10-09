import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

// Specification 39, §3.1: page files open in the HMI Builder; other files stay text.

const PAGE = 'format = 1\ntitle = "Overview"\n';
const SCREEN = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';

function project(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-builder-"));
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.writeFileSync(path.join(root, "hmi", "pages", "overview.toml"), PAGE);
  fs.writeFileSync(path.join(root, "hmi", "screen.toml"), SCREEN);
  fs.writeFileSync(path.join(root, "runtime.toml"), "[bundle]\nversion = 1\n");
  return root;
}

/** The input of the active tab once it shows the file, whatever editor shows it. */
async function tabShowing(file: string): Promise<unknown> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const input = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
    const uri = input instanceof vscode.TabInputCustom || input instanceof vscode.TabInputText ? input.uri : undefined;
    if (uri && path.normalize(uri.fsPath) === path.normalize(file)) return input;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.fail(`no tab shows ${file}`);
}

suite("HMI Builder editor: opening files", function () {
  this.timeout(30000);

  teardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a page file opens in the HMI Builder, and Open as text stays available", async () => {
    const page = path.join(project(), "hmi", "pages", "overview.toml");
    await vscode.commands.executeCommand("vscode.open", vscode.Uri.file(page));
    const input = await tabShowing(page);
    assert.ok(input instanceof vscode.TabInputCustom, `a custom editor, not ${String(input)}`);
    assert.strictEqual(input.viewType, "trust-lsp.hmiBuilder");

    await vscode.commands.executeCommand("vscode.openWith", vscode.Uri.file(page), "default");
    assert.ok((await tabShowing(page)) instanceof vscode.TabInputText, "the text editor");
  });

  test("screen settings open in their editor and runtime TOML opens as text", async () => {
    const root = project();
    for (const file of [path.join(root, "hmi", "screen.toml"), path.join(root, "runtime.toml")]) {
      await vscode.commands.executeCommand("vscode.open", vscode.Uri.file(file));
      const tab = await tabShowing(file);
      if (file.endsWith("screen.toml")) assert.ok(tab instanceof vscode.TabInputCustom, file);
      else assert.ok(tab instanceof vscode.TabInputText, file);
    }
  });
});
