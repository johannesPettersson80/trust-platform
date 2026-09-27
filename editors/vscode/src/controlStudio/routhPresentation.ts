import type { DeclaredControlAnalysis } from "./declaredAnalysis";
export function routhControls(definition: DeclaredControlAnalysis): string {
  const value = definition.rootLocus.selectedValue;
  return `<section data-routh aria-label="Routh–Hurwitz analysis">
<h3>Routh–Hurwitz stability</h3>
<p>Find whether the declared continuous-time loop's natural modes decay. Calculate a Routh table and an analytical stable parameter range. This calculation does not write to the controller.</p>
<div class="field"><label for="routh-parameter-value">Parameter value</label><input id="routh-parameter-value" data-routh-value type="number" step="any" value="${value}"></div>
<button class="primary" data-routh-run>Calculate Routh–Hurwitz</button>
<p data-routh-status role="status">Uses the declared plant, controller and fixed parameters.</p>
<div data-routh-result hidden></div></section>`;
}
