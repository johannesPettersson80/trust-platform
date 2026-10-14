import type { IdentifyRequest } from "../generated/IdentifyRequest";

/** The action `controlStudio/identify` takes (specification 35, §0.1a). */
export type EngineIdentifyAction = IdentifyRequest["action"];

/** The experiment settings of the `identify` action, as the engine's request states them (B3.2). */
export type EngineAcquisitionSettings = Extract<EngineIdentifyAction, { action: "identify" }>["settings"];

/**
 * The engine's action for one Identify command of the page, or undefined when the engine takes none for it, so that the
 * TypeScript route keeps the command. The engine decides every rule; this only names the request.
 *
 * - `recording.refit` fits a kept recording again with the plant input and measured output the page names (B3.6). It names the
 *   recording the route holds, when it holds one; without one the engine fits the study's last kept opened recording, or refuses.
 *   The page sends it for a method other than Recorded-data fit, on a loop the engine serves (§0.1a).
 * - `study.reset` starts a new working study and keeps the previous one as history (A8.4).
 *
 * Identify plant on the engine route is a run (`engine/routes/runRoute.ts`), and the Recorded-data fit's steps belong to the
 * TypeScript route (B3.3), so no other Identify command has an engine action.
 */
export function engineIdentifyAction(
  kind: string,
  payload: Record<string, unknown>,
  recordingId: string | undefined
): EngineIdentifyAction | undefined {
  switch (kind) {
    case "control-studio.recording.refit":
      return { action: "openRecording", ...(recordingId ? { recordingId } : {}), ...recordingSignals(payload) };
    case "control-studio.study.reset":
      return { action: "reset" };
    default:
      return undefined;
  }
}

/** The plant input and measured output the page's selector names for an opened recording (B3.6), each when the page names it. */
function recordingSignals(payload: Record<string, unknown>): { inputs?: string[]; outputs?: string[] } {
  const selection = payload.recordingSelection as { inputs?: unknown; outputs?: unknown } | undefined;
  const names = (value: unknown): string[] | undefined =>
    Array.isArray(value) && value.every(item => typeof item === "string") ? value as string[] : undefined;
  const inputs = names(selection?.inputs), outputs = names(selection?.outputs);
  return { ...(inputs ? { inputs } : {}), ...(outputs ? { outputs } : {}) };
}
