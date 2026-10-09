export interface VisualReview {
  revision: string;
  reviewer: string;
  verdict: "accepted" | "needs-repair";
  issues: Array<{ file: string; objects: string[]; message: string }>;
  attempt: number;
  customerDecisionRequired: boolean;
}
/** Qualitative evidence only; this record never grants deployment/write authority. */
export function reviewRecord(
  input: Omit<VisualReview, "revision" | "attempt" | "customerDecisionRequired">,
  revision: string,
  previousAttempts: number,
): VisualReview {
  if (!input.reviewer.trim() || (input.verdict === "accepted" && input.issues.length))
    throw new Error("INVALID_ARGUMENT: accepted visual reviews cannot contain unresolved issues.");
  const attempt = previousAttempts + 1;
  return { ...input, revision, attempt, customerDecisionRequired: input.verdict === "needs-repair" && attempt >= 3 };
}
