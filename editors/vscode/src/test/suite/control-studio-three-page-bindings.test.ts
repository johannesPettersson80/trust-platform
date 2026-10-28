import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { ThreePageBindings } from "../../controlStudio/webview/threePageBindings";
import { ThreePageController } from "../../controlStudio/webview/threePageController";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

const { JSDOM } = require("jsdom");

function loop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
}

function target(value: any, loopId: string, inputSymbol = value.signals.manipulatedVariable.symbol,
  outputSymbol = value.signals.processVariable.symbol, controllerInstance = value.controller.instance): any {
  return {
    loopId,
    name: value.name,
    configurationPath: `/tmp/${loopId}.json`,
    controller: { instance: controllerInstance, type: value.controller.type, tasks: [{ name: "MainTask", intervalMs: value.controller.taskPeriodMs }] },
    inputs: [{ id: "manipulatedVariable", symbol: inputSymbol, address: "%QL0", iecType: "LREAL", unit: "V", bounds: { minimum: -20, maximum: 20 }, role: "MV", order: 0 }],
    outputs: [{ id: "processVariable", symbol: outputSymbol, address: "%IL0", iecType: "LREAL", unit: "m", bounds: { minimum: -20, maximum: 20 }, role: "PV", order: 0 }],
    controllerCompatibility: { kind: "accepted", families: ["P", "PI", "PD", "PID"] },
  };
}

/**
 * The mapping answer that the engine gives the binding page (B1.2, B3.6): its outcome, whether the mapping blocks the work, its sentence and
 * its controller choices. The engine sets `blocksRun` for the ambiguous and the unmatched outcomes; a test names it with the outcome.
 */
function answer(outcome: string, extra: Record<string, unknown> = {}): Record<string, unknown> {
  return { outcome, changed: false, blocksRun: false, sentence: "", options: [], ...extra };
}

/** One controller choice of the engine's answer: a family that a loop's adapter realizes, with the selection that opens the loop. */
function choice(loopId: string, family: string, label: string, controllerInstance: string): Record<string, unknown> {
  return { loopId, family, label, selection: { loopId, controllerInstance, inputIds: ["manipulatedVariable"], outputIds: ["processVariable"] } };
}

/** The controller the engine offers a loop on open (`controllerOffer`, C2, C4): each family with its settings, and the recommended one. */
function openedController(families: string[], familySettings: Record<string, string[]>, recommended: string | null = null) {
  return { controllerOffer: { families: families.map(family => ({ family, label: family })), recommended, familySettings, settingRows: [] } };
}

function surface(recordingChanged?: (selection: { inputs: string[]; outputs: string[] }) => void):
  { dom: any; root: HTMLElement; posts: Array<{ kind: string; payload: Record<string, unknown> }>; controller: ThreePageController; bindings: ThreePageBindings; gates: boolean[] } {
  const dom = new JSDOM(`<main data-studio-three-page>
    <select data-studio-controller><option value="Main">Main · Control.PID</option></select>
    <select data-studio-inputs multiple></select><select data-studio-outputs multiple></select>
    <button data-engineer-find></button><button data-engineer-tune disabled></button><button data-engineer-verify disabled></button>
    <button data-engineer-apply disabled></button><button data-identify-import></button>
  </main>`);
  const root = dom.window.document.querySelector("main") as HTMLElement;
  (global as any).Option = dom.window.Option;
  const posts: Array<{ kind: string; payload: Record<string, unknown> }> = [];
  const selectedLoop = loop();
  const controller = new ThreePageController(root, selectedLoop, (kind: string, payload: Record<string, unknown> = {}) => posts.push({ kind, payload }));
  const gates: boolean[] = [];
  const bindings = new (ThreePageBindings as any)(root, selectedLoop,
    (kind: string, payload: Record<string, unknown> = {}) => posts.push({ kind, payload }), () => undefined,
    (valid: boolean) => gates.push(valid), recordingChanged);
  return { dom, root, posts, controller, bindings, gates };
}

/** Puts one plant input on the inputs selector, selected alone, and fires its change. */
function selectInput(view: ReturnType<typeof surface>, symbol: string): void {
  const input = view.root.querySelector<HTMLSelectElement>("[data-studio-inputs]")!;
  const chosen = new view.dom.window.Option(`${symbol} (V)`, symbol);
  input.append(chosen);
  Array.from(input.options).forEach(option => { option.selected = option === chosen; });
  input.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
}

