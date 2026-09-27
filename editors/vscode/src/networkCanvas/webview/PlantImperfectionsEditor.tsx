import React from "react";

import {
  createPlantUncertainty,
  movePlantUncertainty,
  plantUncertaintyGeneratorFields,
  plantUncertaintyFieldLabel,
  PLANT_UNCERTAINTY_GENERATORS,
  PLANT_UNCERTAINTY_MODES,
  PLANT_UNCERTAINTY_TARGETS,
  validatePlantUncertainties,
  type PlantUncertaintyDraft,
} from "../plant/uncertaintyEditor";
import {
  applyPlantImperfectionType,
  plantImperfectionExpectation,
  plantImperfectionTypeForRow,
  PLANT_IMPERFECTION_TYPES,
  type PlantImperfectionType,
} from "../plant/imperfectionPresets";

export function PlantImperfectionsEditor({
  rows,
  modelParameters,
  onChange,
  activeIndex,
  onActiveIndexChange,
}: {
  rows: PlantUncertaintyDraft[];
  modelParameters: string[];
  onChange: (rows: PlantUncertaintyDraft[]) => void;
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
}) {
  const [advancedIds, setAdvancedIds] = React.useState<string[]>([]);
  const [adding, setAdding] = React.useState(false);
  const issues = validatePlantUncertainties(rows, modelParameters);
  const update = (index: number, patch: Partial<PlantUncertaintyDraft>) =>
    onChange(rows.map((row, candidate) => candidate === index ? { ...row, ...patch } : row));
  const issueFor = (index: number, field: string) =>
    issues.find((issue) => issue.row === index && issue.field === field);
  const freshRow = () => {
    let sequence = 1;
    while (rows.some((row) => row.id === `effect_${sequence}`)) sequence += 1;
    return createPlantUncertainty(sequence);
  };
  const move = (index: number, delta: -1 | 1) => {
    onChange(movePlantUncertainty(rows, index, delta));
    if (activeIndex === index) onActiveIndexChange(index + delta);
    else if (activeIndex === index + delta) onActiveIndexChange(index);
  };
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
        {PLANT_IMPERFECTION_TYPES.map((type) => (
          <button key={type.id} type="button" className="trust-button" data-plant-uncertainty-add-type={type.id}
            onClick={() => {
              onChange([...rows, applyPlantImperfectionType(type.id, freshRow())]);
              onActiveIndexChange(rows.length);
              setAdding(false);
            }} style={{ textAlign: "left", whiteSpace: "normal" }}>
            <strong>{type.label}</strong><span className="trust-help" style={{ display: "block" }}>{type.expectation}</span>
          </button>
        ))}
      </div>}
      {rows.length === 0 && <p className="trust-help">No imperfections configured.</p>}
      <div style={{ display: "grid", gap: 10 }}>
        {rows.map((row, index) => {
          const type = plantImperfectionTypeForRow(row);
          const advancedOpen = advancedIds.includes(row.id);
          const open = index === Math.min(activeIndex, rows.length - 1);
          const boundary = row.target === "measurement" ? "Sensor" : row.target === "command" ? "Command" : "Process";
          const label = PLANT_IMPERFECTION_TYPES.find((entry) => entry.id === type)?.label ?? type;
          return (
          <div key={`${row.id}:${index}`} data-plant-uncertainty-row={index} data-plant-uncertainty-card={index}
            data-plant-uncertainty-kind={type} data-plant-uncertainty-boundary={boundary}
            data-plant-uncertainty-focused={open ? index : undefined} style={CARD}>
            <div style={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: 6 }}>
              <strong style={{ flex: 1, fontSize: 11.5 }}>{index + 1}. {label}</strong>
              <label style={{ display: "inline-flex", alignItems: "center", gap: 4, fontSize: 10.5 }}>
                <input type="checkbox" checked={row.enabled} onChange={(event) => update(index, { enabled: event.target.checked })} /> Include
              </label>
              <button type="button" className="trust-button" data-plant-uncertainty-edit={index}
                aria-expanded={open} onClick={() => onActiveIndexChange(open ? -1 : index)}>{open ? "Close" : "Edit"}</button>
              <span className="trust-help" style={{ flex: "0 0 100%" }}>{boundary} · {row.enabled ? "Included" : "Excluded"} · {row.id}</span>
              <div style={{ display: "flex", flex: "0 0 100%", justifyContent: "flex-end", gap: 6 }}>
                <button type="button" className="trust-button" data-plant-uncertainty-up aria-label={`Move effect ${index + 1} up`} disabled={index === 0} onClick={() => move(index, -1)}>↑</button>
                <button type="button" className="trust-button" data-plant-uncertainty-down aria-label={`Move effect ${index + 1} down`} disabled={index === rows.length - 1} onClick={() => move(index, 1)}>↓</button>
                <button type="button" className="trust-button" data-plant-uncertainty-duplicate onClick={() => {
                  const copy = { ...row, id: freshRow().id, enabled: true };
                  onChange([...rows.slice(0, index + 1), copy, ...rows.slice(index + 1)]);
                  onActiveIndexChange(index + 1);
                }}>Duplicate</button>
                <button type="button" className="trust-button" data-plant-uncertainty-remove onClick={() => {
                  onChange(rows.filter((_, candidate) => candidate !== index));
                  onActiveIndexChange(activeIndex > index ? activeIndex - 1 : Math.min(activeIndex, rows.length - 2));
                }}>Remove</button>
              </div>
            </div>
            {open && <div style={GRID}>
              <label style={FIELD}><span style={LABEL}>Effect type</span><select data-plant-imperfection-type value={type} onChange={(event) => {
                const requested = event.target.value as PlantImperfectionType;
                onChange(rows.map((candidate, candidateIndex) => candidateIndex === index ? applyPlantImperfectionType(requested, candidate) : candidate));
                if (requested === "custom") setAdvancedIds((current) => current.includes(row.id) ? current : [...current, row.id]);
              }} style={INPUT}>{PLANT_IMPERFECTION_TYPES.map((entry) => <option key={entry.id} value={entry.id}>{entry.label}</option>)}</select></label>
              <p data-plant-imperfection-expectation className="trust-help" style={{ gridColumn: "1 / -1", margin: 0 }}>{plantImperfectionExpectation(type)}</p>
              <NumberField label="Start (s)" field="start_s" value={row.start_s} issue={issueFor(index, "start_s")?.message} onChange={(value) => update(index, { start_s: value })} />
              <NumberField label="End (s, optional)" field="end_s" value={row.end_s} issue={issueFor(index, "end_s")?.message} onChange={(value) => update(index, { end_s: value })} />
              {row.mode !== "hold" && plantUncertaintyGeneratorFields(row.generator).map((field) =>
                field === "dropout_policy" ? (
                  <SelectField key={field} label="Dropout action" field={field} value={row.dropout_policy ?? ""} values={["hold_last", "replace", "fault"]} issue={issueFor(index, field)?.message} onChange={(value) => update(index, { dropout_policy: value as any })} />
                ) : field === "replacement_value" && row.dropout_policy !== "replace" ? null : (
                  <NumberField key={field} label={plantUncertaintyFieldLabel(field)} field={field} value={row[field as keyof PlantUncertaintyDraft] as number | undefined} issue={issueFor(index, field)?.message} onChange={(value) => update(index, { [field]: value })} />
                )
              )}
              <button type="button" className="trust-button" data-plant-imperfection-advanced aria-expanded={advancedOpen} onClick={() => setAdvancedIds((current) => advancedOpen ? current.filter((id) => id !== row.id) : [...current, row.id])} style={{ gridColumn: "1 / -1", justifySelf: "start" }}>
                {advancedOpen ? "Hide" : "Show"} runtime mapping
              </button>
              {advancedOpen && <div style={{ ...GRID, gridColumn: "1 / -1", width: "100%" }}>
                <TextField label="Effect ID" field="id" value={row.id} issue={issueFor(index, "id")?.message} onChange={(value) => update(index, { id: value })} />
                <SelectField label="Target" field="target" value={row.target} values={PLANT_UNCERTAINTY_TARGETS} onChange={(value) => update(index, value === "parameter" ? { target: value as any, parameter: modelParameters[0] ?? "", channel: undefined } : { target: value as any, channel: 0, parameter: undefined })} />
                <SelectField label="Mode" field="mode" value={row.mode} values={PLANT_UNCERTAINTY_MODES} issue={issueFor(index, "mode")?.message} onChange={(value) => update(index, { mode: value as any })} />
                <SelectField label="Generator" field="generator" value={row.generator} values={PLANT_UNCERTAINTY_GENERATORS} issue={issueFor(index, "generator")?.message} onChange={(value) => {
                  const generator = value as PlantUncertaintyDraft["generator"];
                  if (generator === "dropout") update(index, { generator, target: "measurement", mode: "replace", channel: 0, parameter: undefined, rate_per_s: 1, duration_s: 1, dropout_policy: "hold_last" });
                  else if (generator === "quantise") update(index, { generator, mode: "replace", quantum: row.quantum ?? 0.1 });
                  else if (generator === "clip") update(index, { generator, mode: "replace", minimum: row.minimum ?? 0, maximum: row.maximum ?? 1 });
                  else update(index, { generator });
                }} />
                {row.target === "parameter" ? (
                  <SelectField label="Parameter" field="parameter" value={row.parameter ?? ""} values={modelParameters} issue={issueFor(index, "parameter")?.message} onChange={(value) => update(index, { parameter: value })} />
                ) : (
                  <NumberField label="Channel" field="channel" value={row.channel} issue={issueFor(index, "channel")?.message} onChange={(value) => update(index, { channel: value })} />
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
  return <label style={FIELD}><span style={LABEL}>{label}</span><input type="number" step="any" data-plant-uncertainty-field={field} value={value ?? ""} aria-invalid={Boolean(issue)} onChange={(event) => onChange(event.target.value.trim() ? Number(event.target.value) : undefined)} style={INPUT} />{issue && <span data-plant-uncertainty-error={field} style={ERROR}>{issue}</span>}</label>;
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
