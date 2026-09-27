import type { ControlLoopDefinition } from "./loopDefinition";
import { engineerScenarioPresentation } from "./engineerScenarioPresentation";

/** Unit labels are already escaped by the panel view model. */
export function engineerGoalFields(unit: string): string {
  return `<fieldset data-engineer-goal-fields class="engineer-setting-group" hidden><legend>Response target</legend>
<div class="engineer-field-grid">
<label>Target change (${unit})<input data-engineer-reference type="number" value="1" step="any" required></label>
<label>Allowed error (${unit})<input data-engineer-error type="number" value="0.1" min="0" step="any" required></label>
<label>Response time (s)<input data-engineer-settling type="number" value="2" min="0" step="any" required></label>
</div><small>Limit the remaining error after the requested response time.</small><small data-engineer-operating-point hidden></small></fieldset>`;
}

export function engineerSettings(processUnit: string, outputUnit: string, loop?: ControlLoopDefinition): string {
  return `<details data-engineer-options><summary>Settings</summary>
<section data-engineer-settings-stage="model">
<fieldset class="engineer-setting-group"><legend>Identification experiment</legend>
<div data-engineer-acquisition-controls><div class="engineer-field-grid">
<label>Method<select data-engineer-acquisition-method><option value="configured">Use project setup</option><option value="automatic">Choose automatically</option><option value="mv_step">Step output</option><option value="mv_pulse">Pulse output</option><option value="setpoint_step">Step setpoint</option><option value="relay">Feedback relay</option><option value="prbs">PRBS</option><option value="multisine">Multisine</option></select></label>
<label>Duration (s)<input data-engineer-acquisition-duration type="number" min="2" max="600" value="${loop?.experiment?.durationSeconds ?? 10}"></label>
<label class="engineer-check"><input type="checkbox" data-engineer-auto-amplitude checked>Choose amplitude automatically</label>
<label data-engineer-amplitude-field><span data-engineer-amplitude-label>Amplitude (chosen method)</span><input data-engineer-acquisition-amplitude type="number" value="${loop?.experiment?.amplitude ?? 0}" step="any"></label>
<label data-engineer-relay-field>Process direction<select data-engineer-relay-direction><option value="">Choose a direction</option><option value="positive"${loop?.experiment?.relayDirection === "positive" ? " selected" : ""}>More output raises PV</option><option value="negative"${loop?.experiment?.relayDirection === "negative" ? " selected" : ""}>More output lowers PV</option></select></label>
</div><label class="engineer-check"><input data-engineer-reuse-recordings type="checkbox" checked>Reuse suitable recordings</label></div></fieldset>
<div data-engineer-model-options></div></section>
<section data-engineer-settings-stage="tune" hidden>
<fieldset class="engineer-setting-group" data-engineer-applicability="pid"><legend>Controller options</legend><div class="engineer-field-grid">
<label>Response preference<select data-engineer-tuning-objective><option value="balanced">Balanced</option><option value="robust">More robust</option><option value="fast">Faster</option></select></label>
</div></fieldset><details class="engineer-setting-group" data-engineer-applicability="pid"><summary>Design method</summary><div class="engineer-field-grid"><label>Tuning rule<select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="SIMC">SIMC</option><option value="AMIGO">AMIGO</option></select></label>
</div></details><div class="engineer-field-grid" data-engineer-gain-controls>
<label>Gain selection<select data-engineer-gain-mode><option value="automatic">Automatic</option><option value="manual">Keep a specific gain</option></select></label>
<label>Proportional gain Kp (${outputUnit === processUnit ? "1" : `${outputUnit}/${processUnit}`})<input data-engineer-gain-value type="number" value="1" step="any" required disabled></label>
</div>
<details class="engineer-setting-group"><summary>Advanced requirements</summary><div class="engineer-field-grid">
<label>Transient margin (${processUnit})<input data-engineer-band type="number" value="0.02" min="0" step="any"></label>
<label>Trial duration (s)<input data-engineer-duration type="number" value="4" min="0" step="any"></label>
<label>Load change (${outputUnit})<input data-engineer-disturbance type="number" value="0" step="any"></label>
<label>Measurement noise (${processUnit})<input data-engineer-noise type="number" value="0" min="0" step="any"></label>
<label>Relative gain uncertainty (0–1)<input data-engineer-uncertainty type="number" value="0" min="0" max="1" step="any"></label>
</div></details>
<details class="engineer-setting-group"><summary>Keep specific settings</summary><small>Choose values to keep unchanged when tuning again.</small><div class="parameter-grid" data-engineer-setting-pins></div></details></section>
<section data-engineer-settings-stage="verify" hidden>${engineerScenarioPresentation()}</section>
<section data-engineer-settings-stage="apply" hidden><fieldset class="engineer-setting-group"><legend>After applying</legend>
<label>Controller mode<select data-engineer-application-mode><option value="auto">Start automatic control</option><option value="manual">Keep Manual mode</option></select></label>
</fieldset><details class="engineer-setting-group"><summary>Limit output change</summary><small>Leave automatic unless you need a stricter transfer limit.</small>
<label>Maximum output change when switching (${outputUnit})<input data-engineer-transfer-tolerance type="number" min="0" step="any" placeholder="Automatic"></label>
</details></section>
<details class="engineer-setting-group"><summary>Advanced engineering tools</summary><button class="trust-button" data-engineer-tools aria-pressed="false">Open advanced tools</button></details>
<button class="trust-button" data-engineer-rollback>Review rollback</button>
</details>`;
}
