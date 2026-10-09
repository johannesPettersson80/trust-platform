import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";

// Specification 39, §8.9: the editor's host gives its webview the faceplates to place, the
// instances a faceplate fits, and the page's placements bound as the display binds them, again
// whenever an edit changes them.

const SOURCE =
  "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Tank\nVAR_OUTPUT\n    Level : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P1 : Pump;\n    T1 : Tank;\n    P2 : Pump;\nEND_VAR\n    P1();\n    T1();\n    P2();\nEND_PROGRAM\n";
const CARD =
  'format = 1\ntitle = "Pump card"\nsize = [320, 160]\n\n[fields.Running]\naccepts = ["boolean"]\n\n[[object]]\nid = "running"\ntype = "status"\nat = [16, 16]\nsize = [160, 48]\n\n[object.bind]\nstate = "$Running"\n';
const PAGE =
  'format = 1\ntitle = "Cards"\n\n[[object]]\nid = "card"\ntype = "faceplate"\nfaceplate = "pump-card"\ninstance = "Main.P2"\nat = [40, 40]\nsize = [320, 204]\n';
const SCREEN = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\n';

function project(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-placed-"));
  for (const [file, text] of [
    ["src/main.st", SOURCE],
    ["hmi/screen.toml", SCREEN],
    ["hmi/panels/pump-card.toml", CARD],
    ["hmi/pages/cards.toml", PAGE],
  ]) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  return root;
}

suite("HMI Builder: placed faceplates in the editor's host", function () {
  this.timeout(60000);

  test("the editor sends the faceplates, its placements bound, and the instances a faceplate fits", async () => {
    await languageServerReady();
    const page = path.join(project(), "hmi", "pages", "cards.toml");
    const { document, webview, panel } = await openBuilder(page);
    webview.send({ type: "ready" });
    const listed = await until("the faceplates", () => postedOfType(webview, "faceplates")[0]);
    assert.deepStrictEqual(listed.faceplates, [
      { name: "pump-card", title: "Pump card", size: [320, 160], requiresInstance: true },
    ]);
    const placed = await until("the placements", () =>
      postedOfType(webview, "placed").find((message) => message.placed.card),
    );
    assert.strictEqual(placed.placed.card?.model.object[0].bind.state.path, "Main.P2.Running");
    webview.send({ type: "faceplateInstances", faceplate: "pump-card" });
    const answer = await until("the instances", () => postedOfType(webview, "faceplateInstances")[0]);
    assert.deepStrictEqual(answer.instances, ["Main.P1", "Main.P2"]);
    // An edit that binds the placement to another instance sends it bound again.
    const at = document.getText().indexOf('"Main.P2"');
    const edit = new vscode.WorkspaceEdit();
    edit.replace(
      document.uri,
      new vscode.Range(document.positionAt(at), document.positionAt(at + '"Main.P2"'.length)),
      '"Main.P1"',
    );
    await vscode.workspace.applyEdit(edit);
    await until("bound again", () =>
      postedOfType(webview, "placed").some(
        (message) => message.placed.card?.model.object[0].bind.state.path === "Main.P1.Running",
      ),
    );
    panel.dispose();
  });
});
