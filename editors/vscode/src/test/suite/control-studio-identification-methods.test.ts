import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio identification method applicability", () => {

  test("renders a specific Relay prerequisite failure without starting runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    delete loop.experiment.relayDirection;
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("identification method handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      await inner("var m=d.querySelector('[data-engineer-acquisition-method]');m.value='relay';m.dispatchEvent(new Event('change',{bubbles:true}));return 'relay';");
      // The engine words the prerequisite where the method is chosen (specification 35, B3.2 and §0.1a `inputs`): the Method list gives the
      // method the sentence that says why the loop cannot run it, and the engine refuses the choice with that sentence. The page shows it
      // without a press of Identify plant, which would run the method the study holds, not the one refused.
      await pollValue("relay prerequisite", () => inner("return d.querySelector('[data-studio-message=model]').textContent;"), value => /direction|relay/i.test(value), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-three-page]').dataset.busy??'false');"), "false");
      await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-method-prerequisite.png"));
      await inner("var m=d.querySelector('[data-engineer-acquisition-method]');m.value='automatic';m.dispatchEvent(new Event('change',{bubbles:true}));return 'automatic';");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-acquisition-method]').value;"), "automatic");
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
