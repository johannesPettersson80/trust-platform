// An Edit slot: one button per action the view offers, drawn with the action's own label and
// enabled state. Choosing it hands the action back exactly as the view gave it (design §4.5 D1).

import React from "react";

import { slotButtonStyle } from "../styles";
import type { Action } from "../types";

/** Draws one slot and hands its action back unchanged. */
export function Slot({
  action,
  onAction,
}: {
  /** The slot's action. */
  action: Action;
  /** Called with the action when the slot is chosen. */
  onAction?: (action: Action) => void;
}) {
  return (
    <button
      type="button"
      data-slot={action.id}
      disabled={!action.enabled}
      title={action.label}
      onClick={() => {
        if (action.enabled) {
          onAction?.(action);
        }
      }}
      style={slotButtonStyle(action.emphasis, action.enabled)}
    >
      {action.label}
    </button>
  );
}
