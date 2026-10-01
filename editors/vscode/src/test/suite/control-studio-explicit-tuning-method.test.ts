import * as assert from "assert";
import { designPidForTask } from "../../controlStudio/pidTaskDesign";

suite("Control Studio explicit tuning method", () => {
  const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 0], delaySeconds: 0,
    revision: "integrator", source: "provided", validation: "validated", evidence: ["fixture"],
    inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
    outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
  const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };
  test("does not replace an explicit unsupported rule with pole matching", () => {
    assert.match(designPidForTask(model, "PI", task, 20).scope, /coefficient-matched/);
    assert.throws(() => designPidForTask(model, "PI", { ...task, tuningMethod: "SIMC" }, 20), /positive effective delay/);
    assert.throws(() => designPidForTask(model, "PI", { ...task, tuningMethod: "AMIGO" }, 20), /AMIGO/);
  });
});
