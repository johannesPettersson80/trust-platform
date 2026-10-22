import * as assert from "assert";
import { fillProjectTest, renderThreePageExperimentTemplate } from "../../controlStudio/threePageExperimentTemplate";
import { acquisitionPreferences, limitsOf } from "../../controlStudio/webview/threePageLimits";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";
import { ThreePageReference } from "../../controlStudio/webview/threePageReference";
import type { LoopDefinition } from "../../controlStudio/generated/OpenResult";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** The limits the open answer gives a two-input loop (`experimentLimits`, B3.2), as the engine's Rust tests pin them. */
function engineDefaults() {
  return {
    durationSeconds: 120,
    durationWords: "2 min",
    inputs: [{ symbol: "P1.MV", unit: "%", minimum: 10, maximum: 90 }, { symbol: "P1.Load", unit: "kW", minimum: 0, maximum: 50 }],
    outputs: [{ symbol: "P1.PV", unit: "degC", minimum: 0, maximum: 100 }],
    amplitude: 5,
    relayDirection: "positive",
    reviewDurationSeconds: 120,
  } as any;
}

suite("Control Studio open answer fields", () => {
  test("the limits start from the engine's limits, with the review's amplitude and relay direction", () => {
    const limits = limitsOf(engineDefaults());
    // The duration is the engine's words for it (`durationWords`), which the editor shows as they are.
    assert.strictEqual(limits.durationSeconds, "2 min");
    assert.deepStrictEqual(limits.inputs.map(input => input.symbol), ["P1.MV", "P1.Load"]);
    assert.strictEqual(limits.amplitude, 5);
    assert.strictEqual(limits.relayDirection, "positive");
  });

  test("the editor shows each signal's range as the open answer lists it, and merges no record of the study over them", () => {
    // The engine lists the study's own ranges in the open answer (`limits_answer_lists_every_plant_signal_range`): the page maps the list.
    const held = { ...engineDefaults(), durationSeconds: 60, durationWords: "60 s",
      inputs: [{ symbol: "P1.MV", unit: "%", minimum: 20, maximum: 80 }, { symbol: "P1.Load", unit: "kW", minimum: 5, maximum: 40 }],
      outputs: [{ symbol: "P1.PV", unit: "degC", minimum: 10, maximum: 90 }] };
    const limits = limitsOf(held);
    assert.strictEqual(limits.durationSeconds, "60 s");
    assert.deepStrictEqual(limits.inputs.map(input => [input.symbol, input.minimum, input.maximum]), [["P1.MV", 20, 80], ["P1.Load", 5, 40]]);
    assert.deepStrictEqual(limits.outputs.map(output => [output.symbol, output.minimum, output.maximum]), [["P1.PV", 10, 90]]);
  });

  test("the acquisition request carries the engine's amplitude and relay direction, and the range of every signal by its symbol", () => {
    const limits = limitsOf(engineDefaults());
    const request = acquisitionPreferences("automatic", limits);
    assert.strictEqual(request.amplitude, 5);
    assert.strictEqual(request.relayDirection, "positive");
    assert.strictEqual(request.durationSeconds, "2 min", "the duration goes back as the engine's words, which the engine reads as exactly 120 s");
    assert.deepStrictEqual(request.inputRanges, { "P1.MV": { minimum: 10, maximum: 90 }, "P1.Load": { minimum: 0, maximum: 50 } });
    assert.deepStrictEqual(request.outputRanges, { "P1.PV": { minimum: 0, maximum: 100 } });
    assert.strictEqual("inputRange" in request || "outputRange" in request || "reuseRecordings" in request, false,
      "the earlier single range and the reuse flag are no longer sent");
  });

  test("a loop of one input and one output sends its ranges by symbol too: no signal is the single one", () => {
    const limits = limitsOf({ ...engineDefaults(), inputs: [{ symbol: "P1.MV", unit: "%", minimum: 10, maximum: 90 }] });
    const request = acquisitionPreferences("mv_step", limits);
    assert.deepStrictEqual(Object.keys(request.inputRanges ?? {}), ["P1.MV"]);
    assert.deepStrictEqual(Object.keys(request.outputRanges ?? {}), ["P1.PV"]);
    assert.strictEqual(request.method, "mv_step");
  });

  test("the page sends the limits as they stand: every bound and the duration as the editor holds them, and a missing duration as its blank text", () => {
    const declared = engineDefaults();
    const open = limitsOf({ ...declared, durationSeconds: null, durationWords: null });
    assert.strictEqual(open.durationSeconds, undefined, "the loop declares no duration and the engineer has given none");
    assert.strictEqual(acquisitionPreferences("relay", open).durationSeconds, "", "the field is blank: the engine refuses it under the duration's field, and Limits opens");
    const typed = { ...limitsOf(declared), durationSeconds: "15 min" };
    typed.inputs = [{ ...typed.inputs[0], minimum: "", maximum: "90" }, typed.inputs[1]];
    const request = acquisitionPreferences("automatic", typed);
    assert.strictEqual(request.durationSeconds, "15 min", "the duration goes as the text typed");
    assert.deepStrictEqual(request.inputRanges?.["P1.MV"], { minimum: "", maximum: "90" }, "a blank or unreadable bound goes as typed: the engine refuses it under its field");
  });

  test("the project test's controls show the engine's signals, duration and the parameter tool its form names, and no loop rule", () => {
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}${"<div data-three-page-analysis-parameter-slot></div>"}</main>`);
    const root = dom.window.document.querySelector("main") as HTMLElement;
    fillProjectTest(root, {
      signals: [
        { id: "sp", label: "P1.Setpoint · Setpoint", unit: "degC", selected: true },
        { id: "request", label: "P1.Request · MV", unit: null, selected: false },
      ],
      durationSeconds: 30.1,
      baselineSeconds: 5,
      parameterTool: "analysis",
    } as any);
    const options = Array.from(root.querySelectorAll<HTMLOptionElement>("[data-three-page-experiment-signals] option"));
    assert.deepStrictEqual(options.map(option => [option.value, option.textContent, option.selected]),
      [["sp", "P1.Setpoint · Setpoint (°C)", true], ["request", "P1.Request · MV", false]]);
    assert.strictEqual(root.querySelector("[data-three-page-experiment-duration]")!.textContent, "30.1 s · 5 s baseline");
    assert.ok(root.querySelector("[data-three-page-analysis-parameter-slot] [data-three-page-experiment-parameters]"), "the analysis's parameters fill their slot");
    assert.ok(root.querySelector("[data-three-page-analysis-parameter-slot] [data-three-page-experiment-parameter-action]"), "the tool carries its own step");
    assert.strictEqual(root.querySelector("[data-three-page-experiment] [data-three-page-experiment-parameters]"), null, "the project test holds no tool of its own");
    dom.window.close();
  });

  test("the experiment's settings fill the project test's own slot when the form names them, and the Tune page's slot stays empty", () => {
    const dom = new JSDOM(`<main>${renderThreePageExperimentTemplate()}${"<div data-three-page-analysis-parameter-slot></div>"}</main>`);
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const settings = {
      loop_id: "control-lesson-21", task_name: "MainTask", mode: { symbol: "P1.ExperimentEnabled", manual_value: false },
      parameters: [{ symbol: "P1.HoldSeconds", iec_type: "REAL", unit: "s", convention: "held step duration", minimum: 20, maximum: 40 }],
    };
    assert.strictEqual(root.querySelector("[data-three-page-experiment-parameter-slot]")!.children.length, 0, "the slot waits for the engine's form");
    fillProjectTest(root, {
      signals: [{ id: "pv", label: "P1.TemperatureC · Process value", unit: "degC", selected: true }],
      durationSeconds: 45.1,
      parameterTool: "experiment",
      parameterBinding: settings,
    } as any);
    const tool = root.querySelector("[data-three-page-experiment] [data-three-page-experiment-parameter-slot] [data-three-page-experiment-parameters]");
    assert.ok(tool, "the experiment's settings fill the project test's slot");
    assert.strictEqual(tool!.querySelector("summary")!.textContent, "Declared experiment settings");
    assert.ok(tool!.querySelector("[data-three-page-experiment-parameter-action]"), "the tool carries its own step");
    assert.ok(tool!.querySelector("[data-three-page-experiment-restore]"));
    assert.strictEqual(root.querySelector("[data-three-page-analysis-parameter-slot]")!.children.length, 0, "an analysis's slot stays empty");
    dom.window.close();
  });

  test("the setpoint selector shows only on the Setpoint step of a loop without a setpoint, with the setpoints the engine offers (B3.6)", () => {
    const markup = `<main><select data-engineer-acquisition-method><option value="automatic">Automatic</option><option value="setpoint_step">Setpoint step</option></select>
      <label>Outputs <select data-studio-outputs></select></label></main>`;
    const dom = new JSDOM(markup);
    const original = { window: (globalThis as any).window, document: (globalThis as any).document, Option: (globalThis as any).Option, Event: (globalThis as any).Event };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, Option: dom.window.Option, Event: dom.window.Event });
    try {
      setOpenedOffers({ setpointChoices: [{ id: "reference", symbol: "P1.Reference", unit: "degC" }] });
      const root = dom.window.document.querySelector("main") as HTMLElement;
      const method = root.querySelector("[data-engineer-acquisition-method]") as HTMLSelectElement;
      const withoutSetpoint = { signals: { processVariable: { symbol: "P1.PV", unit: "degC" }, manipulatedVariable: { symbol: "P1.MV", unit: "%" } } } as unknown as LoopDefinition;
      const reference = new ThreePageReference(root, withoutSetpoint);
      const select = root.querySelector("select[data-identify-setpoint]") as HTMLSelectElement;
      const row = select.closest("label") as HTMLElement;
      assert.strictEqual(row.hidden, true, "no row while another method is chosen");
      assert.strictEqual(reference.signalId, undefined);
      method.value = "setpoint_step";
      method.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
      assert.strictEqual(row.hidden, false, "the Setpoint step needs the signal it moves");
      assert.deepStrictEqual(Array.from(select.options, option => [option.value, option.textContent]), [["", "Select setpoint"], ["reference", "P1.Reference (°C)"]]);
      assert.strictEqual(reference.signalId, undefined, "no signal is chosen until the engineer chooses one");
      select.value = "reference";
      assert.strictEqual(reference.signalId, "reference");
      method.value = "automatic";
      method.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
      assert.strictEqual(row.hidden, true);
      assert.strictEqual(reference.signalId, "reference",
        "the page sends the signal the engineer chose whatever the method: the engine uses it for the Setpoint step alone (Rust: a_setpoint_signal_is_used_only_by_the_setpoint_step)");
      // A loop that declares its setpoint has no selector at all.
      const second = new JSDOM(markup);
      Object.assign(globalThis, { window: second.window, document: second.window.document, Option: second.window.Option, Event: second.window.Event });
      const declared = { signals: { setpoint: { symbol: "P1.SP", unit: "degC" }, processVariable: { symbol: "P1.PV", unit: "degC" }, manipulatedVariable: { symbol: "P1.MV", unit: "%" } } } as unknown as LoopDefinition;
      new ThreePageReference(second.window.document.querySelector("main") as HTMLElement, declared);
      assert.strictEqual(second.window.document.querySelector("select[data-identify-setpoint]"), null);
      second.window.close();
    } finally { setOpenedOffers({}); Object.assign(globalThis, original); dom.window.close(); }
  });
});
