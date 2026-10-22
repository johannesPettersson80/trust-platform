import type { ParameterTool, ProjectTestForm } from "./generated/OpenResult";
import { displayEngineeringUnit } from "./chartContract";

/**
 * The project test's section (specification 35, B6.1, C12). The template renders its shell hidden: the page fills the signals, the
 * duration and the experiment's settings from the engine's project test form, which the open answer carries (`projectTestForm`). Nothing
 * here decides whether the section applies. The native list of the signals is hidden and out of the tab order from the start, as every
 * native multi-select is (Identify page specification, Signals: a checkbox dropdown for multiple selections): the page gives it its
 * dropdown when the engine fills the section.
 */
export function renderThreePageExperimentTemplate(): string {
  return `<section class="studio-experiment" data-three-page-experiment hidden aria-label="Project experiment">
 <div class="studio-experiment-fields">
  <label class="studio-field studio-experiment-signals">Signals<select data-three-page-experiment-signals multiple size="4" hidden tabindex="-1" aria-hidden="true" aria-label="Declared experiment signals"></select></label>
 </div>
 <div data-three-page-experiment-parameter-slot></div>
 <p class="studio-action-message" data-three-page-experiment-status role="status" hidden></p>
 <dl class="studio-experiment-plan" data-three-page-experiment-plan hidden>
  <dt>Test signal</dt><dd data-three-page-experiment-bound></dd>
  <dt>Duration</dt><dd data-three-page-experiment-duration></dd>
  <dt>After test</dt><dd data-three-page-experiment-restoration></dd>
  <dt>Stop conditions</dt><dd data-three-page-experiment-stops></dd>
 </dl>
</section>`;
}

/** The slot of a declared analysis's parameter tool, on the Tune page after the analysis it serves (C12). The page fills it when the form says so. */
export function renderThreePageAnalysisParameterTemplate(): string {
  return `<div data-three-page-analysis-parameter-slot></div>`;
}

/**
 * The parameter tool the engine names for a project-scripted loop (B6.1, C12): the fields of the parameters it writes, its status, its step
 * and its Restore. Both tools are the engine's `controlStudio/parameters`; only the heading differs. The page fills it into its slot, and
 * wires it in webview/threePageExperiments.ts.
 */
export function renderParameterTool(tool: ParameterTool): string {
  const heading = tool === "analysis" ? "Declared analysis parameters" : "Declared experiment settings";
  return `<details data-three-page-experiment-parameters open>
  <summary>${heading}</summary>
  <p class="field-hint" data-three-page-experiment-parameter-mode></p>
  <div class="parameter-grid" data-three-page-experiment-parameter-fields hidden></div>
  <textarea data-three-page-experiment-parameter-values hidden aria-hidden="true">{}</textarea>
  <p class="studio-action-message" data-three-page-experiment-parameter-status role="status">Inspect the running simulator before editing the declared trial values.</p>
  <button type="button" class="trust-button" data-three-page-experiment-parameter-action>Inspect experiment parameters</button>
  <button type="button" class="trust-button studio-auxiliary" data-three-page-experiment-restore>Restore previous value</button>
 </details>`;
}

/**
 * Fills the project test's controls from the engine's form (B6.1, C12): the signals it offers, the duration and the baseline, and the
 * parameter tool the form names into its slot: the experiment's settings with the project test on the Identify page, or a declared
 * analysis's parameters on the Tune page. The page calls it once the open answer names the form; it decides nothing.
 */
export function fillProjectTest(root: HTMLElement, form: ProjectTestForm): void {
  const document = root.ownerDocument;
  const signals = root.querySelector<HTMLSelectElement>("[data-three-page-experiment-signals]");
  signals?.replaceChildren(...form.signals.map(signal => {
    const option = document.createElement("option");
    option.value = signal.id;
    option.textContent = signal.unit ? `${signal.label} (${displayEngineeringUnit(signal.unit)})` : signal.label;
    option.selected = signal.selected;
    return option;
  }));
  const duration = root.querySelector<HTMLElement>("[data-three-page-experiment-duration]");
  if (duration) duration.textContent = `${form.durationSeconds} s${form.baselineSeconds ? ` · ${form.baselineSeconds} s baseline` : ""}`;
  if (form.parameterTool === "experiment") root.querySelector<HTMLElement>("[data-three-page-experiment-parameter-slot]")?.replaceChildren(...parameterToolNodes(document, "experiment"));
  if (form.parameterTool === "analysis") root.querySelector<HTMLElement>("[data-three-page-analysis-parameter-slot]")?.replaceChildren(...parameterToolNodes(document, "analysis"));
}

function parameterToolNodes(document: Document, tool: ParameterTool): Node[] {
  const host = document.createElement("div");
  host.innerHTML = renderParameterTool(tool);
  return [...host.childNodes];
}
