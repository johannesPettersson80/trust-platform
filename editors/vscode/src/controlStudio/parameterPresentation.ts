export const PARAMETER_CONTROLS = `<section aria-label="ST parameter application" data-parameters>
<h3>Controller parameters</h3>
<div data-parameters-editor>
<div class="workspace-actions"><button class="primary" data-parameters-inspect>Inspect runtime parameters</button><button class="secondary" data-parameters-selected>Use selected analysis values</button></div>
<div class="parameter-grid" data-parameters-fields hidden></div>
<div class="workspace-actions"><button class="secondary" data-parameters-review disabled>Review candidate</button><button class="secondary" data-parameters-rollback disabled>Review rollback</button></div>
</div>
<div data-parameters-review-area hidden>
<table style="width:100%;text-align:left"><thead><tr><th>Parameter</th><th>Current</th><th>Reviewed</th><th>Unit</th></tr></thead><tbody data-parameters-review-table></tbody></table>
<details class="advanced"><summary>Runtime details</summary><p data-parameters-identity style="overflow-wrap:anywhere"></p><p data-parameters-manual style="overflow-wrap:anywhere"></p></details>
<label data-parameters-physical hidden><input type="checkbox" data-parameters-physical-approved>I approve writing these reviewed values to this physical runtime now.</label>
<div class="workspace-actions"><button class="primary" data-parameters-apply disabled>Apply reviewed values</button><button class="secondary" data-parameters-change>Change values</button></div></div>
<p data-parameters-status role="status">Inspect the running controller to edit its parameters.</p>
<details class="advanced"><summary>Symbolic JSON</summary><label for="parameter-values">Candidate symbolic values</label><textarea id="parameter-values" data-parameters-values rows="5" style="width:100%;box-sizing:border-box;background:var(--vscode-input-background);color:var(--vscode-input-foreground);border:1px solid var(--vscode-input-border,var(--vscode-panel-border));font-family:var(--vscode-editor-font-family)">{}</textarea></details>
</section>`;

/** The same parameter editor and write owner, labelled for the declared task. */
export function experimentParameterControls(): string {
  return PARAMETER_CONTROLS.replace("Controller parameters", "Experiment parameters")
    .replace("Review candidate", "Review values")
    .replace("Inspect the running controller to edit its parameters.", "Inspect the running experiment to edit its parameters.");
}