suite("Control Studio three-page binding selectors", () => {
  teardown(() => setOpenedOffers({}));

  test("offers the controller choices that the engine's mapping answer names, and no others", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)],
      mapping: answer("current", { options: [choice("matching", "PI", "Main.MatchingPID · PI", "Main.MatchingPID")] }) });
    const alternatives = Array.from(view.root.querySelectorAll<HTMLOptionElement>("[data-studio-controller] option[data-binding-loop-id]"));
    assert.deepStrictEqual(alternatives.map(option => [option.dataset.bindingLoopId, option.textContent]), [["matching", "Main.MatchingPID · PI"]],
      "the controller choices are the engine's, shown with its labels");
  });

  test("offers an opened recording's channels with the words the engine gives them, and the page knows no channel by name", () => {
    // Wrong behaviour caught: the page knowing the legacy channel identifiers to put words to them. The engine words an older file's
    // `appliedMv` and `pv` (B3.6); the channels of an archive are named by their symbols and carry no words.
    const options = (view: ReturnType<typeof surface>, kind: "inputs" | "outputs") =>
      Array.from(view.root.querySelectorAll<HTMLOptionElement>(`[data-studio-${kind}] option`), option => [option.value, option.textContent, option.selected]);
    const legacy = surface();
    legacy.bindings.receive({ status: "checked", recordingId: "rec-1", recordingCatalog: {
      channels: [{ id: "appliedMv", unit: "%", label: "Applied input" }, { id: "pv", unit: "degC", label: "Measured output" }],
      inputs: [{ id: "appliedMv", unit: "%", label: "Applied input" }], outputs: [{ id: "pv", unit: "degC", label: "Measured output" }],
      selection: { inputs: ["appliedMv"], outputs: ["pv"] } } });
    assert.deepStrictEqual(options(legacy, "inputs"), [["appliedMv", "Applied input (%)", true]]);
    assert.deepStrictEqual(options(legacy, "outputs"), [["pv", "Measured output (°C)", true]]);
    const archive = surface();
    archive.bindings.receive({ status: "checked", recordingId: "rec-2", recordingCatalog: {
      channels: [{ id: "Main.PV", unit: "degC" }, { id: "Main.Flag", unit: "" }],
      inputs: [{ id: "Main.Flag", unit: "" }], outputs: [{ id: "Main.PV", unit: "degC" }],
      selection: { inputs: ["Main.Flag"], outputs: ["Main.PV"] } } });
    assert.deepStrictEqual(options(archive, "inputs"), [["Main.Flag", "Main.Flag", true]], "a channel without a unit shows its name alone");
    assert.deepStrictEqual(options(archive, "outputs"), [["Main.PV", "Main.PV (°C)", true]]);
  });

  test("a changed mapping goes to the engine as the selectors name it, and the page matches nothing itself", () => {
    const view = surface();
    const selected = loop();
    // The selectors start on the live sides that the open answer names (B3.6): the page offers and selects no signal of its own.
    const side = (signalId: string, signal: { symbol: string; unit: string }) => ({ signalId, symbol: signal.symbol, unit: signal.unit });
    setOpenedOffers({ liveSides: { inputs: [side("manipulatedVariable", selected.signals.manipulatedVariable)],
      outputs: [side("processVariable", selected.signals.processVariable)] } } as any);
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)] });
    selectInput(view, "Main.OtherMV");
    const request = view.posts.filter(post => post.kind === "control-studio.binding.list");
    assert.deepStrictEqual(request.map(post => post.payload), [{ mapping: { loopId: selected.id, inputs: ["Main.OtherMV"], outputs: [selected.signals.processVariable.symbol] } }],
      "the page asks the engine; it neither invalidates nor chooses until the answer comes");
    assert.strictEqual(view.bindings.canRun, true, "no decision is taken before the engine answers");
  });

  test("an unmatched mapping from the engine invalidates the model and gates the work", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)],
      mapping: answer("unmatched", { blocksRun: true, sentence: "Choose the complete input and output mapping of a configured loop." }) });
    assert.strictEqual(view.bindings.canRun, false);
    assert.strictEqual(view.root.dataset.bindingDraft, "true");
    assert.ok(view.posts.some(post => post.kind === "control-studio.engineer.invalidate" && post.payload.bindingInvalid === true),
      "an invalid mapping must clear the current model/plot through the existing operation invalidation");
    assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!.disabled, true,
      "binding selection must not enable actions directly");
    assert.strictEqual(view.gates[view.gates.length - 1], false);
  });

  test("a current mapping from the engine releases the gate again", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)],
      mapping: answer("unmatched", { blocksRun: true, sentence: "Choose the complete input and output mapping of a configured loop." }) });
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)], mapping: answer("current") });
    assert.strictEqual(view.bindings.canRun, true);
    assert.strictEqual(view.gates[view.gates.length - 1], true, "the existing operation/application gate receives the valid mapping state");
    assert.strictEqual(view.posts.some(post => post.kind === "control-studio.binding.select"), false,
      "the current loop is not switched to");
  });

  test("the gate follows the engine's blocksRun, not the name of the outcome", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)],
      mapping: answer("unmatched", { sentence: "The engine says this mapping does not stop the work." }) });
    assert.strictEqual(view.bindings.canRun, true, "an outcome the engine does not mark blocking blocks nothing");
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)],
      mapping: answer("current", { blocksRun: true, sentence: "The engine says this mapping stops the work." }) });
    assert.strictEqual(view.bindings.canRun, false, "a mapping the engine marks blocking gates the work");
    assert.strictEqual(view.bindings.blockReason, "The engine says this mapping stops the work.");
  });

  test("the one other loop that the engine names is chosen with the selection it gives", () => {
    const view = surface();
    const selected = loop();
    const named = { loopId: "other", controllerInstance: "Main.OtherPID", inputIds: ["manipulatedVariable"], outputIds: ["processVariable"] };
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)], mapping: answer("other", { selection: named }) });
    assert.deepStrictEqual(view.posts.filter(post => post.kind === "control-studio.binding.select").map(post => post.payload),
      [{ selection: named, page: "model" }], "the page sends the engine's selection, with no family");
    assert.strictEqual(view.bindings.canRun, false, "the choice waits for the other loop to open");
  });

  test("an ambiguous mapping lists the engine's choices behind a placeholder, and the chosen one is sent with its family", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)],
      mapping: answer("ambiguous", { blocksRun: true, sentence: "Select a controller for this plant mapping on Tune.",
        options: [choice("a", "P", "A · P", "Main.A"), choice("b", "PI", "B · PI", "Main.B")] }) });
    const controllerSelect = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    assert.ok(controllerSelect.querySelector("[data-binding-placeholder]"), "an ambiguous mapping starts on the placeholder");
    assert.strictEqual(controllerSelect.value, "");
    controllerSelect.value = "b::PI";
    controllerSelect.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    assert.deepStrictEqual(view.posts.filter(post => post.kind === "control-studio.binding.select").map(post => post.payload),
      [{ selection: { loopId: "b", controllerInstance: "Main.B", inputIds: ["manipulatedVariable"], outputIds: ["processVariable"] }, family: "PI", page: "tune" }]);
  });

  test("the live sides come from the open answer, and a recording change is reported only when the engine says it changed", () => {
    setOpenedOffers({ liveSides: { inputs: [{ signalId: "manipulatedVariable", symbol: "Main.MV", unit: "V" }],
      outputs: [{ signalId: "processVariable", symbol: "Main.PV", unit: "m" }] } } as any);
    const changed: Array<{ inputs: string[]; outputs: string[] }> = [];
    const view = surface(selection => changed.push(selection));
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id, "Main.MV", "Main.PV")] });
    const input = view.root.querySelector<HTMLSelectElement>("[data-studio-inputs]")!;
    const output = view.root.querySelector<HTMLSelectElement>("[data-studio-outputs]")!;
    assert.deepStrictEqual([Array.from(input.selectedOptions).map(option => option.value), Array.from(output.selectedOptions).map(option => option.value)],
      [["Main.MV"], ["Main.PV"]], "each live selector offers its side from the open answer, selected");

    view.bindings.restoreRecording({ inputs: ["appliedMv"], outputs: ["pv"] });
    input.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    const request = view.posts.filter(post => post.kind === "control-studio.binding.list").pop();
    assert.deepStrictEqual(request?.payload, { mapping: { loopId: selected.id, inputs: ["appliedMv"], outputs: ["pv"],
      recordingSelection: { inputs: ["appliedMv"], outputs: ["pv"] } } }, "a recording's selection travels with the mapping request");
    view.bindings.receive({ status: "bindings", targets: [], mapping: answer("recorded", { changed: false }) });
    assert.deepStrictEqual(changed, [], "an unchanged selection refits nothing");
    view.bindings.receive({ status: "bindings", targets: [], mapping: answer("recorded", { changed: true }) });
    assert.deepStrictEqual(changed, [{ inputs: ["appliedMv"], outputs: ["pv"] }], "a change the engine names refits the recording");
  });

  test("a selection changed on a shown recording sends the selection the recording holds beside the selectors' signals", () => {
    // Specification 35, `controlStudio/setupDiscover` (B3.6): while a recording is shown, the mapping request names the recording's selection,
    // from which the engine says whether the selectors changed it. Catches a request that names the selectors' values twice: the engine then
    // never sees a change, nothing refits, and a recorded output the engineer chose is never fitted.
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "recording-needed", recordingId: "rec-1", recordingCatalog: { kind: "archive",
      channels: [{ id: "P1.ManipulatedValue", unit: "%" }, { id: "P1.ProcessValue", unit: "°C" }, { id: "P1.RecordedTemperature", unit: "°C" }],
      inputs: [{ id: "P1.ManipulatedValue", unit: "%" }],
      outputs: [{ id: "P1.ProcessValue", unit: "°C" }, { id: "P1.RecordedTemperature", unit: "°C" }],
      selection: { inputs: ["P1.ManipulatedValue"], outputs: ["P1.ProcessValue"] } } });
    const output = view.root.querySelector<HTMLSelectElement>("[data-studio-outputs]")!;
    for (const option of Array.from(output.options)) option.selected = option.value === "P1.RecordedTemperature";
    output.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    const request = view.posts.filter(post => post.kind === "control-studio.binding.list").pop();
    assert.deepStrictEqual(request?.payload, { mapping: { loopId: selected.id, inputs: ["P1.ManipulatedValue"], outputs: ["P1.RecordedTemperature"],
      recordingSelection: { inputs: ["P1.ManipulatedValue"], outputs: ["P1.ProcessValue"] } } });
  });

  test("the open answer's controller and methods reach the controller, and no binding target replaces them (C2)", () => {
    const view = surface();
    const selected = loop();
    view.controller.receive(openedController(["P", "PI"], { P: ["P1.Kp"], PI: ["P1.Kp", "P1.Ki"] }));
    // A target's own compatibility is the binding's data: it does not reach the controller, so the open answer's settings stay (C2).
    const named = target(selected, selected.id);
    named.controllerCompatibility = { kind: "accepted", families: ["P", "PI"], familySettings: { P: ["P1.Other"] } };
    view.bindings.receive({ status: "bindings", targets: [named] });
    assert.deepStrictEqual([view.controller.symbolsOf("P"), view.controller.symbolsOf("PI")], [["P1.Kp"], ["P1.Kp", "P1.Ki"]]);
    const plain = surface();
    plain.bindings.receive({ status: "bindings", targets: [target(loop(), selected.id)] });
    assert.strictEqual(plain.controller.symbolsOf("P"), undefined, "a controller with no open answer names no family settings");
  });

  test("the open answer for a state law reaches the controller with its recommended family and its one method, and no binding target replaces them", () => {
    const view = surface();
    const selected = loop();
    const offered: string[][] = [];
    view.controller.methodsChanged = methods => offered.push([...methods]);
    // The open answer's controller and methods reach the page through the engine's opened stage (C2, C4); binding targets do not carry them.
    view.controller.receive({ ...openedController(["state-feedback", "LQR"], { "state-feedback": ["P1.K1", "P1.K2", "P1.N"], LQR: ["P1.K1", "P1.K2", "P1.N"] }, "LQR"),
      methods: ["state-space"], methodTitles: { "state-space": "State-space design" } });
    const opened = target(selected, selected.id);
    opened.controllerCompatibility = { kind: "accepted", families: ["state-feedback", "LQR"], recommendedFamily: "PD", methods: ["relay"] };
    view.bindings.receive({ status: "bindings", targets: [opened] });
    assert.deepStrictEqual(offered, [["state-space"]], "the engine's one method reaches the Tune page's list");
    const select = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    assert.strictEqual([...select.options].find(option => option.dataset.family === "LQR")?.textContent, `${selected.controller.instance} · LQR — recommended`,
      "the engine's recommended family is marked");
    assert.strictEqual(view.controller.selectedFamily, "automatic", "Automatic is shown until the engineer chooses (B4.2)");
  });
});
