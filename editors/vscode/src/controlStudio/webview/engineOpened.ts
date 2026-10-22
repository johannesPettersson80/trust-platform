import type { StudioPage } from "./threePageHostState";
import { identificationStatus, textOf } from "./engineIdentify";
import { applyStageStatuses, ENGINE_OPENED_EVENT, record, showChannelsEditor, studioFailure } from "./engineShared";
import { ENGINE_VIEW_EVENT } from "./engineInputs";
import { ENGINE_STAGE_RECEIVERS } from "./engineStages";
import { isSearchAnswer, predictedPlots, previewPayload, searchPayload, showTunePlots, tuneCandidatePayload } from "./engineTune";

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

/**
 * The Identify page's refusal of a failure: the whole text the engine gives (its detail when it has one), and the form field the engine
 * names for it (B3.3), when it names one, so that the page shows the refusal under that field.
 */
function identifyRefusal(failure: Record<string, unknown>): { reason: string; field?: string } {
  const field = textOf(failure.field);
  return { reason: textOf(failure.detail) || textOf(failure.message), ...(field ? { field } : {}) };
}

/**
 * The page that keeps the refusal of a change of the study inputs, when it is not the page of the operation shown last: the engine names the fix
 * (`nextAction`), the editor to reopen, and the refusal stays on that editor's page with its sentence and that action: Channels… and
 * Requirements… on Tune (the Tune page annex, Multichannel controller mapping), Limits… on Identify. A run that is refused for its mapping names the
 * same fix and keeps its own page. The page only places the refusal; the engine decided that it is one.
 */
function refusalPage(method: unknown, failure: Record<string, unknown>): StudioPage | undefined {
  return method === "inputs" && typeof failure.nextAction === "string" ? EDITOR_PAGES[failure.nextAction] : undefined;
}

/** The page each editor that a refused change of the study inputs reopens is on: Channels… and Requirements… on Tune, Limits… on Identify. */
const EDITOR_PAGES: Partial<Record<string, StudioPage>> = { "open-channels": "tune", "open-requirements": "tune", "open-limits": "model" };

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
      // The engine's open answer is what the page waits for, so the page is ready here.
      root.dataset.hostReady = "true";
      showChannelsEditor(root, result.channelChoices);
      // The page's own open fields reach the page as one event: the Method list, the default Limits, the project test and the state design.
      root.ownerDocument.defaultView?.dispatchEvent(new CustomEvent(ENGINE_OPENED_EVENT, { detail: result }));
      const page = PAGE_OF_ENGINE_PAGE.get(String(result.page));
      if (page) ports.select(page, false);
      applyStageStatuses(root, result.stages);
      // The tuning methods the study offers the Tune page (C4, N5.R): the loop's, and the relay rule when the study holds a qualified record.
      if (Array.isArray(result.methods)) ports.receive("engineer", { methods: result.methods, methodTitles: result.methodTitles });
      // The page's own view choices the study kept (view state, A8.1): the page shows them once the recorded inputs are shown.
      const view = record(result.pageView);
      if (view) root.ownerDocument.defaultView?.dispatchEvent(new CustomEvent(ENGINE_VIEW_EVENT, { detail: view }));
      // Each stage shows what the study holds of it (A8.6): Validate's last result, then Apply's offers, so that a reopened loop
      // with Restore available starts with Restore (B6.4).
      for (const stage of new Set(ENGINE_STAGE_RECEIVERS.values())) stage.opened?.(root, result, ports);
      return;
    }
    case "control-studio.engine.failure": {
      // A refused open is an answer too: the page is ready to show the engine's refusal, with its next action (specification 35, §0.1a).
      if (message.method === "open") root.dataset.hostReady = "true";
      const failure = record(message.failure) ?? {};
      // A refused manual preview shows its refusal in the Tune area; it is not a flow (B4.5).
      if (message.method === "preview") {
        ports.receive("tune", { status: "failed", failure: studioFailure(failure), reason: textOf(failure.message) });
        return;
      }
      const owner = refusalPage(message.method, failure);
      ports.receive("flow", { status: "failed", failure: studioFailure(failure), ...(owner ? { page: owner } : {}) });
      // A start the engine cannot take ends the Identify page's wait too, so that its command is not left running (B3.3).
      if (message.method === "identify" || message.method === "start") ports.receive("identify", { status: "failed", ...identifyRefusal(failure) });
      // A refused Tune drops the candidate the page holds (threePageTuning.ts).
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
        // The sentence is the engine's (`message` of the cancelled update, specification 35, B3, B9.4); none is written when the engine sends none.
        const sentence = textOf(update.message);
        ports.receive("flow", { status: "cancelled", flowStage, ...(sentence ? { reason: sentence } : {}) });
        if (identify) {
          ports.receive("identify", { status: "cancelled" });
          if (sentence) identificationStatus(root, sentence);
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
        if (identify) ports.receive("identify", { status: "failed", ...identifyRefusal(failure) });
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
        applyStageStatuses(root, result.stages);
        // A finite search answers its trials and the sentence it ends with, not a candidate (C5): the Tune area shows them, with the
        // stage evidence the answer carries.
        if (isSearchAnswer(result)) {
          ports.receive("engineer", searchPayload(result, record(result.stageEvidence)));
          return;
        }
        // The candidate's settings, margins and outcome, its stages on the tabs, and its predicted response on the chart (B4.3 to B4.6).
        // The candidate goes to the `engineer` stage, the stage the page's router takes for a candidate (threePageWorkflow.ts), with
        // the stage evidence the Tune answer carries.
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
