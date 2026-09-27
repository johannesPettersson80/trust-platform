/** Compact Identify controls shown only when Recorded fit is selected. */
export const RECORDED_FIT_METHOD = "recorded-fit";

export function renderThreePageIdentificationTemplate(): string {
  const fields: Array<[string, string, string]> = [
    ["baselineDurationSeconds", "Baseline window (s)", "5"],
    ["finalDurationSeconds", "Final window (s)", "5"],
    ["maximumWindowRange", "Maximum window range (PV)", "0.1"],
    ["maximumWindowSlopePerSecond", "Maximum window slope (PV/s)", "0.01"],
    ["measurementBound", "Measurement bound ± (PV)", "0.05"],
    ["onsetThreshold", "Departure threshold (PV)", "0.15"],
    ["onsetSamples", "Consecutive samples", "3"],
    ["baselineStep", "Baseline grid step", "0.01"],
    ["gainStep", "Gain grid step (PV/MV)", "0.001"],
    ["timestampShiftSeconds", "Sample time offset (s)", "0"],
    ["delay.minimum", "Delay minimum (s)", "0"],
    ["delay.maximum", "Delay maximum (s)", "2"],
    ["delay.step", "Delay grid step (s)", "0.1"],
    ["timeConstant.minimum", "Time constant minimum (s)", "0.1"],
    ["timeConstant.maximum", "Time constant maximum (s)", "20"],
    ["timeConstant.step", "Time constant grid step (s)", "0.1"],
    ["sensorTimeConstant.minimum", "Additional lag minimum (s)", "0.1"],
    ["sensorTimeConstant.maximum", "Additional lag maximum (s)", "10"],
    ["sensorTimeConstant.step", "Additional lag grid step (s)", "0.1"],
    ["modelAdequacy.maximumAbsoluteResidual", "Maximum model residual (PV)", "0.2"],
    ["modelAdequacy.maximumFinalError", "Maximum final model error (PV)", "0.1"],
  ];
  return `<section class="studio-identification" data-three-page-identification hidden aria-label="Recorded identification">
 <div class="studio-fields">
  <label class="studio-field">Model family<select data-three-page-identification-model>
   <option value="automatic">Choose automatically</option><option value="integrating">Integrating</option><option value="first_order">First order</option><option value="fopdt">First order with dead time</option><option value="second_order">Second order</option><option value="sopdt">Compare additional lag</option>
  </select></label>
  <label class="studio-check"><input type="checkbox" data-three-page-identification-held-step> Use held-step measurement and search</label>
  <details data-three-page-identification-settings hidden><summary>Measurement and search settings</summary><div class="studio-identification-grid">${fields.map(([key, label, value]) => `<label><span data-three-page-identification-label="${key}">${label}</span><input type="number" step="any" data-three-page-identification-setting="${key}" value="${value}"></label>`).join("")}</div></details>
  <details data-three-page-identification-exclusions><summary>Exclude interval</summary><div class="studio-identification-grid"><label>Start (s)<input type="number" step="any" data-three-page-identification-excluded-start></label><label>End (s)<input type="number" step="any" data-three-page-identification-excluded-end></label></div></details>
 </div>
 <p class="studio-action-message" data-three-page-identification-status role="status">Open a complete recording to fit a plant model.</p>
 <details data-model-comparison-evidence hidden><summary>Model comparison</summary><div data-model-comparison-content></div></details>
 <details class="studio-identification-result" data-three-page-identification-result data-step-response-evidence hidden><summary>Step-response measurements</summary><table class="studio-requirements"><tbody data-step-response-report></tbody></table></details>
</section>`;
}
