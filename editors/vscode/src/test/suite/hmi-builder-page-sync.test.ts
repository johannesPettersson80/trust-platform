import { PageSync } from "../../hmiBuilder/editor/pageSync";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { HostToWebview } from "../../hmiBuilder/editor/messages";
import { smallestEdit } from "../../hmiBuilder/editor/pageDocument";
import type { PageFiles } from "../../hmiBuilder/editor/pageDocument";
import { FakePanel, FakeWebview, languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §3.1 and §15.2: the builder reads the page from its text and writes every edit
// back as the smallest text edit, through the language server's own reader and writer.

const PAGE = `format = 1
title = "Overview"

[[object]]
id = "speed"
type = "value"
at = [40, 40]
size = [240, 96]
label = "Pump speed"

[object.bind]
value = "Main.P101.Speed"

[[object]]
id = "level"
type = "bar"
at = [320, 40]
size = [80, 240]
`;

suite("HMI Builder editor: smallest edit", () => {
  test("equal texts need no edit, whatever their line endings", () => {
    assert.strictEqual(smallestEdit("a\nb\n", "a\nb\n"), undefined);
    assert.strictEqual(smallestEdit("a\r\nb\r\n", "a\nb\n"), undefined);
  });

  test("one changed line is replaced alone", () => {
    assert.deepStrictEqual(smallestEdit("a\nb\nc\n", "a\nB\nc\n"), { startLine: 1, endLine: 2, text: "B\n" });
  });

  test("inserted and removed lines touch only themselves", () => {
    assert.deepStrictEqual(smallestEdit("a\nc\n", "a\nb\nc\n"), { startLine: 1, endLine: 1, text: "b\n" });
    assert.deepStrictEqual(smallestEdit("a\nb\nc\n", "a\nc\n"), { startLine: 1, endLine: 2, text: "" });
    assert.deepStrictEqual(smallestEdit("a\nb", "a\nb\nc\n"), { startLine: 1, endLine: 2, text: "b\nc\n" });
  });
});

async function openedPage(
  text = PAGE,
  screen?: string,
): Promise<{ document: vscode.TextDocument; webview: FakeWebview; panel: FakePanel; root: string }> {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-sync-"));
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  const file = path.join(root, "hmi", "pages", "overview.toml");
  fs.writeFileSync(file, text);
  if (screen) fs.writeFileSync(path.join(root, "hmi", "screen.toml"), screen);
  return { ...(await openBuilder(file)), root };
}

function posted<T extends HostToWebview["type"]>(
  webview: FakeWebview,
  type: T,
): Extract<HostToWebview, { type: T }> | undefined {
  return postedOfType(webview, type)[0];
}

function lastPage(webview: FakeWebview, after = 0): HmiPageModel | undefined {
  const pages = webview.posted
    .slice(after)
    .filter((message): message is Extract<HostToWebview, { type: "page" }> => message.type === "page");
  return pages[pages.length - 1]?.model;
}

function changedLines(before: string, after: string): string[] {
  const old = before.split("\n");
  return after.split("\n").filter((line, index) => line !== old[index]);
}

suite("HMI Builder editor: page document", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  test("the editor receives the page the language server reads from the text", async () => {
    const { webview, panel } = await openedPage();
    webview.send({ type: "ready" });
    const page = await until("the page", () => lastPage(webview));
    assert.strictEqual(page.title, "Overview");
    assert.deepStrictEqual(
      page.object.map((object) => object.id),
      ["speed", "level"],
    );
    assert.strictEqual(page.object[0].bind.value.path, "Main.P101.Speed");
    panel.dispose();
  });

  test("a text change reaches the editor; text that does not load keeps the last page and sends the errors", async () => {
    const { document, webview, panel } = await openedPage();
    webview.send({ type: "ready" });
    await until("the first page", () => lastPage(webview));

    const retitle = new vscode.WorkspaceEdit();
    retitle.replace(document.uri, new vscode.Range(1, 0, 2, 0), 'title = "Feed area"\n');
    await vscode.workspace.applyEdit(retitle);
    await until("the new title", () => (lastPage(webview)?.title === "Feed area" ? true : undefined));

    const posted = webview.posted.length;
    const broken = new vscode.WorkspaceEdit();
    broken.insert(document.uri, new vscode.Position(2, 0), 'colour = "red"\n');
    await vscode.workspace.applyEdit(broken);
    const invalid = await until("the errors", () =>
      webview.posted
        .slice(posted)
        .find((message): message is Extract<HostToWebview, { type: "invalid" }> => message.type === "invalid"),
    );
    assert.ok(
      invalid.errors.some((error) => error.line === 3),
      JSON.stringify(invalid.errors),
    );
    assert.strictEqual(lastPage(webview, posted), undefined, "no page is sent while the text does not load");
    panel.dispose();
  });

  test("saving, which changes only whether the page is unsaved, sends no page", async () => {
    const { document, webview, panel } = await openedPage();
    webview.send({ type: "ready" });
    await until("the first page", () => lastPage(webview));
    const retitle = new vscode.WorkspaceEdit();
    retitle.replace(document.uri, new vscode.Range(1, 0, 2, 0), 'title = "Saved"\n');
    await vscode.workspace.applyEdit(retitle);
    await until("the new title", () => (lastPage(webview)?.title === "Saved" ? true : undefined));
    await new Promise((resolve) => setTimeout(resolve, 1000));
    const posted = webview.posted.length;
    assert.ok(await document.save());
    await new Promise((resolve) => setTimeout(resolve, 1500));
    assert.strictEqual(lastPage(webview, posted), undefined, "the text did not change");
    panel.dispose();
  });

  test("moving one object is written by the language server as a one-line edit", async () => {
    const { document, webview, panel } = await openedPage();
    webview.send({ type: "ready" });
    const page = await until("the page", () => lastPage(webview));
    const before = document.getText();
    const moved: HmiPageModel = JSON.parse(JSON.stringify(page));
    moved.object[1].at = [328, 48];
    webview.send({ type: "edit", model: moved });
    await until("the edit", () => (document.getText() !== before ? true : undefined));
    assert.deepStrictEqual(changedLines(before, document.getText()), ["at = [328, 48]"]);
    assert.ok(document.isDirty, "an edit leaves the document unsaved, like any text edit");
    panel.dispose();
  });

  test("on ready the editor sends the component catalogue and the project's screen layout", async () => {
    const screen =
      'format = 1\nsize = [1280, 720]\ntheme = "light"\nsymbols = "flat"\n\n[colors.light]\naccent = "#0e7490"\n';
    const { webview, panel } = await openedPage(PAGE, screen);
    webview.send({ type: "ready" });
    const catalogue = await until("the catalogue", () => posted(webview, "catalogue"));
    assert.ok(
      catalogue.catalogue.components.some((component) => component.type === "value"),
      "the value component",
    );
    const layout = await until("the screen", () => posted(webview, "screen"));
    // With the top bar and the navigation, which leave the page area that previews lay out (§4 item 5).
    assert.deepStrictEqual(layout.screen, {
      size: [1280, 720],
      theme: "light",
      symbols: "flat",
      colors: { light: { accent: "#0e7490" } },
      topBar: false,
      navigation: "side",
    });
    panel.dispose();
  });

  test("a saved change to the screen file reaches the open canvas: theme and size (§3.4)", async () => {
    const screen = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';
    const { webview, panel, root } = await openedPage(PAGE, screen);
    webview.send({ type: "ready" });
    await until("the screen", () => posted(webview, "screen"));
    fs.writeFileSync(
      path.join(root, "hmi", "screen.toml"),
      'format = 1\nsize = [1024, 600]\ntheme = "light"\nsymbols = "flat"\n',
    );
    await new Promise((resolve) => setTimeout(resolve, 3000));
    const changed = postedOfType(webview, "screen").find((message) => message.screen.theme === "light");
    assert.ok(changed, `the canvas got the changed screen: ${JSON.stringify(postedOfType(webview, "screen"))}`);
    assert.deepStrictEqual(
      [changed.screen.size, changed.screen.theme, changed.screen.symbols],
      [[1024, 600], "light", "flat"],
    );
    panel.dispose();
  });

  test("the panel arrangement a webview sends is remembered for the user and sent to every editor on ready", async () => {
    const first = await openedPage();
    const arrangement = {
      left: { width: 320, collapsed: false },
      right: { width: 300, collapsed: true },
      bottom: { open: true, height: 260 },
    };
    first.webview.send({ type: "arrangement", arrangement });
    first.panel.dispose();
    const second = await openedPage();
    second.webview.send({ type: "ready" });
    const sent = await until("the arrangement", () => posted(second.webview, "arrangement"));
    assert.deepStrictEqual(sent.arrangement, arrangement);
    second.panel.dispose();
  });

  test("without a screen file the canvas gets 1920 by 1080 in the dark theme", async () => {
    const { webview, panel } = await openedPage();
    webview.send({ type: "ready" });
    const layout = await until("the screen", () => posted(webview, "screen"));
    assert.deepStrictEqual(layout.screen, { size: [1920, 1080], theme: "dark" });
    panel.dispose();
  });

  test("the editor page loads the webview bundle, the bundled font and the project's assets", async () => {
    const { webview, panel, root } = await openedPage();
    assert.match(webview.html, /hmiBuilderWebview\.js/);
    assert.match(webview.html, /hmiBuilderWebview\.css/);
    assert.match(webview.html, /fonts\/InterVariable\.woff2/);
    assert.match(webview.html, /data-asset-base="[^"]+"/);
    const roots = (webview.options.localResourceRoots ?? []).map((uri) => path.normalize(uri.fsPath));
    assert.ok(roots.includes(path.normalize(root)), `the project folder may be loaded: ${roots.join(", ")}`);
    panel.dispose();
  });

  test("an edit that changes nothing leaves every byte as it was", async () => {
    const { document, webview, panel } = await openedPage();
    webview.send({ type: "ready" });
    const page = await until("the page", () => lastPage(webview));
    const version = document.version;
    webview.send({ type: "edit", model: page });
    await new Promise((resolve) => setTimeout(resolve, 1500));
    assert.strictEqual(document.version, version);
    assert.strictEqual(document.getText(), PAGE);
    panel.dispose();
  });
});

