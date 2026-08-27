import type {
  PlantContractDecodeResult,
  PlantContractErrorDecodeResult,
  PlantContractMessage,
} from "./contract";

export interface PlantViewState {
  confirmedSaved?: PlantContractMessage;
  unsavedEdits: Readonly<Record<string, unknown>>;
  liveStatus?: PlantContractMessage;
  trace?: PlantContractMessage;
  revisionConflict?: PlantContractMessage;
  availability: "available" | "unavailable";
  unavailableReason?: string;
}

export function applyPlantContractDecode(
  state: PlantViewState,
  decoded: PlantContractDecodeResult | PlantContractErrorDecodeResult
): PlantViewState {
  if (decoded.kind === "status_unavailable") {
    return {
      ...state,
      liveStatus: undefined,
      trace: undefined,
      availability: "unavailable",
      unavailableReason: decoded.reason,
    };
  }
  if (decoded.kind === "accepted_error") {
    if (decoded.code === "revision_conflict") {
      return { ...state, revisionConflict: decoded.message };
    }
    if (decoded.code === "status_unavailable" || decoded.code === "trace_disabled") {
      return {
        ...state,
        liveStatus: undefined,
        trace: undefined,
        availability: "unavailable",
        unavailableReason: decoded.code,
      };
    }
    return state;
  }

  switch (decoded.message.operation) {
    case "plant.read":
    case "plant.save":
      return { ...state, confirmedSaved: decoded.message, revisionConflict: undefined };
    case "plant.status":
      return {
        ...state,
        liveStatus: decoded.message,
        availability: "available",
        unavailableReason: undefined,
      };
    case "plant.trace.read":
      return { ...state, trace: decoded.message };
    case "plant.trace.clear":
      return { ...state, trace: undefined };
    default:
      return state;
  }
}
