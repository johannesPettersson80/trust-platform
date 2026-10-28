import { announceModelChange, clearEngineIdentification, showEngineIdentification, textOf } from "./engineIdentify";
import type { EngineHostPorts } from "./engineOpened";
import { applicationReceiver, offersPayload } from "./engineApplication";
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
 * Identify's answers (specification 35, B3.8, A8.4 and A8.6), each shown by the kind the engine names (§0.1a). An identification shows
 * its model, its response plot and its requirements. A reset clears the model, ends the page's Reset, and resumes live observation on
 * the chart (the Identify page specification, Reset). An opened study shows its identification again, and passes on the stage evidence
 * the engine answers for it. An answer of another kind changes nothing.
 */
const identifyReceiver: EngineStageReceiver = {
  opened(root, opened, ports) {
    const identification = record(opened.identification);
    if (identification) showEngineIdentification(root, identification);
    // The engine's evidence for the study reaches the page: the Validate table follows whether a validation is current (A8.4,
    // engineValidate.ts). The stage actions and Apply follow the stage statuses and the offers the engine answers (B9.4, B7).
    const evidence = record(opened.stageEvidence);
    if (evidence) ports.receive("engineer", { stageEvidence: evidence });
  },
  result(root, result, ports) {
    // The engine's evidence for the answer sets the gates: an identification has a model and no candidate, a reset has neither (A8.4).
    const stageEvidence = result.stageEvidence;
    switch (result.kind) {
      case "manualIdentification": {
        // The Recorded-data fit's answer is the Identify page's manual state (B3.3): its stage, checks and primary action.
        ports.receive("identify", result);
        // The study's stage evidence the engine answers with it sets the gates: an accepted model is what Tune designs on (B3.7).
        const evidence = record(stageEvidence);
        if (evidence) ports.receive("engineer", { stageEvidence: evidence });
        // A step the engine completed may have kept a model: the Analyze… list asks which tools apply to it (B9.9).
        if (result.status === "complete") announceModelChange(root);
        return;
      }
      case "projectTest":
        // A project test whose fit the recording cannot support keeps the study's model: it clears nothing (C12).
        ports.receive("flow", { status: "completed", stageEvidence });
        return;
      case "identification":
        showEngineIdentification(root, result);
        ports.receive("flow", { status: "completed", stageEvidence });
        // The tuning methods the identified study offers (C4, N5.R): the relay rule is offered at once after a qualified relay identification.
        if (Array.isArray(result.methods)) ports.receive("engineer", { methods: result.methods, methodTitles: result.methodTitles });
        return;
      case "reset":
        clearEngineIdentification(root, textOf(result.statusSentence));
        ports.receive("flow", { status: "completed", stageEvidence });
        // The reset ends the page's Reset, and the chart resumes live observation with no model comparison (`liveIdentification`, threePageChart.ts).
        ports.receive("engineer", { status: "reset", liveIdentification: true, stageEvidence });
        return;
    }
  },
};

/**
 * Validate's answers (B5.4, B7): the table and the plot (`engineValidate.ts`), and the offers the answer states, which the Apply area
 * shows. A reopened study's Validate result shows its table; the open answer states the offers (`applicationReceiver`).
 */
const validateStage: EngineStageReceiver = {
  opened(root, opened, ports) {
    validateReceiver.opened?.(root, opened, ports);
  },
  result(root, result, ports) {
    validateReceiver.result?.(root, result, ports);
    ports.receive("engineer", offersPayload(result));
  },
};

/**
 * The answers of the study inputs (A8.4, B7): the editors and the stale results (`engineInputs.ts`), and the offers that an answer to a
 * change states, which the Apply area shows. The echo of the recorded inputs that follows an open is no answer of the engine
 * (§0.1a `inputs`): it carries the inputs alone and leaves the open answer's offers as they are.
 */
const inputsStage: EngineStageReceiver = {
  opened(root, opened, ports) {
    inputsReceiver.opened?.(root, opened, ports);
  },
  result(root, result, ports) {
    inputsReceiver.result?.(root, result, ports);
    if (typeof result.changed === "boolean") ports.receive("engineer", offersPayload(result));
  },
  failure(root, failure, ports) {
    inputsReceiver.failure?.(root, failure, ports);
  },
};

/**
 * The receivers by engine method: Identify, Tune, Validate, the study inputs, the automatic flow, and Apply, Restore and Keep. An opened
 * study's hooks run in this order, so Tune's candidate comes before Validate, and Apply's offers (Restore, Keep in project) are set last.
 */
export const ENGINE_STAGE_RECEIVERS: ReadonlyMap<string, EngineStageReceiver> = new Map<string, EngineStageReceiver>([
  ["identify", identifyReceiver],
  ["tune", tuneReceiver],
  ["validate", validateStage],
  ["inputs", inputsStage],
  ["run", runReceiver],
  ["apply", applicationReceiver],
  ["restore", applicationReceiver],
  ["keep", applicationReceiver],
]);
