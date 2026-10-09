import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import {
  addPage,
  deletePage,
  duplicatePage,
  hidePage,
  listPages,
  movePage,
  renamePage,
} from "../../hmiBuilder/editor/pages";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §3.3 item 8: the Pages panel's actions on the project's page files: add,
// rename, duplicate, reorder (written as `order`), hide from the navigation, delete after a
// confirmation; listed in the navigation's order.

function project(): vscode.Uri {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-pages-"));
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

suite("HMI Builder: page operations", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

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
});
