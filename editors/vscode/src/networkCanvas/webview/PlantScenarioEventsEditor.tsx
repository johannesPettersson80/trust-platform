import React from "react";

import type { PlantScenarioEventDraft } from "../plant/scenarioEventEditor";
import {
  numberText,
  typedNumber,
  type PlantEditAction,
  type PlantEditorIssue,
  type PlantEditorTables,
  type PlantEditorView,
  type PlantEditResult,
} from "../plant/editorSession";

/**
 * The scheduled scenario events (specification 47 §12). Rust adds, duplicates, moves and removes
 * events, changes their action with its defaults, reads each time from its seconds text and checks
 * them; plain values go into the draft as typed.
 */
export function PlantScenarioEventsEditor({ rows, facts, kinds, parameters, issues, onChange, onAction }: {
  rows: PlantScenarioEventDraft[];
  facts: PlantEditorView["events"];
  kinds: PlantEditorTables["scenarioEventKinds"];
  parameters: readonly string[];
  issues: readonly PlantEditorIssue[];
  onChange: (rows: PlantScenarioEventDraft[]) => void;
  onAction: (action: PlantEditAction) => Promise<PlantEditResult>;
}) {
  const update = (index: number, patch: Partial<PlantScenarioEventDraft>) =>
    onChange(rows.map((row, candidate) => candidate === index ? { ...row, ...patch } : row));
  const issue = (index: number, field: string) =>
    issues.find((candidate) =>
      candidate.target.kind === "event" && candidate.target.row === index && candidate.target.field === field)?.message;
  return (
    <section className="trust-section" data-plant-scenario-events>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div className="trust-section__title" style={{ flex: 1, margin: 0 }}>Scenario events</div>
        <button type="button" className="trust-button" data-plant-scenario-event-add onClick={() => void onAction({ kind: "add_event" })}>+ Add</button>
      </div>
      <p className="trust-help">Schedule deliberate changes and faults at simulation time. Equal-time events run in the displayed order.</p>
      <p className="trust-help">Times use seconds with 0.001 s precision. For example, 1.001 s is stored as exactly 1001 ms; finer values are not rounded.</p>
      {rows.length === 0 && <p className="trust-help">No scheduled events.</p>}
      <div style={{ display: "grid", gap: 8 }}>
        {rows.map((row, index) => {
          const fact = facts[index];
          const known = kinds.some((kind) => kind.kind === row.kind);
          return (
          <div key={index} data-plant-scenario-event-row={index} style={CARD}>
            <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
              <strong style={{ flex: 1 }}>Event {index + 1}</strong>
              <label style={INLINE}><input type="checkbox" checked={row.enabled} onChange={(event) => update(index, { enabled: event.target.checked })} /> Include</label>
              <button type="button" className="trust-button" aria-label={`Move event ${index + 1} up`} disabled={!fact?.canMoveUp} onClick={() => void onAction({ kind: "move_event", index, delta: -1 })}>↑</button>
              <button type="button" className="trust-button" aria-label={`Move event ${index + 1} down`} disabled={!fact?.canMoveDown} onClick={() => void onAction({ kind: "move_event", index, delta: 1 })}>↓</button>
            </div>
            <div style={GRID}>
              <label style={FIELD}>
                <span style={LABEL}>Time (s)</span>
                <input type="text" inputMode="decimal" data-plant-scenario-event-field="at_s"
                  value={row.timeSecondsText ?? fact?.timeText ?? ""}
                  aria-invalid={Boolean(issue(index, "at_ms"))}
                  aria-describedby={issue(index, "at_ms") ? `plant-scenario-time-error-${index}` : undefined}
                  onChange={(event) => update(index, { timeSecondsText: event.target.value })} style={INPUT} />
                {issue(index, "at_ms") && <span id={`plant-scenario-time-error-${index}`} role="alert" style={ERROR}>{issue(index, "at_ms")}</span>}
              </label>
              <label style={FIELD}><span style={LABEL}>Action</span><select data-plant-scenario-event-field="kind" value={row.kind} aria-invalid={Boolean(issue(index, "kind"))} onChange={(event) => void onAction({ kind: "set_event_kind", index, eventKind: event.target.value })} style={INPUT}>
                {!known && <option value={row.kind}>{row.kind || "Unknown action"}</option>}
                {kinds.map((kind) => <option key={kind.kind} value={kind.kind}>{kind.label}</option>)}
              </select>{issue(index, "kind") && <span style={ERROR}>{issue(index, "kind")}</span>}</label>
              {fact?.showsChannel && (
                <label style={FIELD}>
                  <span style={LABEL}>{fact.channelLabel}</span>
                  <select data-plant-scenario-event-field="channel" value={row.channel ?? 0}
                    aria-invalid={Boolean(issue(index, "channel"))}
                    onChange={(event) => update(index, { channel: Number(event.target.value) })} style={INPUT}>
                    {issue(index, "channel") && <option value={row.channel ?? ""}>Choose a valid quantity</option>}
                    {fact.channelOptions.map((option) => (
                      <option key={option.value} value={option.value}>{option.label}</option>
                    ))}
                  </select>
                  {issue(index, "channel") && <span role="alert" style={ERROR}>{issue(index, "channel")}</span>}
                </label>
              )}
              {fact?.showsParameter && <label style={FIELD}><span style={LABEL}>Parameter</span><select data-plant-scenario-event-field="parameter" value={row.parameter ?? ""} aria-invalid={Boolean(issue(index, "parameter"))} onChange={(event) => update(index, { parameter: event.target.value })} style={INPUT}>{parameters.map((parameter) => <option key={parameter} value={parameter}>{parameter}</option>)}</select>{issue(index, "parameter") && <span style={ERROR}>{issue(index, "parameter")}</span>}</label>}
              {fact?.showsValue && <NumberField label="Value" field="value" value={row.value} error={issue(index, "value")} onChange={(value) => update(index, { value })} />}
            </div>
            <div style={{ display: "flex", gap: 6, justifyContent: "flex-end" }}>
              <button type="button" className="trust-button" data-plant-scenario-event-duplicate onClick={() => void onAction({ kind: "duplicate_event", index })}>Duplicate</button>
              <button type="button" className="trust-button" data-plant-scenario-event-remove onClick={() => void onAction({ kind: "remove_event", index })}>Remove</button>
            </div>
          </div>
          );
        })}
      </div>
    </section>
  );
}

