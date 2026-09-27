// Exists because opening or restoring a saved result left the live-value strip reading
// exactly like a current sample, with nothing marking SP/PV/MV as historical evidence
// (finding W-16 follow-up). workflow.ts sets a shared flag on the studio root when it
// loads a saved result; this module resolves that flag (plus the saved result's own
// timestamp, when available) into the one badge sentence and dimmed state chart.ts — the
// live-value strip's owner — applies to the DOM, and reverts on the next live sample.

export interface ValueStripHistoricalState {
  readonly badgeText: string;
  readonly dimmed: true;
}

/** Resolves whether the value strip should read as historical evidence right now, and
 * what its badge should say. Returns undefined for the live state (no badge, not
 * dimmed), so a caller can pass the result straight through without an extra branch. */
export function valueStripHistoricalState(
  active: boolean,
  openedAtDisplay: string | undefined,
  kind: "saved" | "draft" = "saved"
): ValueStripHistoricalState | undefined {
  if (!active) return undefined;
  if (kind === "draft") return { badgeText: "Working draft", dimmed: true };
  const trimmed = (openedAtDisplay ?? "").trim();
  return { badgeText: trimmed ? `Saved result · ${trimmed}` : "Saved result", dimmed: true };
}
