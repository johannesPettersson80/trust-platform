import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio Identify initial readings", () => {
  test("shows actual selected plant signals before identification and marks stopped readings unavailable", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const capture = (name: string) => captureRenderedWorkbenchCdp(connection, path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), name + ".png"));
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("Identify ready", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__readings=[];d.defaultView.addEventListener('message',e=>{if(e.data.kind==='loop.monitor.snapshot')d.defaultView.__readings.push(e.data.payload);});return 'observing';");
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.ok(started.ok, JSON.stringify(started));
      const samples = JSON.parse(await pollValue("two actual native readings", () => inner("return JSON.stringify(d.defaultView.__readings);"), value => JSON.parse(value).filter((p: any) => p.availability === "available").length >= 2, 20000));
      const reading = samples[samples.length - 1];
      assert.strictEqual(reading.processVariable.value, 20);
      assert.strictEqual(reading.manipulatedVariable.value, 0);
      await capture("initial-readings");
      assert.strictEqual(await inner("return String(d.querySelector('.studio-response').checkVisibility());"), "true", "Identify must show the response plot before an identification result exists");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "2", "Plot the selected plant input and measured output");
      const description = await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');");
      assert.match(description, /P1\.ProcessValue.*20.*°C/);
      assert.match(description, /P1\.ManipulatedValue.*0.*%/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').hidden);"), "true", "No invented model before identification");
      for (const [theme, name] of [["Default Dark Modern", "dark"], ["Default Light Modern", "light"], ["Default High Contrast", "contrast"]]) {
        await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
        await new Promise(resolve => setTimeout(resolve, 400));
        await capture("initial-" + name);
      }
      await runtimeLifecycleService.stopRuntime();
      await pollValue("stopped current values", () => inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), value => /unavailable/i.test(value), 10000);
      await capture("initial-stopped");
    } finally {
      await runtimeLifecycleService.stopRuntime();
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
