import { parentPort, workerData } from "worker_threads";
import { validateFrozenModel } from "./independentModelValidation";
parentPort?.postMessage(validateFrozenModel(workerData.model, workerData.estimation, workerData.record,
  workerData.channels, workerData.maximumNormalizedError));
