import { MainWorkflowError } from "./mainWorkflowFailure";
import { decodePidLaw } from "./controllerLaw";
import type { ControlLoopDefinition, ControlPidInitializationAdapter } from "./loopDefinition";

/**
 * Reads the explicit contract for the implementation's retained PID states.
 * It deliberately does not infer state names from an ST program or reuse the
 * proportional transfer adapter.
 */
export function pidInitializationAdapter(loop: ControlLoopDefinition): ControlPidInitializationAdapter {
  const adapter = loop.controller.pidInitialization;
  if (!adapter || adapter.kind !== "parallel-pid-v1" || !decodePidLaw(loop.controller.law, loop.parameterBinding) || !loop.signals.setpoint) {
    throw new MainWorkflowError("Typed PI/PID initialization adapter is required for Auto commissioning; choose Manual staging after Verify.",
      { cause: "controller-capability", nextAction: "choose-method" });
  }
  return adapter;
}