function NumberField({ label, field, value, error, onChange }: { label: string; field: string; value?: number; error?: string; onChange: (value: number) => void }) {
  return <label style={FIELD}><span style={LABEL}>{label}</span><input type="number" step="any" data-plant-scenario-event-field={field} value={numberText(value)} aria-invalid={Boolean(error)} onChange={(event) => onChange(typedNumber(event.target.value))} style={INPUT} />{error && <span style={ERROR}>{error}</span>}</label>;
}

const CARD: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: 3, padding: 8, display: "grid", gap: 8 };
const GRID: React.CSSProperties = { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 };
const FIELD: React.CSSProperties = { display: "grid", gap: 4, minWidth: 0 };
const INLINE: React.CSSProperties = { display: "inline-flex", alignItems: "center", gap: 4, fontSize: 10.5 };
const LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const INPUT: React.CSSProperties = { boxSizing: "border-box", width: "100%", minWidth: 0, border: "1px solid var(--vscode-input-border, var(--trust-border))", borderRadius: 2, background: "var(--vscode-input-background)", color: "var(--vscode-input-foreground)", font: "inherit", fontSize: 11.5, padding: "5px 7px" };
const ERROR: React.CSSProperties = { color: "var(--trust-danger)", fontSize: 10.5, lineHeight: 1.3 };
