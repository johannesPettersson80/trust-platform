import React from "react";

import { t, tint } from "./theme";

interface CanvasToolbarProps {
  searchDraft: string;
  onSearch: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onClearSearch: () => void;
  fieldIssueLabel?: string;
  fieldIssueTitle?: string;
  fault?: { label: string; targetNodeId: string };
  visibleFaultCount: number;
  onFocusFault: (nodeId: string) => void;
  filterOpen: boolean;
  onToggleFilter: () => void;
  addTargetLabel?: string;
  addOpen: boolean;
  onAdd: () => void;
  discoverOpen: boolean;
  onToggleDiscover: () => void;
  editMode: boolean;
  onToggleEdit: () => void;
}

// The toolbar owns presentation and keyboard controls only. Drawer selection and
// graph state remain in the canvas coordinator so this component cannot invent
// topology or persistence behavior.
export function CanvasToolbar({
  searchDraft,
  onSearch,
  onClearSearch,
  fieldIssueLabel,
  fieldIssueTitle,
  fault,
  visibleFaultCount,
  onFocusFault,
  filterOpen,
  onToggleFilter,
  addTargetLabel,
  addOpen,
  onAdd,
  discoverOpen,
  onToggleDiscover,
  editMode,
  onToggleEdit,
}: CanvasToolbarProps) {
  return (
    <header
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "10px 16px",
        borderBottom: `1px solid ${t.border}`,
        background: t.surface,
        zIndex: 5,
      }}
    >
      <div
        aria-label="truST"
        title="truST"
        style={{ fontWeight: 700, fontSize: 14, whiteSpace: "nowrap", color: t.text, letterSpacing: 0.2 }}
      >
        tru<span style={{ color: t.accent }}>ST</span>
      </div>
      <input
        onChange={onSearch}
        value={searchDraft}
        placeholder="Search nodes, links, faults"
        style={{
          flex: "1 1 240px",
          minWidth: 0,
          background: t.inputBg,
          border: `1px solid ${t.inputBorder}`,
          borderRadius: t.radius,
          color: t.text,
          padding: "6px 10px",
          fontSize: 12,
        }}
      />
      {searchDraft.trim().length > 0 && (
        <button onClick={onClearSearch} title="Clear search" style={toolbarButton(false)}>
          Clear search
        </button>
      )}
      {fieldIssueLabel ? (
        <span style={ISSUE_PILL} title={fieldIssueTitle}>
          {fieldIssueLabel}
        </span>
      ) : fault ? (
        <button
          onClick={() => onFocusFault(fault.targetNodeId)}
          style={{ ...ISSUE_PILL, cursor: "pointer" }}
          title={fault.label}
        >
          {visibleFaultCount} issue{visibleFaultCount === 1 ? "" : "s"} · {fault.label}
        </button>
      ) : null}
      <button
        onClick={onToggleFilter}
        title="Filter connections by protocol"
        style={toolbarButton(filterOpen)}
      >
        Filter
      </button>
      <button
        data-canvas-add
        onClick={onAdd}
        disabled={!addTargetLabel}
        title={
          addTargetLabel
            ? `Add a device or connection to ${addTargetLabel}`
            : "Open or set up a runtime before adding a device or connection"
        }
        style={toolbarButton(addOpen, "primary", !addTargetLabel)}
      >
        + Add
      </button>
      <button
        onClick={onToggleDiscover}
        title="Find devices on the network"
        style={toolbarButton(discoverOpen)}
      >
        Discover
      </button>
      <button
        onClick={onToggleEdit}
        title="Edit mode: shows + on each runtime to add a device or service"
        style={toolbarButton(editMode)}
      >
        {editMode ? "Done" : "Edit"}
      </button>
    </header>
  );
}

function toolbarButton(
  active: boolean,
  variant: "default" | "primary" = "default",
  disabled = false
): React.CSSProperties {
  return {
    border: `1px solid ${active || variant === "primary" ? t.accent : t.border}`,
    background:
      variant === "primary"
        ? t.accent
        : active
          ? tint(t.accent, 0.14)
          : "transparent",
    color: disabled ? t.textSubtle : variant === "primary" ? t.onAccent : t.text,
    borderRadius: t.radius,
    padding: "6px 12px",
    fontSize: 12,
    fontWeight: variant === "primary" ? 650 : 500,
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.62 : 1,
    whiteSpace: "nowrap",
    transition: `background ${t.ease}, border-color ${t.ease}`,
  };
}

const ISSUE_PILL: React.CSSProperties = {
  border: `1px solid ${tint(t.danger, 0.5)}`,
  background: tint(t.danger, 0.12),
  color: t.danger,
  borderRadius: t.radius,
  padding: "6px 10px",
  fontSize: 11,
  fontWeight: 600,
  whiteSpace: "nowrap",
  maxWidth: 360,
  overflow: "hidden",
  textOverflow: "ellipsis",
};
