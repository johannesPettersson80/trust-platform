/** One-to-one input/output pairing from a relative gain array: positive relative gains closest to 1 (Specification 35 §5.3, §17.9). */
export interface MimoPairingDecision { inputId: string; outputId: string; relativeGain: number; status: "candidate" | "rejected" }

export function oneToOnePairing(inputIds: readonly string[], outputIds: readonly string[], rga: readonly (readonly number[])[]): MimoPairingDecision[] {
  const permutations = (remaining: number[], prefix: number[]): number[][] => remaining.length ? remaining.flatMap((candidate, index) => permutations([...remaining.slice(0, index), ...remaining.slice(index + 1)], [...prefix, candidate])) : [prefix];
  const best = permutations(inputIds.map((_id, index) => index), []).sort((left, right) => cost(outputIds.map((_id, row) => rga[row][left[row]])) - cost(outputIds.map((_id, row) => rga[row][right[row]])))[0];
  return outputIds.map((outputId, row) => { const inputId = inputIds[best[row]], relativeGain = rga[row][best[row]]; return { outputId, inputId, relativeGain, status: relativeGain > 0 ? "candidate" : "rejected" }; });
}
function cost(values: readonly number[]): number { return values.reduce((sum, value) => sum + (value > 0 ? Math.abs(value - 1) : 1e9 + Math.abs(value)), 0); }
