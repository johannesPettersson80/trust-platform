/** The design intent of a composition (specification 43, AT-04). The authoring command derives it; this is the
 * shape the proposal review reads. It holds no rule. */
export interface DesignIntent {
  operatorTask: string;
  taskSource: "request" | "pattern" | "source-page";
  rationale: string;
  assumptions: string[];
}
