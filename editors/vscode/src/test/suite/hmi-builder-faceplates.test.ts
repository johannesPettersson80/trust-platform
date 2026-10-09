import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";
import { languageServerCatalogue, languageServerPageFiles } from "../../hmiBuilder/editor/languageServerPageFiles";
import { objectWarnings } from "../../hmiBuilder/editor/quality";

// Specification 39, §8.7: faceplate files open in the HMI Builder with their description and
// fields, edits keep them, and a click can open the faceplates of the project and its libraries.

const FACEPLATE = `format = 1
title = "Pump"
description = "Transfer pump"
size = [400, 300]

[fields.Running]
accepts = ["boolean"]
`;

function project(): { root: string; faceplate: string; page: string } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-faceplates-"));
  const write = (file: string, text: string): string => {
    const full = path.join(root, file);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, text);
    return full;
  };
  write("trust-lsp.toml", '[dependencies]\nequipment = { path = "lib/equipment" }\n');
  write("lib/equipment/trust-lsp.toml", '[package]\nversion = "0.1.0"\n');
  write("lib/equipment/src/equipment.st", "(* sources *)\n");
  write("lib/equipment/hmi/panels/valve.toml", FACEPLATE.replace('title = "Pump"', 'title = "Valve"'));
  const page = write("hmi/pages/overview.toml", 'format = 1\ntitle = "Overview"\n');
  const faceplate = write("hmi/panels/pump.toml", FACEPLATE);
  return { root, faceplate, page };
}

suite("HMI Builder editor: faceplates", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  teardown(async () => {
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a faceplate file opens with its description and fields, and an edit is written as a faceplate", async () => {
    const { faceplate } = project();
    const { document, webview } = await openBuilder(faceplate);
    webview.send({ type: "ready" });
    await until("the file read", () => postedOfType(webview, "page")[0] ?? postedOfType(webview, "invalid")[0]);
    assert.deepStrictEqual(postedOfType(webview, "invalid"), [], "the faceplate loads");
    const page = postedOfType(webview, "page")[0];
    assert.deepStrictEqual(
      [page.model.title, page.model.description, page.model.fields],
      ["Pump", "Transfer pump", { Running: { accepts: ["boolean"] } }],
    );
    webview.send({ type: "edit", model: { ...page.model, size: [480, 300] } });
    await until("the text written", () => document.getText().includes("size = [480, 300]"));
    assert.ok(
      document.getText().includes('description = "Transfer pump"') && document.getText().includes("[fields.Running]"),
      document.getText(),
    );
  });

  test("the editor lists the faceplates of the project and its libraries as click targets", async () => {
    const { page } = project();
    const { webview } = await openBuilder(page);
    webview.send({ type: "ready" });
    await until("the variables", () => postedOfType(webview, "variables")[0]);
    const targets = await until("the faceplates among the targets", () =>
      postedOfType(webview, "targets").find((message) => (message.faceplates ?? []).length > 0),
    );
    assert.deepStrictEqual(targets.faceplates, ["pump", "valve"]);
  });

  test("every shipped faceplate draws its words whole", async () => {
    await languageServerReady();
    const catalogue = await languageServerCatalogue();
    assert.ok(catalogue, "the catalogue");
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const folder = path.join(root, "libraries", "plant", "hmi", "panels");
    const cut: string[] = [];
    for (const file of fs.readdirSync(folder).filter((name) => name.endsWith(".toml"))) {
      const read = await languageServerPageFiles.read(
        fs.readFileSync(path.join(folder, file), "utf8"),
        `hmi/panels/${file}`,
      );
      assert.ok(read.ok, `${file} reads`);
      if (!read.ok) continue;
      for (const warning of objectWarnings(read.model, catalogue, [])) {
        if (warning.kind === "cut-text") cut.push(`${file}: ${warning.message}`);
      }
    }
    assert.deepStrictEqual(cut, []);
  });

  test("the HMI Builder is the editor of faceplate files", () => {
    const manifest = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../package.json"), "utf8")) as {
      contributes: { customEditors: Array<{ viewType: string; selector: Array<{ filenamePattern: string }> }> };
    };
    const builder = manifest.contributes.customEditors.find((editor) => editor.viewType === "trust-lsp.hmiBuilder");
    assert.ok(builder?.selector.some((selector) => selector.filenamePattern === "**/hmi/panels/*.toml"));
  });
});
