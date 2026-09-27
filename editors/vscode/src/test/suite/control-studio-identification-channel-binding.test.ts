import * as assert from "assert";
import * as identification from "../../controlStudio/automaticIdentification";
import type { AutomaticIdentificationResult } from "../../controlStudio/automaticIdentification";
import type { LoopChannels } from "../../controlStudio/loopChannels";
import type { GeneralModel } from "../../controlStudio/generalModel";

type Fitted = Extract<AutomaticIdentificationResult, { model: GeneralModel }>;
const channels: LoopChannels = {
  inputs: [{ modelChannelId: "command", signalId: "mv", actuator: { kind: "continuous", minimum: 0, maximum: 100 } }],
  outputs: [{ modelChannelId: "temperature", signalId: "pv" }, { modelChannelId: "secondary", signalId: "aux" }],
};
function fitted(): Fitted {
  return { model: { kind: "discrete-state-space", revision: "fitted-revision", source: "fitted", validation: "validated", evidence: ["estimation-record", "independent-record"],
    inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
    outputs: ["y", "y2"].map(id => ({ id, unit: "°C", operatingRange: { minimum: 0, maximum: 100 } })),
    states: [{ id: "x", unit: "°C", operatingRange: { minimum: -100, maximum: 100 } }],
    a: [[0.5]], b: [[0.2]], c: [[1], [1]], d: [[0], [0]], delaySeconds: 0, sampleTimeSeconds: 0.1,
    operatingPoint: { inputs: [0], outputs: [20, 20] },
  }, runs: [], fit: { comparisons: [{ order: 1, adequate: true, normalizedErrors: [0, 0], reason: "identity fixture" }],
    validationPredictions: ["y", "y2"].map(channelId => ({ channelId, unit: "°C", modelRevision: "fitted-revision",
      recordingId: "independent-record", points: [{ timeSeconds: 0.1, measured: 20, predicted: 20, residual: 0 },
        { timeSeconds: 0.2, measured: 21, predicted: 21, residual: 0 }] })),
  } };
}
function bind(result: Fitted, mapping: LoopChannels): Fitted {
  const operation = (identification as unknown as { bindIdentificationChannels?: (result: Fitted, mapping: LoopChannels) => Fitted }).bindIdentificationChannels;
  assert.strictEqual(typeof operation, "function", "channel binding must preserve the fitted evidence identity");
  return operation!(result, mapping);
}
suite("Control Studio identification channel binding", () => {
  test("keeps independent predictions attached when model channels receive configured names", () => {
    const original = fitted();
    const mapped = bind(original, channels);
    assert.deepStrictEqual(mapped.model.outputs.map(output => output.id), ["temperature", "secondary"]);
    for (const [index, series] of mapped.fit!.validationPredictions.entries()) {
      assert.strictEqual(series.modelRevision, mapped.model.revision);
      assert.strictEqual(series.channelId, mapped.model.outputs[index].id);
      assert.strictEqual(series.recordingId, "independent-record");
      assert.strictEqual(series.points, original.fit!.validationPredictions[index].points, "binding cannot refit or replace measured predictions");
    }
    assert.strictEqual(original.model.revision, "fitted-revision");
    assert.strictEqual(original.fit!.validationPredictions[0].channelId, "y");
    assert.strictEqual(bind(mapped, channels), mapped, "unchanged names cannot churn the model revision");
  });
  test("does not relabel evidence from a different model as the selected fit", () => {
    const invalid = fitted();
    invalid.fit!.validationPredictions[0].modelRevision = "different-model";
    assert.throws(() => bind(invalid, channels), /prediction.*model|model.*prediction/i);
  });
});
