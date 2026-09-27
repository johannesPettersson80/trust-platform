/** Contextual retained-recording MIMO evidence; inserted into the existing analysis disclosure. */
export function renderThreePageMimoAnalysisTemplate(): string {
  return `<section class="studio-mimo" data-three-page-mimo-analysis hidden aria-live="polite">
 <div class="studio-mimo-controls" data-three-page-mimo-analysis-controls hidden>
  <label class="studio-field">Recording<select data-three-page-mimo-analysis-recording></select></label>
  <div class="studio-mimo-window-scroll"><table class="studio-mimo-windows" data-three-page-mimo-analysis-windows><caption>Settled windows · seconds</caption><thead><tr><th scope="col">Input</th><th scope="col">Base start (s)</th><th scope="col">Base end (s)</th><th scope="col">Settled start (s)</th><th scope="col">Settled end (s)</th></tr></thead><tbody></tbody></table></div>
  <label class="studio-field">Drift tolerance (optional)<input type="number" step="any" data-three-page-mimo-analysis-drift></label>
 </div>
 <p data-three-page-mimo-analysis-status role="status"></p>
 <div data-three-page-mimo-analysis-summary></div>
 <section class="studio-mimo-primary-result" data-three-page-mimo-analysis-gain-details aria-label="Measured gain matrix"><h3>Measured gain matrix</h3><div data-three-page-mimo-analysis-gain></div></section>
 <details data-three-page-mimo-analysis-rga-details><summary>Relative Gain Array</summary><div data-three-page-mimo-analysis-rga></div></details>
 <details data-three-page-mimo-analysis-decoupler-details><summary>Static decoupler</summary><div data-three-page-mimo-analysis-decoupler></div></details>
 <div data-three-page-mimo-analysis-pairing></div>
</section>`;
}
