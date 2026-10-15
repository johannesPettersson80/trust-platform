import * as assert from "assert";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";
import { open } from "./control-studio-loop-setup-form-harness";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

/** A tank level held by one library PID whose program interpolates its gains from a two-point schedule (Specification 35 §17.14). */
const tank = {
  symbols: [
    { symbol: "P1.LevelM", type: "LREAL", address: "%IL0" }, { symbol: "P1.SetpointM", type: "LREAL", address: "%ML0" },
    { symbol: "P1.ValvePct", type: "LREAL", address: "%QL0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
    { symbol: "P1.Auto", type: "BOOL" }, { symbol: "P1.KpLow", type: "LREAL" }, { symbol: "P1.KiLow", type: "LREAL" },
    { symbol: "P1.KpHigh", type: "LREAL" }, { symbol: "P1.KiHigh", type: "LREAL" },
    ...pidSymbols("P1.Loop", "P1.Parameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Loop", type: "Control.PID", tasks: TASK }],
  links: [
    { instance: "P1.Loop", method: "Configure", argument: "Parameters", record: "P1.Parameters", output: null, fields: {} },
    { instance: "P1.Loop", method: "Update", argument: "Input", record: "P1.Input", output: "P1.ValvePct",
      fields: { ManualOutput: ["P1.ManualPct"], ProcessValue: ["P1.LevelM"], Setpoint: ["P1.SetpointM"] } },
  ],
  writes: ["P1.ValvePct", "P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"],
};
const points = [{ name: "Low", proportionalGain: "P1.KpLow", integralGain: "P1.KiLow" }, { name: "High", proportionalGain: "P1.KpHigh", integralGain: "P1.KiHigh" }];
/** The points as the form posts them: each with its value of the scheduling variable and its tolerance (C10). */
const formPoints = [{ name: "Low", value: 0.5, tolerance: 0.1, proportionalGain: "P1.KpLow", integralGain: "P1.KiLow" },
  { name: "High", value: 1.5, tolerance: 0.1, proportionalGain: "P1.KpHigh", integralGain: "P1.KiHigh" }];
const draft = () => ({ ...scope, name: "Tank level", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
  processVariable: { symbol: "P1.LevelM", unit: "m", minimum: 0, maximum: 2 }, manipulatedVariable: { symbol: "P1.ValvePct", unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: "P1.SetpointM" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  points });

suite("Control Studio gain schedule", () => {
  test("writes one loop per operating point whose parameters are that point's gain variables", () => {
    const api = setup();
    assert.strictEqual(typeof api.gainScheduleLoopDefinitions, "function", "a gain schedule needs its own generator");
    const loops = api.gainScheduleLoopDefinitions(tank, draft());
    assert.deepStrictEqual(loops.map((loop: any) => [loop.name, loop.id]), [["Tank level · Low", "loop:tank-level-low"], ["Tank level · High", "loop:tank-level-high"]]);
    for (const [index, loop] of loops.entries()) {
      assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
      const validation: any = validateControlLoopDefinition(loop, scope);
      assert.strictEqual(validation.kind, "accepted", JSON.stringify(validation.reason ?? validation));
      const project = loop.controller.project;
      assert.deepStrictEqual([project.family, project.seed], ["gain-scheduled", { kind: "parallel-pid", family: "PI",
        proportionalSymbol: points[index].proportionalGain, integralSymbol: points[index].integralGain }]);
      assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit]),
        [[points[index].proportionalGain, "%/m"], [points[index].integralGain, "%/(m*s)"]]);
      assert.ok(project.initialization.states.some((state: any) => state.symbol === "P1.Loop.StateValue.NominalOutput"), "the PID starts bumplessly");
      assert.strictEqual(project.objective.requirementId, "target-error");
    }
  });

  test("refuses fewer than two or more than four operating points", () => {
    const api = setup();
    assert.strictEqual(typeof api.gainScheduleLoopDefinitions, "function", "a gain schedule needs its own generator");
    assert.throws(() => api.gainScheduleLoopDefinitions(tank, { ...draft(), points: points.slice(0, 1) }), /two to four/);
  });

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
