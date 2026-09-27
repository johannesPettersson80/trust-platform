import { identifyPhysicalModel } from "./physicalIdentification";
import { parentPort, workerData } from "worker_threads";
import { identifyStateModel } from "./identifiedStateModel";
parentPort?.postMessage(workerData.physical
  ? identifyPhysicalModel(workerData.estimation, workerData.validation, workerData.channels, workerData.physical, workerData.maximumNormalizedError)
  : identifyStateModel(workerData.estimation, workerData.validation, workerData.channels, workerData.maximumNormalizedError, workerData.maximumOrder));
