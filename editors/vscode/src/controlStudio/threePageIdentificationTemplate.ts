/**
 * The Recorded-data fit's section. The settings fields start empty: their numbers are the engine's default form, which the manual
 * identification's answer carries in `form` (specification 35, B3.3), and the page fills them from it. The model select starts empty too: its
 * options, with their words, are the engine's `modelChoices`.
 */
export function renderThreePageIdentificationTemplate(): string {
  const fields: Array<[string, string]> = [
    ["baselineDurationSeconds", "Baseline window (s)"],
    ["finalDurationSeconds", "Final window (s)"],
    ["maximumWindowRange", "Maximum window range (PV)"],
    ["maximumWindowSlopePerSecond", "Maximum window slope (PV/s)"],
    ["measurementBound", "Measurement bound ± (PV)"],
    ["onsetThreshold", "Departure threshold (PV)"],
    ["onsetSamples", "Consecutive samples"],
    ["baselineStep", "Baseline grid step"],
    ["gainStep", "Gain grid step (PV/MV)"],
    ["timestampShiftSeconds", "Sample time offset (s)"],
    ["delay.minimum", "Delay minimum (s)"],
    ["delay.maximum", "Delay maximum (s)"],
    ["delay.step", "Delay grid step (s)"],
    ["timeConstant.minimum", "Time constant minimum (s)"],
    ["timeConstant.maximum", "Time constant maximum (s)"],
    ["timeConstant.step", "Time constant grid step (s)"],
    ["sensorTimeConstant.minimum", "Additional lag minimum (s)"],
    ["sensorTimeConstant.maximum", "Additional lag maximum (s)"],
    ["sensorTimeConstant.step", "Additional lag grid step (s)"],
    ["modelAdequacy.maximumAbsoluteResidual", "Maximum model residual (PV)"],
    ["modelAdequacy.maximumFinalError", "Maximum final model error (PV)"],
  ];
  return `<section class="studio-identification" data-three-page-identification hidden aria-label="Recorded identification">
 <div class="studio-fields">
  <label class="studio-field">Model family<select data-three-page-identification-model></select></label>
  <label class="studio-check"><input type="checkbox" data-three-page-identification-held-step> Use held-step measurement and search</label>
  <details data-three-page-identification-settings hidden><summary>Measurement and search settings</summary><div class="studio-identification-grid">${fields.map(([key, label]) => `<label><span data-three-page-identification-label="${key}">${label}</span><input type="number" step="any" data-three-page-identification-setting="${key}"></label>`).join("")}</div></details>
  <details data-three-page-identification-exclusions><summary>Exclude interval</summary><div class="studio-identification-grid"><label>Start (s)<input type="number" step="any" data-three-page-identification-excluded-start></label><label>End (s)<input type="number" step="any" data-three-page-identification-excluded-end></label></div></details>
 </div>
 <p class="studio-action-message" data-three-page-identification-status role="status">Open a complete recording to fit a plant model.</p>
 <details data-model-comparison-evidence hidden><summary>Model comparison</summary><div data-model-comparison-content></div></details>
 <details class="studio-identification-result" data-three-page-identification-result data-step-response-evidence hidden><summary>Step-response measurements</summary><table class="studio-requirements"><tbody data-step-response-report></tbody></table></details>
</section>`;
}
