import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterConnection } from "./parameterSession";

type HoldTiming = Pick<ControlLoopDefinition, "observedSampleTiming"> & {
  experiment?: Pick<NonNullable<ControlLoopDefinition["experiment"]>, "baselineSeconds">;
};

/**
 * A project step test whose hold is a declared experiment parameter records its
 * baseline, every scan of the applied hold and one restoration sample, so the
 * record ends just after the command returns instead of after a fixed maximum.
 */
export function projectStepRecordingMs(loop: HoldTiming, holdSeconds: number): number {
  const period = loop.observedSampleTiming.nominalPeriodMs;
  const baselineMs = Math.ceil((loop.experiment?.baselineSeconds ?? 0) * 1_000 / period) * period;
  const holdMs = Math.ceil(holdSeconds * 1_000 / period - 1e-9) * period;
  return baselineMs + holdMs + period;
}

/** Reads the applied hold of a project step test from the runtime's declared experiment parameters. */
export async function appliedProjectHoldSeconds(loop: ControlLoopDefinition,
  connect: () => Promise<ParameterConnection>): Promise<number | undefined> {
  const symbol = loop.experiment?.holdParameter;
  if (!symbol) return undefined;
  const status = await (await connect()).client.status();
  const value = status.values[symbol];
  if (!Number.isFinite(value)) throw new Error(`The runtime did not report ${symbol}; inspect and apply the experiment settings first.`);
  return value;
}
