import { analyzeGeneralFeedback, decodeGeneralFeedback } from "./generalFeedback";
import { parentPort, workerData } from "worker_threads";
import { analyzeGeneralModelFrequency } from "./generalModel";
import { frequencyAt, validModelGrid } from "./generalModelPersistence";
import { decodeGeneralModel } from "./generalModel";
import { analyzeGeneralModelTime } from "./generalModelTime";
import { analyzeModelPoleView } from "./generalModelPoleView";
const model = decodeGeneralModel(workerData.model);
if (!model || !validModelGrid(workerData.grid, model.inputs.length * model.outputs.length))
  throw new Error("Invalid bounded model request.");
const frequencies = Array.from({ length: workerData.grid.points }, (_, i) => frequencyAt(workerData.grid, i));
const result = analyzeGeneralModelFrequency(model, frequencies);
const response = workerData.response ? analyzeGeneralModelTime(model, workerData.response, workerData.feedback) : undefined;
const poleView = workerData.poleView ? analyzeModelPoleView(model, workerData.poleView) : undefined;
if (workerData.feedback) {
  const definition = decodeGeneralFeedback(workerData.feedback);
  if (!definition || !validModelGrid(workerData.grid, 2 * model.inputs.length * model.outputs.length + definition.controller.inputs.length * definition.controller.outputs.length)) throw new Error("Feedback exceeds the bounded analysis contract.");
  const feedback = analyzeGeneralFeedback(definition, frequencies);
  if (feedback.status !== "qualified") throw new Error(feedback.reason);
  parentPort?.postMessage({ result, ...(response ? { response } : {}), ...(poleView ? { poleView } : {}), feedback: { definition, result: feedback } });
} else parentPort?.postMessage({ result, ...(response ? { response } : {}), ...(poleView ? { poleView } : {}) });
