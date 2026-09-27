import { parentPort, workerData } from "worker_threads";
import { prepareIdentificationReview } from "./identification";

// Both independent records already exist; do not gate their joint fit on a
// coarse same-run final-value preview that assumes the first trial has settled.
const review = prepareIdentificationReview({ estimation: workerData.estimation, validation: workerData.validation,
  excluded: [], source: "Simulation", requestedKind: "automatic",
  maximumNormalizedError: workerData.maximumNormalizedError ?? 0.15,
  estimationRecordingId: workerData.estimationRecordingId, validationRecordingId: workerData.validationRecordingId });
if (review.stage !== "acceptance_required") throw new Error("Independent validation is required before model selection.");
parentPort?.postMessage({ model: review.selected, fit: review.fit, evidence: review.result.source, review });
