/** The Tune method owner inserts this collapsed, contextual analysis surface. */
export function renderThreePageAnalysisTemplate(): string {
  return `<section data-three-page-analysis hidden>
<section data-three-page-analysis-result hidden aria-live="polite">
 <div data-three-page-analysis-plot role="img" aria-label="Control analysis plot" style="height:280px"></div>
 <div data-three-page-analysis-summary></div>
 <details data-three-page-analysis-table-details hidden><summary>Calculation details</summary><div data-three-page-analysis-details-summary></div><div data-three-page-analysis-table></div></details>
</section>
</section>`;
}
