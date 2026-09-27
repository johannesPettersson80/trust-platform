import type { GeneralModel } from "./generalModel";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { TransferBinding } from "./proportionalTransfer";
import type { TransferStatus, TransferTracking } from "./runtimeTransfer";

/** Controller semantics plug into the existing commissioning lifecycle. */
export interface CommissioningProfile {
  label: string;
  references?(loop: ControlLoopDefinition): readonly string[];
  targets?(model: GeneralModel, task: EngineerRequirements): Record<string, number>;
  trackingKind: string;
  requiredTracking?: readonly string[];
  binding(loop: ControlLoopDefinition): TransferBinding;
  states(binding: TransferBinding, observed: TransferStatus): Record<string, number>;
  tracking(loop: ControlLoopDefinition, binding: TransferBinding, initialize?: boolean): TransferTracking;
}
