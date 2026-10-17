// The inspector: every fact and every action of the selected item, drawn from the Rust view model
// (design §2.1 and §4.5 D2). The breadcrumb, title, kind, health, sections, rows and actions are
// drawn exactly as the JSON gives them; the inspector decides no label, enabled flag or reason.
// Choosing an action hands it back unchanged, and a two-step action asks its confirmation first.

import React from "react";

import {
  actionCellStyle,
  actionReasonStyle,
  confirmButtonStyle,
  confirmRowStyle,
  confirmStyle,
  confirmTextStyle,
  copyButtonStyle,
  inspectorActionStyle,
  inspectorActionsStyle,
  inspectorBreadcrumbStyle,
  inspectorCloseStyle,
  inspectorHeaderStyle,
  inspectorHeadingStyle,
  inspectorHealthStyle,
  inspectorKindStyle,
  inspectorStyle,
  inspectorTitleStyle,
  rowLabelStyle,
  rowStyle,
  rowValueStyle,
  sectionHeadingStyle,
  sectionStyle,
} from "./styles";
import type { Action, Inspector as InspectorModel, Row } from "./types";

/** Copies a row's text to the clipboard; the webview's own clipboard, no rule of the panel. */
function copyToClipboard(text: string): void {
  void window.navigator.clipboard.writeText(text);
}

/**
 * One action, drawn by its own `emphasis` and `enabled`, with its reason when it has one and its
 * confirmation step when it is a two-step action. It is shared by the inspector and the right-click
 * menu, so both draw the same action the same way (design §4.5 D2).
 */
export function InspectorAction({
  action,
  onAction,
}: {
  /** The action, as the view gives it. */
  action: Action;
  /** Called with the action, unchanged, when it is chosen. */
  onAction: (action: Action) => void;
}) {
  const [confirming, setConfirming] = React.useState(false);
  const confirm = action.confirm;
  if (confirming && confirm !== undefined) {
    return (
      <div data-role="action-confirm" data-action={action.id} style={confirmStyle}>
        <span data-role="confirm-text" style={confirmTextStyle}>
          {confirm.text}
        </span>
        <div style={confirmRowStyle}>
          <button
            type="button"
            data-role="confirm-yes"
            style={confirmButtonStyle(true)}
            onClick={() => {
              setConfirming(false);
              onAction(action);
            }}
          >
            {confirm.confirm}
          </button>
          <button
            type="button"
            data-role="confirm-cancel"
            style={confirmButtonStyle(false)}
            onClick={() => setConfirming(false)}
          >
            {confirm.cancel}
          </button>
        </div>
      </div>
    );
  }
  return (
    <div
      data-role="inspector-action"
      data-action={action.id}
      data-emphasis={action.emphasis}
      style={actionCellStyle}
    >
      <button
        type="button"
        data-role="action-button"
        disabled={!action.enabled}
        title={action.reason ?? action.label}
        onClick={() => {
          if (!action.enabled) {
            return;
          }
          if (confirm !== undefined) {
            setConfirming(true);
          } else {
            onAction(action);
          }
        }}
        style={inspectorActionStyle(action.emphasis, action.enabled)}
      >
        {action.label}
      </button>
      {action.reason !== undefined && (
        <span data-role="action-reason" style={actionReasonStyle}>
          {action.reason}
        </span>
      )}
    </div>
  );
}

/** One inspector row: its label, its value and, when it carries one, its copy button. */
function InspectorRow({ row }: { row: Row }) {
  const copy = row.copy;
  return (
    <div data-role="inspector-row" data-wide={row.wide ? "true" : "false"} style={rowStyle(row.wide)}>
      <span data-role="row-label" style={rowLabelStyle}>
        {row.label}
      </span>
      <span data-role="row-value" data-tone={row.tone} style={rowValueStyle(row.tone, row.wide)}>
        {row.value}
      </span>
      {copy !== undefined && (
        <button
          type="button"
          data-role="row-copy"
          aria-label="Copy"
          style={copyButtonStyle}
          onClick={() => copyToClipboard(copy)}
        >
          Copy
        </button>
      )}
    </div>
  );
}

/** Draws one item's inspector: its breadcrumb, title, kind, health, facts and actions. */
export function Inspector({
  inspector,
  onAction,
  onClose,
}: {
  /** The inspector, as the view gives it. */
  inspector: InspectorModel;
  /** Called with an action, unchanged, when it is chosen. */
  onAction: (action: Action) => void;
  /** Called when the close button is chosen. */
  onClose: () => void;
}) {
  return (
    <aside data-role="inspector" style={inspectorStyle}>
      <header style={inspectorHeaderStyle}>
        <div style={inspectorHeadingStyle}>
          <span data-role="inspector-breadcrumb" title={inspector.breadcrumb} style={inspectorBreadcrumbStyle}>
            {inspector.breadcrumb}
          </span>
          <span data-role="inspector-title" title={inspector.title} style={inspectorTitleStyle}>
            {inspector.title}
          </span>
          <span data-role="inspector-kind" style={inspectorKindStyle}>
            {inspector.kind}
          </span>
        </div>
        {inspector.health !== undefined && (
          <span
            data-role="inspector-health"
            data-tone={inspector.health}
            style={inspectorHealthStyle(inspector.health)}
          />
        )}
        <button
          type="button"
          data-role="inspector-close"
          aria-label="Close inspector"
          onClick={onClose}
          style={inspectorCloseStyle}
        >
          ✕
        </button>
      </header>
      <div data-role="inspector-actions" style={inspectorActionsStyle}>
        {inspector.primary.map((action) => (
          <InspectorAction key={`primary-${action.id}`} action={action} onAction={onAction} />
        ))}
        {inspector.secondary.map((action) => (
          <InspectorAction key={`secondary-${action.id}`} action={action} onAction={onAction} />
        ))}
      </div>
      {inspector.sections.map((section, index) => (
        <section
          key={section.heading ?? `section-${index}`}
          data-role="inspector-section"
          style={sectionStyle}
        >
          {section.heading !== undefined && (
            <h3 data-role="section-heading" style={sectionHeadingStyle}>
              {section.heading}
            </h3>
          )}
          {section.rows.map((row, rowIndex) => (
            <InspectorRow key={`${row.label}-${rowIndex}`} row={row} />
          ))}
        </section>
      ))}
    </aside>
  );
}
