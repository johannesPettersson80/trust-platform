// The right pane of the visual editors: its width persists per editor kind (specification 25
// §11.4, specification 16 §6). The editors have no runtime pane of their own (specification 17 §5).

export type RightPaneEditorKind = "ladder" | "statechart" | "blockly" | "sfc";

export interface RightPaneResizeConfig {
  minWidth: number;
  maxWidth: number;
  defaultWidth: number;
  storageKey: string;
}

export interface RightPaneResizeState {
  width: number;
  isResizing: boolean;
}
