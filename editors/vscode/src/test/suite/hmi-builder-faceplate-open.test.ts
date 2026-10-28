import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { openFaceplate } from "../../hmiBuilder/editor/faceplates";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §3.3 item 8: Open faceplate, on a symbol's context menu, opens that faceplate
// of the project or of one of its libraries in the HMI Builder.

function project(): vscode.Uri {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-faceplate-open-"));
  const write = (file: string, text: string) => {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  };
  const faceplate = (title: string) =>
    `format = 1\ntitle = "${title}"\nsize = [400, 300]\n\n[fields.Running]\naccepts = ["boolean"]\n`;
  write("trust-lsp.toml", '[dependencies]\nequipment = { path = "libraries/equipment" }\n');
  write("libraries/equipment/trust-lsp.toml", '[package]\nversion = "0.1.0"\n');
  write("libraries/equipment/src/equipment.st", "(* sources *)\n");
  write("libraries/equipment/hmi/panels/valve.toml", faceplate("Shipped valve"));
  write("hmi/panels/pump.toml", faceplate("Own pump"));
  return vscode.Uri.file(root);
}

function shown(): vscode.Uri | undefined {
  const input = vscode.window.tabGroups.activeTabGroup.activeTab?.input;
  return input instanceof vscode.TabInputCustom && input.viewType === "trust-lsp.hmiBuilder" ? input.uri : undefined;
}

suite("HMI Builder: open a symbol's faceplate", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);
  teardown(() => vscode.commands.executeCommand("workbench.action.closeAllEditors"));

  test("the project's faceplate and a library's open in the HMI Builder; a missing one opens nothing", async () => {
    const root = project();
    assert.strictEqual(await openFaceplate(root, "pump"), true);
    assert.strictEqual(shown()?.fsPath, path.join(root.fsPath, "hmi", "panels", "pump.toml"));
    assert.strictEqual(await openFaceplate(root, "valve"), true);
    assert.strictEqual(
      shown()?.fsPath,
      path.join(root.fsPath, "libraries", "equipment", "hmi", "panels", "valve.toml"),
    );
    assert.strictEqual(await openFaceplate(root, "mixer"), false);
  });
});
