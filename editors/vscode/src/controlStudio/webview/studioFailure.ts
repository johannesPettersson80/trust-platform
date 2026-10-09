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

/** The next actions that show a button. A wait, an approval and none show the message alone. */
const BUTTON_KINDS = ["connect-runtime", "open-loop-setup", "open-limits", "open-requirements", "open-channels",
  "choose-method", "identify", "tune", "validate", "restore"] as const;

/** A failure's fix is a button only when its kind opens or starts something and the host names it: an empty label shows none. */
export function nextActionHasButton(action: StudioNextAction): boolean {
  return (BUTTON_KINDS as readonly string[]).includes(action.kind) && action.label.trim() !== "";
}

/** Engine reasons may end with a "Next: …" sentence from earlier hosts; the page shows the message only. */
export function plainReason(text: string): string {
  return text.replace(/(?<=[.!?])\s+Next: [^]*$/, "");
}

/**
 * Reads the typed failure of a result. A result without `failure.message` keeps its plain `reason` as the message,
 * with no button, because a reason alone does not name a fix.
 */
export function studioFailureOf(payload: Record<string, unknown>): StudioFailure | undefined {
  const typed = payload.failure;
  if (isRecord(typed) && typeof typed.message === "string" && typed.message.trim()) {
    const action = isRecord(typed.nextAction) ? typed.nextAction : undefined;
    return {
      message: plainReason(typed.message).trim(),
      ...(typeof typed.detail === "string" && typed.detail.trim() ? { detail: typed.detail } : {}),
      nextAction: { kind: typeof action?.kind === "string" ? action.kind : "none", label: typeof action?.label === "string" ? action.label : "" },
    };
  }
  if (typeof payload.reason === "string" && payload.reason.trim()) {
    return { message: plainReason(payload.reason).trim(), nextAction: { kind: "none", label: "" } };
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
