import { parentPort, workerData } from "worker_threads";
import { prepareIdentificationReview } from "./identification";

// The session owns inputs, cancellation and adoption; this worker only calculates.
parentPort?.postMessage(prepareIdentificationReview(workerData));
