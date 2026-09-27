import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { acquisitionSettings, configuredAcquisition } from "../../controlStudio/acquisitionSettings";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { pidLoop } from "./control-studio-pid-fixtures";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

function openLoop(base = pidLoop()): any {
  const loop = structuredClone(base);
  loop.id += "-reference-selection"; loop.parameterBinding.loop_id = loop.id;
  loop.topology = "open-loop"; loop.controller.design = "project";
  delete loop.controller.law; delete loop.controller.pidInitialization; delete loop.signals.setpoint;
  loop.signals.additional = [{ id: "additional-reference", channel: "primary", role: "SP", symbol: "Main.Reference", address: "%ML64", iecType: "LREAL", direction: "read", unit: loop.signals.processVariable.unit, range: { ...loop.signals.processVariable.range } }];
  return loop;
}

suite("Control Studio conditional setpoint identification", () => {
  test("uses only an explicitly selected declared reference without changing the controller target", () => {
    const loop = openLoop();
    const settings = { method: "setpoint_step" as const, durationSeconds: 10, amplitude: 1, reuseRecordings: true, setpointSignalId: "additional-reference" };
    const decoded = acquisitionSettings(settings);
    assert.ok(decoded);
    const configured = configuredAcquisition(loop, decoded);
    assert.strictEqual(configured.signals.setpoint?.symbol, "Main.Reference");
    assert.strictEqual(loop.signals.setpoint, undefined);
    assert.deepStrictEqual(configured.controller, loop.controller);
    assert.throws(() => configuredAcquisition(loop, acquisitionSettings({ ...settings, setpointSignalId: "unknown" })), /reference|setpoint|signal/i);
    assert.throws(() => configuredAcquisition(loop, acquisitionSettings({ ...settings, setpointSignalId: "manipulatedVariable" })), /reference|setpoint|signal/i);
    const wrongUnit = openLoop(); wrongUnit.signals.additional[0].unit = "wrong-unit";
    assert.throws(() => configuredAcquisition(wrongUnit, decoded), /unit|reference|setpoint/i);
  });

  test("shows the reference selector only when Setpoint step needs a configured signal", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = openLoop(JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8")));
    const validity = validateControlLoopDefinition(loop, loop);
    assert.strictEqual(validity.kind, "accepted", JSON.stringify(validity));
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("reference selection ready", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-identify-setpoint]')?.checkVisibility());"), "false");
      await inner("const m=d.querySelector('[data-engineer-acquisition-method]');m.value='setpoint_step';m.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'method';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-identify-setpoint]')?.checkVisibility());"), "true");
      assert.match(await inner("return d.querySelector('[data-identify-setpoint]').textContent;"), /Main.Reference/);
      await inner("const s=d.querySelector('[data-identify-setpoint]');s.value='additional-reference';s.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'reference';");
      await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-reference.png"));
      await inner("const m=d.querySelector('[data-engineer-acquisition-method]');m.value='automatic';m.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'automatic';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-identify-setpoint]')?.checkVisibility());"), "false");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Selecting a reference starts no runtime operation");
    } finally { connection.close(); await vscode.commands.executeCommand("workbench.action.closeAllEditors"); }
  });
});
