import { createContext, useContext } from "react";

// A slot the user can fill in Edit mode. `add` = what gets created; `targetId` = the parent it
// attaches to (runtime id for a device, host id for a runtime, undefined for a host).
export interface AddSlotRequest {
  add: "device" | "runtime" | "host";
  targetId?: string;
}

// Edit mode: a secondary topology-placement toggle. The default first-user add path is the toolbar
// + Add button; when Edit is on, layout.ts emits dashed empty-slot buttons for precise placement
// (per runtime: add connection; per host: set up runtime; canvas: add host).
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
