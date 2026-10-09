/**
 * The independent model check's pass rule (Specification 35, B5.2): the frozen model's free-running prediction must agree
 * with the recorded measured output within the identification error limit over the whole response AND over its final 10 %.
 * Each error is the root-mean-square residual divided by the span of the measured output.
 */
export interface ResponsePoint {
  readonly measured: number;
  readonly residual: number;
}

export interface IndependentResponseScore {
  readonly normalizedError: number;
  readonly finalNormalizedError: number;
  readonly wholeWithinLimit: boolean;
  readonly finalWithinLimit: boolean;
  readonly status: "Pass" | "Fail";
}

/** The first index of the final 10 % of `count` points; that final part always holds at least one point. */
export function finalTenthStart(count: number): number {
  return count - Math.max(1, Math.ceil(count / 10));
}

export function scoreIndependentResponse(points: readonly ResponsePoint[], limit: number): IndependentResponseScore {
  const measured = points.map(point => point.measured);
  const span = Math.max(Math.max(...measured) - Math.min(...measured), 1e-9);
  const normalized = (part: readonly ResponsePoint[]) =>
    Math.sqrt(part.reduce((sum, point) => sum + point.residual ** 2, 0) / part.length) / span;
  const normalizedError = normalized(points);
  const finalNormalizedError = normalized(points.slice(finalTenthStart(points.length)));
  const wholeWithinLimit = Number.isFinite(normalizedError) && normalizedError <= limit;
  const finalWithinLimit = Number.isFinite(finalNormalizedError) && finalNormalizedError <= limit;
  return { normalizedError, finalNormalizedError, wholeWithinLimit, finalWithinLimit,
    status: wholeWithinLimit && finalWithinLimit ? "Pass" : "Fail" };
}

/** The reason a channel's score gives; a failure says what to change, as B5.5 requires. */
export function independentResponseReason(score: IndependentResponseScore): string {
  if (score.status === "Pass") return "Frozen predictions agree with independent plant measurements over the whole response and its final 10 %.";
  if (!score.wholeWithinLimit) {
    return "The fitted model does not predict the independent plant response within its error limit over the whole response. "
      + "Identify again with a different method, a larger step, or a longer duration so the fit covers the response it must predict.";
  }
  return "The fitted model predicts the start of the independent response but not its final 10 %, where the response settles. "
    + "Identify again with a longer duration so the recording covers the settled response, or with a different method.";
}
