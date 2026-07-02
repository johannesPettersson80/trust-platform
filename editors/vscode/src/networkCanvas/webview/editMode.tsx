import { createContext, useContext } from "react";

// A slot the user can fill in Edit mode. `add` = what gets created; `targetId` = the parent it
// attaches to (runtime id for a device, host id for a runtime, undefined for a host).
export interface AddSlotRequest {
  add: "device" | "runtime" | "host";
  targetId?: string;
}

// Edit mode (LOCKED 2026-06-18, spec §0.4): a toolbar toggle. When ON, layout.ts emits dashed
// EMPTY-SLOT buttons on the canvas (per runtime: add connection; per host: set up runtime;
// canvas: add host). Clicking a slot calls onPickSlot → the app opens the matching right pane.
// This is NOT a "+" button in a node header — the affordance is the explicit empty-slot button.
export interface EditModeValue {
  editMode: boolean;
  onPickSlot: (slot: AddSlotRequest) => void;
}

export const EditModeContext = createContext<EditModeValue>({
  editMode: false,
  onPickSlot: () => {},
});

export function useEditMode(): EditModeValue {
  return useContext(EditModeContext);
}
