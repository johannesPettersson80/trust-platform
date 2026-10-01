import { parentPort, workerData } from "worker_threads";
import { identifyStateModel } from "./identifiedStateModel";
parentPort?.postMessage(identifyStateModel(workerData.estimation, workerData.validation, workerData.channels, workerData.maximumNormalizedError, workerData.maximumOrder));
