export const FINITE_SEARCH_CONTROLS = `<section aria-label="Finite ST parameter search" data-finite-search>
<h3>Search a finite set of ST parameters</h3><p>Each trial starts a fresh isolated copy of your loaded ST and configured simulated plant. Bounds are checked before ranking. The running controller is unchanged.</p>
<button class="secondary" data-search-inspect>Prepare from current bindings</button>
<label for="search-request">Parameters, initial conditions, events and requirements (JSON)</label>
<textarea id="search-request" data-search-request rows="10" maxlength="65536" style="width:100%;box-sizing:border-box;background:var(--vscode-input-background);color:var(--vscode-input-foreground);border:1px solid var(--vscode-input-border,var(--vscode-panel-border));font-family:var(--vscode-editor-font-family)">{}</textarea>
<p>Edit the finite values and requirements before reviewing. Preparation includes an explicit initial mode value for the isolated copy; check it against your ST logic.</p>
<div class="workspace-actions"><button class="secondary" data-search-review>Review search</button><button class="primary" data-search-start disabled>Start reviewed search</button><button class="danger" data-search-cancel disabled>Cancel search</button><button class="secondary" data-search-save disabled>Save search</button></div>
<p data-search-status role="status">Prepare a request or enter a complete bounded search definition.</p>
<details data-search-review-details hidden><summary>Exact reviewed request</summary><pre data-search-reviewed style="white-space:pre-wrap;overflow-wrap:anywhere"></pre></details>
<section data-search-result hidden><p data-search-summary></p><table style="width:100%;table-layout:fixed;overflow-wrap:anywhere;text-align:left"><thead><tr><th>Candidate</th><th>Outcome</th><th>Score / reason</th><th>Selection</th></tr></thead><tbody data-search-trials></tbody></table></section></section>`;
