import type { GeneralIdentificationFit } from "./generalModelFitPresentation";
import type { AcceptedIdentificationModel } from "./identification";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { GeneralModel } from "./generalModel";

export interface IdentificationCampaignRecord {
  status: "running" | "completed" | "failed" | "cancelled";
  reason?: string;
  runs: readonly RuntimeVerificationResult[];
}

export type AutomaticIdentificationResult =
  | { accepted: AcceptedIdentificationModel; runs: readonly RuntimeVerificationResult[] }
  | { model: GeneralModel; runs: readonly RuntimeVerificationResult[]; fit?: GeneralIdentificationFit };
