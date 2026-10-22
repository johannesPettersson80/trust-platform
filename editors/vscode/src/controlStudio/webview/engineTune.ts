/**
 * Shows the engine's answers to Tune and to the manual preview on the Tune page (specification 35, B4.3 to B4.6 and B9.6), and the
 * candidate an opened study holds (A8.6). The engine decides every value. This module only renames the answer's fields to the names
 * the page's renderers read: the candidate's settings under the loop's symbols, its margins, its outcome and notes, the requirements it
 * used, and its predicted response for the chart. It computes none of them.
 */
import type { EngineStageReceiver } from "./engineStages";
import { textOf } from "./engineIdentify";
import { OPENED_OFFERS_EVENT, setOpenedOffers, type OpenedOffers } from "./openedOffers";

type Fields = Record<string, unknown>;

/**
 * The candidate the Tune page shows, in the names that threePageTuning.ts and threePageResults.ts read (B4.3 to B4.6). A Tune answer
 * passes its own stage evidence, which the engine computed for it (B9.4); an opened candidate passes none, because the open answer's
 * evidence sets the gates before it (`engineStages.ts`). A program's own law answers with `projectCandidate` and no `candidate` (C2): its
 * structure is the family, and its values by variable are the values the page shows. A measured-disturbance feedforward's answer (C8) goes
 * under `design.feedforward`, the field the page's feedforward view reads, as the engine gives it. A state law's answer (C4) goes under
 * `state`, which a live Tune and a reopened study both carry, so both show it the same way. A PID group's answer (C7) goes under
 * `design.group`, and a gain schedule's (C10) under `design.schedule`, which structureResults.ts reads as the engine gives them.
 */
export function tuneCandidatePayload(result: Fields, stageEvidence?: Fields): Fields {
  const project = record(result.projectCandidate);
  const candidate = record(result.candidate) ?? project;
  const state = record(result.state);
  const margins = record(result.margins);
  const marginsUnavailable = textOf(result.marginsUnavailable);
  const feedforward = record(result.feedforward);
  const group = record(result.group);
  const schedule = record(result.schedule);
  // The engine's words of the answer (TuneWords, tune_words.rs): its margins in words, its outcome sentence and its setting rows.
  const words = record(result.words);
  return {
    status: "candidate",
    family: textOf(candidate?.structure) || textOf(state?.family),
    // A Control.PID candidate's values are its settings; a program's law gives them by variable in `projectCandidate.values`.
    values: record(project?.values ?? result.settings) ?? {},
    ...(margins ? { robustness: robustnessOf(margins, result.delayMarginSeconds, words) } : {}),
    ...(typeof result.methodDescription === "string" ? { methodDescription: result.methodDescription } : {}),
    outcome: record(result.outcome),
    ...(words ? { words } : {}),
    comparison: arrayOf(result.comparison),
    unavailable: marginsUnavailable ? [marginsUnavailable] : [],
    task: record(result.requirements),
    ...(feedforward || group || schedule ? { design: {
      ...(feedforward ? { feedforward } : {}), ...(group ? { group } : {}), ...(schedule ? { schedule } : {}),
    } } : {}),
    ...(state ? { state } : {}),
    ...(stageEvidence ? { stageEvidence } : {}),
  };
}

/**
 * The study's candidate on an opened loop (A8.6): the `tune` the open answer holds, shown as a Tune answer is shown. The candidate adds
 * no stage evidence: the open answer's evidence, which Identify's receiver passes first, sets the gates. It draws a predicted response
 * only when it has one; the open answer keeps no prediction, and its outcome gives the reason. The open answer's stage statuses stay.
 */
export const tuneReceiver: EngineStageReceiver = {
  opened(_root, opened, ports) {
    // The offers the engine made on open (C2, C4, B4.3): the controller that the Tune page offers with the rows of its manual editor, the
    // setpoints a loop without one may take, and the scenario template. The controller reads the offer here; the editors read the store.
    const offers: OpenedOffers = {
      controllerOffer: (opened.controllerOffer ?? null) as OpenedOffers["controllerOffer"],
      setpointChoices: (opened.setpointChoices ?? null) as OpenedOffers["setpointChoices"],
      scenarioTemplate: (opened.scenarioTemplate ?? null) as OpenedOffers["scenarioTemplate"],
      scenarioRowSignal: (opened.scenarioRowSignal ?? null) as OpenedOffers["scenarioRowSignal"],
      stateDeclaredText: (opened.stateDeclaredText ?? null) as OpenedOffers["stateDeclaredText"],
      liveSides: (opened.liveSides ?? null) as OpenedOffers["liveSides"],
      requirementOutputs: (opened.requirementOutputs ?? null) as OpenedOffers["requirementOutputs"],
    };
    setOpenedOffers(offers);
    // Editors built before the answer listen on the page's window; a page test without a window has none to tell.
    if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent(OPENED_OFFERS_EVENT));
    ports.receive("engineer", { controllerOffer: offers.controllerOffer });
    // A kept finite search reopens as its trials (C5, A8.6). Its open answer's candidate holds no law, so the trials are what the page shows.
    const search = record(opened.searchCandidate);
    if (search) {
      ports.receive("engineer", searchPayload(search));
      return;
    }
    const tune = record(opened.tune);
    if (!tune) return;
    ports.receive("engineer", tuneCandidatePayload(tune));
    const plots = predictedPlots(tune);
    if (plots) showTunePlots(plots);
  },
};

