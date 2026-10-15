/**
 * What the page's engine modules share (specification 35, A11 and D7): the stage tabs that show the status and the words the engine
 * gives each stage, and the failure shape with the words the engine gives its next action (D10). Presentation only.
 */

/** The engine's stage each tab shows (D7). */
const ENGINE_PAGE_OF_TAB = new Map<string, string>([
  ["model", "Identify"],
  ["tune", "Tune"],
  ["verify", "Validate"],
]);

/** Each stage tab shows the status and words the engine gives its stage, and a stage that does not apply shows the engine's reason. */
export function applyStageStatuses(root: HTMLElement, stages: unknown): void {
  if (!Array.isArray(stages)) return;
  for (const tab of Array.from(root.querySelectorAll<HTMLElement>("[data-engineer-stage-tab]"))) {
    const stage = ENGINE_PAGE_OF_TAB.get(tab.dataset.engineerStageTab ?? "");
    const entry = stages.map(record).find((item) => item?.stage === stage);
    if (!stage || !entry) continue;
    tab.dataset.status = String(entry.status);
    tab.setAttribute("aria-label", `${stage}, ${String(entry.label)}`);
    showStageReason(root, tab.dataset.engineerStageTab ?? "", entry);
  }
}

/**
 * A stage the engine answers Not applicable (C12) shows the engine's reason at the top of its page, and the page's controls are
 * inert, so they cannot be used. Any other status clears both. The engine decides the status; this only presents it.
 */
function showStageReason(root: HTMLElement, tabId: string, entry: Record<string, unknown>): void {
  const page = root.querySelector<HTMLElement>(`[data-studio-page="${tabId}"]`);
  if (!page) return;
  const notApplicable = entry.status === "not-applicable";
  page.toggleAttribute("inert", notApplicable);
  let note = page.querySelector<HTMLElement>(":scope > [data-engineer-stage-reason]");
  if (!notApplicable) {
    note?.remove();
    return;
  }
  if (!note) {
    note = root.ownerDocument.createElement("p");
    note.dataset.engineerStageReason = "";
    page.prepend(note);
  }
  note.textContent = typeof entry.reason === "string" ? entry.reason : "";
}

/**
 * The page's failure shape (webview/studioFailure.ts): the message, the detail, and the next action with the words the engine
 * gives it (`nextActionLabel`); an action without words shows no button (A11.2).
 */
export function studioFailure(failure: Record<string, unknown>): Record<string, unknown> {
  const next = typeof failure.nextAction === "string" ? failure.nextAction : "none";
  const label = typeof failure.nextActionLabel === "string" ? failure.nextActionLabel : "";
  return {
    message: typeof failure.message === "string" ? failure.message : "",
    ...(typeof failure.detail === "string" && failure.detail ? { detail: failure.detail } : {}),
    // The form field a refusal concerns (D-B12): the page shows its sentence under that field.
    ...(typeof failure.field === "string" && failure.field ? { field: failure.field } : {}),
    nextAction: { kind: next, label },
  };
}

/** A JSON object, or undefined for any other value. */
export function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}
