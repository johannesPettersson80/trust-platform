import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { pidLoop } from "./control-studio-pid-fixtures";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

type Method = "automatic" | "mv_step" | "mv_pulse" | "setpoint_step" | "relay" | "prbs" | "multisine";

function applicability(loop: any, method: Method): { available: boolean; reason?: string } {
  let module: any = {}; try { module = require("../../controlStudio/identificationMethodApplicability"); } catch { /* Expected pre-implementation API absence. */ }
  assert.strictEqual(typeof module.identificationMethodApplicability, "function",
    "the method selector must expose one pure applicability decision before Identify can start");
  return module.identificationMethodApplicability(loop, method);
}

suite("Control Studio identification method applicability", () => {
  test("reports the missing declared process direction for Relay", () => {
    const loop = pidLoop();
    delete loop.experiment.relayDirection;
    const result = applicability(loop, "relay");
    assert.strictEqual(result.available, false);
    assert.match(result.reason ?? "", /process direction|relay direction/i);
  });

  test("rejects frequency excitation for an ON/OFF actuator", () => {
    const loop = pidLoop();
    loop.controller.onOff = { lowOutput: -20, highOutput: 20 };
    assert.strictEqual(applicability(loop, "prbs").available, false);
    assert.strictEqual(applicability(loop, "multisine").available, false);
  });

  test("rejects explicit scalar step and pulse methods for a multichannel target", () => {
    const loop = pidLoop();
    loop.signals.additional = [
      { id: "mv2", channel: "secondary", role: "MV", symbol: "Main.MV2", address: "%QL8", iecType: "LREAL", direction: "write", unit: "V", range: { minimum: -20, maximum: 20 } },
      { id: "pv2", channel: "secondary", role: "PV", symbol: "Main.PV2", address: "%IL8", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
    ];
    loop.channels = {
      inputs: [{ modelChannelId: "u1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: -20, maximum: 20 } }, { modelChannelId: "u2", signalId: "mv2", actuator: { kind: "continuous", minimum: -20, maximum: 20 } }],
      outputs: [{ modelChannelId: "y1", signalId: "processVariable" }, { modelChannelId: "y2", signalId: "pv2" }],
    };
    assert.strictEqual(applicability(loop, "mv_step").available, false);
    assert.strictEqual(applicability(loop, "mv_pulse").available, false);
  });

  test("requires the configured reference for setpoint excitation and allows Automatic to choose", () => {
    const loop = pidLoop();
    assert.strictEqual(applicability(loop, "setpoint_step").available, true);
    delete loop.signals.setpoint;
    assert.strictEqual(applicability(loop, "setpoint_step").available, false);
    assert.strictEqual(applicability(loop, "automatic").available, true);
  });

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
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
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
