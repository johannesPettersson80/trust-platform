import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §4.9: popup files open in the HMI Builder at their own size, and the editor
// tells the webview which pages and popups a click can go to.

const POPUP = `format = 1
title = "Pump P101"
size = [400, 240]
pinnable = true
`;

function project(): { root: string; popup: string; page: string } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-popups-"));
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.mkdirSync(path.join(root, "hmi", "popups"), { recursive: true });
  const page = path.join(root, "hmi", "pages", "overview.toml");
  fs.writeFileSync(page, 'format = 1\ntitle = "Overview"\n');
  fs.writeFileSync(path.join(root, "hmi", "pages", "trends.toml"), 'format = 1\ntitle = "Trends"\n');
  const popup = path.join(root, "hmi", "popups", "pump.toml");
  fs.writeFileSync(popup, POPUP);
  return { root, popup, page };
}

suite("HMI Builder editor: popups", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a popup file opens with its size and pin, and an edit is written as a popup", async () => {
    const { popup } = project();
    const { document, webview } = await openBuilder(popup);
    webview.send({ type: "ready" });
    // The editor answers with the model, or with why the file does not load.
    await until("the file read", () => postedOfType(webview, "page")[0] ?? postedOfType(webview, "invalid")[0]);
    assert.deepStrictEqual(postedOfType(webview, "invalid"), [], "the popup loads");
    const page = postedOfType(webview, "page")[0];
    assert.deepStrictEqual([page.model.title, page.model.size, page.model.pinnable], ["Pump P101", [400, 240], true]);
    webview.send({ type: "edit", model: { ...page.model, size: [480, 240] } });
    await until("the text written", () => document.getText().includes("size = [480, 240]"));
    assert.ok(
      document.getText().startsWith('format = 1\ntitle = "Pump P101"\nsize = [480, 240]\npinnable = true\n'),
      document.getText(),
    );
  });

  test("the editor lists the project's pages and popups as click targets", async () => {
    const { page } = project();
    const { webview } = await openBuilder(page);
    webview.send({ type: "ready" });
    // The targets come with the rest of the start, before the variables.
    await until("the variables", () => postedOfType(webview, "variables")[0]);
    const targets = postedOfType(webview, "targets")[0];
    assert.ok(targets, "the targets");
    assert.deepStrictEqual([targets.pages, targets.popups], [["overview", "trends"], ["pump"]]);
  });

  test("the HMI Builder is the editor of popup files", () => {
    const manifest = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../package.json"), "utf8")) as {
      contributes: { customEditors: Array<{ viewType: string; selector: Array<{ filenamePattern: string }> }> };
    };
    const builder = manifest.contributes.customEditors.find((editor) => editor.viewType === "trust-lsp.hmiBuilder");
    assert.ok(builder?.selector.some((selector) => selector.filenamePattern === "**/hmi/popups/*.toml"));
  });
});