suite("HMI/PLC source transaction conflicts", function () {
  this.timeout(20000);
  test("a page changed during preparation is never overwritten", async () => {
    const document = await vscode.workspace.openTextDocument({ language: "toml", content: "before" });
    const model: HmiPageModel = { title: "before", object: [], pipe: [] };
    const messages: HostToWebview[] = [];
    const sync = new PageSync(
      document,
      (message) => messages.push(message),
      {
        read: async () => ({ ok: true, model }),
        write: async () => ({ ok: true, text: "generated" }),
      },
      undefined,
      async () => {
        const manual = new vscode.WorkspaceEdit();
        manual.insert(document.uri, new vscode.Position(0, 0), "manual ");
        await vscode.workspace.applyEdit(manual);
        return undefined;
      },
    );
    await sync.refresh();
    await sync.apply({ ...model, title: "generated" });
    assert.strictEqual(document.getText(), "manual before");
    assert.ok(messages.some((message) => message.type === "editRefused"));
  });
  test("an edit made on the page as it was before a change outside the canvas is refused, and that change is kept", async () => {
    const document = await vscode.workspace.openTextDocument({ language: "toml", content: 'title = "Start"\n' });
    const messages: HostToWebview[] = [];
    const sync = new PageSync(document, (message) => messages.push(message), titledPages());
    await sync.refresh();
    const typed = new vscode.WorkspaceEdit();
    typed.replace(document.uri, new vscode.Range(0, 0, 1, 0), 'title = "Typed"\n');
    assert.ok(await vscode.workspace.applyEdit(typed));
    // The canvas has not had the typed page yet: its edit was made on "Start".
    await sync.apply(titledPage("Dragged"));
    assert.strictEqual(document.getText(), 'title = "Typed"\n', "the change made outside the canvas is kept");
    const refused = messages.filter(
      (message): message is Extract<HostToWebview, { type: "editRefused" }> => message.type === "editRefused",
    );
    assert.strictEqual(refused.length, 1, "the conflict is reported");
    assert.match(refused[0].errors[0].message, /not applied/);
    // Once the canvas has the changed page, its edits are written again.
    await sync.refresh();
    await sync.apply(titledPage("Dragged"));
    assert.strictEqual(document.getText(), 'title = "Dragged"\n');
  });
  test("preparation failure leaves the HMI document unchanged", async () => {
    const document = await vscode.workspace.openTextDocument({ language: "toml", content: "before" });
    const model: HmiPageModel = { title: "before", object: [], pipe: [] };
    const messages: HostToWebview[] = [];
    const sync = new PageSync(
      document,
      (message) => messages.push(message),
      {
        read: async () => ({ ok: true, model }),
        write: async () => ({ ok: true, text: "generated" }),
      },
      undefined,
      async () => {
        throw new Error("PLC document conflict");
      },
    );
    await sync.refresh();
    await sync.apply({ ...model, title: "generated" });
    assert.strictEqual(document.getText(), "before");
    assert.ok(
      messages.some(
        (message) => message.type === "editRefused" && message.errors[0].message === "PLC document conflict",
      ),
    );
  });
});

