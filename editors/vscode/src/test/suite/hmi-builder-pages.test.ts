import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { createHmiFile } from "../../hmiBuilder/editor/explorerFiles";
import {
  addPage,
  deletePage,
  duplicatePage,
  hidePage,
  listPages,
  movePage,
  renamePage,
} from "../../hmiBuilder/editor/pages";
import { disposeFixtureProject } from "./fixtureDocuments";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §3.3 item 8: the Pages panel's actions on the project's page files: add,
// rename, duplicate, reorder (written as `order`), hide from the navigation, delete after a
// confirmation; listed in the navigation's order. New HMI Page and New HMI Panel, and an Explorer
// rename that keeps the links. The rules are the language server's (`trust-lsp.hmiPages`,
// `trust-lsp.hmiFiles`, proven by its Rust tests); these tests prove the extension's wiring.

const fixtures: string[] = [];

function project(): vscode.Uri {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-pages-"));
  fixtures.push(root);
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  const page = (name: string, title: string, order?: number) =>
    fs.writeFileSync(
      path.join(root, "hmi", "pages", `${name}.toml`),
      `format = 1\ntitle = "${title}"\n${order === undefined ? "" : `order = ${order}\n`}`,
    );
  page("overview", "Overview", 0);
  page("trends", "Trends", 1);
  page("alarms", "Alarms");
  return vscode.Uri.file(root);
}

const listed = async (root: vscode.Uri) => (await listPages(root)).map((page) => [page.name, page.title, page.hidden]);

const textOf = async (uri: vscode.Uri): Promise<string> => (await vscode.workspace.openTextDocument(uri)).getText();

suite("HMI Builder: page operations", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    for (const root of fixtures.splice(0)) await disposeFixtureProject(root);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("the pages are listed in the navigation's order", async () => {
    assert.deepStrictEqual(await listed(project()), [
      ["overview", "Overview", false],
      ["trends", "Trends", false],
      ["alarms", "Alarms", false],
    ]);
  });

  test("rename, hide, duplicate, move and add change the page files", async () => {
    const root = project();
    await renamePage(root, "trends", "Trends 24 h");
    await hidePage(root, "alarms", true);
    await duplicatePage(root, "overview");
    await movePage(root, "alarms", "overview");
    const added = await addPage(root);
    assert.strictEqual(added, "page-1");
    assert.deepStrictEqual(await listed(root), [
      ["alarms", "Alarms", true],
      ["overview", "Overview", false],
      ["overview-copy", "Overview (copy)", false],
      ["trends", "Trends 24 h", false],
      ["page-1", "Page 1", false],
    ]);
    const alarms = fs.readFileSync(path.join(root.fsPath, "hmi", "pages", "alarms.toml"), "utf8");
    assert.match(alarms, /^order = 0$/m);
    assert.match(alarms, /^hidden = true$/m);
  });

  test("a page is deleted only after the confirmation", async () => {
    const root = project();
    await deletePage(root, "trends", async () => false);
    assert.ok(fs.existsSync(path.join(root.fsPath, "hmi", "pages", "trends.toml")), "kept without the confirmation");
    await deletePage(root, "trends", async () => true);
    assert.deepStrictEqual(
      (await listed(root)).map(([name]) => name),
      ["overview", "alarms"],
    );
  });

  test("New HMI Page and New HMI Panel create the files the language server gives, with a screen layout", async () => {
    const root = vscode.Uri.file(fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-new-")));
    fixtures.push(root.fsPath);
    const hmi = vscode.Uri.joinPath(root, "hmi");
    const screen = vscode.Uri.joinPath(hmi, "screen.toml");
    const page = await createHmiFile("page", hmi, "room_heating-2");
    assert.ok(page, "the page is created");
    assert.strictEqual(page.fsPath, path.join(root.fsPath, "hmi", "pages", "room_heating-2.toml"));
    assert.strictEqual(await textOf(page), 'format = 1\ntitle = "Room heating 2"\n', "titled with its name in words");
    assert.match(await textOf(screen), /^size = \[1920, 1080\]$/m, "the default screen layout comes with it");
    const panel = await createHmiFile("panel", hmi, "nav");
    assert.ok(panel, "the panel is created");
    assert.strictEqual(panel.fsPath, path.join(root.fsPath, "hmi", "panels", "nav.toml"));
    assert.match(await textOf(panel), /^size = \[240, 720\]$/m);
    await assert.rejects(createHmiFile("page", hmi, "my page"), /Invalid HMI file name/);
    assert.strictEqual(fs.existsSync(path.join(root.fsPath, "hmi", "pages", "my page.toml")), false);
    for (const uri of [page, panel, screen]) await (await vscode.workspace.openTextDocument(uri)).save();
  });

  test("an Explorer rename of a page updates the links to it through the language server", async () => {
    const root = project();
    fs.writeFileSync(
      path.join(root.fsPath, "hmi", "screen.toml"),
      'format = 1\nsize = [1280, 720]\ntheme = "dark"\nstart_page = "overview"\nsymbols = "flat"\n\n[navigation]\nstyle = "side"\n',
    );
    fs.mkdirSync(path.join(root.fsPath, "hmi", "popups"));
    fs.writeFileSync(
      path.join(root.fsPath, "hmi", "popups", "help.toml"),
      'format = 1\ntitle = "Help"\nsize = [400, 300]\n\n[[object]]\nid = "back"\ntype = "page-button"\nat = [16, 16]\nsize = [160, 48]\n\n[object.click]\npage = "overview"\n',
    );
    const edit = new vscode.WorkspaceEdit();
    edit.renameFile(
      vscode.Uri.joinPath(root, "hmi", "pages", "overview.toml"),
      vscode.Uri.joinPath(root, "hmi", "pages", "process.toml"),
    );
    assert.ok(await vscode.workspace.applyEdit(edit), "the page is renamed");
    assert.match(await textOf(vscode.Uri.joinPath(root, "hmi", "screen.toml")), /^start_page = "process"$/m);
    assert.match(await textOf(vscode.Uri.joinPath(root, "hmi", "popups", "help.toml")), /^page = "process"$/m);
    assert.strictEqual(
      await textOf(vscode.Uri.joinPath(root, "hmi", "pages", "trends.toml")),
      'format = 1\ntitle = "Trends"\norder = 1\n',
      "a file without a link to the page keeps every byte",
    );
  });
});
