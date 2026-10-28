import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §14.4: the editor brings the project's own symbols with the catalogue and
// again when one changes, the Problems view checks their definitions, and HMI: Import Symbol makes
// one from an SVG file.

const SCREEN = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';
const PAGE = 'format = 1\ntitle = "Mixing"\n';
const DEFINITION =
  'format = 1\ntitle = "Agitator"\nsize = [128, 108]\n\n[properties]\nrunning = "boolean"\n\n[[part]]\nid = "rotor"\nspin = "running"\n';
const DRAWING =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 32"><rect id="body" width="64" height="32"/><g id="rotor"><rect width="8" height="30"/></g></svg>';

function project(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-symbols-"));
  fs.mkdirSync(path.join(root, "src"), { recursive: true });
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.mkdirSync(path.join(root, "hmi", "symbols"), { recursive: true });
  fs.writeFileSync(path.join(root, "src", "main.st"), "PROGRAM Main\nVAR\n    Running : BOOL;\nEND_VAR\nEND_PROGRAM\n");
  fs.writeFileSync(path.join(root, "hmi", "screen.toml"), SCREEN);
  fs.writeFileSync(path.join(root, "hmi", "pages", "mixing.toml"), PAGE);
  fs.writeFileSync(path.join(root, "hmi", "symbols", "agitator.toml"), DEFINITION);
  fs.writeFileSync(path.join(root, "hmi", "symbols", "agitator.svg"), DRAWING);
  return root;
}

suite("HMI Builder: My Symbols", function () {
  this.timeout(90000);

  suiteSetup(async () => languageServerReady());
  teardown(async () => vscode.commands.executeCommand("workbench.action.closeAllEditors"));

  test("a symbol definition opens visually and an edited rule changes only its TOML document", async () => {
    const root = project();
    const { document, webview, panel } = await openBuilder(path.join(root, "hmi", "symbols", "agitator.toml"));
    try {
      assert.match(webview.html, /data-view="symbol"/, "the definition uses the visual symbol editor");
      webview.send({ type: "ready" });
      const loaded = await until("the symbol", () =>
        (webview.posted as any[]).find((message) => message.type === "symbol"),
      );
      assert.strictEqual(loaded.model.name, "agitator");
      const model = { ...loaded.model, parts: [{ part: "rotor", active: "running" }] };
      webview.send({ type: "editSymbol", model } as any);
      await until("the rule written", () => document.getText().includes('active = "running"'));
      assert.ok(document.isDirty, "the visual edit is an unsaved document edit");
      assert.strictEqual(fs.readFileSync(path.join(root, "hmi", "symbols", "agitator.svg"), "utf8"), DRAWING);
      assert.strictEqual(fs.readFileSync(document.uri.fsPath, "utf8"), DEFINITION, "save remains the user's action");
    } finally {
      panel.dispose();
    }
  });

  test("the editor sends the project's own symbols with the catalogue, and again when one changes", async () => {
    const root = project();
    const { webview, panel } = await openBuilder(path.join(root, "hmi", "pages", "mixing.toml"));
    webview.send({ type: "ready" });
    const first = await until("the catalogue", () => postedOfType(webview, "catalogue")[0]);
    assert.ok(
      first.catalogue.components.some((component) => component.type === "my:agitator"),
      "the symbol is a component",
    );
    assert.strictEqual(first.symbols?.[0]?.name, "agitator");
    // VS Code starts a watcher of a folder outside the workspace in the background and gives no signal when it is ready, so a
    // file written at once can come before it. The symbol is written again every few seconds until the catalogue follows: each
    // write is a change the editor must answer with the catalogue.
    const writeSymbol = (): void => {
      fs.writeFileSync(path.join(root, "hmi", "symbols", "stirrer.svg"), DRAWING);
      fs.writeFileSync(path.join(root, "hmi", "symbols", "stirrer.toml"), DEFINITION.replace("Agitator", "Stirrer"));
    };
    writeSymbol();
    const rewrite = setInterval(writeSymbol, 3000);
    try {
      await until(
        "the catalogue with the new symbol",
        () =>
          postedOfType(webview, "catalogue").find((message) =>
            message.symbols?.some((symbol) => symbol.name === "stirrer"),
          ),
        30000,
      );
    } finally {
      clearInterval(rewrite);
    }
    panel.dispose();
  });

  test("a symbol definition's problems are errors at their lines in the Problems view", async () => {
    const root = project();
    const definition = path.join(root, "hmi", "symbols", "agitator.toml");
    fs.writeFileSync(definition, DEFINITION.replace('id = "rotor"', 'id = "stirrer"'));
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(definition));
    await vscode.window.showTextDocument(document);
    const deadline = Date.now() + 30000;
    for (;;) {
      const found = vscode.languages
        .getDiagnostics(document.uri)
        .filter((diagnostic) => diagnostic.source === "truST HMI");
      if (
        found.some((diagnostic) => /no element "stirrer"/.test(diagnostic.message) && diagnostic.range.start.line === 8)
      )
        break;
      if (Date.now() > deadline)
        assert.fail(
          `diagnostics: ${JSON.stringify(found.map((diagnostic) => [diagnostic.range.start.line, diagnostic.message]))}`,
        );
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
  });

  test("Import Symbol copies the drawing and writes a definition naming its elements, then opens it", async () => {
    const root = project();
    const source = path.join(root, "Mixer Unit.svg");
    fs.writeFileSync(source, DRAWING);
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.importSymbol", {
      file: source,
      name: "mixer-unit",
      project: root,
    });
    const copied = path.join(root, "hmi", "symbols", "mixer-unit.svg");
    assert.ok(fs.existsSync(copied), "the drawing is copied into hmi/symbols");
    assert.strictEqual(fs.readFileSync(copied, "utf8"), DRAWING, "the drawing as it was");
    const written = fs.readFileSync(path.join(root, "hmi", "symbols", "mixer-unit.toml"), "utf8");
    assert.match(
      written,
      /^format = 1\ntitle = "Mixer Unit"\nsize = \[128, 76\]\n/,
      "its title and a size in the drawing's proportions",
    );
    assert.match(written, /# id = "body"/);
    assert.match(written, /# id = "rotor"/);
    const shown = await until(
      "the definition shown",
      () =>
        vscode.window.activeTextEditor?.document.uri.fsPath === path.join(root, "hmi", "symbols", "mixer-unit.toml") &&
        vscode.window.activeTextEditor,
    );
    assert.ok(shown);
  });
});
