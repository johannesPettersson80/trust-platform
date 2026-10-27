import React from "react";

import type { PlantUncertaintyDraft } from "../plant/uncertaintyEditor";
import {
  numberText,
  type PlantEditAction,
  type PlantEditorIssue,
  type PlantEditorTables,
  type PlantEditorView,
  type PlantEditResult,
} from "../plant/editorSession";

/**
 * The ordered imperfection effects (specification 47 §11). Rust adds, duplicates, moves, removes
 * and re-types effects with their defaults and checks them; plain values go into the draft as
 * typed.
 */
export function PlantImperfectionsEditor({
  rows,
  facts,
  tables,
  parameters,
  issues,
  onChange,
  onAction,
  activeIndex,
  onActiveIndexChange,
}: {
  rows: PlantUncertaintyDraft[];
  facts: PlantEditorView["effects"];
  tables: PlantEditorTables;
  parameters: readonly string[];
  issues: readonly PlantEditorIssue[];
  onChange: (rows: PlantUncertaintyDraft[]) => void;
  onAction: (action: PlantEditAction) => Promise<PlantEditResult>;
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
}) {
  const [advancedIds, setAdvancedIds] = React.useState<string[]>([]);
  const [adding, setAdding] = React.useState(false);
  const update = (index: number, patch: Partial<PlantUncertaintyDraft>) =>
    onChange(rows.map((row, candidate) => candidate === index ? { ...row, ...patch } : row));
  const issueFor = (index: number, field: string) =>
    issues.find((issue) =>
      issue.target.kind === "effect" && issue.target.row === index && issue.target.field === field)?.message;
  // An applied edit names the row it created or changed; the editor opens that row.
  const act = async (action: PlantEditAction) => {
    const result = await onAction(action);
    const outcome = result.ok ? result.answer.outcome : undefined;
    if (outcome?.kind === "applied" && outcome.index !== undefined) onActiveIndexChange(outcome.index);
    return outcome;
  };
  const move = async (index: number, delta: -1 | 1) => {
    const result = await onAction({ kind: "move_effect", index, delta });
    if (!result.ok || result.answer.outcome?.kind !== "applied") return;
    if (activeIndex === index) onActiveIndexChange(index + delta);
    else if (activeIndex === index + delta) onActiveIndexChange(index);
  };
  const remove = async (index: number) => {
    const result = await onAction({ kind: "remove_effect", index });
    if (!result.ok || result.answer.outcome?.kind !== "applied") return;
    onActiveIndexChange(activeIndex > index ? activeIndex - 1 : Math.min(activeIndex, rows.length - 2));
  };
  const generators = tables.uncertaintyGenerators.map((generator) => generator.id);
  return (
    <section className="trust-section">
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
        <div className="trust-section__title" style={{ margin: 0 }}>Configured effects</div>
        <button
          type="button"
          className="trust-button"
          data-plant-uncertainty-add
          aria-expanded={adding}
          onClick={() => setAdding(!adding)}
        >
          {adding ? "Cancel adding" : "+ Add effect"}
        </button>
      </div>
      <p className="trust-help">Optional ordered effects make the simulated process less ideal. Order changes behavior.</p>
      {adding && <div role="group" aria-label="Choose an effect" style={{ display: "grid", gap: 5 }}>
        {tables.imperfectionTypes.map((type) => (
          <button key={type.id} type="button" className="trust-button" data-plant-uncertainty-add-type={type.id}
            onClick={() => {
              setAdding(false);
              void act({ kind: "add_effect", preset: type.id });
            }} style={{ textAlign: "left", whiteSpace: "normal" }}>
            <strong>{type.label}</strong><span className="trust-help" style={{ display: "block" }}>{type.expectation}</span>
          </button>
        ))}
      </div>}
      {rows.length === 0 && <p className="trust-help">No imperfections configured.</p>}
      <div style={{ display: "grid", gap: 10 }}>
        {rows.map((row, index) => {
          const fact = facts[index];
          const preset = fact?.preset ?? "custom";
          const advancedOpen = advancedIds.includes(row.id);
          const open = index === Math.min(activeIndex, rows.length - 1);
          const boundary = fact?.boundary ?? "";
          return (
          <div key={`${row.id}:${index}`} data-plant-uncertainty-row={index} data-plant-uncertainty-card={index}
            data-plant-uncertainty-kind={preset} data-plant-uncertainty-boundary={boundary}
            data-plant-uncertainty-focused={open ? index : undefined} style={CARD}>
            <div style={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: 6 }}>
              <strong style={{ flex: 1, fontSize: 11.5 }}>{index + 1}. {fact?.label ?? row.generator}</strong>
              <label style={{ display: "inline-flex", alignItems: "center", gap: 4, fontSize: 10.5 }}>
                <input type="checkbox" checked={row.enabled} onChange={(event) => update(index, { enabled: event.target.checked })} /> Include
              </label>
              <button type="button" className="trust-button" data-plant-uncertainty-edit={index}
                aria-expanded={open} onClick={() => onActiveIndexChange(open ? -1 : index)}>{open ? "Close" : "Edit"}</button>
              <span className="trust-help" style={{ flex: "0 0 100%" }}>{boundary} · {row.enabled ? "Included" : "Excluded"} · {row.id}</span>
              <div style={{ display: "flex", flex: "0 0 100%", justifyContent: "flex-end", gap: 6 }}>
                <button type="button" className="trust-button" data-plant-uncertainty-up aria-label={`Move effect ${index + 1} up`} disabled={index === 0} onClick={() => void move(index, -1)}>↑</button>
                <button type="button" className="trust-button" data-plant-uncertainty-down aria-label={`Move effect ${index + 1} down`} disabled={index === rows.length - 1} onClick={() => void move(index, 1)}>↓</button>
                <button type="button" className="trust-button" data-plant-uncertainty-duplicate onClick={() => void act({ kind: "duplicate_effect", index })}>Duplicate</button>
                <button type="button" className="trust-button" data-plant-uncertainty-remove onClick={() => void remove(index)}>Remove</button>
              </div>
            </div>
            {open && <div style={GRID}>
              <label style={FIELD}><span style={LABEL}>Effect type</span><select data-plant-imperfection-type value={preset} onChange={(event) => {
                const requested = event.target.value;
                void act({ kind: "apply_effect_preset", index, preset: requested });
                if (requested === "custom") setAdvancedIds((current) => current.includes(row.id) ? current : [...current, row.id]);
              }} style={INPUT}>{tables.imperfectionTypes.map((entry) => <option key={entry.id} value={entry.id}>{entry.label}</option>)}</select></label>
              <p data-plant-imperfection-expectation className="trust-help" style={{ gridColumn: "1 / -1", margin: 0 }}>{fact?.expectation}</p>
              <NumberField label="Start (s)" field="start_s" value={row.start_s} issue={issueFor(index, "start_s")} onChange={(value) => update(index, { start_s: value })} />
              <NumberField label="End (s, optional)" field="end_s" value={row.end_s} issue={issueFor(index, "end_s")} onChange={(value) => update(index, { end_s: value })} />
              {(fact?.fields ?? []).map((field) =>
                field === "dropout_policy" ? (
                  <SelectField key={field} label={tables.uncertaintyFieldLabels[field] ?? field} field={field} value={row.dropout_policy ?? ""} values={tables.uncertaintyDropoutPolicies} issue={issueFor(index, field)} onChange={(value) => update(index, { dropout_policy: value })} />
                ) : (
                  <NumberField key={field} label={tables.uncertaintyFieldLabels[field] ?? field} field={field} value={row[field as keyof PlantUncertaintyDraft] as number | undefined} issue={issueFor(index, field)} onChange={(value) => update(index, { [field]: value })} />
                )
              )}
              <button type="button" className="trust-button" data-plant-imperfection-advanced aria-expanded={advancedOpen} onClick={() => setAdvancedIds((current) => advancedOpen ? current.filter((id) => id !== row.id) : [...current, row.id])} style={{ gridColumn: "1 / -1", justifySelf: "start" }}>
                {advancedOpen ? "Hide" : "Show"} runtime mapping
              </button>
              {advancedOpen && <div style={{ ...GRID, gridColumn: "1 / -1", width: "100%" }}>
                <TextField label="Effect ID" field="id" value={row.id} issue={issueFor(index, "id")} onChange={(value) => update(index, { id: value })} />
                <SelectField label="Target" field="target" value={row.target} values={tables.uncertaintyTargets} issue={issueFor(index, "target")} onChange={(value) => void act({ kind: "set_effect_target", index, target: value })} />
                <SelectField label="Mode" field="mode" value={row.mode} values={tables.uncertaintyModes} issue={issueFor(index, "mode")} onChange={(value) => update(index, { mode: value })} />
                <SelectField label="Generator" field="generator" value={row.generator} values={generators} issue={issueFor(index, "generator")} onChange={(value) => void act({ kind: "set_effect_generator", index, generator: value })} />
                {row.target === "parameter" ? (
                  <SelectField label="Parameter" field="parameter" value={row.parameter ?? ""} values={parameters} issue={issueFor(index, "parameter")} onChange={(value) => update(index, { parameter: value })} />
                ) : (
                  <NumberField label="Channel" field="channel" value={row.channel} issue={issueFor(index, "channel")} onChange={(value) => update(index, { channel: value })} />
                )}
              </div>}
            </div>}
          </div>
          );
        })}
      </div>
    </section>
  );
}

