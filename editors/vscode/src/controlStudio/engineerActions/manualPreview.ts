import { engineerPrediction } from "../engineerPrediction";
import { isControllerFamily } from "../controllerFamilies";
import type { ControlLoopDefinition } from "../loopDefinition";
import { manualControllerCandidate } from "../manualControllerCandidate";
import { classifyMainWorkflowFailure, presentMainWorkflowFailure, type MainWorkflowClassification } from "../mainWorkflowFailure";
import type { EngineerActionHost } from "./host";

/**
 * Manual preview (B4.5): the predicted response of the entered settings on the selected model, from the same prediction
 * Tune draws. It validates every setting against its parameter bounds, writes nothing to the runtime and changes no
 * candidate. The caller answers only the latest request.
 */
export async function previewManualSettings(host: EngineerActionHost, settings: Readonly<Record<string, number>>): Promise<Record<string, unknown>> {
  const model = host.model();
  if (!model) return failed("Identify the plant or supply a model before previewing settings.", { cause: "model-or-data", nextAction: "identify" });
  const loop = host.loop();
  const outside = outOfBounds(loop, settings);
  if (outside) return failed(outside, { cause: "settings", nextAction: "none" });
  try {
    const family = isControllerFamily(host.state.controllerChoice) ? host.state.controllerChoice : host.state.family;
    const { design } = manualControllerCandidate(loop, model, family, settings, host.state.settingsPins?.values);
    const plots = engineerPrediction(model, loop, design.candidate, host.state.task, await host.runningControllerGains());
    return plots
      ? { status: "preview", plots, reason: "Predicted response of the entered settings on the selected model." }
      : { status: "preview", reason: "No predicted response is available for these settings. Validate the controller to check it on the actual ST." };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return failed(reason, classifyMainWorkflowFailure(error));
  }
}

/** The first setting that is not a parameter of this loop, or lies outside its bounds, in words; undefined when all are valid. */
function outOfBounds(loop: ControlLoopDefinition, settings: Readonly<Record<string, number>>): string | undefined {
  const parameters = loop.parameterBinding?.parameters ?? [];
  for (const [symbol, value] of Object.entries(settings)) {
    const parameter = parameters.find(candidate => candidate.symbol === symbol);
    if (!parameter) return `${symbol} is not a setting of this controller.`;
    if (!Number.isFinite(value) || value < parameter.minimum || value > parameter.maximum) {
      return `${symbol} must be between ${parameter.minimum} and ${parameter.maximum} ${parameter.unit}; ${value} is outside that range.`;
    }
  }
  return undefined;
}

/** Answers only the latest of several requests: each `begin` supersedes the requests before it (B4.5). */
export function latestRequestGate(): { begin(): number; isLatest(serial: number): boolean } {
  let serial = 0;
  return { begin: () => ++serial, isLatest: candidate => candidate === serial };
}

function failed(reason: string, failure: MainWorkflowClassification): Record<string, unknown> {
  return { status: "failed", failure: presentMainWorkflowFailure(reason, failure), reason };
}
