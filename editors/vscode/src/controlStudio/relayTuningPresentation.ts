export interface RelayTuningPresentation {
  readonly phase: "hidden" | "ready" | "calculated" | "not-qualified";
  readonly visible: boolean;
  readonly calculateVisible: boolean;
  readonly calculateEnabled: boolean;
  readonly candidateVisible: boolean;
  readonly cycles: number | undefined;
  readonly status: string;
  readonly candidate: RelayPiCandidateView | undefined;
}

export interface RelayPiCandidateView {
  readonly proportionalGain: number;
  readonly integralTimeSeconds: number;
  readonly integralGain: number;
  readonly applied: false;
}

export function initialRelayTuningPresentation(): RelayTuningPresentation {
  return {
    phase: "hidden", visible: false, calculateVisible: false, calculateEnabled: false,
    candidateVisible: false, cycles: undefined, status: "", candidate: undefined,
  };
}

/** Reduces terminal relay evidence and the later explicit Calculate response into Test-pane state. */
export function reduceRelayTuningPresentation(
  previous: RelayTuningPresentation,
  phase: string,
  payload: Record<string, unknown> | undefined,
): RelayTuningPresentation {
  const relay = record(payload?.relayTuning) ? payload!.relayTuning : undefined;
  if (phase === "terminal") {
    if (relay?.status === "ready" && finitePositiveInteger(relay.cycles) && typeof relay.message === "string") {
      return {
        phase: "ready", visible: true, calculateVisible: true, calculateEnabled: true,
        candidateVisible: false, cycles: relay.cycles, status: relay.message, candidate: undefined,
      };
    }
    if (relay?.status === "not_qualified" && typeof relay.reason === "string") {
      return {
        phase: "not-qualified", visible: true, calculateVisible: false, calculateEnabled: false,
        candidateVisible: false, cycles: undefined, status: `Relay candidate unavailable: ${relay.reason}`, candidate: undefined,
      };
    }
    return initialRelayTuningPresentation();
  }
  if (phase === "relay-candidate") {
    const candidate = relayCandidate(relay?.candidate);
    if (relay?.status === "qualified" && candidate) {
      return {
        phase: "calculated", visible: true, calculateVisible: false, calculateEnabled: false,
        candidateVisible: true, cycles: previous.cycles,
        status: "Relay candidate calculated from the retained feedback cycles. It is an unapplied PI proposal; tune and verify it before Apply.",
        candidate,
      };
    }
    if (relay?.status === "not_qualified" && typeof relay.reason === "string") {
      return {
        phase: "not-qualified", visible: true, calculateVisible: false, calculateEnabled: false,
        candidateVisible: false, cycles: previous.cycles, status: `Relay candidate unavailable: ${relay.reason}`, candidate: undefined,
      };
    }
  }
  return previous;
}

function relayCandidate(value: unknown): RelayPiCandidateView | undefined {
  if (!record(value) || !finite(value.proportionalGain) || !finite(value.integralTimeSeconds) ||
      !finite(value.integralGain) || value.applied !== false) return undefined;
  return {
    proportionalGain: value.proportionalGain,
    integralTimeSeconds: value.integralTimeSeconds,
    integralGain: value.integralGain,
    applied: false,
  };
}

function finitePositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
