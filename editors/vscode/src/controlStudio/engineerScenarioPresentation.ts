/** Markup for the bounded Verify scenario editor. The textarea remains the host contract. */
export function engineerScenarioPresentation(): string {
  return `<details data-engineer-scenario-controls open><summary>Verification scenarios</summary>
<p>Default response and limit checks are included. Add extra scenarios when needed.</p>
<div class="workspace-actions"><button type="button" class="trust-button" data-engineer-scenario-add>Add scenario</button></div>
<div data-engineer-scenario-error role="alert" aria-live="polite"></div><div data-engineer-scenario-list></div>
<details data-engineer-scenario-advanced><summary>Advanced scenario JSON</summary>
<p>Edit the complete validated definition when a field is not shown above. Unknown fields are retained.</p>
<textarea id="engineer-scenarios-json" data-engineer-scenarios rows="8" spellcheck="false">[]</textarea></details>
</details>`;
}

export const engineerScenarioEditorMarkup = engineerScenarioPresentation;
