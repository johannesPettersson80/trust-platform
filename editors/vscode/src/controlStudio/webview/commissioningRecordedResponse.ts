import type { CommissioningResult } from "../proportionalCommissioning";
import type { Evaluation } from "./engineerRecordedResponses";

/** Plot projection only: preserves native samples without inventing run conditions. */
export function commissioningRecordedResponse(result: CommissioningResult): Evaluation | undefined {
  const raw = result.recording?.rawSamples;
  const program = result.recording?.evidence.programRevision;
  if (!raw?.length || !program || result.channels.length !== 3) return undefined;
  const channels = result.channels;
  const events = raw.flatMap((sample, index) => index && sample.sp !== null && sample.sp !== raw[index - 1].sp
    ? [{ atSeconds: sample.elapsedSeconds, channel: channels[0].id, value: sample.sp }] : []);
  return { status: result.status, scope: "Actual commissioned controller in the running simulator; selected model identity is retained separately.",
    eventDescription: "Reference changes observed in native samples; marker times are sample times.",
    run: { evidence: `${raw[0].source} native recording`, channels,
      channelRoles: { setpoint: channels[0].id, processVariable: channels[1].id, manipulatedVariable: channels[2].id },
      conditions: { modelRevision: result.modelRevision, programRevision: program, events },
      samples: raw.map(sample => ({ timeSeconds: sample.elapsedSeconds, values: {
        [channels[0].id]: { value: sample.sp, quality: sample.quality },
        [channels[1].id]: { value: sample.pv, quality: sample.quality },
        [channels[2].id]: { value: sample.appliedMv, quality: sample.quality },
      } })),
    }, specification: [{ kind: "bounds", channel: channels[2].id,
      minimum: raw[0].outputLimits.minimum, maximum: raw[0].outputLimits.maximum,
      window: { start: raw[0].elapsedSeconds, end: raw[raw.length - 1].elapsedSeconds } }],
  };
}
