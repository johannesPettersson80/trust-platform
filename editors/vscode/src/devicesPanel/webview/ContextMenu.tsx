// The runtime's right-click menu: the actions of `Runtime.context_menu`, in their order, drawn the
// same way as the inspector's (design §4.5 D2, specification 25 §5.11). It draws only the actions
// the view gives it and hands the chosen one back unchanged; it decides no label, enabled flag or
// reason of its own.

import React from "react";

import { InspectorAction } from "./Inspector";
import { contextMenuStyle } from "./styles";
import type { Action } from "./types";

/** Draws the given actions as a menu at the pointer, in their order. */
export function ContextMenu({
  actions,
  x,
  y,
  onAction,
}: {
  /** The actions to list, in their order. */
  actions: Action[];
  /** The pointer's horizontal position. */
  x: number;
  /** The pointer's vertical position. */
  y: number;
  /** Called with an action, unchanged, when it is chosen. */
  onAction: (action: Action) => void;
}) {
  return (
    <div data-role="context-menu" role="menu" style={contextMenuStyle(x, y)}>
      {actions.map((action) => (
        <InspectorAction key={action.id} action={action} onAction={onAction} />
      ))}
    </div>
  );
}
