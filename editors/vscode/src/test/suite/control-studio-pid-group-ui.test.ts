import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { ThreePageController } from "../../controlStudio/webview/threePageController";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";
import { editControllerChannels } from "../../controlStudio/webview/threePageChannels";
import { projectPidGroupLoop } from "./control-studio-pid-group-fixtures";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

function fixture() {
  const loop = projectPidGroupLoop();
  loop.id += "-ui"; loop.parameterBinding.loop_id = loop.id;
  loop.controller.project.axes = ["P1.Parameters", "P1.SecondaryParameters"].flatMap(owner => ["ProportionalGain", "IntegralGain"].map(name => ({ symbol: `${owner}.${name}`, values: [1, 2] })));
  const values = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, Math.min(parameter.maximum, Math.max(parameter.minimum, 1))]));
  // The engine's words of the answer (TuneWords): one row for each tuned gain of each member, named by its instance.
  const words = { outcome: "Meets the requirements", gainLabels: { proportional: "Proportional gain (Kp)", integral: "Integral gain (Ki)", derivative: "Derivative gain (Kd)" }, settingLabels: [
    { symbol: "P1.Parameters.ProportionalGain", label: "P1.Controller · Proportional gain (Kp)" },
    { symbol: "P1.Parameters.IntegralGain", label: "P1.Controller · Integral gain (Ki)" },
    { symbol: "P1.SecondaryParameters.ProportionalGain", label: "P1.Secondary · Proportional gain (Kp)" },
    { symbol: "P1.SecondaryParameters.IntegralGain", label: "P1.Secondary · Integral gain (Ki)" },
  ] };
  // The engine's controller for the group (C2, C4): its one family, labelled, with the four gain rows of the members (their labels, units and bounds).
  const settingRows = words.settingLabels.map(({ symbol, label }) => {
    const parameter = loop.parameterBinding.parameters.find((item: any) => item.symbol === symbol);
    return { symbol, label, unit: parameter.unit, minimum: parameter.minimum, maximum: parameter.maximum };
  });
  const controllerOffer = { families: [{ family: "paired-PI", label: "Paired PI" }], familySettings: { "paired-PI": settingRows.map(row => row.symbol) }, settingRows };
  return { loop, values, controllerOffer, payload: { status: "candidate", family: "paired-PI", controllerOffer, flowStage: "Tune", values, words,
    stageEvidence: { hasModel: true, hasCandidate: true } } };
}
function withDom(run: (root: HTMLElement) => void) {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM('<main><select data-studio-controller></select><select data-engineer-tuning-method></select><div data-studio-tuning-result></div></main>');
  const previous = new Map<string, unknown>();
  for (const key of ["document", "window", "Option", "HTMLElement"]) { previous.set(key, (globalThis as any)[key]); (globalThis as any)[key] = key === "window" ? dom.window : dom.window[key]; }
  dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new dom.window.Event("close")); };
  try { run(dom.window.document.querySelector("main")); }
  finally { setOpenedOffers({}); for (const [key, value] of previous) (globalThis as any)[key] = value; dom.window.close(); }
}

/** The rows the engine answers for a loop's channels: each side's signals as its choices, the owner of each channel and its signal now. */
function engineRows(loop: any) {
  const signal = (id: string) => id === "setpoint" ? loop.signals.setpoint : id === "processVariable" ? loop.signals.processVariable
    : id === "manipulatedVariable" ? loop.signals.manipulatedVariable : loop.signals.additional?.find((item: any) => item.id === id);
  const owner = (kind: "input" | "output", id: string) => loop.controller.group?.members.find((member: any) =>
    (kind === "input" ? member.inputChannelId : member.outputChannelId) === id)?.instance;
  const side = (kind: "input" | "output", channels: any[]) => channels.map(channel => ({
    modelChannelId: channel.modelChannelId, owner: owner(kind, channel.modelChannelId), signalId: channel.signalId,
    choices: channels.map(item => ({ signalId: item.signalId, symbol: signal(item.signalId).symbol, unit: signal(item.signalId).unit })),
  }));
  return { inputs: side("input", loop.channels.inputs), outputs: side("output", loop.channels.outputs) };
}