function TextField({ label, field, value, issue, onChange }: { label: string; field: string; value: string; issue?: string; onChange: (value: string) => void }) {
  return <label style={FIELD}><span style={LABEL}>{label}</span><input data-plant-uncertainty-field={field} value={value} aria-invalid={Boolean(issue)} onChange={(event) => onChange(event.target.value)} style={INPUT} />{issue && <span data-plant-uncertainty-error={field} style={ERROR}>{issue}</span>}</label>;
}

function NumberField({ label, field, value, issue, onChange }: { label: string; field: string; value?: number; issue?: string; onChange: (value: number | undefined) => void }) {
  return <label style={FIELD}><span style={LABEL}>{label}</span><input type="number" step="any" data-plant-uncertainty-field={field} value={numberText(value)} aria-invalid={Boolean(issue)} onChange={(event) => onChange(event.target.value.trim() ? Number(event.target.value) : undefined)} style={INPUT} />{issue && <span data-plant-uncertainty-error={field} style={ERROR}>{issue}</span>}</label>;
}

function SelectField({ label, field, value, values, issue, onChange }: { label: string; field: string; value: string; values: readonly string[]; issue?: string; onChange: (value: string) => void }) {
  return <label style={FIELD}><span style={LABEL}>{label}</span><select data-plant-uncertainty-field={field} value={value} aria-invalid={Boolean(issue)} onChange={(event) => onChange(event.target.value)} style={INPUT}>{values.map((option) => <option key={option} value={option}>{option.replace(/_/g, " ")}</option>)}</select>{issue && <span data-plant-uncertainty-error={field} style={ERROR}>{issue}</span>}</label>;
}

const CARD: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: 3, padding: 8, display: "grid", gap: 8 };
const GRID: React.CSSProperties = { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 };
const FIELD: React.CSSProperties = { display: "grid", gap: 4, minWidth: 0 };
const LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const INPUT: React.CSSProperties = { boxSizing: "border-box", width: "100%", minWidth: 0, border: "1px solid var(--vscode-input-border, var(--trust-border))", borderRadius: 2, background: "var(--vscode-input-background)", color: "var(--vscode-input-foreground)", font: "inherit", fontSize: 11.5, padding: "5px 7px" };
const ERROR: React.CSSProperties = { color: "var(--trust-danger)", fontSize: 10.5, lineHeight: 1.3 };
