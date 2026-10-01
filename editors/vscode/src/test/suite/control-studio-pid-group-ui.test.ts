import * as assert from "assert";
import * as path from "path";
import * as vscode from "vscode";
import { ThreePageController } from "../../controlStudio/webview/threePageController";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";
import { editControllerChannels } from "../../controlStudio/webview/threePageChannels";
import { controllerChannelMapping } from "../../controlStudio/controllerChannelPreferences";
import { projectPidGroupLoop } from "./control-studio-pid-group-fixtures";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

function fixture() {
  const loop = projectPidGroupLoop();
  loop.id += "-ui"; loop.parameterBinding.loop_id = loop.id;
  loop.controller.project.axes = ["P1.Parameters", "P1.SecondaryParameters"].flatMap(owner => ["ProportionalGain", "IntegralGain"].map(name => ({ symbol: `${owner}.${name}`, values: [1, 2] })));
  const values = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, Math.min(parameter.maximum, Math.max(parameter.minimum, 1))]));
  return { loop, values, payload: { status: "candidate", family: "paired-PI", availableFamilies: ["paired-PI"], flowStage: "Tune", values, stageEvidence: { hasModel: true, hasCandidate: true } } };
}
function withDom(run: (root: HTMLElement) => void) {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM('<main><select data-studio-controller></select><select data-engineer-tuning-method></select><div data-studio-tuning-result></div></main>');
  const previous = new Map<string, unknown>();
  for (const key of ["document", "window", "Option", "HTMLElement"]) { previous.set(key, (globalThis as any)[key]); (globalThis as any)[key] = key === "window" ? dom.window : dom.window[key]; }
  dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
  try { run(dom.window.document.querySelector("main")); }
  finally { for (const [key, value] of previous) (globalThis as any)[key] = value; dom.window.close(); }
}

