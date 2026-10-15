import type { IdentifyRequest } from "../generated/IdentifyRequest";

/** The action `controlStudio/identify` takes (specification 35, §0.1a). */
export type EngineIdentifyAction = IdentifyRequest["action"];

/** The experiment settings of the `identify` action, as the engine's request states them (B3.2). */
export type EngineAcquisitionSettings = Extract<EngineIdentifyAction, { action: "identify" }>["settings"];

/** The form of the Recorded-data fit, as the page holds it (`IdentificationFormDraft`, B3.3): the engine decodes and checks it. */
type ManualForm = Extract<EngineIdentifyAction, { action: "manualEdit" }>["form"];

/**
 * The page's commands of the Recorded-data fit (B3.3). On a served loop the route claims each of them and sends the engine's manual
 * action (`engineIdentifyAction`); a parked loop keeps them on the TypeScript route until V7.17.
 */
export const MANUAL_IDENTIFY_COMMANDS: ReadonlySet<string> = new Set([
  "control-studio.identify.edit",
  "control-studio.identify.run",
  "control-studio.identify.fit",
  "control-studio.identify.accept",
  "control-studio.identify.choose-structure",
  "control-studio.identify.import",
]);

/**
 * The engine's action for one Identify command of the page, or undefined when the engine takes none for it, so that the
 * TypeScript route keeps the command. The engine decides every rule; this only names the request.
 *
 * - `recording.refit` fits a kept recording again with the plant input and measured output the page names (B3.6). It names the
 *   recording the route holds, when it holds one; without one the engine fits the study's last kept opened recording, or refuses.
 *   The page sends it for a method other than Recorded-data fit, on a loop the engine serves (§0.1a).
 * - `study.reset` starts a new working study and keeps the previous one as history (A8.4).
 * - The Recorded-data fit (B3.3): `identify.edit` stores the page's form, `identify.run` checks the recording `recordingId` names
 *   with the form, `identify.fit` fits it, `identify.choose-structure` keeps the sensor-lag structure, `identify.accept` accepts the
 *   model, and `identify.import` checks and keeps the file the page opened (B3.6). Each answer is the engine's manual answer.
 *
 * Identify plant on the engine route is a run (`engine/routes/runRoute.ts`), so no other Identify command has an engine action.
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
    case "control-studio.identify.edit":
      return { action: "manualEdit", form: payload.form as ManualForm };
    case "control-studio.identify.run":
      return recordingId ? { action: "manualQualify", recordingId, form: payload.form as ManualForm } : undefined;
    case "control-studio.identify.fit":
      return { action: "manualFit" };
    case "control-studio.identify.accept":
      return { action: "manualAccept" };
    case "control-studio.identify.choose-structure":
      return { action: "manualChooseStructure", choice: payload.choice as Extract<EngineIdentifyAction, { action: "manualChooseStructure" }>["choice"] };
    case "control-studio.identify.import":
      return typeof payload.json === "string" ? { action: "manualImport", json: payload.json } : undefined;
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
