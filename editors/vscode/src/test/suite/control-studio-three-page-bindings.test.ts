import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { ThreePageBindings } from "../../controlStudio/webview/threePageBindings";
import { ThreePageController } from "../../controlStudio/webview/threePageController";

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

function surface(): { dom: any; root: HTMLElement; posts: Array<{ kind: string; payload: Record<string, unknown> }>; controller: ThreePageController; bindings: ThreePageBindings; gates: boolean[] } {
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
  const bindings = new (ThreePageBindings as any)(root, selectedLoop, controller,
    (kind: string, payload: Record<string, unknown> = {}) => posts.push({ kind, payload }), () => undefined,
    (valid: boolean) => gates.push(valid));
  return { dom, root, posts, controller, bindings, gates };
}

suite("Control Studio three-page binding selectors", () => {
  test("offers controller alternatives only for the currently selected plant input/output mapping", () => {
    const view = surface();
    const selected = loop();
    const matching = loop(); matching.id = "matching"; matching.name = "Matching controller";
    const mismatched = loop(); mismatched.id = "mismatched"; mismatched.name = "Mismatched controller";
    view.bindings.receive({ status: "bindings", targets: [
      target(selected, selected.id),
      target(matching, matching.id, selected.signals.manipulatedVariable.symbol, selected.signals.processVariable.symbol, "Main.MatchingPID"),
      target(mismatched, mismatched.id, "Main.OtherMV", "Main.OtherPV", "Main.MismatchedPID"),
    ] });
    const alternatives = Array.from(view.root.querySelectorAll<HTMLOptionElement>("[data-studio-controller] option[data-binding-loop-id]"));
    assert.deepStrictEqual([...new Set(alternatives.map(option => option.dataset.bindingLoopId))], ["matching"],
      "controller choices must follow the selected plant input/output identities");
  });

  test("the engine's manual-settings symbols for the selected loop reach the controller per family, and a loop that names none has none (C2)", () => {
    const view = surface();
    const selected = loop();
    const named = target(selected, selected.id);
    named.controllerCompatibility = { kind: "accepted", families: ["P", "PI"], familySettings: { P: ["P1.Kp"], PI: ["P1.Kp", "P1.Ki"] } };
    view.bindings.receive({ status: "bindings", targets: [named] });
    assert.deepStrictEqual([view.controller.symbolsOf("P"), view.controller.symbolsOf("PI")], [["P1.Kp"], ["P1.Kp", "P1.Ki"]]);
    const plain = surface();
    plain.bindings.receive({ status: "bindings", targets: [target(loop(), selected.id)] });
    assert.strictEqual(plain.controller.symbolsOf("P"), undefined, "an answer without the field leaves the page its TypeScript reading (a parked loop)");
  });

  test("invalid or ambiguous mapping invalidates current model evidence and gates work", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)] });
    const input = view.root.querySelector<HTMLSelectElement>("[data-studio-inputs]")!;
    const output = view.root.querySelector<HTMLSelectElement>("[data-studio-outputs]")!;
    const invalid = new view.dom.window.Option("Other MV (V)", "Main.OtherMV");
    input.append(invalid);
    Array.from(input.options).forEach(option => { option.selected = option === invalid; });
    output.options[0].selected = true;
    input.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    assert.strictEqual(view.bindings.canRun, false);
    assert.strictEqual(view.root.dataset.bindingDraft, "true");
    assert.ok(view.posts.some(post => post.kind === "control-studio.engineer.invalidate" && post.payload.bindingInvalid === true),
      "an invalid mapping must clear the current model/plot through the existing operation invalidation");
    assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!.disabled, true,
      "binding selection must not enable actions directly");
    assert.strictEqual(view.gates[view.gates.length - 1], false);
  });

  test("reverting to the exact current mapping restores the external operation gate without changing current instance or family", () => {
    const view = surface();
    const selected = loop();
    view.bindings.receive({ status: "bindings", targets: [target(selected, selected.id)] });
    const input = view.root.querySelector<HTMLSelectElement>("[data-studio-inputs]")!;
    const output = view.root.querySelector<HTMLSelectElement>("[data-studio-outputs]")!;
    const invalid = new view.dom.window.Option("Other MV (V)", "Main.OtherMV");
    input.append(invalid);
    Array.from(input.options).forEach(option => { option.selected = option === invalid; });
    output.options[0].selected = true;
    input.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    Array.from(input.options).forEach(option => { option.selected = option.value === selected.signals.manipulatedVariable.symbol; });
    input.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    assert.strictEqual(view.bindings.canRun, true);
    assert.strictEqual(view.gates[view.gates.length - 1], true, "the existing operation/application gate receives the valid mapping state");
    const controllerSelect = view.root.querySelector<HTMLSelectElement>("[data-studio-controller]")!;
    controllerSelect.value = `${selected.controller.instance}::PI`;
    controllerSelect.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
    assert.strictEqual(view.controller.choice, "PI");
    assert.strictEqual(controllerSelect.selectedOptions[0]?.dataset.bindingLoopId, undefined,
      "the current configured controller remains the current target");
    assert.strictEqual(view.posts.some(post => post.kind === "control-studio.binding.select"), false,
      "the current loop/family does not trigger a target switch");
  });
  test("the engine's answer for an opened state law reaches the controller with its recommended family and its one method", () => {
    const view = surface();
    const selected = loop();
    const offered: string[][] = [];
    view.controller.methodsChanged = methods => offered.push([...methods]);
    const opened = target(selected, selected.id);
    opened.controllerCompatibility = { kind: "accepted", families: ["state-feedback", "LQR"],
      familySettings: { "state-feedback": ["P1.K1", "P1.K2", "P1.N"], LQR: ["P1.K1", "P1.K2", "P1.N"] },
      recommendedFamily: "LQR", methods: ["state-space"] };
    view.bindings.receive({ status: "bindings", targets: [opened] });
    assert.deepStrictEqual(offered, [["state-space"]], "the engine's one method reaches the Tune page's list");
    assert.strictEqual(view.controller.selectedFamily, "LQR", "the engine's recommended family is selected");
  });
});
