import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { attachControlStudio, attachWebview } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio ON/OFF setup rendered", () => {
  test("sets up the program's own switching logic and opens it in Control Studio", async function () {
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
      await inner(`const set=(name,value)=>{const e=d.querySelector('[data-setup-field="'+name+'"]');e.value=value;e.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));};
        for(const [name,value] of [['structure','onoff'],['name','Cold room'],['action','reverse'],['onoff.instance','P1'],['onoff.processVariable','P1.RoomC'],
          ['onoff.processVariable.unit','°C'],['onoff.processVariable.minimum','-10'],['onoff.processVariable.maximum','20'],['onoff.setpoint','P1.SetpointC'],
          ['onoff.manipulatedVariable','P1.CompressorPct'],['onoff.manipulatedVariable.unit','%'],['onoff.manipulatedVariable.minimum','0'],
          ['onoff.manipulatedVariable.maximum','100'],['onoff.mode','P1.AutoMode'],['onoff.manualOutput','P1.ManualPct'],['onoff.hysteresis','P1.HysteresisC'],
          ['onoff.minimumOn','P1.MinimumOnS'],['onoff.minimumOff','P1.MinimumOffS'],['onoff.latch','P1.CoolingOn'],['onoff.dwellTimer','P1.DwellS']]) set(name,value);return 'filled';`);
      const state = JSON.parse(await inner(`return JSON.stringify({save:d.querySelector('[data-setup-save]').disabled,message:d.querySelector('[data-setup-message]').textContent,
        startup:d.querySelector('[data-setup-startup]').hidden,visible:[...d.querySelectorAll('[data-setup-section]')].filter(s=>!s.hidden).map(s=>s.dataset.setupSection)});`));
      assert.deepStrictEqual([state.save, state.startup, state.visible], [false, true, ["onoff"]],
        `the program writes only its own switching state, so no startup flag is needed: ${state.message}`);
      await shot("completed");
      await inner("d.querySelector('[data-setup-save]').click();return 'save';");
      const studio = await attachControlStudio(connection, port, "", "loop:cold-room");
      await pollValue("Control Studio ready", () => evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await shot("studio");
      const saved = JSON.parse(fs.readFileSync(path.join(project, "control-loops", "cold-room.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(saved, { projectId: saved.projectId, runtimeId: saved.runtimeId }).kind, "accepted");
      assert.deepStrictEqual([saved.controller.onOff.hysteresisSymbol, saved.controller.onOff.lowOutput, saved.controller.onOff.highOutput], ["P1.HysteresisC", 0, 100]);
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
