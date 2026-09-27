import { parentPort, workerData } from "worker_threads";
import { calculateDeclaredRouth } from "./routhHurwitz";

if (!parentPort) throw new Error("Routh worker requires a parent port.");
parentPort.postMessage(calculateDeclaredRouth(workerData.definition, workerData.selected));
