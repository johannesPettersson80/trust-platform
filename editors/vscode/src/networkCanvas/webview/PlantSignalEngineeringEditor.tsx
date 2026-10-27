import React from "react";

import type { PlantDraft } from "../plant/addFlow";
import {
  numberText,
  typedNumber,
  type PlantEditorIssue,
  type PlantEditorView,
} from "../plant/editorSession";

type VectorKey = "initialInput" | "inputScale" | "inputOffset" | "outputScale" | "outputOffset";

/**
 * The signal conversions and the plant step (specification 47 §8). Values go into the draft as
 * typed; a conversion the draft does not hold shows Rust's effective value.
 */
export function PlantSignalEngineeringEditor({
  draft,
  rows,
  issues,
  onChange,
}: {
  draft: PlantDraft;
  rows: PlantEditorView["signals"];
  issues: readonly PlantEditorIssue[];
  onChange: (draft: PlantDraft) => void;
}) {
  const issue = (key: string) =>
    issues.find((candidate) => candidate.target.kind === "signal" && candidate.target.key === key)?.message;
  const valueOf = (key: VectorKey, index: number, effective: number | null | undefined): string => {
    const own = draft[key];
    return numberText(own && index < own.length ? own[index] : effective);
  };
  const updateVector = (key: VectorKey, index: number, value: number) => {
    const current = [...(draft[key] ?? [])];
    current[index] = value;
    onChange({ ...draft, [key]: current });
  };
  return (
    <section className="trust-section">
      <div className="trust-section__title">Signal engineering</div>
      <p className="trust-help">
        Convert ordinary PLC values to and from model engineering values. Defaults are explicit and stored with the plant.
      </p>
      <label style={FIELD}>
        <span style={LABEL}>Plant step override (ms, optional)</span>
        <input
          data-plant-engineering-field="step_ms"
          type="number"
          min={1}
          step={1}
          value={draft.stepMs ?? ""}
          placeholder="Use runtime default"
          aria-invalid={Boolean(issue("step_ms"))}
          aria-describedby={issue("step_ms") ? "plant-engineering-error-step_ms" : undefined}
          onChange={(event) => onChange({
            ...draft,
            stepMs: event.target.value.trim() ? Number(event.target.value) : undefined,
          })}
          style={INPUT}
        />
        {issue("step_ms") && (
          <span id="plant-engineering-error-step_ms" style={ERROR}>{issue("step_ms")}</span>
        )}
      </label>
      {draft.commandSources.map((address, index) => (
        <div key={`command:${address}:${index}`} style={GROUP}>
          <strong>Command {index + 1} · <code>{address}</code></strong>
          <div style={GRID}>
            <VectorField label="Initial input" field="initial_input" index={index} value={valueOf("initialInput", index, rows.commands[index]?.initialInput)} issue={issue("initial_input")} onChange={(value) => updateVector("initialInput", index, value)} />
            <VectorField label="Input scale" field="input_scale" index={index} value={valueOf("inputScale", index, rows.commands[index]?.inputScale)} issue={issue("input_scale")} onChange={(value) => updateVector("inputScale", index, value)} />
            <VectorField label="Input offset" field="input_offset" index={index} value={valueOf("inputOffset", index, rows.commands[index]?.inputOffset)} issue={issue("input_offset")} onChange={(value) => updateVector("inputOffset", index, value)} />
          </div>
        </div>
      ))}
      {draft.feedbackTargets.map((address, index) => (
        <div key={`feedback:${address}:${index}`} style={GROUP}>
          <strong>Feedback {index + 1} · <code>{address}</code></strong>
          <div style={GRID}>
            <VectorField label="Output scale" field="output_scale" index={index} value={valueOf("outputScale", index, rows.feedbacks[index]?.outputScale)} issue={issue("output_scale")} onChange={(value) => updateVector("outputScale", index, value)} />
            <VectorField label="Output offset" field="output_offset" index={index} value={valueOf("outputOffset", index, rows.feedbacks[index]?.outputOffset)} issue={issue("output_offset")} onChange={(value) => updateVector("outputOffset", index, value)} />
          </div>
        </div>
      ))}
    </section>
  );
}

function VectorField({
  label,
  field,
  index,
  value,
  issue,
  onChange,
}: {
  label: string;
  field: string;
  index: number;
  value: string;
  issue?: string;
  onChange: (value: number) => void;
}) {
  return (
    <label style={FIELD}>
      <span style={LABEL}>{label}</span>
      <input
        data-plant-engineering-field={`${field}.${index}`}
        type="number"
        step="any"
        value={value}
        aria-invalid={Boolean(issue)}
        aria-describedby={issue ? `plant-engineering-error-${field}` : undefined}
        onChange={(event) => onChange(typedNumber(event.target.value))}
        style={INPUT}
      />
      {issue && <span id={`plant-engineering-error-${field}`} style={ERROR}>{issue}</span>}
    </label>
  );
}

const GROUP: React.CSSProperties = { display: "grid", gap: 6, marginTop: 9, fontSize: 11 };
const GRID: React.CSSProperties = { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 };
const FIELD: React.CSSProperties = { display: "grid", gap: 4, minWidth: 0, marginTop: 7 };
const LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const INPUT: React.CSSProperties = { boxSizing: "border-box", width: "100%", minWidth: 0, border: "1px solid var(--vscode-input-border, var(--trust-border))", borderRadius: 2, background: "var(--vscode-input-background)", color: "var(--vscode-input-foreground)", font: "inherit", fontSize: 11.5, padding: "5px 7px" };
const ERROR: React.CSSProperties = { color: "var(--trust-danger)", fontSize: 10.5, lineHeight: 1.35 };
