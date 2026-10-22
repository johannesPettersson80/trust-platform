import { showEngineIdentification } from "./engineIdentify";
import type { EngineHostPorts } from "./engineOpened";
import { record } from "./engineShared";
import type { EngineStageReceiver } from "./engineStages";

/**
 * Shows the engine's automatic flow (specification 35, B8 and §0.1a `run`): the stage the run is in, selected on its tab while it
 * runs; each stage's status on its tab, the identification the run made on the Identify page, and the input the run waits for.
 * The page takes the stage evidence the engine answers for its gates; it computes none of these values.
 */
export const runReceiver: EngineStageReceiver = {
  /** Follows the stage the run's update names (`stage`, B8): the page selects that stage's tab and shows the run's progress there. */
  update(_root, update, ports: EngineHostPorts) {
    const flowStage = typeof update.stage === "string" ? update.stage : undefined;
    if (update.state !== "running" || !flowStage) return;
    const reason = typeof update.progress === "string" && update.progress ? update.progress : undefined;
    ports.receive("flow", { status: "running", flowStage, reason });
  },
  result(root, result, ports: EngineHostPorts) {
    const identification = record(result.identification);
    // A project test whose fit the recording cannot support answers no model (C12): the model the page shows stays.
    if (identification && typeof identification.modelType === "string") showEngineIdentification(root, identification);
    const needs = typeof result.needs === "string" ? result.needs : undefined;
    if (needs) {
      // The page opens the editor that owns the input, and Done runs the flow again (B8). The sentence that asks for the input, and the field
      // it concerns, are the engine's (`needsMessage`, `needsField`); the page writes none.
      ports.receive("flow", {
        status: "paused",
        state: "paused",
        ...(typeof result.needsMessage === "string" ? { reason: result.needsMessage } : {}),
        ...(typeof result.needsField === "string" ? { missingField: result.needsField } : {}),
        missing: needs,
        stageEvidence: result.stageEvidence,
      });
      return;
    }
    ports.receive("flow", { status: "completed", stageEvidence: result.stageEvidence });
  },
};
