import type { DesignIntent } from "../agent/intent";
import type { PageChanges } from "../agent/review";

export interface ProposalFile {
  file: string;
  title: string;
  width: number;
  height: number;
  before?: string;
  after: string;
  changes: PageChanges;
}
export interface ProposalModel {
  exploration?: import("./explorationModel").ExplorationModel;
  draftId: string;
  revision: string;
  engineeringValid: boolean;
  intent?: DesignIntent;
  files: ProposalFile[];
  findings: Array<{ file?: string; object?: string; message?: string; severity?: string }>;
}
export type ProposalStatus = "ready" | "applied" | "discarded" | "stale" | "error";