suite("Control Studio PID group UI", () => {
  // The group's wiring rule (a PID group keeps its declared signals) is the engine's: Rust pins it in `loop_definition/channel_selection_tests.rs`.
  test("sends the plant signal each row picks, for the engine to check, and keeps the rows the engine answered", () => withDom(() => {
    const { loop } = fixture();
    // Assigned inside the callback, so the initial value keeps the type from narrowing to `undefined` at the assertion.
    let sent = undefined as { inputs: { modelChannelId: string; signalId: string }[] } | undefined;
    const dialog = editControllerChannels(engineRows(loop), selection => { sent = selection; });
    const fields = [...dialog.querySelectorAll<HTMLSelectElement>("[data-studio-channel-field]")];
    [fields[0].value, fields[1].value] = [fields[1].value, fields[0].value];
    dialog.querySelector<HTMLButtonElement>("[data-studio-editor-done]")!.click();
    assert.deepStrictEqual(sent?.inputs.map(pick => pick.signalId), [loop.channels.inputs[1].signalId, loop.channels.inputs[0].signalId]);
    assert.match(fields[0].closest("fieldset")!.textContent!, /P1.Controller.*P1.ManipulatedValue/s);
  }));
  test("names every actual member in the same controller choice", () => withDom(root => {
    const { loop, payload } = fixture();
    new ThreePageController(root, loop, () => undefined).receive(payload);
    const select = root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    assert.match(select.selectedOptions[0].textContent!, /P1\.Controller.*P1\.Secondary.*Paired PI/);
    assert.strictEqual([...select.options].filter(option => option.dataset.family === "paired-PI").length, 1);
  }));
  test("names each owner and edits only the declared tuning axes, reading each entry as typed", () => withDom(root => {
    const { loop, values, controllerOffer, payload } = fixture();
    renderTunedController(root, loop, payload);
    const resultLabels = [...root.querySelectorAll("dt")].map(label => label.textContent!);
    assert.strictEqual(resultLabels.length, 4, "Only the declared tunable settings belong in the compact result");
    assert.strictEqual(resultLabels.filter(label => label.includes("P1.Controller")).length, 2);
    assert.strictEqual(resultLabels.filter(label => label.includes("P1.Secondary")).length, 2);
    setOpenedOffers({ controllerOffer } as any);
    const manual = initializeManualTuneSettings(root, undefined, family => family === "paired-PI" ? controllerOffer.familySettings["paired-PI"] : undefined);
    manual.render(payload);
    const inputs = [...root.querySelectorAll<HTMLInputElement>("[data-studio-manual-value]")];
    assert.strictEqual(inputs.length, 4);
    for (const input of inputs) {
      assert.match(input.labels![0].textContent!, /P1\.(?:Controller|Secondary)/);
      assert.match(input.labels![0].textContent!, /(?:Proportional gain \(Kp\)|Integral gain \(Ki\))/, "a member's gains read as a single PID's do");
    }
    const edited = inputs.find(input => input.dataset.studioManualValue === "P1.SecondaryParameters.IntegralGain")!;
    edited.value = "2";
    const read = manual.read();
    const gains = ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain", "P1.SecondaryParameters.ProportionalGain", "P1.SecondaryParameters.IntegralGain"];
    assert.deepStrictEqual(read.values, Object.fromEntries(gains.map(symbol => [symbol, symbol === "P1.SecondaryParameters.IntegralGain" ? "2" : String(values[symbol])])),
      "each member's gain is read as typed, and the other gains keep their values");
  }));
  // V7.17: nothing sends `design.decentralized`. A group's Tune answer is `group` (structureResults.ts), so the page's own table of each member's path
  // (K, τ, θ, relative gain) was dead code and went with its test; the paired members' paths are pinned by the Rust test
  // `a_paired_group_designs_each_member_on_its_own_path_with_no_decoupler` (study/tune/design/pid_group_tests.rs).
  test("shows every member's proportional and integral gain for a group tuned from its model, named by its instance", () => withDom(root => {
    const { loop, values, payload } = fixture();
    loop.controller.project.axes = [];
    loop.controller.project.seed = { kind: "decentralized-pid", family: "PI" };
    renderTunedController(root, loop, payload);
    const labels = [...root.querySelectorAll("dt")].map(label => label.textContent!);
    assert.deepStrictEqual(labels.map(label => label.replace(/ · .*/, "")).sort(), ["P1.Controller", "P1.Controller", "P1.Secondary", "P1.Secondary"],
      "a group designed from its model shows every member's proportional and integral gain");
    assert.ok(labels.every(label => / · (?:Proportional gain \(Kp\)|Integral gain \(Ki\))$/.test(label)), labels.join(" | "));
    assert.ok(values);
  }));

  test("identifies the owning PID on each command and feedback mapping row", () => withDom(() => {
    const { loop } = fixture(); let commits = 0;
    const dialog = editControllerChannels(engineRows(loop), () => { commits += 1; });
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
    // The loop is saved into the project like any other loop before it is opened, as `withStudio` does: the engine finds its declaring
    // folder through `projectLoops`, and an open of a loop that no folder declares is refused without a page.
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.mkdirSync(path.join(project, "control-loops"), { recursive: true });
    fs.writeFileSync(path.join(project, "control-loops", `${loop.id.replace(/[^a-z0-9-]+/gi, "-")}.json`), `${JSON.stringify(loop, null, 2)}\n`);
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
      // The engine tunes this loop by its finite search (C5), so its open answer names no method beside Automatic and refuses a named one
      // ("A search law is tuned by its finite search: choose Automatic."): the Tuning method selector offers no Manual method, and so no
      // manual rows for the members. The manual rows of a family the engine offers are pinned above, in the page's own test.
      assert.strictEqual(await inner("return JSON.stringify([...d.querySelectorAll('[data-engineer-tuning-method] option')].map(option=>option.value));"), JSON.stringify(["automatic"]),
        "the engine offers Automatic alone for a search law, as the Tuning method selector lists it");
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
