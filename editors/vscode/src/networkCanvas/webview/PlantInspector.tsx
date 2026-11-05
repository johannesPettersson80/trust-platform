import React from "react";

import { iconBtn, PANEL_STYLE } from "./inspectorChrome";
import type { InspectorNode } from "./NodeInspector";
import { healthColor } from "./nodes";
import { t, tint } from "./theme";

interface Props {
  node: InspectorNode;
  /** Opens the setup editor; absent when the saved configuration cannot be opened. */
  onConfigure?: () => void;
  /** Opens Control Studio with this plant (specification 47 §14.1); absent before it is saved. */
  onOpenControlStudio?: () => void;
  onFocus: (nodeId: string) => void;
  onClose: () => void;
}

function text(value: unknown): string {
  return value === undefined || value === null ? "" : String(value);
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.map(text) : [];
}

/** Each address of the mapping on its own line, in Rust's order, with the live value when Rust gave
 * one (specification 47 §5.1, §5.3). */
function mappingLines(addresses: string[], values: unknown): string {
  const live = Array.isArray(values) ? values : undefined;
  return addresses
    .map((address, index) => (live && index < live.length ? `${address} = ${text(live[index])}` : address))
    .join("\n");
}

/** A content revision as a short form beside its full value, like a short Git SHA: `sha256:` and the
 * first 12 digits. Other values show whole. */
function revisionText(revision: string): { shown: string; full?: string } {
  const match = /^(sha256:[0-9a-f]{12})[0-9a-f]+$/.exec(revision);
  return match ? { shown: `${match[1]}…`, full: revision } : { shown: revision };
}

function parameterLines(parameters: unknown): string {
  if (!parameters || typeof parameters !== "object") return "";
  return Object.entries(parameters as Record<string, unknown>)
    .map(([name, value]) => `${name} = ${JSON.stringify(value)}`)
    .join("\n");
}

/**
 * The shared inspector's view of a simulated plant (specification 47 §5): its exact ID, its states,
 * model, complete ordered mapping and parameters, and its saved apart from its running configuration,
 * all as Rust's `plant.topology` answered them. Configure plant opens the setup editor (§6.1).
 */
export function PlantInspector({ node, onConfigure, onOpenControlStudio, onFocus, onClose }: Props) {
  const d = node.data;
  const live = d.liveSample as { command?: unknown; feedback?: unknown } | undefined;
  const configuration = d.configuration as { parameters?: unknown } | undefined;
  const otherStates = strings(d.secondaryStates);
  const saved = revisionText(text(d.savedConfigurationRevision));
  // Rust gives the running revision only when it knows one (specification 47 §20.6).
  const running = revisionText(text(d.activeConfigurationRevision) || "None reported");
  const rows: Array<[string, string, string?]> = [
    ["Plant ID", text(d.plantId)],
    ["State", text(d.state)],
    ["Other states", otherStates.join("\n")],
    ["Model", text(d.modelName)],
    ["Starting point", text(d.startingPointName)],
    ["Command", mappingLines(strings(d.commandSources), live?.command)],
    ["Feedback", mappingLines(strings(d.feedbackTargets), live?.feedback)],
    ["Saved configuration", saved.shown, saved.full],
    ["Running configuration", running.shown, running.full],
    ["Parameters", parameterLines(configuration?.parameters)],
  ];
  const health = text(d.health);

  return (
    <aside className="trust-inspector" style={PANEL_STYLE} aria-label="Plant summary">
      <header className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <span className="trust-inspector__eyebrow" style={{ display: "block", marginBottom: 2 }}>
            Devices & Connections / Simulated plant
          </span>
          <strong
            className="trust-inspector__title"
            style={{ display: "block", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}
          >
            {text(d.displayName) || text(d.name)}
          </strong>
        </div>
        {health && (
          <span
            title={health}
            style={{
              flex: "none",
              width: 10,
              height: 10,
              borderRadius: "50%",
              background: healthColor(health),
              boxShadow: `0 0 0 2px ${tint(healthColor(health), 0.18)}`,
            }}
          />
        )}
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>
      <div className="trust-section trust-section--grow" style={{ paddingBottom: 18 }}>
        {rows
          .filter(([, value]) => value)
          .map(([label, value, full]) => (
            <div
              key={label}
              data-plant-inspector-row={label}
              style={{ display: "grid", gridTemplateColumns: "132px 1fr", gap: 10, fontSize: 12, lineHeight: 1.55, marginBottom: 7 }}
            >
              <span style={{ color: t.textMuted, overflowWrap: "anywhere" }}>{label}</span>
              <span title={full} style={{ color: t.text, overflowWrap: "anywhere", whiteSpace: "pre-line" }}>{value}</span>
            </div>
          ))}
      </div>
      <footer
        className="trust-section"
        style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none", borderTop: `1px solid ${t.border}`, background: t.surface }}
      >
        <button
          type="button"
          data-plant-configure
          onClick={onConfigure}
          disabled={!onConfigure}
          title={onConfigure ? undefined : "This plant's saved configuration cannot be opened."}
          className="trust-button trust-button--primary"
          style={{ flexBasis: "100%" }}
        >
          Configure plant
        </button>
        {onOpenControlStudio && (
          <button
            type="button"
            data-plant-open-control-studio
            onClick={onOpenControlStudio}
            className="trust-button"
            style={{ flex: 1 }}
          >
            Open in Control Studio
          </button>
        )}
        <button type="button" onClick={() => onFocus(node.id)} className="trust-button" style={{ flex: 1 }}>
          Focus
        </button>
      </footer>
    </aside>
  );
}
