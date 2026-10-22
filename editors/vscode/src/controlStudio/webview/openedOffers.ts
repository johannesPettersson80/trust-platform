import type { ControllerOffer, EngineerScenario, LiveSides, RequirementOutput, SetpointChoice } from "../generated/OpenResult";

/**
 * The offers that the engine made when the loop opened (specification 35, C2, C4, B3.6 and B4.3): the controller the Tune page offers,
 * the setpoints a loop without a declared one may take, and the scenario the page adds to its list. The engine computes each of them;
 * this module only keeps the answer for the editors that read it when the engineer uses them.
 */
export interface OpenedOffers {
  controllerOffer?: ControllerOffer | null;
  setpointChoices?: SetpointChoice[] | null;
  scenarioTemplate?: EngineerScenario | null;
  /** The signal a new initial value or timed change of a scenario starts at (B4.3); absent when the loop declares no setpoint. */
  scenarioRowSignal?: string | null;
  /** The text the design form shows for the loop's declared design when the study holds no draft (C4): each matrix as JSON. */
  stateDeclaredText?: Record<string, string> | null;
  /** The signals each live selector offers, inputs and outputs, as the engine names them from the loop (B3.6). */
  liveSides?: LiveSides | null;
  /** The controlled outputs of the Requirements editor, each with the requirements its rows start from (B4.3); empty for a loop without channels. */
  requirementOutputs?: RequirementOutput[] | null;
}

/** The window event that announces each open answer's offers, so that an editor built before the answer refills its choices. */
export const OPENED_OFFERS_EVENT = "trust-control-studio-opened-offers";

let offers: OpenedOffers = {};

/** Holds the offers of the loop that was opened, as the engine answered them. */
export function setOpenedOffers(next: OpenedOffers): void {
  offers = next;
}

/** The offers of the loop that was opened. Empty until an open answer arrives. */
export function openedOffers(): OpenedOffers {
  return offers;
}

/** The label the engine gives a family in the controller selector (C2, C4); the family's own name when the engine offers none. */
export function familyLabelOf(family: string): string {
  return offers.controllerOffer?.families.find(item => item.family === family)?.label ?? family;
}
