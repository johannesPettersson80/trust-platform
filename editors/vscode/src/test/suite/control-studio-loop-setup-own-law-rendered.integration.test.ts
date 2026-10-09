import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { attachControlStudio, attachWebview } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio own P or PI law setup rendered", () => {
  test("sets up the program's own PI and opens it in Control Studio", async function () {
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
      const set = (fields: [string, string][]) => inner(`const set=(name,value)=>{const e=d.querySelector('[data-setup-field="'+name+'"]');e.value=value;e.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));};
        for(const [name,value] of ${JSON.stringify(fields)}) set(name,value);return 'filled';`);
      const state = async () => JSON.parse(await inner(`return JSON.stringify({save:d.querySelector('[data-setup-save]').disabled,message:d.querySelector('[data-setup-message]').textContent,
        integral:!d.querySelector('[data-own-integral]').hidden,
        startup:!d.querySelector('[data-setup-startup]').hidden,visible:[...d.querySelectorAll('[data-setup-section]')].filter(s=>!s.hidden).map(s=>s.dataset.setupSection)});`));
      await set([["structure", "own-law"], ["name", "Temperature"], ["own.instance", "P1"], ["own.law", "P"]]);
      assert.strictEqual((await state()).integral, false, "a P law shows no integral fields");
      await set([["own.law", "PI"], ["own.processVariable", "P1.TemperatureC"], ["own.processVariable.unit", "degC"], ["own.processVariable.minimum", "0"],
        ["own.processVariable.maximum", "200"], ["own.setpoint", "P1.SetpointC"], ["own.manipulatedVariable", "P1.HeaterPct"], ["own.manipulatedVariable.unit", "%"],
        ["own.manipulatedVariable.minimum", "0"], ["own.manipulatedVariable.maximum", "100"], ["own.mode", "P1.AutoMode"], ["own.manualOutput", "P1.ManualPct"],
        ["own.proportionalGain", "P1.Kp"], ["own.proportionalGain.minimum", "0"], ["own.proportionalGain.maximum", "20"],
        ["own.integralGain", "P1.Ki"], ["own.integralGain.minimum", "0"], ["own.integralGain.maximum", "10"], ["own.integralTerm", "P1.Integral"]]);
      const completed = await state();
      assert.deepStrictEqual([completed.save, completed.integral, completed.startup, completed.visible], [false, true, false, ["own-law"]],
        completed.message);
      await shot("completed");
      await inner("d.querySelector('[data-setup-save]').click();return 'save';");
      const studio = await attachControlStudio(connection, port, "", "loop:temperature");
      await pollValue("Control Studio ready", () => evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await shot("studio");
      const saved = JSON.parse(fs.readFileSync(path.join(project, "control-loops", "temperature.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(saved, { projectId: saved.projectId, runtimeId: saved.runtimeId }).kind, "accepted");
      assert.deepStrictEqual(saved.controller.project.seed, { kind: "parallel-pid", family: "PI", proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki" });
      assert.deepStrictEqual(saved.controller.project.initialization.states.map((state: { symbol: string }) => state.symbol), ["P1.Integral"]);
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
