/**
 * A failed operation as the page shows it (Specification 35 A11.2, B9.3 and B9.9): one plain sentence, an optional
 * technical reason that opens in place, and at most one button for the typed next action.
 */
export interface StudioNextAction {
  kind: string;
  label: string;
}

export interface StudioFailure {
  message: string;
  detail?: string;
  nextAction: StudioNextAction;
}

/**
 * A failure's fix is a button when the engine names its words: the label is the engine's `nextActionLabel` (`NextAction::label`), and a
 * wait or no action carries none, so it shows the message alone (specification 35, A11.2). The page keeps no list of its own.
 */
export function nextActionHasButton(action: StudioNextAction): boolean {
  return action.label.trim() !== "";
}

/**
 * Reads the typed failure of a result. A result without `failure.message` keeps its plain `reason` as the message,
 * with no button, because a reason alone does not name a fix. The engine's words are shown as they are.
 */
export function studioFailureOf(payload: Record<string, unknown>): StudioFailure | undefined {
  const typed = payload.failure;
  if (isRecord(typed) && typeof typed.message === "string" && typed.message.trim()) {
    const action = isRecord(typed.nextAction) ? typed.nextAction : undefined;
    return {
      message: typed.message.trim(),
      ...(typeof typed.detail === "string" && typed.detail.trim() ? { detail: typed.detail } : {}),
      nextAction: { kind: typeof action?.kind === "string" ? action.kind : "none", label: typeof action?.label === "string" ? action.label : "" },
    };
  }
  if (typeof payload.reason === "string" && payload.reason.trim()) {
    return { message: payload.reason.trim(), nextAction: { kind: "none", label: "" } };
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
