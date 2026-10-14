import type { StudioPage } from "./threePageHostState";
import { identificationStatus, textOf } from "./engineIdentify";
import { applyStageStatuses, record, studioFailure } from "./engineShared";
import { ENGINE_STAGE_RECEIVERS } from "./engineStages";
import { predictedPlots, previewPayload, showTunePlots, tuneCandidatePayload } from "./engineTune";

/**
 * Presents the engine's answers on the three-page panel (specification 35, §0.1a and A11). The engine decides; this
 * module only names the pages, shows each stage's status on its tab, shows each failure with the words the engine gives
 * its next action (D10), and shows an identification the engine answered on the Identify page, and a Tune answer or a manual preview on the Tune page. Progress and failures reach
 * the page through the flow messages it already shows.
 */

/** The engine's page names, as the page names its tabs (D7). */
const PAGE_OF_ENGINE_PAGE = new Map<string, StudioPage>([
  ["Identify", "model"],
  ["Tune", "tune"],
  ["Validate", "verify"],
]);
const FLOW_STAGE_OF_COMMAND = new Map<string, string>([
  ["identify", "Identify"],
  ["tune", "Tune"],
  ["validate", "Validate"],
]);

export interface EngineHostPorts {
  /** Shows a page of the three-page panel. */
  readonly select: (page: StudioPage, persist: boolean) => void;
  /** The page's stage router, which shows `flow` progress and failures and each stage's own state. */
  readonly receive: (stage: string, payload: Record<string, unknown>) => void;
}

/** Shows one engine message. The host state has already checked its scope. */
export function receiveEngineMessage(root: HTMLElement, message: Record<string, unknown>, ports: EngineHostPorts): void {
  switch (message.kind) {
    case "control-studio.engine.opened": {
      const result = record(message.result);
      if (!result || result.kind !== "opened") return;
      // The engine's answer replaces the TypeScript restore of the page, so the page is ready here.
      root.dataset.hostReady = "true";
      const page = PAGE_OF_ENGINE_PAGE.get(String(result.page));
      if (page) ports.select(page, false);
      applyStageStatuses(root, result.stages);
      // Each stage shows what the study holds of it (A8.6): Validate's last result, then Apply's offers, so that a reopened loop
      // with Restore available starts with Restore (B6.4).
      for (const stage of new Set(ENGINE_STAGE_RECEIVERS.values())) stage.opened?.(root, result, ports);
      return;
    }
    case "control-studio.engine.failure": {
      const failure = record(message.failure) ?? {};
      // A refused manual preview shows its refusal in the Tune area; it is not a flow (B4.5).
      if (message.method === "preview") {
        ports.receive("tune", { status: "failed", failure: studioFailure(failure), reason: textOf(failure.message) });
        return;
      }
      ports.receive("flow", { status: "failed", failure: studioFailure(failure) });
      if (message.method === "identify") ports.receive("identify", { status: "failed", reason: textOf(failure.message) });
      // A refused Tune drops the candidate the page holds, as the TypeScript Tune does (threePageTuning.ts).
      if (message.method === "tune") ports.receive("engineer", { status: "failed", flowStage: "Tune", failure: studioFailure(failure), reason: textOf(failure.message) });
      ENGINE_STAGE_RECEIVERS.get(String(message.method))?.failure?.(root, failure, ports);
      return;
    }
    case "control-studio.engine.update": {
      const update = record(message.update);
      if (!update) return;
      const flowStage = FLOW_STAGE_OF_COMMAND.get(String(update.command));
      const identify = update.command === "identify";
      if (update.state === "running") {
        const progress = typeof update.progress === "string" && update.progress ? update.progress : undefined;
        ports.receive("flow", { status: "running", flowStage, reason: progress });
        if (identify) ports.receive("identify", { status: "running", reason: progress });
      } else if (update.state === "cancelled") {
        ports.receive("flow", { status: "cancelled", flowStage });
        if (identify) {
          ports.receive("identify", { status: "cancelled" });
          identificationStatus(root, "Identification cancelled.");
        }
      } else if (update.state === "finished") {
        // The result follows as `control-studio.engine.result`; this only ends the page's running state, with the evidence the
        // finished identification carries in its result.
        const evidence = identify ? record(record(update.result)?.stageEvidence) : undefined;
        ports.receive("flow", { status: "completed", flowStage, ...(evidence ? { stageEvidence: evidence } : {}) });
        if (identify) ports.receive("identify", { status: "complete" });
      } else if (update.state === "failed") {
        const failure = record(update.failure) ?? {};
        ports.receive("flow", { status: "completed", flowStage });
        ports.receive("flow", { status: "failed", failure: studioFailure(failure) });
        if (identify) ports.receive("identify", { status: "failed", reason: textOf(failure.message) });
        if (update.command === "tune") ports.receive("engineer", { status: "failed", flowStage, failure: studioFailure(failure), reason: textOf(failure.message) });
        ENGINE_STAGE_RECEIVERS.get(String(update.command))?.failure?.(root, failure, ports);
      }
      ENGINE_STAGE_RECEIVERS.get(String(update.command))?.update?.(root, update, ports);
      return;
    }
    case "control-studio.engine.result": {
      const result = record(message.result);
      if (!result) return;
      if (message.method === "tune") {
        // The candidate's settings, margins and outcome, its stages on the tabs, and its predicted response on the chart (B4.3 to B4.6).
        // The candidate goes to the `engineer` stage, the stage the page's router takes for a candidate (threePageWorkflow.ts), with
        // the stage evidence the Tune answer carries.
        applyStageStatuses(root, result.stages);
        ports.receive("engineer", tuneCandidatePayload(result, record(result.stageEvidence)));
        showTunePlots(predictedPlots(result));
        return;
      }
      if (message.method === "preview") {
        const plots = predictedPlots(result);
        ports.receive("tune", previewPayload(result, plots));
        showTunePlots(plots);
        return;
      }
      // Each stage's receiver shows its own answer (engineStages.ts), Identify's included.
      const stage = ENGINE_STAGE_RECEIVERS.get(String(message.method));
      if (stage) {
        applyStageStatuses(root, result.stages);
        stage.result?.(root, result, ports);
      }
      return;
    }
    default:
      return;
  }
}
