/**
 * Whether consecutive sample sequence values step by one steady record period k (specification 35, A4.2; runtime protocol
 * annex, E4.4). Two full steps establish k, so it is read from the first step of three or more samples; two samples must be
 * consecutive, as at the runtime's default k = 1. The final sample of a stopped recording may average fewer executions and
 * so step by less; `finalMayBePartial` allows that only for the last step. Any other change of step means a missed
 * execution (annex E4.5).
 */
export function steadyRecordPeriod(sequences: readonly number[], finalMayBePartial: boolean): boolean {
  if (sequences.length < 2) return true;
  const period = sequences.length >= 3 ? sequences[1] - sequences[0] : 1;
  if (!Number.isSafeInteger(period) || period < 1) return false;
  for (let index = 1; index < sequences.length; index += 1) {
    const step = sequences[index] - sequences[index - 1];
    const partialFinal = finalMayBePartial && index === sequences.length - 1;
    if (partialFinal ? !(step >= 1 && step <= period) : step !== period) return false;
  }
  return true;
}
