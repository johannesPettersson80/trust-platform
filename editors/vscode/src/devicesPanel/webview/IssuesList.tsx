// The issues list: every fault and warning of the view, grouped by item (specification 25 §5.11 and
// §5.14). The button's text and tone, each group's item name, and each issue's severity, text,
// `Show` and recovery action are drawn exactly as the Rust view gives them; the list decides no
// label, count, order or state of its own. Choosing an action (the `Show` or the recovery) hands it
// back unchanged, so its `op` reaches the extension untouched.

import React from "react";

import { InspectorAction } from "./Inspector";
import {
  inspectorCloseStyle,
  issueGroupNameStyle,
  issueGroupStyle,
  issuesActionsStyle,
  issuesHeaderStyle,
  issuesListStyle,
  issuesTitleStyle,
  issueSeverityStyle,
  issueStyle,
  issueTextStyle,
} from "./styles";
import type { Action, Issues } from "./types";

/** Draws the issues list and hands the chosen `Show` or recovery action back unchanged. */
export function IssuesList({
  issues,
  onAction,
  onClose,
}: {
  /** The issues button and list, as the view gives them. */
  issues: Issues;
  /** Called with an action, unchanged, when it is chosen. */
  onAction: (action: Action) => void;
  /** Called when the close button is chosen. */
  onClose: () => void;
}) {
  return (
    <aside data-role="issues-list" style={issuesListStyle}>
      <header style={issuesHeaderStyle}>
        <span
          data-role="issues-title"
          data-tone={issues.button.tone}
          style={issuesTitleStyle(issues.button.tone)}
        >
          {issues.button.text}
        </span>
        <button
          type="button"
          data-role="issues-close"
          aria-label="Close the issues list"
          onClick={onClose}
          style={inspectorCloseStyle}
        >
          ✕
        </button>
      </header>
      {issues.groups.map((group, index) => (
        <section
          key={group.item ?? `issue-group-${index}`}
          data-role="issue-group"
          data-item-name={group.item_name}
          style={issueGroupStyle}
        >
          <h3 data-role="issue-group-name" style={issueGroupNameStyle}>
            {group.item_name}
          </h3>
          {group.issues.map((issue) => (
            <div key={issue.id} data-role="issue" data-severity={issue.severity} style={issueStyle}>
              <span
                data-role="issue-severity"
                data-severity={issue.severity}
                style={issueSeverityStyle}
              >
                {issue.severity}
              </span>
              <span data-role="issue-text" style={issueTextStyle}>
                {issue.text}
              </span>
              <div style={issuesActionsStyle}>
                {issue.show !== undefined && (
                  <InspectorAction action={issue.show} onAction={onAction} />
                )}
                {issue.recovery !== undefined && (
                  <InspectorAction action={issue.recovery} onAction={onAction} />
                )}
              </div>
            </div>
          ))}
        </section>
      ))}
    </aside>
  );
}
