import { parentPort, workerData } from "worker_threads";
import { identifyPlant, modelFitEvidence, type IdentificationSample } from "./identification";
import { provisionalIdentificationModel } from "./selectedModelAnalysis";
import { recordingFingerprint } from "./independentModelValidation";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { ControlSignalBinding } from "./loopDefinition";

const run = workerData.run as RuntimeVerificationResult;
const input = workerData.input as ControlSignalBinding, output = workerData.output as ControlSignalBinding;
// The fit starts at the steady baseline window; the recording and its evidence stay whole (B3.4).
const fitFrom = typeof workerData.fitFromSeconds === "number" ? workerData.fitFromSeconds : 0;
const samples: IdentificationSample[] = run.run.samples.filter(sample => sample.timeSeconds >= fitFrom - 1e-9).map(sample => ({ timeSeconds: sample.timeSeconds,
  input: sample.values[input.symbol]?.value ?? NaN, output: sample.values[output.symbol]?.value ?? NaN,
  quality: sample.values[input.symbol]?.quality === "good" && sample.values[output.symbol]?.quality === "good" ? "good" : "bad",
  inputUnit: input.unit, outputUnit: output.unit }));
const identified = identifyPlant(samples, [], "Simulation", undefined, workerData.maximumNormalizedError ?? 0.15,
  { declaredProcess: workerData.declaredProcess });
// A refusal is the answer, not a failed attempt: the recorded-model fit reports it, and no state model replaces it.
const refusal = identified.refusal ?? (identified.recommended.kind === "integrating" ? identified.integratorRefusal : undefined);
if (refusal !== undefined) {
  parentPort?.postMessage({ refusal });
} else {
  const channels = { inputs: [{ id: workerData.inputId ?? input.symbol, unit: input.unit, operatingRange: input.range }],
    outputs: [{ id: workerData.outputId ?? output.symbol, unit: output.unit, operatingRange: output.range }] };
  const model = provisionalIdentificationModel(identified.recommended, samples, channels, recordingFingerprint(run.run));
  const fit = modelFitEvidence(identified.recommended, samples);
  const span = Math.max(...samples.map(sample => sample.output)) - Math.min(...samples.map(sample => sample.output));
  const normalizedError = fit.rootMeanSquareError / Math.max(span, 1e-9);
  parentPort?.postMessage({ model, runs: [run], fit: {
    comparisons: [{ order: model.denominator.length - 1, normalizedErrors: [normalizedError], adequate: true,
      reason: "Step-response estimation fit; independent validation is pending." }],
    validationPredictions: [], estimationPredictions: [{ channelId: model.outputs[0].id, unit: output.unit,
      recordingId: run.run.id, modelRevision: model.revision, points: fit.points }],
  } });
}
