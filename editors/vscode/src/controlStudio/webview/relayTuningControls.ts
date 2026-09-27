import { formatEngineeringNumber } from "../chartContract";
import {
  initialRelayTuningPresentation,
  reduceRelayTuningPresentation,
  type RelayTuningPresentation,
} from "../relayTuningPresentation";

export interface RelayTuningControls {
  receive(phase: string, payload: Record<string, unknown> | undefined): void;
  reset(): void;
}

/** Renders retained feedback-relay evidence and one engineer-requested, unapplied PI calculation. */
export function initializeRelayTuningControls(dependencies: {
  readonly post: (kind: string, payload?: Record<string, unknown>) => void;
}): RelayTuningControls {
  const card = document.querySelector<HTMLElement>("[data-relay-tuning]");
  const status = document.querySelector<HTMLElement>("[data-relay-tuning-status]");
  const cycles = document.querySelector<HTMLElement>("[data-relay-cycles]");
  const calculate = document.querySelector<HTMLButtonElement>("[data-relay-calculate]");
  const candidate = document.querySelector<HTMLElement>("[data-relay-candidate]");
  const proportionalGain = document.querySelector<HTMLElement>("[data-relay-kp]");
  const integralTime = document.querySelector<HTMLElement>("[data-relay-ti]");
  const integralGain = document.querySelector<HTMLElement>("[data-relay-ki]");
  let state = initialRelayTuningPresentation();

  calculate?.addEventListener("click", () => {
    if (!state.calculateEnabled) return;
    calculate.disabled = true;
    if (status) status.textContent = "Calculating the unapplied PI candidate from the retained feedback-relay cycles…";
    dependencies.post("control-studio.relay.calculate");
  });

  const render = () => {
    if (card) card.hidden = !state.visible;
    if (status) status.textContent = state.status;
    if (cycles) cycles.textContent = state.cycles === undefined ? "—" : String(state.cycles);
    if (calculate) {
      calculate.hidden = !state.calculateVisible;
      calculate.disabled = !state.calculateEnabled;
    }
    if (candidate) candidate.hidden = !state.candidateVisible;
    if (state.candidate) {
      if (proportionalGain) proportionalGain.textContent = formatEngineeringNumber(state.candidate.proportionalGain, 6);
      if (integralTime) integralTime.textContent = formatEngineeringNumber(state.candidate.integralTimeSeconds, 6);
      if (integralGain) integralGain.textContent = formatEngineeringNumber(state.candidate.integralGain, 6);
    }
  };

  render();
  return {
    receive(phase, payload) {
      state = reduceRelayTuningPresentation(state, phase, payload);
      render();
    },
    reset() {
      state = initialRelayTuningPresentation();
      render();
    },
  };
}
