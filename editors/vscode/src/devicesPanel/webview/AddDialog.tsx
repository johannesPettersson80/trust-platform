// The Add dialog, drawn from the Rust view model (design §4.5 "D3: forms and the Add dialog",
// specification 25 §5.9 and §5.14). Rust gives the title, the target runtime and the runtimes to
// choose from, the discover entry and its help, the finds of the last search, the `Runtimes` group,
// the catalog's groups, the advanced toggle, the quick starts and, when the catalog is missing, the
// notice that replaces the groups; this component draws them in that order and decides no order,
// word, count or rule. It keeps one piece of view state: whether the advanced groups are shown
// (§5.9's toggle). Choosing an entry hands its action back unchanged (design §2.1).

import React from "react";

import type {
  AddDialog as AddDialogModel,
  AddEntry,
  AddGroup,
  FoundEntry,
  QuickStart,
} from "./addTypes";
import { rowLabelStyle, rowStyle, rowValueStyle, sectionHeadingStyle, sectionStyle } from "./styles";
import {
  addActionButtonStyle,
  addAdvancedCountStyle,
  addAdvancedLabelStyle,
  addAdvancedToggleStyle,
  addBodyStyle,
  addCatalogNoticeStyle,
  addChoiceStyle,
  addChoicesStyle,
  addCloseStyle,
  addDialogStyle,
  addDiscoverStyle,
  addEntryBadgeStyle,
  addEntryPurposeStyle,
  addEntryReasonStyle,
  addEntryStyle,
  addEntryTitleStyle,
  addFoundEntryStyle,
  addFoundTitleStyle,
  addHeaderStyle,
  addHelpStyle,
  addTargetStyle,
  addTitleStyle,
} from "./styles/addDialog";
import type { Action } from "./types";

/** Chooses an action the way the rest of the panel does: a disabled action decides nothing. */
function choose(action: Action, onAction: (action: Action) => void): void {
  if (action.enabled) {
    onAction(action);
  }
}

/**
 * Draws the Add dialog: its title; the target runtime and the runtimes to choose from; `Discover`
 * and its help; `Found on the network` and each find; the `Runtimes` group, first; the catalog's
 * groups in their order, or, when the catalog is missing, its notice; the advanced toggle; and the
 * quick starts. An advanced group stays hidden until the toggle shows it (specification 25 §5.9 and
 * §5.14).
 */
export function AddDialog({
  dialog,
  onAction,
  onClose,
}: {
  /** The dialog, as the view gives it. */
  dialog: AddDialogModel;
  /** Called with the chosen action, unchanged. */
  onAction: (action: Action) => void;
  /** Called when the close button is chosen. */
  onClose: () => void;
}) {
  const [showAdvanced, setShowAdvanced] = React.useState(false);
  const target = dialog.target;
  const toggle = dialog.advanced_toggle;
  return (
    <div data-role="add-dialog" style={addDialogStyle}>
      <header style={addHeaderStyle}>
        <span data-role="add-title" title={dialog.title} style={addTitleStyle}>
          {dialog.title}
        </span>
        <button type="button" data-role="add-close" aria-label={dialog.close_label} onClick={onClose} style={addCloseStyle}>
          ✕
        </button>
      </header>
      <div style={addBodyStyle}>
        {target !== undefined && (
          <div data-role="add-target" style={addTargetStyle}>
            {target.name}
          </div>
        )}
        {dialog.runtime_choices.length > 0 && (
          <div data-role="add-runtime-choices" style={addChoicesStyle}>
            {dialog.runtime_choices.map((choice) => (
              <span
                key={choice.id}
                data-role="add-runtime-choice"
                data-runtime={choice.id}
                data-selected={choice.id === target?.id ? "true" : "false"}
                style={addChoiceStyle(choice.id === target?.id)}
              >
                {choice.name}
              </span>
            ))}
          </div>
        )}
        <div data-role="add-discover" style={addDiscoverStyle}>
          <button
            type="button"
            data-role="add-discover-button"
            data-action={dialog.discover.id}
            disabled={!dialog.discover.enabled}
            title={dialog.discover.reason ?? dialog.discover.label}
            onClick={() => choose(dialog.discover, onAction)}
            style={addActionButtonStyle(dialog.discover.enabled)}
          >
            {dialog.discover.label}
          </button>
          <span data-role="add-discover-help" style={addHelpStyle}>
            {dialog.discover_help}
          </span>
        </div>
        {dialog.found.length > 0 && (
          <section data-role="add-found" style={sectionStyle}>
            <h3 data-role="add-found-heading" style={sectionHeadingStyle}>
              {dialog.found_heading}
            </h3>
            {dialog.found.map((entry) => (
              <Found key={entry.card} entry={entry} onAction={onAction} />
            ))}
          </section>
        )}
        <Group group={dialog.runtimes} onAction={onAction} />
        {dialog.catalog_state !== undefined ? (
          <div data-role="add-catalog-notice" style={addCatalogNoticeStyle}>
            {dialog.catalog_state}
          </div>
        ) : (
          dialog.groups
            .filter((group) => !group.advanced || showAdvanced)
            .map((group) => <Group key={group.label} group={group} onAction={onAction} />)
        )}
        {toggle !== undefined && (
          <button
            type="button"
            data-role="add-advanced-toggle"
            data-shown={showAdvanced ? "true" : "false"}
            onClick={() => setShowAdvanced((shown) => !shown)}
            style={addAdvancedToggleStyle}
          >
            <span data-role="add-advanced-label" style={addAdvancedLabelStyle}>
              {showAdvanced ? toggle.hide_label : toggle.show_label}
            </span>
            <span data-role="add-advanced-count" style={addAdvancedCountStyle}>
              {toggle.count}
            </span>
          </button>
        )}
        {dialog.quick_starts.length > 0 && (
          <section data-role="add-quick-starts" style={sectionStyle}>
            {dialog.quick_starts.map((start) => (
              <Quick key={start.action.id} start={start} onAction={onAction} />
            ))}
          </section>
        )}
      </div>
    </div>
  );
}

