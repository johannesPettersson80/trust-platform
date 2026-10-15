import { clearEngineIdentification, showEngineIdentification } from "./engineIdentify";
import type { EngineHostPorts } from "./engineOpened";
import { applicationReceiver } from "./engineApplication";
import { inputsReceiver } from "./engineInputs";
import { record } from "./engineShared";
import { runReceiver } from "./engineRun";
import { tuneReceiver } from "./engineTune";
import { validateReceiver } from "./engineValidate";

/**
 * How the page shows one stage's engine answers (specification 35, §0.1a and A11). Each stage's module renders its own result and
 * the parts of a failure beyond the flow failure that the page always shows; the engine decides, the module only presents.
 */
export interface EngineStageReceiver {
  /**
   * Shows what an opened loop holds of the stage (`control-studio.engine.opened`, A8.6): the answer of `controlStudio/open`, of which
   * the stage takes its own fields. A stage with nothing to show in an opened loop has no hook.
   */
  opened?(root: HTMLElement, opened: Record<string, unknown>, ports: EngineHostPorts): void;
  /** Shows the stage's result (`control-studio.engine.result` with the stage's method). */
  result?(root: HTMLElement, result: Record<string, unknown>, ports: EngineHostPorts): void;
  /** Shows the stage's failure (`control-studio.engine.failure`, or a failed update of the stage's operation). */
  failure?(root: HTMLElement, failure: Record<string, unknown>, ports: EngineHostPorts): void;
  /** Shows one update of the stage's operation (`control-studio.engine.update`): its progress, or its end when cancelled. */
  update?(root: HTMLElement, update: Record<string, unknown>, ports: EngineHostPorts): void;
}

/**
 * Identify's answers (specification 35, B3.8, A8.4 and A8.6). An identification shows its model, its response plot and its
 * requirements. A reset clears the model, ends the page's Reset, and resumes live observation on the chart (the Identify page
 * specification, Reset). An opened study shows its identification again, and passes the stage evidence the engine answers for it,
 * so that the page's gates follow the engine.
 */
const identifyReceiver: EngineStageReceiver = {
  opened(root, opened, ports) {
    const identification = record(opened.identification);
    if (identification) showEngineIdentification(root, identification);
    // The engine's evidence for the study sets the gates (Tune needs a model, Validate a candidate, Apply a pass); the page computes none of it.
    const evidence = record(opened.stageEvidence);
    if (evidence) ports.receive("engineer", { stageEvidence: evidence });
  },
  result(root, result, ports) {
    // The Recorded-data fit's answer is the Identify page's manual state (B3.3): its stage, checks and primary action, not a model or a reset.
    if (typeof result.primaryAction === "string") {
      ports.receive("identify", result);
      // The study's stage evidence the engine answers with it sets the gates: an accepted model is what Tune designs on (B3.7).
      const evidence = record(result.stageEvidence);
      if (evidence) ports.receive("engineer", { stageEvidence: evidence });
      return;
    }
    // The engine's evidence for the answer sets the gates: an identification has a model and no candidate, a reset has neither (A8.4).
    const stageEvidence = result.stageEvidence;
    // A project test whose fit the recording cannot support keeps the study's model: it clears nothing and is not a reset (C12).
    if (typeof result.fitRefusal === "string") {
      ports.receive("flow", { status: "completed", stageEvidence });
      return;
    }
    // An identification shows its model; a reset (which has no model type) clears the model the page shows.
    if (typeof result.modelType === "string") {
      showEngineIdentification(root, result);
      ports.receive("flow", { status: "completed", stageEvidence });
      return;
    }
    clearEngineIdentification(root);
    ports.receive("flow", { status: "completed", stageEvidence });
    // The reset ends the page's Reset, and the chart resumes live observation with no model comparison (`liveIdentification`, threePageChart.ts).
    ports.receive("engineer", { status: "reset", liveIdentification: true, stageEvidence });
  },
};

/**
 * The receivers by engine method: Identify, Tune, Validate, the study inputs, the automatic flow, and Apply, Restore and Keep. An opened
 * study's hooks run in this order, so Tune's candidate comes before Validate, and Apply's offers (Restore, Keep in project) are set last.
 */
export const ENGINE_STAGE_RECEIVERS: ReadonlyMap<string, EngineStageReceiver> = new Map<string, EngineStageReceiver>([
  ["identify", identifyReceiver],
  ["tune", tuneReceiver],
  ["validate", validateReceiver],
  ["inputs", inputsReceiver],
  ["run", runReceiver],
  ["apply", applicationReceiver],
  ["restore", applicationReceiver],
  ["keep", applicationReceiver],
]);
