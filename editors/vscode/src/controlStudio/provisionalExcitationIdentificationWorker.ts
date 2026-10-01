import { parentPort, workerData } from "worker_threads";
import type { IdentificationSample } from "./identification";
import { identifyPlantFromExcitation } from "./excitationIdentification";
import { provisionalIdentificationModel } from "./selectedModelAnalysis";
import { recordingFingerprint } from "./independentModelValidation";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { ControlSignalBinding } from "./loopDefinition";

const run = workerData.run as RuntimeVerificationResult;
const input = workerData.input as ControlSignalBinding, output = workerData.output as ControlSignalBinding;
const samples: IdentificationSample[] = run.run.samples.map(sample => ({ timeSeconds: sample.timeSeconds,
  input: sample.values[input.symbol]?.value ?? NaN, output: sample.values[output.symbol]?.value ?? NaN,
  quality: sample.values[input.symbol]?.quality === "good" && sample.values[output.symbol]?.quality === "good" ? "good" : "bad",
  inputUnit: input.unit, outputUnit: output.unit }));
const fit = identifyPlantFromExcitation(samples, workerData.maximumNormalizedError);
const channels = { inputs: [{ id: workerData.inputId ?? input.symbol, unit: input.unit, operatingRange: input.range }],
  outputs: [{ id: workerData.outputId ?? output.symbol, unit: output.unit, operatingRange: output.range }] };
const model = provisionalIdentificationModel(fit.recommended, samples, channels, recordingFingerprint(run.run), fit);
parentPort?.postMessage({ model, runs: [run], fit: {
  comparisons: [{ order: model.denominator.length - 1, normalizedErrors: [fit.recommended.normalizedError], adequate: true,
    reason: "Estimation fit to the recorded excitation; independent validation is pending." }],
  validationPredictions: [], estimationPredictions: [{ channelId: model.outputs[0].id, unit: output.unit,
    recordingId: run.run.id, modelRevision: model.revision, points: fit.points }],
} });
