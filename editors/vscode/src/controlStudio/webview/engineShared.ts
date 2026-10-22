/**
 * What the page's engine modules share (specification 35, A11 and D7): the stage tabs that show the status and the words the engine
 * gives each stage, and the failure shape with the words the engine gives its next action (D10). Presentation only.
 */

/** The window event that carries the engine's open answer to the page (specification 35, B3.2, B3.3, B6.1, C4): the Method list, the default Limits, the project test's form and the state design. */
export const ENGINE_OPENED_EVENT = "trust-control-studio-engine-opened";

/** The engine's stage each tab shows (D7). */
const ENGINE_PAGE_OF_TAB = new Map<string, string>([
  ["model", "Identify"],
  ["tune", "Tune"],
  ["verify", "Validate"],
]);

/**
 * Each stage tab shows the status and words the engine gives its stage, and a stage that names a reason shows it. The tab also keeps
 * whether the stage's own action can start now (`available`, B9.4): the stage actions are enabled from it. The Apply stage has no tab:
 * the Validate page's application area keeps whether Apply's own action can start now, which the Apply area reads.
 */
export function applyStageStatuses(root: HTMLElement, stages: unknown): void {
  if (!Array.isArray(stages)) return;
  for (const tab of Array.from(root.querySelectorAll<HTMLElement>("[data-engineer-stage-tab]"))) {
    const stage = ENGINE_PAGE_OF_TAB.get(tab.dataset.engineerStageTab ?? "");
    const entry = stages.map(record).find((item) => item?.stage === stage);
    if (!stage || !entry) continue;
    tab.dataset.status = String(entry.status);
    tab.dataset.available = String(entry.available === true);
    tab.setAttribute("aria-label", `${stage}, ${String(entry.label)}`);
    showStageReason(root, tab.dataset.engineerStageTab ?? "", entry);
  }
  const apply = stages.map(record).find((item) => item?.stage === "Apply");
  const application = root.querySelector<HTMLElement>("[data-studio-application]");
  if (apply && application) application.dataset.available = String(apply.available === true);
}

/**
 * A stage the engine gives a reason shows it at the top of its page: a stage it answers Not applicable (C12), whose controls are also
 * inert so they cannot be used, and a stage an Apply's checkpoint holds until its Restore (B6.4, B6.7), whose page stays usable, so that
 * Restore previous settings stays in reach. A stage without a reason shows no note, and only a stage that is Not applicable is inert. The
 * engine decides the status and the reason; this only presents them.
 */
function showStageReason(root: HTMLElement, tabId: string, entry: Record<string, unknown>): void {
  const page = root.querySelector<HTMLElement>(`[data-studio-page="${tabId}"]`);
  if (!page) return;
  page.toggleAttribute("inert", entry.status === "not-applicable");
  let note = page.querySelector<HTMLElement>(":scope > [data-engineer-stage-reason]");
  if (typeof entry.reason !== "string" || !entry.reason) {
    note?.remove();
    return;
  }
  if (!note) {
    note = root.ownerDocument.createElement("p");
    note.dataset.engineerStageReason = "";
    page.prepend(note);
  }
  note.textContent = entry.reason;
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

/**
 * The Tune page's Channels… button is offered when the engine's open answer carries channel rows (A8.1): a loop with one input and one
 * output carries none. The rows are kept on the page for the Channels editor, which the button opens.
 */
export function showChannelsEditor(root: HTMLElement, choices: unknown): void {
  const offered = record(choices) !== undefined;
  root.dataset.studioChannelChoices = offered ? JSON.stringify(choices) : "null";
  const button = root.querySelector<HTMLButtonElement>("[data-engineer-channels]");
  if (button) button.hidden = !offered;
}

/** A JSON object, or undefined for any other value. */
export function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}
