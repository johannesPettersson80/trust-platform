/**
 * Shows the engine's answers to Tune and to the manual preview on the Tune page (specification 35, B4.3 to B4.6 and B9.6), and the
 * candidate an opened study holds (A8.6). The engine decides every value. This module only renames the answer's fields to the names
 * the page's renderers read: the candidate's settings under the loop's symbols, its margins, its outcome and notes, the requirements it
 * used, and its predicted response for the chart. It computes none of them.
 */
import type { EngineStageReceiver } from "./engineStages";
import { textOf } from "./engineIdentify";

type Fields = Record<string, unknown>;

/**
 * The candidate the Tune page shows, in the names that threePageTuning.ts and threePageResults.ts read (B4.3 to B4.6). A Tune answer
 * passes its own stage evidence, which the engine computed for it (B9.4); an opened candidate passes none, because the open answer's
 * evidence sets the gates before it (`engineStages.ts`). A program's own law answers with `projectCandidate` and no `candidate` (C2): its
 * structure is the family, and its values by variable are the values the page shows. A measured-disturbance feedforward's answer (C8) goes
 * under `design.feedforward`, the field the page's feedforward view reads, as the engine gives it.
 */
export function tuneCandidatePayload(result: Fields, stageEvidence?: Fields): Fields {
  const project = record(result.projectCandidate);
  const candidate = record(result.candidate) ?? project;
  const margins = record(result.margins);
  const marginsUnavailable = textOf(result.marginsUnavailable);
  const feedforward = record(result.feedforward);
  return {
    status: "candidate",
    family: textOf(candidate?.structure),
    // A Control.PID candidate's values are its settings; a program's law gives them by variable in `projectCandidate.values`.
    values: record(project?.values ?? result.settings) ?? {},
    ...(margins ? { robustness: robustnessOf(margins, result.delayMarginSeconds) } : {}),
    ...(typeof result.methodDescription === "string" ? { methodDescription: result.methodDescription } : {}),
    outcome: record(result.outcome),
    comparison: arrayOf(result.comparison),
    unavailable: marginsUnavailable ? [marginsUnavailable] : [],
    task: record(result.requirements),
    ...(feedforward ? { design: { feedforward } } : {}),
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
    const tune = record(opened.tune);
    if (!tune) return;
    ports.receive("engineer", tuneCandidatePayload(tune));
    const plots = predictedPlots(tune);
    if (plots) showTunePlots(plots);
  },
};

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
  if (plots) window.dispatchEvent(new CustomEvent("trust-control-studio-engineer", { detail: { plots } }));
}

function robustnessOf(margins: Fields, delay: unknown): Fields {
  return {
    gainMarginDb: marginOf(margins.gainMarginDb),
    phaseMarginDegrees: marginOf(margins.phaseMarginDegrees),
    delayMarginSeconds: marginOf(delay),
    peakSensitivity: margins.peakSensitivity,
  };
}

/** A margin as the page shows it: its number, or the word for an unbounded margin or one the grid does not show (B9.5). */
function marginOf(value: unknown): unknown {
  const finite = record(value)?.finite;
  if (typeof finite === "number") return finite;
  return value === "unbounded" ? "unbounded" : "not observed in range";
}

function record(value: unknown): Fields | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Fields) : undefined;
}

function arrayOf(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}