/** One group of the dialog: its own label and its entries, in the view's order. */
function Group({ group, onAction }: { group: AddGroup; onAction: (action: Action) => void }) {
  return (
    <section
      data-role="add-group"
      data-group={group.label}
      data-advanced={group.advanced ? "true" : "false"}
      style={sectionStyle}
    >
      <h3 data-role="add-group-label" style={sectionHeadingStyle}>
        {group.label}
      </h3>
      {group.entries.map((entry) => (
        <Entry key={entry.protocol} entry={entry} onAction={onAction} />
      ))}
    </section>
  );
}

/**
 * One entry: its own title, badge and purpose, and the action it posts unchanged. A disabled entry
 * stays visible with its reason, as the rest of the panel draws a disabled action (specification 25
 * §5.2 and N24).
 */
function Entry({ entry, onAction }: { entry: AddEntry; onAction: (action: Action) => void }) {
  const action = entry.action;
  return (
    <button
      type="button"
      data-role="add-entry"
      data-entry={entry.protocol}
      data-action={action.id}
      disabled={!action.enabled}
      title={action.reason ?? entry.title}
      onClick={() => choose(action, onAction)}
      style={addEntryStyle(action.enabled)}
    >
      <span data-role="add-entry-title" style={addEntryTitleStyle}>
        {entry.title}
      </span>
      <span data-role="add-entry-badge" style={addEntryBadgeStyle}>
        {entry.badge}
      </span>
      <span data-role="add-entry-purpose" style={addEntryPurposeStyle}>
        {entry.purpose}
      </span>
      {!action.enabled && action.reason !== undefined && (
        <span data-role="add-entry-reason" style={addEntryReasonStyle}>
          {action.reason}
        </span>
      )}
    </button>
  );
}

/** One quick start: its own title and description, and the action it posts unchanged. */
function Quick({ start, onAction }: { start: QuickStart; onAction: (action: Action) => void }) {
  const action = start.action;
  return (
    <button
      type="button"
      data-role="add-quick-start"
      data-action={action.id}
      disabled={!action.enabled}
      title={action.reason ?? start.title}
      onClick={() => choose(action, onAction)}
      style={addEntryStyle(action.enabled)}
    >
      <span data-role="add-quick-start-title" style={addEntryTitleStyle}>
        {start.title}
      </span>
      <span data-role="add-quick-start-description" style={addEntryPurposeStyle}>
        {start.description}
      </span>
    </button>
  );
}

/** One find of the last search: its own title, its rows and its action. */
function Found({ entry, onAction }: { entry: FoundEntry; onAction: (action: Action) => void }) {
  const action = entry.action;
  return (
    <div data-role="add-found-entry" data-card={entry.card} style={addFoundEntryStyle}>
      <span data-role="add-found-title" style={addFoundTitleStyle}>
        {entry.title}
      </span>
      {entry.rows.map((row, index) => (
        <div key={`${row.label}-${index}`} data-role="add-found-row" style={rowStyle(row.wide)}>
          <span data-role="add-found-row-label" style={rowLabelStyle}>
            {row.label}
          </span>
          <span data-role="add-found-row-value" data-tone={row.tone} style={rowValueStyle(row.tone, row.wide)}>
            {row.value}
          </span>
        </div>
      ))}
      <button
        type="button"
        data-role="add-found-action"
        data-action={action.id}
        disabled={!action.enabled}
        title={action.reason ?? action.label}
        onClick={() => choose(action, onAction)}
        style={addActionButtonStyle(action.enabled)}
      >
        {action.label}
      </button>
    </div>
  );
}
