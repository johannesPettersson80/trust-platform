import { controllerInstanceLabel } from "./controllerGroupPresentation";
import type { ControlStudioView } from "./panelTemplate";
import { displayEngineeringUnit } from "./chartContract";
import { THREE_PAGE_STYLES } from "./threePageStyles";
import { hasProjectExperiment, renderThreePageAnalysisParameterTemplate, renderThreePageExperimentTemplate } from "./threePageExperimentTemplate";
import { renderThreePageIdentificationTemplate } from "./threePageIdentificationTemplate";
import { renderThreePageAnalysisTemplate } from "./threePageAnalysisTemplate";
import { renderThreePageMimoAnalysisTemplate } from "./threePageMimoAnalysisTemplate";

/** The three page layout consumes the same loop identity and host session as the runtime services. */
/** `targetLabel` names the selected target's kind and speed (specification 35, B9.2), such as "Simulation ×10" or "Physical". */
export function renderThreePageStudioHtml(view: ControlStudioView, escape: (value: string) => string, targetLabel?: string): string {
  const loop = view.loopDefinition;
  const option = (symbol: string | undefined, unit: string | undefined) => symbol
    ? `<option value="${escape(symbol)}" selected>${escape(symbol)}${unit ? ` (${escape(displayEngineeringUnit(unit))})` : ""}</option>` : "";
  const bound = Boolean(loop);
  return `<!doctype html><html lang="en"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src ${view.cspSource} 'unsafe-inline'; script-src ${view.cspSource} 'nonce-${view.nonce}';">
${view.themeStyles ? `<link rel="stylesheet" href="${view.themeStyles}">` : ""}
<style>${THREE_PAGE_STYLES}</style></head><body>
<main class="control-studio" data-control-studio data-studio-three-page data-engineer-workflow
 data-project-id="${view.project}" data-editor-session-id="${view.session}" data-loop-id="${view.loop}"
 data-loop-revision="${view.loopRevision}" data-runtime-id="${encodeURIComponent(loop?.runtimeId ?? "")}" data-plant-id="${escape(loop?.plantId ?? "")}" data-plant-revision="${view.revision}"
 data-process-unit="${view.processUnit}" data-output-unit="${view.outputUnit}" data-studio-loop="${escape(JSON.stringify(loop ?? null))}">
<p class="studio-target" data-studio-target style="margin:0 0 6px;font-size:12px;opacity:.85"${targetLabel ? "" : " hidden"}>${escape(targetLabel ?? "")}</p>
<nav class="studio-navigation" role="tablist" aria-label="Control Studio pages">
${[["model", "Identify"], ["tune", "Tune"], ["verify", "Validate"]].map(([id, label], index) => `<button type="button" role="tab" id="studio-tab-${id}" data-engineer-stage-tab="${id}" aria-selected="${index === 0}" aria-controls="studio-page-${id}" tabindex="${index === 0 ? 0 : -1}">${label}</button>`).join("")}
</nav>
<section id="studio-page-model" data-studio-page="model" role="tabpanel" aria-labelledby="studio-tab-model">
 <div class="studio-fields">
  <label class="studio-field">Plant inputs<select data-studio-inputs multiple size="1" aria-label="Plant inputs">${option(loop?.signals.manipulatedVariable.symbol, loop?.signals.manipulatedVariable.unit)}</select></label>
  <label class="studio-field">Measured outputs<select data-studio-outputs multiple size="1" aria-label="Measured outputs">${option(loop?.signals.processVariable.symbol, loop?.signals.processVariable.unit)}</select></label>
  <label class="studio-field">Method<select data-engineer-acquisition-method><option value="automatic">Automatic</option><option value="mv_step">Step response</option><option value="mv_pulse">Pulse response</option><option value="setpoint_step">Setpoint step</option><option value="relay">Feedback relay</option><option value="prbs">Pseudo-random binary sequence (PRBS)</option><option value="multisine">Multisine</option><option value="recorded-fit">Recorded-data fit</option><option value="supplied-model">Supplied plant model</option>${hasProjectExperiment(loop) ? '<option value="project">Project experiment</option>' : ''}</select></label>
 </div>
 <div class="studio-actions"><button class="trust-button trust-button--primary" data-engineer-find${bound ? "" : " disabled"}>Identify plant</button><button class="trust-button" data-identify-import>Open recording…</button><button class="trust-button" data-studio-reset-chart>Reset</button><button class="trust-button studio-auxiliary" data-studio-limits>Limits…</button></div>
 <input type="file" data-identify-import-file accept=".json,application/json" hidden aria-label="Open recording">
 <p class="studio-action-message" data-studio-message="model" role="status" hidden></p><p class="studio-failure-detail" id="studio-failure-detail-model" data-studio-failure-detail="model" hidden></p><button type="button" class="trust-button studio-failure-action" data-studio-failure-action="model" hidden></button>
 ${loop ? renderThreePageExperimentTemplate(loop, escape) : ""}
 ${renderThreePageIdentificationTemplate()}
 <div class="studio-result studio-model-result" data-studio-model-result hidden><p data-studio-model-type></p><p class="studio-model-equation" data-studio-model-equation></p><p class="studio-model-numbers" data-studio-model-numbers hidden></p><ul class="studio-model-warnings" data-studio-model-warnings hidden></ul><p data-studio-relay-cycle hidden></p></div>
</section>
<section id="studio-page-tune" data-studio-page="tune" role="tabpanel" aria-labelledby="studio-tab-tune" hidden>
 <div class="studio-fields">
  <div class="studio-controller-row"><label class="studio-field">Controller<select data-studio-controller>${loop ? `<option value="${escape(loop.controller.instance)}">${escape(controllerInstanceLabel(loop.controller))} · ${escape(loop.controller.type)}</option>` : '<option value="">Select controller</option>'}</select></label>${loop?.channels && (loop.channels.inputs.length > 1 || loop.channels.outputs.length > 1) ? '<button type="button" class="trust-button studio-auxiliary" data-engineer-channels>Channels…</button>' : ''}</div>
  <label class="studio-field">Tuning method<select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="SIMC">Simple internal model control (SIMC)</option><option value="AMIGO">Approximate M-constrained integral gain optimization (AMIGO)</option><option value="relay" hidden disabled>Relay rule (Ziegler–Nichols PI)</option><option value="manual">Manual</option></select></label>
 </div>
 <div class="studio-actions"><button class="trust-button trust-button--primary" data-engineer-tune disabled>Tune controller</button><button class="trust-button studio-auxiliary" data-studio-requirements>Requirements…</button><button type="button" class="trust-button studio-auxiliary" data-studio-analyze hidden aria-haspopup="true" aria-expanded="false">Analyze…</button><button type="button" class="trust-button studio-auxiliary" data-studio-analysis-close hidden>Close</button></div>
 <div class="studio-analyze-list" data-studio-analyze-list role="group" aria-label="Analysis tools" hidden></div>
 <p class="studio-action-message" data-studio-message="tune" role="status">Identify a plant model first.</p><p class="studio-failure-detail" id="studio-failure-detail-tune" data-studio-failure-detail="tune" hidden></p><button type="button" class="trust-button studio-failure-action" data-studio-failure-action="tune" hidden></button>
 <div class="studio-result" data-studio-tuning-result hidden></div>
 ${loop?.controller.stateFeedback ? '<section class="studio-fields studio-result" data-studio-state-design hidden aria-label="State-space design inputs"></section>' : ''}
 ${renderThreePageAnalysisTemplate()}
 ${loop ? renderThreePageAnalysisParameterTemplate(loop) : ""}
 ${renderThreePageMimoAnalysisTemplate()}
</section>
<section id="studio-page-verify" data-studio-page="verify" role="tabpanel" aria-labelledby="studio-tab-verify" hidden>
 <div class="studio-actions"><button class="trust-button" data-engineer-verify disabled>Validate controller</button></div>
 <p class="studio-action-message" data-studio-message="verify" role="status">Tune a controller first.</p><p class="studio-failure-detail" id="studio-failure-detail-verify" data-studio-failure-detail="verify" hidden></p><button type="button" class="trust-button studio-failure-action" data-studio-failure-action="verify" hidden></button>
 <div class="studio-validation-scroll studio-result" role="region" aria-label="Validation requirements" tabindex="0"><table class="studio-requirements" data-studio-validation-result hidden><thead><tr><th scope="col">Requirement</th><th scope="col">Measured</th><th scope="col">Limit</th><th scope="col">Result</th></tr></thead><tbody></tbody></table></div>
 <div class="studio-application" data-studio-application hidden><button class="trust-button trust-button--primary" data-engineer-apply disabled>Apply</button><button class="trust-button" data-engineer-keep hidden disabled title="Write the applied values into the project, so a restart or a new download keeps them">Keep in project</button></div>
</section>
<section class="studio-response" aria-label="Response plot"><h2 class="studio-response-title" data-studio-response-title hidden></h2><div class="studio-chart-frame"><div class="studio-chart" data-vscode-context='{"webviewSection":"control-studio-response","preventDefaultContextMenuItems":true}' data-control-studio-chart role="img" aria-label="Response plot" tabindex="0"></div><p class="studio-chart-empty" data-studio-chart-empty role="status" hidden></p></div></section>
</main><script nonce="${view.nonce}" src="${view.chartScript}"></script><script nonce="${view.nonce}" src="${view.workflowScript}"></script></body></html>`;
}
