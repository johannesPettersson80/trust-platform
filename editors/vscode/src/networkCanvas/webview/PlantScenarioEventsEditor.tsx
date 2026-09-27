import React from "react";

import {
  applyPlantScenarioEventKind,
  createPlantScenarioEvent,
  movePlantScenarioEvent,
  PLANT_SCENARIO_EVENT_KINDS,
  scenarioMillisecondsFromSeconds,
  validatePlantScenarioEvents,
  type PlantScenarioEventContext,
  type PlantScenarioEventDraft,
  type PlantScenarioEventKind,
} from "../plant/scenarioEventEditor";

const LABELS: Record<PlantScenarioEventKind, string> = {
  set_state: "Set state",
  add_state: "Add to state",
  set_parameter: "Change model parameter",
  set_sensor_bias: "Set sensor bias",
  stick_actuator: "Stick actuator",
  release_actuator: "Release actuator",
  reset_plant: "Reset plant",
};

export function PlantScenarioEventsEditor({ rows, context, onChange }: {
  rows: PlantScenarioEventDraft[];
  context: PlantScenarioEventContext;
  onChange: (rows: PlantScenarioEventDraft[]) => void;
}) {
  const issues = validatePlantScenarioEvents(rows, context);
  const update = (index: number, patch: Partial<PlantScenarioEventDraft>) =>
    onChange(rows.map((row, candidate) => candidate === index ? { ...row, ...patch } : row));
  const issue = (index: number, field: string) =>
    issues.find((candidate) => candidate.row === index && candidate.field === field)?.message;
  return (
    <section className="trust-section" data-plant-scenario-events>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div className="trust-section__title" style={{ flex: 1, margin: 0 }}>Scenario events</div>
        <button type="button" className="trust-button" data-plant-scenario-event-add onClick={() => onChange([...rows, createPlantScenarioEvent(rows.length + 1)])}>+ Add</button>
      </div>
      <p className="trust-help">Schedule deliberate changes and faults at simulation time. Equal-time events run in the displayed order.</p>
      <p className="trust-help">Times use seconds with 0.001 s precision. For example, 1.001 s is stored as exactly 1001 ms; finer values are not rounded.</p>
      {rows.length === 0 && <p className="trust-help">No scheduled events.</p>}
      <div style={{ display: "grid", gap: 8 }}>
        {rows.map((row, index) => (
          <div key={index} data-plant-scenario-event-row={index} style={CARD}>
            <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
              <strong style={{ flex: 1 }}>Event {index + 1}</strong>
              <label style={INLINE}><input type="checkbox" checked={row.enabled} onChange={(event) => update(index, { enabled: event.target.checked })} /> Include</label>
              <button type="button" className="trust-button" aria-label={`Move event ${index + 1} up`} disabled={index === 0 || rows[index - 1]?.atMs !== row.atMs} onClick={() => onChange(movePlantScenarioEvent(rows, index, -1))}>↑</button>
              <button type="button" className="trust-button" aria-label={`Move event ${index + 1} down`} disabled={index === rows.length - 1 || rows[index + 1]?.atMs !== row.atMs} onClick={() => onChange(movePlantScenarioEvent(rows, index, 1))}>↓</button>
            </div>
            <div style={GRID}>
              <label style={FIELD}>
                <span style={LABEL}>Time (s)</span>
                <input type="text" inputMode="decimal" data-plant-scenario-event-field="at_s"
                  value={row.timeSecondsText ?? String(row.atMs / 1000)}
                  aria-invalid={Boolean(issue(index, "at_ms"))}
                  aria-describedby={issue(index, "at_ms") ? `plant-scenario-time-error-${index}` : undefined}
                  onChange={(event) => update(index, {
                    timeSecondsText: event.target.value,
                    atMs: scenarioMillisecondsFromSeconds(event.target.value),
                  })} style={INPUT} />
                {issue(index, "at_ms") && <span id={`plant-scenario-time-error-${index}`} role="alert" style={ERROR}>{issue(index, "at_ms")}</span>}
              </label>
              <label style={FIELD}><span style={LABEL}>Action</span><select data-plant-scenario-event-field="kind" value={row.kind} aria-invalid={Boolean(issue(index, "kind"))} onChange={(event) => onChange(rows.map((candidate, candidateIndex) => candidateIndex === index ? applyPlantScenarioEventKind(candidate, event.target.value as PlantScenarioEventKind, context.parameters) : candidate))} style={INPUT}>{PLANT_SCENARIO_EVENT_KINDS.map((kind) => <option key={kind} value={kind}>{LABELS[kind]}</option>)}</select>{issue(index, "kind") && <span style={ERROR}>{issue(index, "kind")}</span>}</label>
              {(["set_state", "add_state", "set_sensor_bias"] as string[]).includes(row.kind) && (
                <label style={FIELD}>
                  <span style={LABEL}>{row.kind === "set_sensor_bias" ? "Measured quantity" : "State quantity"}</span>
                  <select data-plant-scenario-event-field="channel" value={row.channel ?? 0}
                    aria-invalid={Boolean(issue(index, "channel"))}
                    onChange={(event) => update(index, { channel: Number(event.target.value) })} style={INPUT}>
                    {issue(index, "channel") && <option value={row.channel ?? ""}>Choose a valid quantity</option>}
                    {Array.from({ length: row.kind === "set_sensor_bias" ? context.outputCount : context.stateCount }, (_, channel) => (
                      <option key={channel} value={channel}>{row.kind === "set_sensor_bias"
                        ? context.outputLabels?.[channel] ?? `Output ${channel + 1}`
                        : context.stateLabels?.[channel] ?? `State ${channel + 1}`}</option>
                    ))}
                  </select>
                  {issue(index, "channel") && <span role="alert" style={ERROR}>{issue(index, "channel")}</span>}
                </label>
              )}
              {row.kind === "set_parameter" && <label style={FIELD}><span style={LABEL}>Parameter</span><select data-plant-scenario-event-field="parameter" value={row.parameter ?? ""} aria-invalid={Boolean(issue(index, "parameter"))} onChange={(event) => update(index, { parameter: event.target.value })} style={INPUT}>{context.parameters.map((parameter) => <option key={parameter} value={parameter}>{parameter}</option>)}</select>{issue(index, "parameter") && <span style={ERROR}>{issue(index, "parameter")}</span>}</label>}
              {(["set_state", "add_state", "set_parameter", "set_sensor_bias"] as string[]).includes(row.kind) && <NumberField label="Value" field="value" value={row.value ?? 0} error={issue(index, "value")} onChange={(value) => update(index, { value })} />}
            </div>
            <div style={{ display: "flex", gap: 6, justifyContent: "flex-end" }}>
              <button type="button" className="trust-button" data-plant-scenario-event-duplicate onClick={() => onChange([...rows.slice(0, index + 1), { ...row }, ...rows.slice(index + 1)])}>Duplicate</button>
              <button type="button" className="trust-button" data-plant-scenario-event-remove onClick={() => onChange(rows.filter((_, candidate) => candidate !== index))}>Remove</button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function NumberField({ label, field, value, error, onChange }: { label: string; field: string; value: number; error?: string; onChange: (value: number) => void }) {
  return <label style={FIELD}><span style={LABEL}>{label}</span><input type="number" step={field === "at_ms" || field === "channel" ? 1 : "any"} data-plant-scenario-event-field={field} value={value} aria-invalid={Boolean(error)} onChange={(event) => onChange(Number(event.target.value))} style={INPUT} />{error && <span style={ERROR}>{error}</span>}</label>;
}

const CARD: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: 3, padding: 8, display: "grid", gap: 8 };
const GRID: React.CSSProperties = { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 };
const FIELD: React.CSSProperties = { display: "grid", gap: 4, minWidth: 0 };
const INLINE: React.CSSProperties = { display: "inline-flex", alignItems: "center", gap: 4, fontSize: 10.5 };
const LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const INPUT: React.CSSProperties = { boxSizing: "border-box", width: "100%", minWidth: 0, border: "1px solid var(--vscode-input-border, var(--trust-border))", borderRadius: 2, background: "var(--vscode-input-background)", color: "var(--vscode-input-foreground)", font: "inherit", fontSize: 11.5, padding: "5px 7px" };
const ERROR: React.CSSProperties = { color: "var(--trust-danger)", fontSize: 10.5, lineHeight: 1.3 };