/** Whether a Tune answer is a finite search's (C5), by the kind the engine names (`searchAnswer`, §0.1a). */
export function isSearchAnswer(result: Fields): boolean {
  return result.kind === "searchAnswer";
}

/**
 * A finite search's answer, or the kept candidate a reopened study holds (C5), as the Tune area reads it: its status and sentence when it
 * did not complete (a kept candidate has neither), its selected trial, the unit of J and its trials in the engine's order. It computes none of them.
 */
export function searchPayload(result: Fields, stageEvidence?: Fields): Fields {
  return {
    status: "search",
    search: {
      ...(typeof result.status === "string" ? { status: result.status } : {}),
      ...(typeof result.message === "string" ? { message: result.message } : {}),
      ...(typeof result.selected === "number" ? { selected: result.selected } : {}),
      scoreUnit: textOf(result.scoreUnit),
      trials: arrayOf(result.trials),
    },
    ...(stageEvidence ? { stageEvidence } : {}),
  };
}

/** The page's answer to a manual preview (B4.5): the engine's reason, or why it has no predicted response. */
export function previewPayload(result: Fields, plots: Fields | undefined): Fields {
  return { status: "preview", reason: plots ? textOf(result.reason) : textOf(result.predictionUnavailable) || textOf(result.reason) };
}

/**
 * The predicted response on its time base, in the plots shape that the chart draws (B4.5, B9.6): the candidate's measurement and
 * command, and the current settings' measurement and command when the engine predicts them beside it (the traces `current-measurement`
 * and `current-command`). Undefined without the candidate's measurement.
 */
export function predictedPlots(result: Fields): Fields | undefined {
  const response = record(result.response);
  const times = arrayOf(response?.timeSeconds);
  const traces = arrayOf(response?.traces).map(record);
  const valuesOf = (role: string): unknown[] | undefined => {
    const trace = traces.find(item => item?.role === role);
    return trace ? arrayOf(trace.values) : undefined;
  };
  const candidatePv = valuesOf("measurement");
  if (!candidatePv || times.length === 0) return undefined;
  const candidateMv = valuesOf("command"), currentPv = valuesOf("current-measurement"), currentMv = valuesOf("current-command");
  return { timeResponse: times.map((timeSeconds, index) => ({
    timeSeconds,
    candidatePv: candidatePv[index] ?? null,
    ...(candidateMv ? { candidateMv: candidateMv[index] ?? null } : {}),
    ...(currentPv ? { currentPv: currentPv[index] ?? null } : {}),
    ...(currentMv ? { currentMv: currentMv[index] ?? null } : {}),
  })) };
}

/** Draws a predicted response on the Tune chart, which reads the plots of the engineer's events (B9.6). */
export function showTunePlots(plots: Fields | undefined): void {
  if (plots && typeof window !== "undefined") window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: { plots } }));
}

/** The candidate's margins as the page shows them: each number, or the engine's word for a margin that is not a number (B9.5). */
function robustnessOf(margins: Fields, delay: unknown, words: Fields | undefined): Fields {
  return {
    gainMarginDb: marginOf(margins.gainMarginDb, words?.gainMargin),
    phaseMarginDegrees: marginOf(margins.phaseMarginDegrees, words?.phaseMargin),
    delayMarginSeconds: marginOf(delay, words?.delayMargin),
    peakSensitivity: margins.peakSensitivity,
  };
}

/** A margin as the page shows it: its number when it is finite, otherwise the word the engine gives it (TuneWords, B9.5). */
function marginOf(value: unknown, word: unknown): unknown {
  const finite = record(value)?.finite;
  return typeof finite === "number" ? finite : word;
}

function record(value: unknown): Fields | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Fields) : undefined;
}

function arrayOf(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}
