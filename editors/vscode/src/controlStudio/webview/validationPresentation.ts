/** Projects recording/candidate availability without granting validation authority. */
export function renderValidationPresentation(recording: boolean, candidate: boolean): void {
  const pane = document.querySelector<HTMLElement>('[data-workflow-pane="Validate"]');
  if (!pane) return;
  const recordingOnly = recording && !candidate;
  const heading = pane.querySelector<HTMLElement>("[data-validation-heading]");
  const description = pane.querySelector<HTMLElement>("[data-validation-description]");
  if (heading) heading.textContent = recordingOnly ? "Recorded response" : "Control.PID model comparison";
  if (description) description.textContent = recordingOnly
    ? "Save the recording or open a saved result to inspect its measured response."
    : "Compare current and candidate controllers under identical required scenarios. Validation never applies gains.";
  for (const element of pane.querySelectorAll<HTMLElement>("[data-validation-candidate-only],[data-validate-run]")) {
    element.hidden = recordingOnly;
  }
  const save = pane.querySelector<HTMLButtonElement>("[data-result-save]");
  save?.classList.toggle("primary", recordingOnly);
  save?.classList.toggle("secondary", !recordingOnly);
}
