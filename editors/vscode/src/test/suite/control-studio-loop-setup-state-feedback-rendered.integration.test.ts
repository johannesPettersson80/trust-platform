import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { attachControlStudio, attachWebview } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio state-feedback setup rendered", () => {
  test("sets up the program's own state-feedback law and opens it in Control Studio", async function () {
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
      const hidden = () => inner("return JSON.stringify([...d.querySelectorAll('[data-feedback-state]')].map(row=>row.hidden));");
      await set([["structure", "state-feedback"], ["name", "Tank 2 level"], ["feedback.instance", "P1"], ["feedback.manipulatedVariable", "P1.PumpM3PerS"],
        ["feedback.manipulatedVariable.unit", "m3/s"], ["feedback.manipulatedVariable.minimum", "0"], ["feedback.manipulatedVariable.maximum", "0.02"],
        ["feedback.processVariable", "P1.Tank2M"], ["feedback.processVariable.unit", "m"], ["feedback.processVariable.minimum", "0"],
        ["feedback.processVariable.maximum", "0.5"], ["feedback.setpoint", "P1.Tank2SetpointM"], ["feedback.gain", "P1.K2"], ["feedback.referenceGain", "P1.N"]]);
      assert.deepStrictEqual(JSON.parse(await hidden()), [false, true, true], "one further measured output is offered at a time");
      await set([["feedback.state.0", "P1.Tank1M"], ["feedback.state.0.unit", "m"], ["feedback.state.0.minimum", "0"], ["feedback.state.0.maximum", "0.5"],
        ["feedback.state.0.gain", "P1.K1"], ["feedback.mode", "P1.FeedbackEnabled"], ["feedback.manualOutput", "P1.ManualM3PerS"], ["feedback.experimentDuration", "300"]]);
      assert.deepStrictEqual(JSON.parse(await hidden()), [false, false, true]);
      const state = JSON.parse(await inner(`return JSON.stringify({save:d.querySelector('[data-setup-save]').disabled,message:d.querySelector('[data-setup-message]').textContent,
        step:d.querySelector('[data-setup-field="feedback.experimentStep"]').value,visible:[...d.querySelectorAll('[data-setup-section]')].filter(s=>!s.hidden).map(s=>s.dataset.setupSection)});`));
      assert.deepStrictEqual([state.save, state.step, state.visible], [false, "0.002", ["state-feedback"]], state.message);
      await shot("completed");
      await inner("d.querySelector('[data-setup-save]').click();return 'save';");
      const studio = await attachControlStudio(connection, port, "", "loop:tank-2-level");
      await pollValue("Control Studio ready", () => evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await shot("studio");
      const saved = JSON.parse(fs.readFileSync(path.join(project, "control-loops", "tank-2-level.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(saved, { projectId: saved.projectId, runtimeId: saved.runtimeId }).kind, "accepted");
      assert.deepStrictEqual(saved.controller.stateFeedback.gainSymbols, [["P1.K2", "P1.K1"]]);
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
