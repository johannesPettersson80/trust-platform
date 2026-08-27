import React from "react";

import {
  appendPiecewisePoint,
  movePiecewisePoint,
  piecewiseParametersFromPoints,
  piecewisePointsFromParameters,
  validatePiecewisePoints,
  type PlantPiecewisePoint,
} from "../plant/piecewiseEditor";
import type { PlantEditorDraft } from "../plant/editorModel";

export function PlantPiecewiseEditor({
  draft,
  onChange,
}: {
  draft: PlantEditorDraft;
  onChange: (draft: PlantEditorDraft) => void;
}) {
  const points = piecewisePointsFromParameters(draft.parameters);
  const issues = validatePiecewisePoints(points);
  const commit = (next: PlantPiecewisePoint[]) => onChange({
    ...draft,
    parameters: { ...draft.parameters, ...piecewiseParametersFromPoints(next) },
    modelValuesTouched: true,
  });
  const update = (index: number, patch: Partial<PlantPiecewisePoint>) =>
    commit(points.map((point, candidate) => candidate === index ? { ...point, ...patch } : point));
  const issueFor = (index: number, field: "input" | "output") =>
    issues.find((issue) => issue.row === index && issue.field === field)?.message;

  return (
    <div data-plant-piecewise-editor style={{ display: "grid", gap: 8 }}>
      <p className="trust-help" style={{ margin: 0 }}>
        Define a static input/output curve. Values between points are linearly interpolated; input points must increase from top to bottom.
      </p>
      {points.map((point, index) => {
        const up = movePiecewisePoint(points, index, -1);
        const down = movePiecewisePoint(points, index, 1);
        return (
          <div key={index} data-plant-piecewise-row={index} style={ROW}>
            <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
              <strong style={{ flex: 1, fontSize: 11.5 }}>Point {index + 1}</strong>
              <button type="button" className="trust-button" data-plant-piecewise-up aria-label={`Move point ${index + 1} up`} disabled={!up.moved} onClick={() => commit(up.points)}>↑</button>
              <button type="button" className="trust-button" data-plant-piecewise-down aria-label={`Move point ${index + 1} down`} disabled={!down.moved} onClick={() => commit(down.points)}>↓</button>
              <button type="button" className="trust-button" data-plant-piecewise-remove disabled={points.length <= 2} onClick={() => commit(points.filter((_, candidate) => candidate !== index))}>Remove</button>
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 }}>
              <NumberField label="Input" role="input" value={point.input} issue={issueFor(index, "input")} onChange={(input) => update(index, { input })} />
              <NumberField label="Output" role="output" value={point.output} issue={issueFor(index, "output")} onChange={(output) => update(index, { output })} />
            </div>
          </div>
        );
      })}
      <button type="button" className="trust-button" data-plant-piecewise-add onClick={() => commit(appendPiecewisePoint(points))}>
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
  value: number;
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
        value={Number.isFinite(value) ? value : ""}
        aria-invalid={Boolean(issue)}
        onChange={(event) => onChange(event.target.value.trim() ? Number(event.target.value) : Number.NaN)}
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
