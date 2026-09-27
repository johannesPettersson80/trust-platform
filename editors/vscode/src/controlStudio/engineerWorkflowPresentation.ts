import { engineerGoalFields, engineerSettings } from "./engineerSettingsPresentation";
import type { ControlLoopDefinition } from "./loopDefinition";
/** Unit labels arrive HTML-escaped from the panel view model. */
export function engineerWorkflowControls(processUnit: string, canIdentify: boolean, outputUnit: string, supportsPi: boolean, loop?: ControlLoopDefinition): string {
  return `<section data-engineer-workflow data-engineer-parameter-descriptors="${encodeURIComponent(JSON.stringify(loop?.parameterBinding?.parameters.map(({ symbol, unit }) => ({ symbol, unit })) ?? []))}" data-engineer-builtin-tuning="${!loop?.controller.project}" data-engineer-process-unit="${processUnit}" data-engineer-output-unit="${outputUnit}" data-stage="start" data-process-symbol="${encodeURIComponent(loop?.signals.processVariable.symbol ?? "")}" data-setpoint-symbol="${encodeURIComponent(loop?.signals.setpoint?.symbol ?? "")}" data-process-minimum="${loop?.signals.processVariable.range.minimum ?? 0}" data-process-maximum="${loop?.signals.processVariable.range.maximum ?? 1}" data-sample-period="${loop?.controller.taskPeriodMs ?? 1}" data-experiment-kind="${loop?.experiment?.kind ?? "observation"}" style="grid-column:1/-1;min-width:0;width:100%" aria-label="Model to verified controller">
<nav class="engineer-stages" role="tablist" aria-label="Control engineering stages">
${["Model", "Tune", "Verify", "Apply"].map((name, index) => `<button type="button" role="tab" class="trust-button engineer-stage" id="engineer-stage-${name.toLowerCase()}" data-engineer-stage-tab="${name.toLowerCase()}" aria-selected="${index === 0}" aria-controls="engineer-stage-content" tabindex="${index === 0 ? 0 : -1}">${name}<span data-engineer-stage-result="${name.toLowerCase()}" aria-hidden="true"></span></button>`).join("")}
</nav>
<div data-engineer-manual-toolbar hidden><span>Advanced tools</span><button class="trust-button" data-engineer-manual-view>Show plot</button><button class="trust-button" data-engineer-return>Back to workflow</button></div>
${engineerGoalFields(processUnit)}
<div id="engineer-stage-content" class="workspace-actions" data-engineer-actions role="tabpanel">
<button class="trust-button" data-engineer-prerequisite-action hidden>Go to Model</button>
<span data-engineer-readiness class="engineer-readiness" role="status">No model selected</span>
<button class="trust-button trust-button--primary" data-engineer-find ${canIdentify ? "" : "disabled"} title="${canIdentify ? "Run bounded identification and independent validation trials in the local simulator" : "Bind controller mode and a Manual command or reference before identification"}">Identify model</button>
<button class="trust-button" data-engineer-supply>Import model…</button>
<button class="trust-button" data-engineer-use-model hidden>Continue to Tune</button>
<label data-engineer-controller-choice>Controller <select data-engineer-family><option value="automatic">Automatic</option><option value="P">P</option><option value="PI"${supportsPi ? "" : " disabled"}>PI${supportsPi ? "" : ""}</option><option value="PD"${supportsPi ? "" : " disabled"}>PD${supportsPi ? "" : ""}</option><option value="PID"${supportsPi ? "" : " disabled"}>PID${supportsPi ? "" : ""}</option></select></label>
<button class="trust-button" data-engineer-tune disabled>Tune controller</button><button class="trust-button" data-engineer-verify disabled>Run verification</button>
<button class="trust-button" data-engineer-apply disabled>Review changes</button><button class="trust-button" data-engineer-view-results hidden>View results</button><button class="trust-button" data-engineer-cancel hidden>Cancel</button>
<button class="trust-button" data-engineer-configure-acquisition hidden>Review identification controls</button>
<button class="trust-button" data-engineer-configure-scenarios hidden>Review scenarios</button>
<button class="trust-button" data-engineer-review-runtime hidden>Review restarted runtime</button>
<button class="trust-button" data-engineer-manual-staging hidden>Verify for Manual staging</button>
<button class="trust-button" data-engineer-inspect hidden>Review ST implementation</button>
<button class="trust-button" data-engineer-confirm hidden>Confirm application</button><button class="trust-button" data-engineer-retry-recovery hidden>Retry recovery</button>
</div>
<p data-engineer-prerequisite role="status"></p>
<p data-engineer-task-summary hidden role="status"></p>
${engineerSettings(processUnit, outputUnit, loop)}
<p data-engineer-status role="status"></p>
<details data-engineer-results><summary>Details</summary><div data-engineer-evidence></div><div data-engineer-rollback-access hidden></div></details>
</section>`;
}

/** Concise empty-state copy; never suppress a specific physical or acquisition fault. */
export function unavailableLoopPresentation(source: string, reason: string): { status: string; explanation: string } {
  if (source === "Simulation" && ["Runtime is not producing scans", "Runtime is stopped"].includes(reason)) {
    return { status: "Simulator stopped", explanation: "Start the simulator to observe the response." };
  }
  return { status: source === "Physical" ? "Process unavailable" : "Data unavailable", explanation: reason };
}

/** Product status copy has no dependency on model IDs, runtime IDs or revision hashes. */
export function modelReadyReason(model: { source: string; validation?: string }, supportsAlternatives: boolean | readonly string[]): string {
  const ready = model.validation === "unvalidated" ? "Provisional model ready for tuning." : model.source === "fitted" ? "Model found and validated." : "Model ready.";
  if (Array.isArray(supportsAlternatives)) return `${ready} ${supportsAlternatives.length ? `Choose and tune a compatible controller: ${supportsAlternatives.join(", ")}.` : "Declare a compatible ST controller adapter before tuning."}`;
  return supportsAlternatives
    ? `${ready} Recommend P control first; PI, PD and PID are available if the task needs them.`
    : `${ready} Tune the P controller for your requirements.`;
}
