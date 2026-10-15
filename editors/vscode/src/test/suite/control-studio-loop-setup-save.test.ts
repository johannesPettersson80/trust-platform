import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** Loaded dynamically so that a missing save owner fails at the behaviour assertion. */
function saving(): any {
  try { return require("../../controlStudio/loopSetup/loopSetupSave"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const PARAMETER_FIELDS = ["ProportionalGain", "IntegralGain", "DerivativeGain", "ProportionalSetpointWeight", "DerivativeSetpointWeight",
  "Bias", "OutputMinimum", "OutputMaximum", "RateMinimum", "RateMaximum", "TrackingTime"];
const STATE_FIELDS = ["Setpoint", "SetpointRate", "ProcessValue", "ProcessValueRate", "FeedForward", "NominalOutput", "SaturatedOutput"];
const catalog = {
  symbols: [
    { symbol: "P1.VesselC", type: "LREAL", address: "%IL0" }, { symbol: "P1.JacketC", type: "LREAL", address: "%IL8" },
    { symbol: "P1.VesselSetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.JacketSetpointC", type: "LREAL", address: "%ML8" },
    { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" }, { symbol: "P1.OuterAuto", type: "BOOL" }, { symbol: "P1.InnerAuto", type: "BOOL" },
    { symbol: "P1.OuterManual", type: "LREAL", address: "%ML16" }, { symbol: "P1.InnerManual", type: "LREAL", address: "%ML24" },
    ...["Outer", "Inner"].flatMap(name => [...PARAMETER_FIELDS.map(field => ({ symbol: `P1.${name}Parameters.${field}`, type: "LREAL" })),
      ...STATE_FIELDS.map(field => ({ symbol: `P1.${name}.StateValue.${field}`, type: "LREAL" }))]),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Inner", type: "Control.PID", tasks: TASK }, { instance: "P1.Outer", type: "Control.PID", tasks: TASK }],
  links: [{ instance: "P1.Outer", method: "Update", argument: "Input", record: "P1.OuterInput", output: "P1.JacketSetpointC", fields: { Setpoint: ["P1.VesselSetpointC"] } },
    { instance: "P1.Inner", method: "Update", argument: "Input", record: "P1.InnerInput", output: "P1.HeaterPct", fields: { Setpoint: ["P1.JacketSetpointC"] } }],
};
const scope = { projectId: "project:save", runtimeId: "runtime-1" };
const signal = (symbol: string, unit: string, minimum: number, maximum: number) => ({ symbol, unit, minimum, maximum });

suite("Control Studio loop setup saving", () => {
  test("writes an identify-only loop that records the chosen plant input and output", async () => {
    const api = saving();
    assert.strictEqual(typeof api.saveLoopSetup, "function");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-loop-setup-"));
    try {
      const [request] = await api.saveLoopSetup(root, scope, catalog, { structure: "identify", name: "Jacket response",
        manipulatedVariable: signal("P1.HeaterPct", "%", 0, 100), processVariable: signal("P1.JacketC", "degC", 0, 90), experimentDurationSeconds: 120 });
      const loop = request.loopDefinition;
      assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
      assert.deepStrictEqual([loop.topology, loop.controller.instance, loop.controller.type, loop.signals.setpoint], ["open-loop", "P1", "Main", undefined]);
      assert.deepStrictEqual([loop.signals.processVariable.address, loop.signals.manipulatedVariable.address], ["%IL8", "%QL0"]);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
