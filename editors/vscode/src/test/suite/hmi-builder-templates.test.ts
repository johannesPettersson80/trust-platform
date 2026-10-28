import { disposeFixtureProject } from "./fixtureDocuments";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §4.10: New HMI Screen creates a screen layout and a first page from a
// template.

async function parsed(kind: string, file: string): Promise<{ ok?: boolean; model?: any }> {
  assert.ok(fs.existsSync(file), `${file} is created`);
  const text = await edited(file);
  return (await vscode.commands.executeCommand("trust-lsp.hmiFileParse", { kind, text, file })) as {
    ok?: boolean;
    model?: any;
  };
}

async function edited(file: string): Promise<string> {
  return (await vscode.workspace.openTextDocument(vscode.Uri.file(file))).getText();
}

suite("HMI Builder editor: templates", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);
  const fixtures: string[] = [];
  function fixture(): string {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-template-"));
    fixtures.push(root);
    return root;
  }

  teardown(async () => {
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    for (const root of fixtures.splice(0)) await disposeFixtureProject(root);
  });

  test("the operator panel template creates its screen layout and a first page", async () => {
    // The screen's title is the project folder's name in words (§4 item 8).
    const root = path.join(fixture(), "room-heating");
    fs.mkdirSync(root);
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.newScreen", {
      template: "operator-panel",
      folder: vscode.Uri.file(root).toString(),
    });
    const screen = await parsed("screen", path.join(root, "hmi", "screen.toml"));
    assert.strictEqual(screen.ok, true, JSON.stringify(screen));
    for (const file of ["screen.toml", "pages/overview.toml"]) {
      assert.match(
        fs.readFileSync(path.join(root, "hmi", file), "utf8"),
        /^format = 1$/m,
        `${file} is on disk, not in an unsaved buffer`,
      );
    }
    assert.deepStrictEqual(screen.model.size, [1688, 1024]);
    assert.deepStrictEqual(screen.model.navigation, { style: "side", position: "left" });
    assert.deepStrictEqual(
      [
        screen.model.top_bar?.title,
        screen.model.top_bar?.clock,
        screen.model.top_bar?.alarms,
        screen.model.top_bar?.user,
      ],
      ["Room heating", true, true, true],
    );
    const page = await parsed("page", path.join(root, "hmi", "pages", "overview.toml"));
    assert.strictEqual(page.ok, true, JSON.stringify(page));
    assert.deepStrictEqual([page.model.title, page.model.icon, page.model.order], ["Overview", "overview", 0]);
  });

  test("the overview and single page templates choose their sizes and navigation", async () => {
    const overview = fixture();
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.newScreen", {
      template: "overview",
      folder: vscode.Uri.file(overview).toString(),
    });
    const tabs = await parsed("screen", path.join(overview, "hmi", "screen.toml"));
    assert.deepStrictEqual(
      [tabs.model.size, tabs.model.navigation.style, tabs.model.top_bar?.alarms],
      [[1280, 696], "tabs", false],
    );
    const single = fixture();
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.newScreen", {
      template: "single-page",
      folder: vscode.Uri.file(single).toString(),
    });
    const plain = await parsed("screen", path.join(single, "hmi", "screen.toml"));
    assert.deepStrictEqual(
      [plain.model.size, plain.model.navigation.style, plain.model.top_bar],
      [[1024, 768], "none", undefined],
    );
  });

  test("a new screen installs definitions without generating PLC programs or changing task execution", async () => {
    const root = fixture();
    fs.mkdirSync(path.join(root, "src"), { recursive: true });
    fs.writeFileSync(path.join(root, "trust-lsp.toml"), '[project]\ninclude_paths = ["src"]\n');
    fs.writeFileSync(
      path.join(root, "src", "main.st"),
      "PROGRAM Control\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n\nCONFIGURATION Plant\nRESOURCE Cpu ON PLC\n    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM Main WITH Fast : Control;\nEND_RESOURCE\nEND_CONFIGURATION\n",
    );
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.newScreen", {
      template: "operator-panel",
      folder: vscode.Uri.file(root).toString(),
    });
    const shipped = path.resolve(__dirname, "../../../../..", "libraries", "hmi");
    for (const file of ["src/hmi.st", "trust-lsp.toml"]) {
      assert.strictEqual(
        await edited(path.join(root, "libraries", "hmi", file)),
        fs.readFileSync(path.join(shipped, file), "utf8"),
        `the HMI library's ${file}, unchanged`,
      );
    }
    assert.match(
      await edited(path.join(root, "trust-lsp.toml")),
      /^Hmi = \{ path = "libraries\/hmi", version = "0\.1\.0" \}$/m,
    );
    assert.strictEqual(fs.existsSync(path.join(root, "src", "screens.st")), false);
    assert.strictEqual(
      vscode.workspace.textDocuments.some((document) => document.uri.fsPath === path.join(root, "src", "screens.st")),
      false,
    );
    assert.strictEqual(
      await edited(path.join(root, "src", "main.st")),
      fs.readFileSync(path.join(root, "src", "main.st"), "utf8"),
      "page creation never edits PLC execution",
    );
  });

  test("a project whose program already calls Hmi.Display keeps its program as it is", async () => {
    const root = fixture();
    fs.mkdirSync(path.join(root, "src"), { recursive: true });
    const main = "PROGRAM Control\nVAR\n    Screens : Hmi.Display;\nEND_VAR\n    Screens();\nEND_PROGRAM\n";
    fs.writeFileSync(path.join(root, "src", "main.st"), main);
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.newScreen", {
      template: "overview",
      folder: vscode.Uri.file(root).toString(),
    });
    assert.ok(fs.existsSync(path.join(root, "hmi", "screen.toml")), "the screen layout is created");
    assert.strictEqual(await edited(path.join(root, "src", "main.st")), main);
    assert.strictEqual(fs.existsSync(path.join(root, "src", "screens.st")), false);
  });

  test("an existing screen layout is never overwritten", async () => {
    const root = fixture();
    fs.mkdirSync(path.join(root, "hmi"), { recursive: true });
    fs.writeFileSync(path.join(root, "hmi", "screen.toml"), "format = 1\n# mine\n");
    await vscode.commands.executeCommand("trust-lsp.hmiBuilder.newScreen", {
      template: "overview",
      folder: vscode.Uri.file(root).toString(),
    });
    assert.strictEqual(fs.readFileSync(path.join(root, "hmi", "screen.toml"), "utf8"), "format = 1\n# mine\n");
    assert.strictEqual(fs.existsSync(path.join(root, "hmi", "pages", "overview.toml")), false);
  });
});
