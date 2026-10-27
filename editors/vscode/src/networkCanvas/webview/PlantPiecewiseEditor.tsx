import React from "react";

import type { PlantDraft } from "../plant/addFlow";
import {
  numberText,
  typedNumber,
  type PlantEditAction,
  type PlantEditorIssue,
  type PlantEditorView,
  type PlantEditResult,
} from "../plant/editorSession";

/**
 * The piecewise-linear curve's points (specification 47 §9). Rust adds and moves points and checks
 * them; a point's values go into the draft as typed.
 */
export function PlantPiecewiseEditor({
  draft,
  facts,
  issues,
  onChange,
  onAction,
}: {
  draft: PlantDraft;
  facts: NonNullable<PlantEditorView["piecewise"]>;
  issues: readonly PlantEditorIssue[];
  onChange: (draft: PlantDraft) => void;
  onAction: (action: PlantEditAction) => Promise<PlantEditResult>;
}) {
  const points = (key: "input_points" | "output_points") => {
    const value = draft.parameters[key];
    return Array.isArray(value) ? [...value] as number[] : [];
  };
  const write = (inputs: number[], outputs: number[]) => onChange({
    ...draft,
    parameters: { ...draft.parameters, input_points: inputs, output_points: outputs },
    modelValuesTouched: true,
  });
  const update = (index: number, field: "input" | "output", value: number) => {
    const inputs = points("input_points");
    const outputs = points("output_points");
    (field === "input" ? inputs : outputs)[index] = value;
    write(inputs, outputs);
  };
  const remove = (index: number) => write(
    points("input_points").filter((_, candidate) => candidate !== index),
    points("output_points").filter((_, candidate) => candidate !== index)
  );
  // The values show as the draft holds them, so typing never waits for Rust; the moves and the
  // removal follow Rust's last answer.
  const inputs = points("input_points");
  const outputs = points("output_points");
  const issueFor = (index: number, field: "input" | "output") =>
    issues.find((issue) =>
      issue.target.kind === "piecewise" && issue.target.row === index && issue.target.field === field)?.message;

  return (
    <div data-plant-piecewise-editor style={{ display: "grid", gap: 8 }}>
      <p className="trust-help" style={{ margin: 0 }}>
        Define a static input/output curve. Values between points are linearly interpolated; input points must increase from top to bottom.
      </p>
      {inputs.map((input, index) => (
        <div key={index} data-plant-piecewise-row={index} style={ROW}>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <strong style={{ flex: 1, fontSize: 11.5 }}>Point {index + 1}</strong>
            <button type="button" className="trust-button" data-plant-piecewise-up aria-label={`Move point ${index + 1} up`} disabled={!facts.points[index]?.canMoveUp} onClick={() => void onAction({ kind: "move_point", index, delta: -1 })}>↑</button>
            <button type="button" className="trust-button" data-plant-piecewise-down aria-label={`Move point ${index + 1} down`} disabled={!facts.points[index]?.canMoveDown} onClick={() => void onAction({ kind: "move_point", index, delta: 1 })}>↓</button>
            <button type="button" className="trust-button" data-plant-piecewise-remove disabled={!facts.canRemove} onClick={() => remove(index)}>Remove</button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 }}>
            <NumberField label="Input" role="input" value={input} issue={issueFor(index, "input")} onChange={(value) => update(index, "input", value)} />
            <NumberField label="Output" role="output" value={outputs[index]} issue={issueFor(index, "output")} onChange={(value) => update(index, "output", value)} />
          </div>
        </div>
      ))}
      <button type="button" className="trust-button" data-plant-piecewise-add onClick={() => void onAction({ kind: "append_point" })}>
        + Add point
      </button>
    </div>
  );
}

function NumberField({
  label,
  role,
  value,
  issue,
  onChange,
}: {
  label: string;
  role: "input" | "output";
  value: unknown;
  issue?: string;
  onChange: (value: number) => void;
}) {
  return (
    <label style={{ display: "grid", gap: 4, minWidth: 0 }}>
      <span style={LABEL}>{label}</span>
      <input
        type="number"
        step="any"
        data-plant-piecewise-input={role === "input" ? "" : undefined}
        data-plant-piecewise-output={role === "output" ? "" : undefined}
        value={numberText(value)}
        aria-invalid={Boolean(issue)}
        onChange={(event) => onChange(typedNumber(event.target.value))}
        style={INPUT}
      />
      {issue && <span data-plant-piecewise-error={role} style={ERROR}>{issue}</span>}
    </label>
  );
}

const ROW: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: 3, padding: 8, display: "grid", gap: 7 };
const LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const INPUT: React.CSSProperties = { boxSizing: "border-box", minWidth: 0, width: "100%", border: "1px solid var(--vscode-input-border, var(--trust-border))", borderRadius: 2, background: "var(--vscode-input-background)", color: "var(--vscode-input-foreground)", font: "inherit", fontSize: 11.5, padding: "5px 7px" };
const ERROR: React.CSSProperties = { color: "var(--trust-danger)", fontSize: 10.5, lineHeight: 1.3 };
