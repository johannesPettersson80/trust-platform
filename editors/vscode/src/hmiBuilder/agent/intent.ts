import { COMPOSITION_PATTERNS } from "./patterns";

export interface DesignIntent {
  operatorTask: string;
  taskSource: "request" | "pattern" | "source-page";
  rationale: string;
  assumptions: string[];
}

export function compositionIntent(input: Record<string, unknown>, title: string): DesignIntent {
  const pattern = COMPOSITION_PATTERNS.find((pattern) => pattern.id === input.pattern);
  return {
    operatorTask: typeof input.operatorTask === "string" ? input.operatorTask : (pattern?.task ?? `Refine ${title}`),
    taskSource: typeof input.operatorTask === "string" ? "request" : pattern ? "pattern" : "source-page",
    rationale:
      typeof input.rationale === "string"
        ? input.rationale
        : pattern
          ? `Uses ${pattern.title.toLowerCase()} with the supplied components and layout constraints.`
          : "Preserves the existing page as the starting point for refinement.",
    assumptions: [
      ...new Set([
        "Preview values are synthetic.",
        "Process connections are supplied, not inferred.",
        ...((input.assumptions as string[]) ?? []),
      ]),
    ],
  };
}