/** Page files whose text is the page's title alone; writing a title takes `delays[title]`
 * milliseconds, as the language server's answer can. */
function titledPages(delays: Record<string, number> = {}): PageFiles {
  return {
    read: async (text) => {
      const title = /^title = "(.*)"$/m.exec(text)?.[1];
      return title === undefined
        ? { ok: false, errors: [{ message: "no title" }] }
        : { ok: true, model: titledPage(title) };
    },
    write: async (model) => {
      await new Promise((resolve) => setTimeout(resolve, delays[model.title ?? ""] ?? 0));
      return { ok: true, text: `title = "${model.title}"\n` };
    },
  };
}

function titledPage(title: string): HmiPageModel {
  return { title, object: [], pipe: [] };
}

suite("HMI Builder editor: canvas edits in order (§21.3 item 4)", function () {
  this.timeout(20000);

  test("a second edit made while the first is being written lands after it, with no conflict message and no step back", async () => {
    const document = await vscode.workspace.openTextDocument({ language: "toml", content: 'title = "Start"\n' });
    const messages: HostToWebview[] = [];
    // The first edit's text takes longer to write than the second's.
    const sync = new PageSync(document, (message) => messages.push(message), titledPages({ First: 200 }));
    await sync.refresh();
    const before = messages.length;
    const texts: string[] = [];
    // As the editor does: every change of the text is read again for the canvas.
    const watch = vscode.workspace.onDidChangeTextDocument((change) => {
      if (change.document.uri.toString() !== document.uri.toString() || change.contentChanges.length === 0) return;
      texts.push(change.document.getText());
      void sync.refresh();
    });
    try {
      const first = sync.apply(titledPage("First"));
      const second = sync.apply(titledPage("Second"));
      await Promise.all([first, second]);
      await until("the page after both edits", () =>
        messages.slice(before).some((message) => message.type === "page" && message.model.title === "Second"),
      );
    } finally {
      watch.dispose();
    }
    assert.deepStrictEqual(
      texts,
      ['title = "First"\n', 'title = "Second"\n'],
      "each edit is written on top of the one before",
    );
    assert.strictEqual(document.getText(), 'title = "Second"\n');
    assert.deepStrictEqual(
      messages.filter((message) => message.type === "editRefused"),
      [],
      "no conflict is reported for the canvas's own edit",
    );
    const pages = messages.slice(before).flatMap((message) => (message.type === "page" ? [message.model.title] : []));
    assert.ok(
      pages.every((title) => title === "Second"),
      `the canvas is never sent back to the first edit: ${JSON.stringify(pages)}`,
    );
  });
});
