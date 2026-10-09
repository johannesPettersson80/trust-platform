import { disposeFixtureProject } from "./fixtureDocuments";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §7.8: renaming a bound variable in the program also renames it in the HMI
// files, and Find All References lists the bindings.

const SOURCE =
  "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P101 : Pump;\nEND_VAR\n    P101();\nEND_PROGRAM\n";
const PAGE =
  'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "pump"\ntype = "status"\nat = [0, 0]\nsize = [240, 48]\n\n[object.bind]\nstate = "Main.P101.Running"\n';

/** A project in a folder of its own, which need not be a workspace folder. */
const fixtureProjects: string[] = [];

function project(): { source: vscode.Uri; page: vscode.Uri } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-rename-"));
  fixtureProjects.push(root);
  fs.mkdirSync(path.join(root, "src"), { recursive: true });
  fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
  fs.writeFileSync(path.join(root, "src", "main.st"), SOURCE);
  fs.writeFileSync(path.join(root, "hmi", "pages", "overview.toml"), PAGE);
  return {
    source: vscode.Uri.file(path.join(root, "src", "main.st")),
    page: vscode.Uri.file(path.join(root, "hmi", "pages", "overview.toml")),
  };
}

const RUNNING = new vscode.Position(2, 6);

suite("HMI Builder: rename and references reach the bindings", function () {
  this.timeout(60000);

  teardown(async () => {
    for (const root of fixtureProjects.splice(0)) await disposeFixtureProject(root);
  });

  test("renaming a bound member edits its binding in the page file", async () => {
    await languageServerReady();
    const { source, page } = project();
    await vscode.workspace.openTextDocument(source);
    const edit = await vscode.commands.executeCommand<vscode.WorkspaceEdit>(
      "vscode.executeDocumentRenameProvider",
      source,
      RUNNING,
      "IsRunning",
    );
    const binding = edit.get(page);
    assert.strictEqual(
      binding.length,
      1,
      JSON.stringify(edit.entries().map(([uri, edits]) => [uri.fsPath, edits.length])),
    );
    assert.strictEqual(binding[0].newText, "IsRunning");
    assert.strictEqual(binding[0].range.start.line, 10);
  });

  test("Find All References lists the binding", async () => {
    await languageServerReady();
    const { source, page } = project();
    await vscode.workspace.openTextDocument(source);
    const found = await vscode.commands.executeCommand<vscode.Location[]>(
      "vscode.executeReferenceProvider",
      source,
      RUNNING,
    );
    assert.ok(
      found.some((location) => location.uri.fsPath === page.fsPath && location.range.start.line === 10),
      JSON.stringify(found.map((location) => location.uri.fsPath)),
    );
  });
});
