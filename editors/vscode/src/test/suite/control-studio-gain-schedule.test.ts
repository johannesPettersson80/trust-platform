import * as assert from "assert";
import { open } from "./control-studio-loop-setup-form-harness";

/** A tank level held by one library PID whose program interpolates its gains from a two-point schedule (Specification 35 §17.14). */


/** The points as the form posts them: each with its value of the scheduling variable and its tolerance (C10). */



/** The points as the form posts them: every operating-point row the template draws (the engine's four), each number as the text typed;
 * the engine takes the rows that have an entry (B1.3, C10). */
const blankPoint = { name: "", value: "", tolerance: "", proportionalGain: "", integralGain: "" };
const formPoints = [{ name: "Low", value: "0.5", tolerance: "0.1", proportionalGain: "P1.KpLow", integralGain: "P1.KiLow" },
  { name: "High", value: "1.5", tolerance: "0.1", proportionalGain: "P1.KpHigh", integralGain: "P1.KiHigh" }, blankPoint, blankPoint];

suite("Control Studio gain schedule", () => {

  test("sets up the schedule's points in the form", () => {
    const view = open({ structure: "pid", addressedReal: ["P1.LevelM", "P1.ManualPct", "P1.SetpointM", "P1.ValvePct"], booleans: ["P1.Auto"], modes: ["P1.Auto"],
      records: ["P1.Parameters"], writes: ["P1.ValvePct", "P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"], initializes: [],
      pids: [{ instance: "P1.Loop", task: { name: "MainTask", intervalMs: 100 } }], instances: ["P1"],
      reals: ["P1.KiHigh", "P1.KiLow", "P1.KpHigh", "P1.KpLow", "P1.LevelM", "P1.ManualPct", "P1.SetpointM", "P1.ValvePct"],
      prefill: { instance: "P1.Loop", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.ValvePct", setpoint: "P1.SetpointM", manualOutput: "P1.ManualPct" } });
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("schedule"), `a gain schedule has its own structure: ${structures}`);
      for (const [field, value] of [["structure", "schedule"], ["name", "Tank level"], ["processVariable", "P1.LevelM"], ["processVariable.unit", "m"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "2"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"], ["mode", "P1.Auto"], ["schedule.variable", "P1.LevelM"], ["schedule.variable.unit", "m"]]) view.set(field, value);
      for (const [field, value] of [["schedule.0.name", "Low"], ["schedule.0.value", "0.5"], ["schedule.0.tolerance", "0.1"],
        ["schedule.0.proportionalGain", "P1.KpLow"], ["schedule.0.integralGain", "P1.KiLow"],
        ["schedule.1.name", "High"], ["schedule.1.value", "1.5"], ["schedule.1.tolerance", "0.1"],
        ["schedule.1.proportionalGain", "P1.KpHigh"], ["schedule.1.integralGain", "P1.KiHigh"]]) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, "the engine, not the form, checks the points");
      save.click();
      assert.deepStrictEqual(view.posts[0]?.draft?.points, formPoints);
      assert.deepStrictEqual(view.posts[0]?.draft?.scheduling, { symbol: "P1.LevelM", unit: "m" });
      assert.strictEqual(view.posts[0]?.draft?.structure, "schedule");
    } finally { view.close(); }
  });
});
