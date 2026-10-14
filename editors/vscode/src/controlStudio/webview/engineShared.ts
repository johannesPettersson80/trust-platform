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

/** Each stage tab shows the status and words the engine gives its stage. */
export function applyStageStatuses(root: HTMLElement, stages: unknown): void {
  if (!Array.isArray(stages)) return;
  for (const tab of Array.from(root.querySelectorAll<HTMLElement>("[data-engineer-stage-tab]"))) {
    const stage = ENGINE_PAGE_OF_TAB.get(tab.dataset.engineerStageTab ?? "");
    const entry = stages.map(record).find((item) => item?.stage === stage);
    if (!stage || !entry) continue;
    tab.dataset.status = String(entry.status);
    tab.setAttribute("aria-label", `${stage}, ${String(entry.label)}`);
  }
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
    nextAction: { kind: next, label },
  };
}

/** A JSON object, or undefined for any other value. */
export function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}
