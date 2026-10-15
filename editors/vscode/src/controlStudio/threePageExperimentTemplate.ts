import type { ControlLoopDefinition } from "./loopDefinition";
import { displayEngineeringUnit } from "./chartContract";

export const PROJECT_EXPERIMENT_METHOD = "project";

/** Project-owned Test is available only when the loop declares its execution owner. */
export function hasProjectExperiment(loop: ControlLoopDefinition | undefined): boolean {
  return loop?.experiment?.execution === "project-scripted";
}

/** The experiment's own declared settings, which the project test on the Identify page shows (B6.1). A declared analysis has its own tool. */
export function hasExperimentParameters(loop: ControlLoopDefinition | undefined): boolean {
  return hasProjectExperiment(loop) && loop?.parameterPurpose === "experiment" && !hasAnalysisParameters(loop) && Boolean(loop?.parameterBinding);
}

/** Whether the loop's declared analysis maps parameters that the page reviews and writes through the engine (B6.1, C12). */
export function hasAnalysisParameters(loop: ControlLoopDefinition | undefined): boolean {
  return loop?.analysisParameterSymbols !== undefined;
}

/** The parameter tool of a declared analysis, which the Tune page shows after the analysis it serves (the course's Tune → Root locus step). */
export function hasAnalysisParameterTool(loop: ControlLoopDefinition | undefined): boolean {
  return hasProjectExperiment(loop) && hasAnalysisParameters(loop) && Boolean(loop?.parameterBinding);
}

/**
 * Contextual Test controls for the connected three-page workflow.
 *
 * The project owns excitation, reset and restoration. The workspace therefore
 * presents the declared plan and signal identities while the existing host
 * experiment handlers retain review, approval, runtime recording and recovery.
 */
export function renderThreePageExperimentTemplate(
  loop: ControlLoopDefinition,
  escape: (value: string) => string,
): string {
  if (!hasProjectExperiment(loop)) return "";
  const experiment = loop.experiment!;
  const duration = Number.isFinite(experiment.durationSeconds) ? experiment.durationSeconds : 30;
  const baseline = typeof experiment.baselineSeconds === "number" && Number.isFinite(experiment.baselineSeconds) ? experiment.baselineSeconds : 0;
  const signals = declaredExperimentSignals(loop);
  const options = signals.map(signal => `<option value="${escape(signal.id)}"${signal.selected ? " selected" : ""}>${escape(signal.label)}</option>`).join("");
  const parameters = hasExperimentParameters(loop) ? parameterTool(loop) : "";
  return `<section class="studio-experiment" data-three-page-experiment hidden aria-label="Project experiment">
 <div class="studio-experiment-fields">
  <label class="studio-field studio-experiment-signals">Signals<select data-three-page-experiment-signals multiple size="4" aria-label="Declared experiment signals">${options}</select></label>
 </div>
 ${parameters}
 <p class="studio-action-message" data-three-page-experiment-status role="status" hidden></p>
 <dl class="studio-experiment-plan" data-three-page-experiment-plan hidden>
  <dt>Test signal</dt><dd data-three-page-experiment-bound></dd>
  <dt>Duration</dt><dd data-three-page-experiment-duration>${escape(String(duration))} s${baseline > 0 ? ` · ${escape(String(baseline))} s baseline` : ""}</dd>
  <dt>After test</dt><dd data-three-page-experiment-restoration></dd>
  <dt>Stop conditions</dt><dd data-three-page-experiment-stops></dd>
 </dl>
</section>`;
}

/** The parameter tool of a declared analysis, for the Tune page after the analysis it serves (C12); it opens on its step. */
export function renderThreePageAnalysisParameterTemplate(loop: ControlLoopDefinition): string {
  return hasAnalysisParameterTool(loop) ? parameterTool(loop) : "";
}

/**
 * The parameter tool: the fields of the parameters it writes, its status and, for a declared analysis, its step and its Restore
 * (B6.1, C12). The controls are wired in webview/threePageExperiments.ts.
 */
function parameterTool(loop: ControlLoopDefinition): string {
  const analysis = hasAnalysisParameters(loop);
  return `<details data-three-page-experiment-parameters${analysis ? " open" : ""}>
  <summary>${analysis ? "Declared analysis parameters" : "Declared experiment settings"}</summary>
  <p class="field-hint" data-three-page-experiment-parameter-mode></p>
  <div class="parameter-grid" data-three-page-experiment-parameter-fields hidden></div>
  <textarea data-three-page-experiment-parameter-values hidden aria-hidden="true">{}</textarea>
  <p class="studio-action-message" data-three-page-experiment-parameter-status role="status">Inspect the running simulator before editing the declared trial values.</p>
  ${analysis ? '<button type="button" class="trust-button" data-three-page-experiment-parameter-action>Inspect experiment parameters</button>' : ""}
  ${analysis ? '<button type="button" class="trust-button studio-auxiliary" data-three-page-experiment-restore>Restore previous value</button>' : ""}
 </details>`;
}

interface DeclaredExperimentSignal {
  id: string;
  label: string;
  selected: boolean;
}

function declaredExperimentSignals(loop: ControlLoopDefinition): DeclaredExperimentSignal[] {
  const primary: DeclaredExperimentSignal[] = [
    ...(loop.signals.setpoint ? [{ id: "sp", label: `${loop.signals.setpoint.symbol} · Setpoint (${displayEngineeringUnit(loop.signals.setpoint.unit)})`, selected: true }] : []),
    { id: "pv", label: `${loop.signals.processVariable.symbol} · Process value (${displayEngineeringUnit(loop.signals.processVariable.unit)})`, selected: true },
    { id: "mv", label: `${loop.signals.actuator?.symbol ?? loop.signals.manipulatedVariable.symbol} · Applied output (${displayEngineeringUnit(loop.signals.manipulatedVariable.unit)})`, selected: true },
    ...(loop.signals.actuator ? [{ id: "requestedMv", label: `${loop.signals.manipulatedVariable.symbol} · Requested output (${displayEngineeringUnit(loop.signals.manipulatedVariable.unit)})`, selected: false }] : []),
  ];
  const additional = (loop.signals.additional ?? []).map(signal => ({
    id: signal.id,
    label: `${signal.symbol} · ${signal.role}${signal.unit ? ` (${displayEngineeringUnit(signal.unit)})` : ""}`,
    selected: signal.role === "SP" && !loop.signals.setpoint,
  }));
  return [...primary, ...additional];
}
