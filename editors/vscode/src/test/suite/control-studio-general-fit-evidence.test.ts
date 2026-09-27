import * as assert from "assert";
import { identifyStateModel } from "../../controlStudio/identifiedStateModel";
import type { RecordedRun } from "../../controlStudio/runValidation";

// Synthetic numerical fixture, not runtime or physical acceptance evidence.
function recording(id: string, phase: number): RecordedRun {
  let temperature = 20, pressure = 5;
  const samples = Array.from({ length: 160 }, (_, index) => {
    const input = index === 0 ? 0 : ((index * (17 + phase) % 31) / 10);
    const row = { sequence: index, timeSeconds: index / 10, values: {
      command: { value: input, quality: "good" as const },
      temperature: { value: temperature, quality: "good" as const },
      pressure: { value: pressure, quality: "good" as const },
    } };
    temperature = 20 + 0.9 * (temperature - 20) + 0.2 * input;
    pressure = 5 + 0.8 * (pressure - 5) + 0.1 * input;
    return row;
  });
  return { id, evidence: "ST-executed", parameters: {}, channels: [{ id: "command", unit: "%" }, { id: "temperature", unit: "°C" }, { id: "pressure", unit: "bar" }],
    conditions: { programRevision: "same-program", modelRevision: "same-plant", parameterSchemaRevision: "same-parameters",
      initialState: {}, events: [], durationSeconds: 15.9, seed: phase,
      sampleTimesSeconds: samples.map(sample => sample.timeSeconds) }, samples };
}
const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 100 } });

suite("Control Studio general identification fit evidence", () => {
  test("retains measured-minus-predicted residuals for independent measurement noise", () => {
    const validation = recording("noisy-validation", 4);
    validation.samples[50].values.temperature.value! += 0.05;
    const result: any = identifyStateModel(recording("estimation", 0), validation, {
      inputs: [channel("command", "%")], outputs: [channel("temperature", "°C"), channel("pressure", "bar")],
    }, 0.01, 3);
    const point = result.validationPredictions[0].points.find((point: any) => point.timeSeconds === 5);
    assert.ok(point.residual > 0.049 && point.residual < 0.051,
      "a positive measurement noise spike must have a positive residual, matching the existing model-fit contract");
  });
  test("retains the selected model's independent free predictions in each output coordinate", () => {
    const validation = recording("validation", 4);
    const result: any = identifyStateModel(recording("estimation", 0), validation, {
      inputs: [channel("command", "%")], outputs: [channel("temperature", "°C"), channel("pressure", "bar")],
    }, 0.0001, 3);
    assert.ok(Array.isArray(result.validationPredictions), "retain the exact independent predictions already used to qualify the selected model");
    assert.deepStrictEqual(result.validationPredictions.map((entry: any) => [entry.channelId, entry.unit]), [["temperature", "°C"], ["pressure", "bar"]]);
    for (const output of result.validationPredictions) {
      assert.strictEqual(output.recordingId, validation.id);
      assert.strictEqual(output.modelRevision, result.model.revision);
      assert.ok(output.points.length > 100);
      assert.ok(output.points[0].timeSeconds > 0, "measured history used to initialize the predictor is not labelled as prediction");
      for (const point of output.points) {
        const sample = validation.samples.find(sample => sample.timeSeconds === point.timeSeconds)!;
        assert.strictEqual(point.measured, sample.values[output.channelId].value);
        assert.ok(Math.abs(point.predicted - point.measured) < 1e-6, "predictions remain in absolute physical coordinates");
        assert.ok(Math.abs(point.residual - (point.measured - point.predicted)) < 1e-12);
      }
    }
  });
});
