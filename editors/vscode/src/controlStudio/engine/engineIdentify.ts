import type { IdentifyRequest } from "../generated/IdentifyRequest";

/** The action `controlStudio/identify` takes (specification 35, §0.1a). */
export type EngineIdentifyAction = IdentifyRequest["action"];

/** The experiment settings of the `identify` action, as the engine's request states them (B3.2). */
export type EngineAcquisitionSettings = Extract<EngineIdentifyAction, { action: "identify" }>["settings"];

/**
 * The engine's action for one Identify command of the page, or undefined when the engine takes none for it, so that the
 * TypeScript route keeps the command. The engine decides every rule; this only names the request.
 *
 * - `identify.run` starts a live campaign with the study's acquisition settings (B3.2 to B3.4). The page's model choice is
 *   not carried: the engine fits the forms in order and keeps the simplest that meets the error limit (B3.5). A run that
 *   names exclusions or step-response settings has no engine rule yet, so it stays on TypeScript.
 * - `identify.fit` fits the study's last recording again (B3.6), when the engine has produced one.
 * - `study.reset` starts a new working study and keeps the previous one as history (A8.4).
 */
export function engineIdentifyAction(
  kind: string,
  payload: Record<string, unknown>,
  settings: EngineAcquisitionSettings | undefined,
  recordingId: string | undefined
): EngineIdentifyAction | undefined {
  switch (kind) {
    case "control-studio.identify.run":
      return settings && Object.keys(payload).every((key) => key === "model") ? { action: "identify", settings } : undefined;
    case "control-studio.identify.fit":
      return recordingId ? { action: "openRecording", recordingId } : undefined;
    case "control-studio.study.reset":
      return { action: "reset" };
    default:
      return undefined;
  }
}
