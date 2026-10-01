import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { attachControlStudio, attachWebview } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio loop setup form rendered", () => {
  test("sets up the project's Control.PID from compiled facts and opens it in Control Studio", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(90000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const shot = (name: string) => captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, `-${name}.png`));
    try {
      void vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
      const form = await attachWebview(connection, port, "[data-loop-setup][data-ready=\"true\"]");
      const inner = (body: string) => evaluateInWebview(connection, form, body);
      const field = (name: string) => inner(`return d.querySelector('[data-setup-field="${name}"]').value;`);
      assert.deepStrictEqual(await Promise.all(["structure", "instance", "parameterRecord", "processVariable", "manipulatedVariable", "setpoint", "manualOutput"].map(field)),
        ["pid", "P1.Controller", "P1.Parameters", "P1.ProcessValue", "P1.ManipulatedValue", "P1.Setpoint", "P1.ManualOutput"],
        "the compiled calls prefill the controller, its parameter record and its signals");
      const rows = JSON.parse(await inner(`return JSON.stringify(['unit','minimum','maximum'].map(part=>Math.round(d.querySelector("[data-setup-field='processVariable."+part+"']").getBoundingClientRect().top)));`));
      assert.ok(rows.every((top: number) => top === rows[0]), `a signal's unit, minimum and maximum share one row: ${rows}`);
      await shot("prefilled");
      await inner(`const set=(name,value)=>{const e=d.querySelector('[data-setup-field="'+name+'"]');e.value=value;e.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));};
        set('name','Fixture temperature');set('processVariable.unit','°C');set('processVariable.minimum','0');set('processVariable.maximum','100');
        set('manipulatedVariable.unit','%');set('manipulatedVariable.minimum','0');set('manipulatedVariable.maximum','100');set('experimentDuration','30');set('mode','P1.AutoMode');return 'filled';`);
      assert.match(await inner("return d.querySelector('[data-setup-message]').textContent;"), /initializes P1\.Controller.*startup/,
        "the program's startup code initializes the PID, so validation needs the flag that marks startup as done");
      await inner(`const e=d.querySelector('[data-setup-field="startupFlag"]');e.value='P1.Initialized';e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'flag';`);
      assert.strictEqual(await inner("return String(d.querySelector('[data-setup-save]').disabled);"), "false", await inner("return d.querySelector('[data-setup-message]').textContent;"));
      await shot("completed");
      await inner("d.querySelector('[data-setup-save]').click();return 'save';");
      const studio = await attachControlStudio(connection, port, "", "loop:fixture-temperature");
      await pollValue("Control Studio ready", () => evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      assert.strictEqual(await evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]').dataset.loopId;"), "loop:fixture-temperature");
      await shot("studio");
      const saved = JSON.parse(fs.readFileSync(path.join(project, "control-loops", "fixture-temperature.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(saved, { projectId: saved.projectId, runtimeId: saved.runtimeId }).kind, "accepted");
      assert.strictEqual(saved.controller.law.parameterRecordSymbol, "P1.Parameters");
      assert.strictEqual(saved.experiment.durationSeconds, 30, "identification runs for the duration entered in the form");
      assert.deepStrictEqual(saved.controller.pidInitialization.applicationStates.map((state: { symbol: string }) => state.symbol), ["P1.Initialized"],
        "validation starts with the program's startup done, as the reviewed hand-written loop of this project declares");
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
