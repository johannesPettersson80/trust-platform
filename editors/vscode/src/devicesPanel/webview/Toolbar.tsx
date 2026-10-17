// The toolbar: one button per action the view gives, in the view's own order, with the action's
// label and enabled state exactly as Rust sent them (design §2.2 and §4.5 D1). Before them it draws
// the issues button, whose text and tone come from `Issues.button`, and, while the view has a list,
// the `Topology`/`List` switch, whose two words come from `Toolbar.view_switch`; which view is shown
// stays the panel's own state (specification 25 §5.11, design §3.4).

import React from "react";
import {
  issuesButtonStyle,
  toolbarButtonStyle,
  toolbarStyle,
  viewSwitchButtonStyle,
  viewSwitchStyle,
} from "./styles";
import type { Action, IssuesButton, PanelView, Toolbar as ToolbarModel } from "./types";

/** Draws the issues button, the view switch and every toolbar action the view gives. */
export function Toolbar({
  toolbar,
  issues,
  view,
  onSelectView,
  onOpenIssues,
  onAction,
}: {
  /** The toolbar's actions, in the order Rust declares them. */
  toolbar: ToolbarModel;
  /** The issues button, when the view gives one. */
  issues?: IssuesButton;
  /** The shown view, when the view has a list to switch to. */
  view?: PanelView;
  /** Called with the view the switch chooses. */
  onSelectView?: (view: PanelView) => void;
  /** Called when the issues button is chosen. */
  onOpenIssues?: () => void;
  /** Called with the chosen action; `App` posts it to the extension. */
  onAction: (action: Action) => void;
}) {
  return (
    <div role="toolbar" aria-label="Devices & Connections" style={toolbarStyle}>
      {issues !== undefined && (
        <button
          type="button"
          data-role="issues-button"
          data-tone={issues.tone}
          title={issues.text}
          onClick={() => onOpenIssues?.()}
          style={issuesButtonStyle(issues.tone)}
        >
          {issues.text}
        </button>
      )}
      {view !== undefined && (
        <div data-role="view-switch" style={viewSwitchStyle}>
          <button
            type="button"
            data-view="topology"
            data-selected={view === "topology" ? "true" : "false"}
            onClick={() => onSelectView?.("topology")}
            style={viewSwitchButtonStyle(view === "topology")}
          >
            {toolbar.view_switch?.topology}
          </button>
          <button
            type="button"
            data-view="list"
            data-selected={view === "list" ? "true" : "false"}
            onClick={() => onSelectView?.("list")}
            style={viewSwitchButtonStyle(view === "list")}
          >
            {toolbar.view_switch?.list}
          </button>
        </div>
      )}
      {[toolbar.add, toolbar.discover, toolbar.add_host_slot].map((action) => (
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
