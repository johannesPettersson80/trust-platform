import { parentPort, workerData } from "worker_threads";
import { evaluateRecordedRun, compareRecordedRuns } from "./runValidation";
if (!Array.isArray(workerData.requirements) || workerData.requirements.length === 0 || workerData.requirements.length > 64) {
  throw new Error("Declare 1–64 requirements.");
}
const evaluation = evaluateRecordedRun(workerData.current, workerData.requirements);
const comparison = workerData.candidate ? compareRecordedRuns(workerData.current, workerData.candidate, workerData.requirements, workerData.objective) : undefined;
parentPort?.postMessage({ origin: "imported-record-contract", current: evaluation.run,
  ...(comparison ? { candidate: comparison.candidate.run, comparison } : {}),
  requirements: evaluation.specification, ...(workerData.objective ? { objective: workerData.objective } : {}), evaluation });
