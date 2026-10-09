import type { AcquisitionSettings } from "./acquisitionSettings";
import type { EngineerStudy } from "./engineerStudyPersistence";
import { decodeEngineerStudy } from "./engineerStudyPersistence";
import type { ControlLoopDefinition } from "./loopDefinition";
import { isRecord } from "./recordingFile";

/**
 * The draft a changed loop definition starts with (A8.2). Requirements and limits are carried over only where their
 * signals still exist in the new loop definition; recordings, results and pins are not carried over.
 */
export function carriedOverDraft(previous: unknown, loop: ControlLoopDefinition):
  { loopRevision: string; study: EngineerStudy } | undefined {
  if (!isRecord(previous) || !isRecord(previous.study)) return undefined;
  const study = decodeEngineerStudy(previous.study);
  if (!study) return undefined;
  const outputs = new Set((loop.channels?.outputs ?? []).map(output => output.modelChannelId));
  const symbols = declaredSymbols(loop);
  const task = study.task && (study.task.channelRequirements
    ? { ...study.task, channelRequirements: keepKeys(study.task.channelRequirements, outputs) }
    : study.task);
  const settings = study.acquisitionSettings && carriedSettings(study.acquisitionSettings, symbols);
  if (!task && !settings) return undefined;
  return {
    loopRevision: loop.revision,
    study: { campaigns: [], ...(task ? { task } : {}), ...(settings ? { acquisitionSettings: settings } : {}) },
  };
}

/** Requirements are keyed by model output; limits are keyed by the signal symbol they bound. */
function carriedSettings(settings: AcquisitionSettings, symbols: ReadonlySet<string>): AcquisitionSettings {
  const { setpointSignalId, inputRanges, outputRanges, ...rest } = settings;
  return {
    ...rest,
    ...(setpointSignalId !== undefined && symbols.has(setpointSignalId) ? { setpointSignalId } : {}),
    ...(inputRanges ? { inputRanges: keepKeys(inputRanges, symbols) } : {}),
    ...(outputRanges ? { outputRanges: keepKeys(outputRanges, symbols) } : {}),
  };
}

function declaredSymbols(loop: ControlLoopDefinition): Set<string> {
  const signals = [loop.signals.processVariable, loop.signals.manipulatedVariable, loop.signals.setpoint, ...(loop.signals.additional ?? [])];
  return new Set(signals.flatMap(signal => signal ? [signal.symbol] : []));
}

function keepKeys<T>(entries: Readonly<Record<string, T>>, keep: ReadonlySet<string>): Record<string, T> {
  return Object.fromEntries(Object.entries(entries).filter(([key]) => keep.has(key)));
}