suite("Control Studio PID group UI", () => {
  test("keeps each compiled PID attached to its declared signals when editing the model mapping", () => withDom(() => {
    const { loop } = fixture();
    const swapped = structuredClone(loop.channels);
    [swapped.inputs[0].signalId, swapped.inputs[1].signalId] = [swapped.inputs[1].signalId, swapped.inputs[0].signalId];
    assert.throws(() => controllerChannelMapping(loop, swapped), /P1.Controller|declared.*signal|bound/i);
    let commits = 0;
    const dialog = editControllerChannels(loop, () => { commits += 1; })!;
    const fields = [...dialog.querySelectorAll<HTMLSelectElement>("[data-studio-channel-field]")];
    [fields[0].value, fields[1].value] = [fields[1].value, fields[0].value];
    dialog.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
    assert.strictEqual(commits, 0);
    assert.strictEqual(fields[0].getAttribute("aria-invalid"), "true");
    assert.match(fields[0].closest("fieldset")!.textContent!, /P1.Controller.*P1.ManipulatedValue/s);
  }));
  test("names every actual member in the same controller choice", () => withDom(root => {
    const { loop, payload } = fixture();
    new ThreePageController(root, loop, () => undefined).receive(payload);
    const select = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    assert.match(select.selectedOptions[0].textContent!, /P1\.Controller.*P1\.Secondary.*Paired PI/);
    assert.strictEqual([...select.options].filter(option => option.dataset.family === "paired-PI").length, 1);
  }));
  test("names each owner and edits only the declared tuning axes while retaining fixed parameters", () => withDom(root => {
    const { loop, values, payload } = fixture();
    renderTunedController(root, loop, payload);
    const resultLabels = [...root.querySelectorAll("dt")].map(label => label.textContent!);
    assert.strictEqual(resultLabels.length, 4, "Only the declared tunable settings belong in the compact result");
    assert.strictEqual(resultLabels.filter(label => label.includes("P1.Controller")).length, 2);
    assert.strictEqual(resultLabels.filter(label => label.includes("P1.Secondary")).length, 2);
    const manual = initializeManualTuneSettings(root, loop); manual.render(payload);
    const inputs = [...root.querySelectorAll<HTMLInputElement>("[data-studio-manual-value]")];
    assert.strictEqual(inputs.length, 4);
    for (const input of inputs) {
      assert.match(input.labels![0].textContent!, /P1\.(?:Controller|Secondary)/);
      assert.match(input.labels![0].textContent!, /(?:Proportional gain \(Kp\)|Integral gain \(Ki\))/, "a member's gains read as a single PID's do");
    }
    const edited = inputs.find(input => input.dataset.studioManualValue === "P1.SecondaryParameters.IntegralGain")!;
    edited.value = "2";
    const read = manual.read();
    assert.deepStrictEqual(read.values, { ...values, "P1.SecondaryParameters.IntegralGain": 2 }, "The full declared record survives editing a single member gain");
  }));
  test("shows each member's designed gains and its path for a group tuned from its model", () => withDom(root => {
    const { loop, values, payload } = fixture();
    loop.controller.project.axes = [];
    loop.controller.project.seed = { kind: "decentralized-pid", family: "PI" };
    const decentralized = { members: [
      { inputId: "u1", outputId: "y1", pathGain: 0.2198, relativeGain: 1.125, effectiveGain: 0.1954, timeConstantSeconds: 7.7, delaySeconds: 0.2 },
      { inputId: "u2", outputId: "y2", pathGain: 0.2198, relativeGain: 1.125, effectiveGain: 0.1954, timeConstantSeconds: 7.7, delaySeconds: 0.2 }] };
    renderTunedController(root, loop, { ...payload, design: { decentralized } });
    const labels = [...root.querySelectorAll("dt")].map(label => label.textContent!);
    assert.deepStrictEqual(labels.map(label => label.replace(/ · .*/, "")).sort(), ["P1.Controller", "P1.Controller", "P1.Secondary", "P1.Secondary"],
      "a group designed from its model shows every member's proportional and integral gain");
    assert.ok(labels.every(label => / · (?:Proportional gain \(Kp\)|Integral gain \(Ki\))$/.test(label)), labels.join(" | "));
    const rows = [...root.querySelectorAll<HTMLTableRowElement>("[data-studio-decentralized-member]")].map(row => [...row.cells].map(cell => cell.textContent));
    assert.deepStrictEqual(rows.map(row => row[0]), ["P1.Controller", "P1.Secondary"], "one design row per member, named by its controller");
    assert.deepStrictEqual(rows[0].slice(1), ["K = 0.2198 °C/%, τ = 7.7 s, θ = 0.2 s", "1.13", "0.1954 °C/%"],
      "the path gain and the gain with the other loops closed carry the member's output-per-input unit");
    assert.ok(values);
  }));

  test("identifies the owning PID on each command and feedback mapping row", () => withDom(() => {
    const { loop } = fixture(); let commits = 0;
    const dialog = editControllerChannels(loop, () => { commits += 1; })!;
    const labels = [...dialog.querySelectorAll("legend")].map(label => label.textContent!);
    assert.strictEqual(labels.filter(label => label.includes("P1.Controller")).length, 2);
    assert.strictEqual(labels.filter(label => label.includes("P1.Secondary")).length, 2);
    dialog.querySelector<HTMLButtonElement>("[data-studio-editor-cancel]")!.click();
    assert.strictEqual(commits, 0);
  }));
});

suite("Control Studio three-page PID group rendered review", () => {
  test("shows group members in controller settings and channel mapping in the real editor", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const { loop, payload } = fixture();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      await pollValue("group host restoration", () => inner("return d.querySelector('[data-studio-three-page]').dataset.hostReady;"), value => value === "true", 10000);
      await inner(`const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engineering.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,stage:'engineer',payload:${JSON.stringify(payload)}},'*');await new Promise(resolve=>setTimeout(resolve,100));d.querySelector('[data-engineer-stage-tab=tune]').click();return 'candidate';`);
      assert.match(await inner("return d.querySelector('[data-studio-controller]').selectedOptions[0].textContent;"), /P1\.Controller.*P1\.Secondary/);
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-tuning-result] dt').length);"), "4");
      await capture("group-tune");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-manual-value]').length);"), "4");
      await capture("group-manual");
      await inner("d.querySelector('[data-engineer-channels]').click();return 'channels';");
      const rows = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('dialog legend')].map(node=>node.textContent));")) as string[];
      assert.strictEqual(rows.filter(text => text.includes("P1.Controller")).length, 2);
      assert.strictEqual(rows.filter(text => text.includes("P1.Secondary")).length, 2);
      await capture("group-channels");
      await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'cancel';");
      async function capture(name: string) {
        const file = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT");
        await captureRenderedWorkbenchCdp(connection, path.join(path.dirname(file), `${name}.png`));
      }
    } finally { connection.close(); await vscode.commands.executeCommand("workbench.action.closeAllEditors"); }
  });
});
