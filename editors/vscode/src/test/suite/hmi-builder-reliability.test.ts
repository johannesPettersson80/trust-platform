import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { HostToWebview } from "../../hmiBuilder/editor/messages";
import { FakePanel, FakeWebview, languageServerReady, openBuilder, until } from "./hmiBuilderHost";

// Specification 39, §3.1 items 3–5 and §19.2: split views, changes on disk while edits are unsaved,
// and recovery after a restart.

const PAGE = `format = 1
title = "Overview"

[[object]]
id = "speed"
type = "value"
at = [40, 40]
size = [240, 96]
`;

function lastPage(webview: FakeWebview): HmiPageModel | undefined {
  const pages = webview.posted.filter(
    (message): message is Extract<HostToWebview, { type: "page" }> => message.type === "page",
  );
  return pages[pages.length - 1]?.model;
}

async function builderOn(
  file: string,
): Promise<{ document: vscode.TextDocument; webview: FakeWebview; panel: FakePanel }> {
  const opened = await openBuilder(file);
  opened.webview.send({ type: "ready" });
  await until("the page", () => lastPage(opened.webview));
  return opened;
}

function pageFile(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-reliability-"));
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  const file = path.join(root, "hmi", "pages", "overview.toml");
  fs.writeFileSync(file, PAGE);
  return file;
}

function moved(page: HmiPageModel, at: [number, number]): HmiPageModel {
  const next: HmiPageModel = JSON.parse(JSON.stringify(page));
  next.object[0].at = at;
  return next;
}

suite("HMI Builder editor: reliability", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a page open in the builder and in a text editor beside it stays the same in both", async () => {
    const { document, webview, panel } = await builderOn(pageFile());
    const editor = await vscode.window.showTextDocument(document, vscode.ViewColumn.Beside);
    await editor.edit((edit) => edit.replace(new vscode.Range(1, 0, 2, 0), 'title = "Split"\n'));
    await until("the builder sees the text editor's change", () => lastPage(webview)?.title === "Split");
    webview.send({ type: "edit", model: moved(lastPage(webview) as HmiPageModel, [48, 48]) });
    await until("the text editor sees the builder's change", () => editor.document.getText().includes("at = [48, 48]"));
    panel.dispose();
  });

  test("closing the builder while it is still loading raises no error", async () => {
    const errors: string[] = [];
    const record = (reason: unknown): void => {
      errors.push(String(reason));
    };
    process.on("unhandledRejection", record);
    try {
      const { webview, panel } = await openBuilder(pageFile());
      webview.send({ type: "ready" });
      panel.dispose();
      await new Promise((resolve) => setTimeout(resolve, 3000));
      assert.deepStrictEqual(errors, []);
    } finally {
      process.off("unhandledRejection", record);
    }
  });

  test("a change on disk never replaces the builder's unsaved edit, and the file keeps the other version", async () => {
    const file = pageFile();
    const { document, webview, panel } = await builderOn(file);
    webview.send({ type: "edit", model: moved(lastPage(webview) as HmiPageModel, [48, 48]) });
    await until("the unsaved edit", () => document.isDirty && document.getText().includes("at = [48, 48]"));
    fs.writeFileSync(file, PAGE.replace('title = "Overview"', 'title = "Changed on disk"'));
    await new Promise((resolve) => setTimeout(resolve, 2500));
    assert.ok(document.getText().includes("at = [48, 48]"), "the unsaved edit stays");
    assert.ok(document.isDirty, "and stays unsaved");
    assert.ok(fs.readFileSync(file, "utf8").includes("Changed on disk"), "the file keeps the version written on disk");
    assert.ok(
      !webview.posted.some((message) => message.type === "page" && message.model.title === "Changed on disk"),
      "the builder never shows the disk version over the edit",
    );
    panel.dispose();
  });
});

suite("HMI Builder editor: restart recovery", function () {
  this.timeout(60000);

  // After a restart VS Code restores a page's unsaved text into its text document (hot exit; VS Code
  // keeps no backups for extension-development windows, so a real restart is checked with the
  // installed extension by runners/cdp_hmi_builder_restart.js). The builder's part: it shows the
  // document's text, unsaved changes included, never the file on disk.
  test("the builder shows a page's unsaved text, not the file on disk", async () => {
    const file = pageFile();
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(1, 0, 2, 0), 'title = "Unsaved after restart"\n');
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(document.isDirty);
    const { webview, panel } = await builderOn(file);
    assert.strictEqual(lastPage(webview)?.title, "Unsaved after restart");
    assert.ok(
      !fs.readFileSync(file, "utf8").includes("Unsaved after restart"),
      "the file on disk still has the saved text",
    );
    panel.dispose();
  });
});
