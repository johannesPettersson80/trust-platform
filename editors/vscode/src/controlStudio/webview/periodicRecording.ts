/** The recording of a project experiment that completed, which the analyses may measure; nothing for other phases. */
export function completedExperimentRecording(message: { kind?: unknown; phase?: unknown; payload?: unknown }): Record<string, unknown> | undefined {
  if (message.kind !== "control-studio.experiment.result" || message.phase !== "terminal" || !isRecord(message.payload)
    || message.payload.status !== "completed" || !isRecord(message.payload.recording) || !Array.isArray(message.payload.recording.rawSamples)) return undefined;
  return message.payload.recording;
}

function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
