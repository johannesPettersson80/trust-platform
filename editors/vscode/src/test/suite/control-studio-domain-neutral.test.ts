import * as assert from "assert";
import { decodeGeneralModel } from "../../controlStudio/generalModel";

/** A heater loop in W and °C: exactly the units the removed thermal family keyed on. */
function heaterLoop() {
  const { identifyLoopDefinition } = require("../../controlStudio/loopSetup/pidLoopSetup");
  return identifyLoopDefinition({
    symbols: [{ symbol: "P1.HeaterW", type: "LREAL", address: "%QL0" }, { symbol: "P1.TemperatureC", type: "LREAL", address: "%IL0" }],
    controllers: [{ instance: "P1", type: "Main", tasks: [{ name: "MainTask", intervalMs: 100 }] }], links: [],
  }, { projectId: "project:neutral", runtimeId: "runtime-1", name: "Heater", experimentDurationSeconds: 60,
    manipulatedVariable: { symbol: "P1.HeaterW", unit: "W", minimum: 0, maximum: 2000 },
    processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 100 } });
}

suite("Control Studio domain-neutral models", () => {
  test("never offers a built-in physical model structure while setting up channels, whatever the units", async () => {
    const { configureLoopChannels } = require("../../controlStudio/configureLoopChannels");
    const loop = heaterLoop();
    const titles: string[] = [];
    const ui = {
      showQuickPick: async (items: readonly any[], options?: { title?: string }) => { titles.push(options?.title ?? ""); return items[0]; },
      showInputBox: async () => "",
    };
    assert.strictEqual(await configureLoopChannels(loop, [], [], ui), true);
    assert.ok(!titles.includes("Identification model structure"), `setup asked: ${titles.join(" | ")}`);
    assert.strictEqual(loop.physicalIdentification, undefined);
  });

  test("reads no physical-balance model: a supplied plant is a transfer function or a state-space model", () => {
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 1 } });
    assert.strictEqual(decodeGeneralModel({ kind: "physical-balance", revision: "tanks", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "m3/s")], outputs: [channel("y1", "m"), channel("y2", "m")], delaySeconds: 0,
      physical: { kind: "gravity-tank-cascade", areaM2: 0.5, outflowCoefficient: 0.02 }, operatingPoint: { inputs: [0.01], outputs: [0.25, 0.25] } }), undefined);
  });

  test("identifies with the domain-neutral models even when an older loop names a physical balance", async () => {
    const { fitRecordedStateModel } = require("../../controlStudio/recordedStateIdentification");
    const loop = { ...heaterLoop(), physicalIdentification: { kind: "thermal-balance" } };
    let job: any;
    const fitting = { run: async (_script: string, data: unknown) => { job = data; return { model: {}, comparisons: [], validationPredictions: [] }; } };
    await fitRecordedStateModel(loop, [{ run: { id: "estimation" } }], fitting, () => false);
    assert.strictEqual(job.physical, undefined, "no physical family is fitted");
  });
});
