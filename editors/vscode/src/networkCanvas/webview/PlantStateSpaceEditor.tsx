import React, { useState } from "react";

import {
  parseStateSpacePaste,
  planStateSpaceResize,
  stateSpaceDimensions,
  validateStateSpace,
  validateStateSpaceInputDelays,
  type PlantStateSpaceDimensions,
} from "../plant/stateSpaceEditor";
import type { PlantEditorDraft } from "../plant/editorModel";

type MatrixName = "a" | "b" | "c" | "d";

export function PlantStateSpaceEditor({
  draft,
  onChange,
}: {
  draft: PlantEditorDraft;
  onChange: (draft: PlantEditorDraft) => void;
}) {
  const dimensions = stateSpaceDimensions(draft.parameters);
  const [pending, setPending] = useState<ReturnType<typeof planStateSpaceResize>>();
  const [pasteError, setPasteError] = useState<string>();
  const issues = validateStateSpace({ parameters: draft.parameters, initialState: draft.initialState, dimensions });
  const delays = draft.parameters.input_delays_s;
  const delayValues = Array.from({ length: dimensions.inputs }, (_, index) =>
    delays === undefined ? 0 : Array.isArray(delays) && typeof delays[index] === "number" ? delays[index] as number : Number.NaN);
  const delayIssues = validateStateSpaceInputDelays({ ...draft, inputs: dimensions.inputs });
  const commit = (parameters: PlantEditorDraft["parameters"], initialState = draft.initialState) => {
    setPasteError(undefined);
    onChange({ ...draft, parameters, initialState, modelValuesTouched: true });
  };
  const requestDimension = (field: keyof PlantStateSpaceDimensions, value: number) => {
    if (!Number.isSafeInteger(value) || value < 1) return;
    const plan = planStateSpaceResize({
      parameters: draft.parameters,
      initialState: draft.initialState,
      dimensions,
      requested: { ...dimensions, [field]: value },
    });
    if (plan.kind === "confirm") setPending(plan);
    else commit(plan.parameters, plan.initialState);
  };
  const updateMatrixCell = (name: MatrixName, row: number, column: number, value: number) => {
    const matrix = (draft.parameters[name] as number[][]).map((candidate) => [...candidate]);
    matrix[row][column] = value;
    commit({ ...draft.parameters, [name]: matrix });
  };
  const pasteMatrix = (event: React.ClipboardEvent, name: MatrixName, rows: number, columns: number) => {
    const parsed = parseStateSpacePaste(event.clipboardData.getData("text"));
    if (!parsed || parsed.length !== rows || parsed.some((row) => row.length !== columns)) {
      setPasteError(`${name.toUpperCase()} paste must contain ${rows} rows and ${columns} columns.`);
      return;
    }
    event.preventDefault();
    commit({ ...draft.parameters, [name]: parsed });
  };

  return (
    <div data-plant-state-space-editor style={{ display: "grid", gap: 9 }}>
      <p className="trust-help" style={{ margin: 0 }}>
        Set states, PLC inputs, and measured outputs, then edit the indexed A/B/C/D cells. Paste tab- or comma-separated rows into any matrix cell.
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 7 }}>
        {(["states", "inputs", "outputs"] as const).map((field) => (
          <label key={field} style={FIELD}>
            <span style={LABEL}>{field[0].toUpperCase() + field.slice(1)}</span>
            <input type="number" min={1} step={1} data-plant-state-space-dimension={field} value={dimensions[field]} onChange={(event) => requestDimension(field, Number(event.target.value))} style={INPUT} />
          </label>
        ))}
      </div>
      {pending?.kind === "confirm" && (
        <div data-plant-state-space-resize-confirm role="alert" style={CONFIRM}>
          <span>{pending.message}</span>
          <div style={{ display: "flex", gap: 6 }}>
            <button type="button" className="trust-button" data-plant-state-space-resize-cancel onClick={() => setPending(undefined)}>Cancel</button>
            <button type="button" className="trust-button trust-button--primary" data-plant-state-space-resize-apply onClick={() => { commit(pending.parameters, pending.initialState); setPending(undefined); }}>Resize and remove values</button>
          </div>
        </div>
      )}
      {pasteError && <div data-plant-state-space-error role="alert" style={ERROR}>{pasteError}</div>}
      <div data-plant-input-delays style={MATRIX_CARD}>
        <strong style={{ fontSize: 11.5 }}>Input delays (s)</strong>
        <p className="trust-help" style={{ margin: 0 }}>Time before each input reaches the process. Zero means no delay.</p>
        <div style={{ overflowX: "auto", paddingBottom: 2 }}>
          <div style={{ display: "grid", alignItems: "start", gridTemplateColumns: `repeat(${dimensions.inputs}, minmax(80px, 1fr))`, gap: 5, minWidth: dimensions.inputs * 84 }}>
            {delayValues.map((value, column) => {
              const issue = delayIssues.find((candidate) => candidate.column === column);
              const errorId = `plant-input-delay-error-${column}`;
              return <label key={column} style={FIELD}>
                <span style={LABEL}>Input {column + 1}</span>
                <input type="number" min={0} step="any" data-plant-input-delay-cell={column}
                  aria-label={`Input ${column + 1} delay (seconds)`} value={Number.isFinite(value) ? value : ""}
                  aria-invalid={Boolean(issue) || delayIssues.some((candidate) => candidate.column === undefined)}
                  aria-describedby={issue ? errorId : undefined}
                  onChange={(event) => {
                    const next = Array.isArray(delays) ? [...delays] : [...delayValues];
                    next[column] = event.target.value.trim() ? Number(event.target.value) : Number.NaN;
                    commit({ ...draft.parameters, input_delays_s: next as number[] });
                  }} style={INPUT} />
                {issue && <span id={errorId} data-plant-input-delay-error style={ERROR}>{issue.message}</span>}
              </label>;
            })}
          </div>
        </div>
        {delayIssues.filter((issue) => issue.column === undefined).map((issue) =>
          <span key={issue.message} role="alert" data-plant-input-delay-error style={ERROR}>{issue.message}</span>)}
        {delays !== undefined && <button type="button" className="trust-button" data-plant-input-delay-defaults
          style={{ justifySelf: "start" }} onClick={() => {
            const next = { ...draft.parameters };
            delete next.input_delays_s;
            commit(next);
          }}>Use zero defaults</button>}
      </div>
      <MatrixGrid name="a" label="A · state dynamics" value={draft.parameters.a as number[][]} issues={issues} onChange={updateMatrixCell} onPaste={(event) => pasteMatrix(event, "a", dimensions.states, dimensions.states)} />
      <MatrixGrid name="b" label="B · input to state" value={draft.parameters.b as number[][]} issues={issues} onChange={updateMatrixCell} onPaste={(event) => pasteMatrix(event, "b", dimensions.states, dimensions.inputs)} />
      <MatrixGrid name="c" label="C · state to output" value={draft.parameters.c as number[][]} issues={issues} onChange={updateMatrixCell} onPaste={(event) => pasteMatrix(event, "c", dimensions.outputs, dimensions.states)} />
      <MatrixGrid name="d" label="D · direct input to output" value={draft.parameters.d as number[][]} issues={issues} onChange={updateMatrixCell} onPaste={(event) => pasteMatrix(event, "d", dimensions.outputs, dimensions.inputs)} />
      <div style={MATRIX_CARD}>
        <strong style={{ fontSize: 11.5 }}>Initial state</strong>
        <div style={{ overflowX: "auto", paddingBottom: 2 }}>
          <div style={{ display: "grid", gridTemplateColumns: `repeat(${dimensions.states}, minmax(58px, 1fr))`, gap: 5, minWidth: dimensions.states * 62 }}>
            {draft.initialState.map((value, column) => {
              const issue = issues.find((candidate) => candidate.matrix === "initial_state" && candidate.column === column);
              return <label key={column} style={FIELD}><span style={INDEX}>x{column + 1}</span><input type="number" step="any" data-plant-initial-state-cell={column} value={Number.isFinite(value) ? value : ""} aria-invalid={Boolean(issue)} onChange={(event) => { const next = [...draft.initialState]; next[column] = event.target.value.trim() ? Number(event.target.value) : Number.NaN; commit(draft.parameters, next); }} style={INPUT} />{issue && <span data-plant-state-space-error style={ERROR}>{issue.message}</span>}</label>;
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

function MatrixGrid({ name, label, value, issues, onChange, onPaste }: {
  name: MatrixName;
  label: string;
  value: number[][];
  issues: ReturnType<typeof validateStateSpace>;
  onChange: (name: MatrixName, row: number, column: number, value: number) => void;
  onPaste: (event: React.ClipboardEvent) => void;
}) {
  const columns = value[0]?.length ?? 0;
  return (
    <div style={MATRIX_CARD} onPaste={onPaste}>
      <strong style={{ fontSize: 11.5 }}>{label}</strong>
      <div style={{ overflowX: "auto", paddingBottom: 2 }}>
        <div style={{ display: "grid", gridTemplateColumns: `28px repeat(${columns}, minmax(58px, 1fr))`, gap: 5, minWidth: 28 + columns * 62 }}>
          <span />
          {Array.from({ length: columns }, (_, column) => <span key={column} style={INDEX}>c{column + 1}</span>)}
          {value.flatMap((row, rowIndex) => [
            <span key={`r${rowIndex}`} style={INDEX}>r{rowIndex + 1}</span>,
            ...row.map((cell, columnIndex) => {
              const issue = issues.find((candidate) => candidate.matrix === name && candidate.row === rowIndex && candidate.column === columnIndex);
              return <label key={`${rowIndex}:${columnIndex}`} style={FIELD}><input type="number" step="any" data-plant-matrix-cell={`${name}:${rowIndex}:${columnIndex}`} aria-label={`${name.toUpperCase()} row ${rowIndex + 1} column ${columnIndex + 1}`} value={Number.isFinite(cell) ? cell : ""} aria-invalid={Boolean(issue)} onChange={(event) => onChange(name, rowIndex, columnIndex, event.target.value.trim() ? Number(event.target.value) : Number.NaN)} style={INPUT} />{issue && <span data-plant-state-space-error style={ERROR}>{issue.message}</span>}</label>;
            }),
          ])}
        </div>
      </div>
    </div>
  );
}

const FIELD: React.CSSProperties = { display: "grid", gap: 4, minWidth: 0 };
const LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const INDEX: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10, textAlign: "center", alignSelf: "center" };
const INPUT: React.CSSProperties = { boxSizing: "border-box", minWidth: 0, width: "100%", border: "1px solid var(--vscode-input-border, var(--trust-border))", borderRadius: 2, background: "var(--vscode-input-background)", color: "var(--vscode-input-foreground)", font: "inherit", fontSize: 11.5, padding: "5px 7px" };
const MATRIX_CARD: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: 3, padding: 8, display: "grid", gap: 7, minWidth: 0 };
const CONFIRM: React.CSSProperties = { borderLeft: "2px solid var(--vscode-editorWarning-foreground)", background: "var(--vscode-textBlockQuote-background)", padding: 8, display: "grid", gap: 7, fontSize: 11 };
const ERROR: React.CSSProperties = { color: "var(--trust-danger)", fontSize: 10.5, lineHeight: 1.3 };
