// The toolbar: one button per action the view gives, in the view's own order, with the action's
// label and enabled state exactly as Rust sent them (design §2.2 and §4.5 D1).

import React from "react";

import { toolbarButtonStyle, toolbarStyle } from "./styles";
import type { Action, Toolbar as ToolbarModel } from "./types";

/** Draws every toolbar action the view gives and posts the chosen one back unchanged. */
export function Toolbar({
  toolbar,
  onAction,
}: {
  /** The toolbar's actions, in the order Rust declares them. */
  toolbar: ToolbarModel;
  /** Called with the chosen action; `App` posts it to the extension. */
  onAction: (action: Action) => void;
}) {
  return (
    <div role="toolbar" aria-label="Devices & Connections" style={toolbarStyle}>
      {Object.values(toolbar).map((action) => (
        <button
          key={action.id}
          type="button"
          data-toolbar-item={action.id}
          disabled={!action.enabled}
          title={action.label}
          onClick={() => {
            if (action.enabled) {
              onAction(action);
            }
          }}
          style={toolbarButtonStyle(action.emphasis, action.enabled)}
        >
          {action.label}
        </button>
      ))}
    </div>
  );
}
