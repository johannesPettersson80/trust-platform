import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { connectedModelChannels, validateLoopChannelBindings, type LoopChannels } from "../../controlStudio/loopChannels";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { pidLoop } from "./control-studio-pid-fixtures";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

function mimoLoop(): any {
  const loop = pidLoop(); loop.schemaVersion = 2;
  loop.signals = { ...loop.signals, additional: [
    { id: "mv-secondary", channel: "secondary", role: "MV", symbol: "Main.MV2", address: "%QL8", iecType: "LREAL", direction: "write", unit: "V", range: { minimum: -20, maximum: 20 } },
    { id: "pv-secondary", channel: "secondary", role: "PV", symbol: "Main.PV2", address: "%IL8", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
    { id: "sp-secondary", channel: "secondary", role: "SP", symbol: "Main.SP2", address: "%ML8", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
  ] };
  loop.channels = {
    inputs: [
      { modelChannelId: "u1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: -20, maximum: 20 } },
      { modelChannelId: "u2", signalId: "mv-secondary", actuator: { kind: "continuous", minimum: -20, maximum: 20 } },
    ],
    outputs: [
      { modelChannelId: "y1", signalId: "processVariable", setpointSignalId: "setpoint" },
      { modelChannelId: "y2", signalId: "pv-secondary", setpointSignalId: "sp-secondary" },
    ],
  } satisfies LoopChannels;
  loop.id = "control-studio-mimo"; loop.revision = `${loop.revision}-mimo`; loop.parameterBinding.loop_id = loop.id;
  return loop;
}

function model() {
  return { kind: "discrete-state-space", revision: "mimo-model", source: "fitted", validation: "validated", evidence: ["fit"],
    inputs: [{ id: "u1", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }, { id: "u2", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
    outputs: [{ id: "y1", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }, { id: "y2", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }],
    states: [{ id: "x1", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }],
    a: [[0.5]], b: [[0.2, 0.1]], c: [[1], [1]], d: [[0, 0], [0, 0]], delaySeconds: 0, sampleTimeSeconds: 0.1,
    operatingPoint: { inputs: [0, 0], outputs: [0, 0] } };
}

suite("Control Studio Tune channel mapping", () => {
  test("retains an identified MIMO model while requiring a compatible controller on Tune", () => {
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const selected = model(), loop = mimoLoop();
    const engine = new EngineerWorkflowSession(loop, () => selected, () => {});
    try {
      const result = engine.modelChanged();
      assert.strictEqual(result.status, "model-ready");
      assert.strictEqual(result.model, selected);
      assert.strictEqual(result.compatibleControllerRequired, true);
      assert.strictEqual(result.stageEvidence.hasCandidate, false);
    } finally { engine.dispose(); }
  });
  test("retains an explicit mapping in the same study and rejects modified actuator authority", async () => {
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const loop = mimoLoop(), source = structuredClone(loop);
    const engine = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "channel-edit", loopId: loop.id, loopRevision: loop.revision };
    const draft = structuredClone(loop.channels);
    draft.inputs[0].signalId = "mv-secondary"; draft.inputs[1].signalId = "manipulatedVariable";
    const posts: any[] = [];
    const edit = (mapping: unknown) => engine.handle({ schemaVersion: 1, kind: "control-studio.engineer.invalidate",
      messageId: String(posts.length), ...scope, expectedLoopRevision: scope.loopRevision, payload: { channelMapping: mapping } }, scope,
      async (payload: unknown) => { posts.push(payload); });
    try {
      await edit(draft);
      assert.deepStrictEqual(engine.snapshotStudy()?.channelMapping, draft);
      assert.deepStrictEqual(loop, source, "Mapping changes do not mutate the declared safety limits");
      const invalid = structuredClone(draft); invalid.inputs[0].actuator.maximum = 1000;
      await edit(invalid);
      assert.strictEqual(posts[posts.length - 1].status, "failed");
      assert.deepStrictEqual(engine.snapshotStudy().channelMapping, draft, "Invalid mapping leaves the accepted draft intact");
    } finally { engine.dispose(); }
  });
  test("validates a complete two-channel MV/PV/SP mapping and preserves model order", () => {
    const loop = mimoLoop();
    validateLoopChannelBindings(loop);
    const connected = connectedModelChannels(loop, model() as any);
    assert.deepStrictEqual(connected.inputs.map(channel => channel.signal.symbol), ["Main.MV", "Main.MV2"]);
    assert.deepStrictEqual(connected.outputs.map(channel => channel.signal.symbol), ["Main.PV", "Main.PV2"]);
    assert.deepStrictEqual(connected.outputs.map(channel => channel.reference?.symbol), ["Main.SP", "Main.SP2"]);
  });

  test("rejects incomplete, duplicate, wrong-role and incompatible-unit mapping drafts", () => {
    const loop = mimoLoop();
    assert.throws(() => connectedModelChannels({ ...loop, channels: { ...loop.channels, inputs: loop.channels.inputs.slice(0, 1) } }, model() as any), /Invalid|exactly one|channel/i);
    assert.throws(() => validateLoopChannelBindings({ ...loop, channels: { ...loop.channels, inputs: loop.channels.inputs.map((input: any) => ({ ...input, signalId: "manipulatedVariable" })) } }), /assigned to multiple|duplicate|Invalid/i);
    assert.throws(() => validateLoopChannelBindings({ ...loop, channels: { ...loop.channels, outputs: loop.channels.outputs.map((output: any) => ({ ...output, signalId: "manipulatedVariable" })) } }), /role|PV|Invalid/i);
    assert.throws(() => validateLoopChannelBindings({ ...loop, channels: { ...loop.channels, outputs: loop.channels.outputs.map((output: any) => ({ ...output, setpointSignalId: "mv-secondary" })) } }), /unit|setpoint|Invalid/i);
  });

  test("renders Channels… only for MIMO Tune targets and exposes exact declared signals", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const siso = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: siso.projectId, runtimeId: siso.runtimeId }, loopDefinition: siso });
      const sisoSession = await attachControlStudio(connection, port, "", siso.id);
      const sisoText = await evaluateInWebview(connection, sisoSession, "return d.querySelector('[data-engineer-workflow]').textContent;");
      assert.ok(!sisoText.includes("Channels…"), "SISO Tune must not expose a multivariable Channels editor");
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");

      const loop = mimoLoop();
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, siso.id, loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("MIMO Tune handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.ready;"), value => value === "true", 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-channels]')));"), "true");
      await inner("d.querySelector('[data-engineer-channels]').click();return 'channels';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-studio-channel-mapping]')));"), "true");
      const labels = await inner("return JSON.stringify(Array.from(d.querySelectorAll('[data-studio-channel-mapping] select option')).map(o=>o.textContent.trim()));");
      assert.match(labels, /Main\.MV|Main\.MV2|Main\.PV|Main\.PV2|Main\.SP|Main\.SP2/);
      await captureRenderedWorkbenchCdp(connection,
        requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-channels.png"));
      assert.strictEqual(await inner(`const field=d.querySelector('[data-studio-channel-field]');
        const url=d.defaultView.getComputedStyle(field).backgroundImage.match(/^url\\(["']?(.*?)["']?\\)$/)?.[1];
        if(!url)return 'false';return await new Promise(resolve=>{const image=new d.defaultView.Image();image.onload=()=>resolve('true');image.onerror=()=>resolve('false');image.src=url;});`), "true",
      "Channel selectors must render the shared dropdown arrow");
      await inner(`const fields=d.querySelectorAll('[data-studio-channel-field]');fields[0].value=fields[1].value;
        d.querySelector('[data-studio-editor-done]').click();return 'duplicate';`);
      assert.strictEqual(await inner(`const field=d.querySelector('[data-studio-channel-field]');return String(field.getAttribute('aria-invalid')==='true'&&field.closest('fieldset').querySelector('[role=alert]').checkVisibility());`), "true",
      "A duplicate command mapping identifies the affected command row");
      await captureRenderedWorkbenchCdp(connection,
        requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-channels-error.png"));
      await inner("d.querySelector('[data-studio-editor-cancel]').click();d.querySelector('[data-engineer-channels]').click();return 'cancel and reopen';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-channel-field]').value;"), "manipulatedVariable", "Cancel leaves the declared mapping unchanged");
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
