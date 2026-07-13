import React from "react";

import type { NCFault } from "./types";
import { issuePillStyle, toolbarButtonStyle } from "./networkCanvasStyles";
import { t } from "./theme";

export function NetworkCanvasHeader({
  searchValue,
  onSearchChange,
  onClearSearch,
  fieldIssueCount,
  fieldIssueMessage,
  fault,
  faultCount,
  onFocusFault,
  filterActive,
  onToggleFilter,
  addActive,
  addTargetLabel,
  onAdd,
  discoverActive,
  onToggleDiscover,
  editActive,
  onToggleEdit,
}: {
  searchValue: string;
  onSearchChange: (value: string) => void;
  onClearSearch: () => void;
  fieldIssueCount: number;
  fieldIssueMessage?: string;
  fault?: NCFault;
  faultCount: number;
  onFocusFault: (nodeId: string) => void;
  filterActive: boolean;
  onToggleFilter: () => void;
  addActive: boolean;
  addTargetLabel?: string;
  onAdd: () => void;
  discoverActive: boolean;
  onToggleDiscover: () => void;
  editActive: boolean;
  onToggleEdit: () => void;
}) {
  const fieldIssueLabel =
    fieldIssueCount > 0
      ? `${fieldIssueCount} field issue${fieldIssueCount === 1 ? "" : "s"} · fix highlighted fields`
      : undefined;
  const fieldIssueTitle =
    fieldIssueCount > 0
      ? fieldIssueMessage || "Fix the highlighted fields and try again."
      : undefined;

  return (
    <header style={HEADER}>
      <div aria-label="truST" title="truST" style={BRAND}>
        tru<span style={{ color: t.accent }}>ST</span>
      </div>
      <input
        onChange={(event) => onSearchChange(event.target.value)}
        value={searchValue}
        placeholder="Search nodes, links, faults"
        style={SEARCH}
      />
      {searchValue.trim().length > 0 && (
        <button
          onClick={onClearSearch}
          title="Clear search"
          style={toolbarButtonStyle(false)}
        >
          Clear search
        </button>
      )}
      {fieldIssueLabel ? (
        <span style={issuePillStyle} title={fieldIssueTitle}>
          {fieldIssueLabel}
        </span>
      ) : fault ? (
        <button
          onClick={() => onFocusFault(fault.targetNodeId)}
          style={{ ...issuePillStyle, cursor: "pointer" }}
          title={fault.label}
        >
          {faultCount} issue{faultCount === 1 ? "" : "s"} · {fault.label}
        </button>
      ) : null}
      <button
        onClick={onToggleFilter}
        title="Filter connections by protocol"
        style={toolbarButtonStyle(filterActive)}
      >
        Filter
      </button>
      <button
        onClick={onToggleDiscover}
        title="Find ADS devices on this computer and the local network"
        style={toolbarButtonStyle(discoverActive, "primary")}
      >
        Discover ADS devices
      </button>
      <button
        onClick={onAdd}
        disabled={!addTargetLabel}
        title={
          addTargetLabel
            ? `Add a device or connection to ${addTargetLabel}`
            : "Open or set up a runtime before adding a device or connection"
        }
        style={toolbarButtonStyle(addActive, "default", !addTargetLabel)}
      >
        + Add
      </button>
      <button
        onClick={onToggleEdit}
        title="Edit mode: shows + on each runtime to add a device or service"
        style={toolbarButtonStyle(editActive)}
      >
        {editActive ? "Done" : "Edit"}
      </button>
    </header>
  );
}

const HEADER: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 12,
  padding: "10px 16px",
  borderBottom: `1px solid ${t.border}`,
  background: t.surface,
  zIndex: 5,
};

const BRAND: React.CSSProperties = {
  fontWeight: 700,
  fontSize: 14,
  whiteSpace: "nowrap",
  color: t.text,
  letterSpacing: 0.2,
};

const SEARCH: React.CSSProperties = {
  flex: "1 1 240px",
  minWidth: 0,
  background: t.inputBg,
  border: `1px solid ${t.inputBorder}`,
  borderRadius: t.radius,
  color: t.text,
  padding: "6px 10px",
  fontSize: 12,
};
