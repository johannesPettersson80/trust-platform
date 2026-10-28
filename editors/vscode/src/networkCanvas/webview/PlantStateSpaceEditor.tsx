import React, { useState } from "react";

import type { PlantDraft } from "../plant/addFlow";
import {
  numberText,
  typedNumber,
  type PlantEditAction,
  type PlantEditorIssue,
  type PlantEditorView,
  type PlantEditResult,
} from "../plant/editorSession";

type MatrixName = "a" | "b" | "c" | "d";
type Dimensions = { states: number; inputs: number; outputs: number };

/**
 * The state-space matrices, input delays and initial state (specification 47 §9). Rust resizes the
 * matrices, reads pasted blocks and checks every cell; cell values go into the draft as typed.
 */
export function PlantStateSpaceEditor({
  draft,
  facts,
  issues,
  onChange,
  onAction,
}: {
  draft: PlantDraft;
  facts: NonNullable<PlantEditorView["stateSpace"]>;
  issues: readonly PlantEditorIssue[];
  onChange: (draft: PlantDraft) => void;
  onAction: (action: PlantEditAction) => Promise<PlantEditResult>;
}) {
  const { dimensions } = facts;
  const [pending, setPending] = useState<{ message: string; requested: Partial<Dimensions> }>();
  const [refusal, setRefusal] = useState<string>();
  const ask = async (action: PlantEditAction) => {
    const result = await onAction(action);
    if (!result.ok) {
      setRefusal(result.message);
      return undefined;
    }
    const outcome = result.answer.outcome;
    setRefusal(outcome?.kind === "rejected" ? outcome.message : undefined);
    return outcome;
  };
  // A change names only its dimension: Rust keeps the draft's others as they are when it applies it.
  const requestDimension = async (field: keyof Dimensions, text: string) => {
    const requested: Partial<Dimensions> = { [field]: Number(text) };
    const outcome = await ask({ kind: "resize_state_space", ...requested });
    if (outcome?.kind === "confirm") setPending({ message: outcome.message, requested });
  };
  const write = (parameters: PlantDraft["parameters"], initialState = draft.initialState) =>
    onChange({ ...draft, parameters, initialState, modelValuesTouched: true });
  const updateMatrixCell = (name: MatrixName, row: number, column: number, value: number) => {
    const matrix = matrixOf(draft.parameters[name]).map((cells) => [...cells]);
    if (!matrix[row]) return;
    matrix[row][column] = value;
    write({ ...draft.parameters, [name]: matrix });
  };
  // Every paste goes to Rust: a block the matrix's size replaces it, one number the cell pasted into.
  const pasteMatrix = (event: React.ClipboardEvent, name: MatrixName) => {
    event.preventDefault();
    const cell = (event.target as HTMLElement).dataset?.plantMatrixCell?.split(":");
    void ask({
      kind: "paste_matrix",
      matrix: name,
      text: event.clipboardData.getData("text"),
      ...(cell ? { row: Number(cell[1]), column: Number(cell[2]) } : {}),
    });
  };
  const delays = draft.parameters.input_delays_s;
  const delayIssues = issues.flatMap((issue) => issue.target.kind === "input_delay"
    ? [{ column: issue.target.column, message: issue.message }] : []);
  const cellIssue = (matrix: string, row: number, column: number) => issues.find((issue) =>
    issue.target.kind === "matrix" && issue.target.matrix === matrix &&
    issue.target.row === row && issue.target.column === column)?.message;

  return (
    <div data-plant-state-space-editor style={{ display: "grid", gap: 9 }}>
      <p className="trust-help" style={{ margin: 0 }}>
        Set states, PLC inputs, and measured outputs, then edit the indexed A/B/C/D cells. Paste tab- or comma-separated rows into any matrix cell.
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 7 }}>
        {(["states", "inputs", "outputs"] as const).map((field) => (
          <label key={field} style={FIELD}>
            <span style={LABEL}>{field[0].toUpperCase() + field.slice(1)}</span>
            <input type="number" min={1} step={1} data-plant-state-space-dimension={field} value={dimensions[field]} onChange={(event) => void requestDimension(field, event.target.value)} style={INPUT} />
          </label>
        ))}
      </div>
      {pending && (
        <div data-plant-state-space-resize-confirm role="alert" style={CONFIRM}>
          <span>{pending.message}</span>
          <div style={{ display: "flex", gap: 6 }}>
            <button type="button" className="trust-button" data-plant-state-space-resize-cancel onClick={() => setPending(undefined)}>Cancel</button>
            <button type="button" className="trust-button trust-button--primary" data-plant-state-space-resize-apply onClick={() => {
              const { requested } = pending;
              setPending(undefined);
              void ask({ kind: "resize_state_space", ...requested, confirmed: true });
            }}>Resize and remove values</button>
          </div>
        </div>
      )}
      {refusal && <div data-plant-state-space-error role="alert" style={ERROR}>{refusal}</div>}
      <div data-plant-input-delays style={MATRIX_CARD}>
        <strong style={{ fontSize: 11.5 }}>Input delays (s)</strong>
        <p className="trust-help" style={{ margin: 0 }}>Time before each input reaches the process. Zero means no delay.</p>
        <div style={{ overflowX: "auto", paddingBottom: 2 }}>
          <div style={{ display: "grid", alignItems: "start", gridTemplateColumns: `repeat(${dimensions.inputs}, minmax(80px, 1fr))`, gap: 5, minWidth: dimensions.inputs * 84 }}>
            {facts.delays.map((value, column) => {
              const issue = delayIssues.find((candidate) => candidate.column === column);
              const errorId = `plant-input-delay-error-${column}`;
              return <label key={column} style={FIELD}>
                <span style={LABEL}>Input {column + 1}</span>
                <input type="number" min={0} step="any" data-plant-input-delay-cell={column}
                  aria-label={`Input ${column + 1} delay (seconds)`}
                  value={Array.isArray(delays) ? numberText(delays[column]) : numberText(value)}
                  aria-invalid={Boolean(issue) || delayIssues.some((candidate) => candidate.column === null)}
                  aria-describedby={issue ? errorId : undefined}
                  onChange={(event) => {
                    const next = Array.isArray(delays) ? [...delays] : facts.delays.map((delay) => delay ?? Number.NaN);
                    next[column] = typedNumber(event.target.value);
                    write({ ...draft.parameters, input_delays_s: next as number[] });
                  }} style={INPUT} />
                {issue && <span id={errorId} data-plant-input-delay-error style={ERROR}>{issue.message}</span>}
              </label>;
            })}
          </div>
        </div>
        {delayIssues.filter((issue) => issue.column === null).map((issue) =>
          <span key={issue.message} role="alert" data-plant-input-delay-error style={ERROR}>{issue.message}</span>)}
        {facts.delaysExplicit && <button type="button" className="trust-button" data-plant-input-delay-defaults
          style={{ justifySelf: "start" }} onClick={() => {
            const next = { ...draft.parameters };
            delete next.input_delays_s;
            write(next);
          }}>Use zero defaults</button>}
      </div>
      {([
        ["a", "A · state dynamics"],
        ["b", "B · input to state"],
        ["c", "C · state to output"],
        ["d", "D · direct input to output"],
      ] as const).map(([name, label]) => (
        <MatrixGrid key={name} name={name} label={label} value={matrixOf(draft.parameters[name])}
          cellIssue={cellIssue} onChange={updateMatrixCell}
          onPaste={(event) => pasteMatrix(event, name)} />
      ))}
      <div style={MATRIX_CARD}>
        <strong style={{ fontSize: 11.5 }}>Initial state</strong>
        <div style={{ overflowX: "auto", paddingBottom: 2 }}>
          <div style={{ display: "grid", gridTemplateColumns: `repeat(${dimensions.states}, minmax(58px, 1fr))`, gap: 5, minWidth: dimensions.states * 62 }}>
            {draft.initialState.map((value, column) => {
              const issue = cellIssue("initial_state", 0, column);
              return <label key={column} style={FIELD}><span style={INDEX}>x{column + 1}</span><input type="number" step="any" data-plant-initial-state-cell={column} value={Number.isFinite(value) ? value : ""} aria-invalid={Boolean(issue)} onChange={(event) => { const next = [...draft.initialState]; next[column] = typedNumber(event.target.value); write(draft.parameters, next); }} style={INPUT} />{issue && <span data-plant-state-space-error style={ERROR}>{issue}</span>}</label>;
            })}
          </div>
        </div>
        {draft.initialState.length === 0 && cellIssue("initial_state", 0, 0) &&
          <span data-plant-state-space-error style={ERROR}>{cellIssue("initial_state", 0, 0)}</span>}
      </div>
    </div>
  );
}

function matrixOf(value: unknown): number[][] {
  return Array.isArray(value) ? value.map((row) => (Array.isArray(row) ? row as number[] : [])) : [];
}

function MatrixGrid({ name, label, value, cellIssue, onChange, onPaste }: {
  name: MatrixName;
  label: string;
  value: number[][];
  cellIssue: (matrix: string, row: number, column: number) => string | undefined;
  onChange: (name: MatrixName, row: number, column: number, value: number) => void;
  onPaste: (event: React.ClipboardEvent) => void;
}) {
  const columns = value[0]?.length ?? 0;
  // A matrix without cells shows its shape issue (reported at its first cell) here.
  const shapeIssue = columns === 0 ? cellIssue(name, 0, 0) : undefined;
  return (
    <div style={MATRIX_CARD} onPaste={onPaste}>
      <strong style={{ fontSize: 11.5 }}>{label}</strong>
      {shapeIssue && <span data-plant-state-space-error style={ERROR}>{shapeIssue}</span>}
      <div style={{ overflowX: "auto", paddingBottom: 2 }}>
        <div style={{ display: "grid", gridTemplateColumns: `28px repeat(${columns}, minmax(58px, 1fr))`, gap: 5, minWidth: 28 + columns * 62 }}>
          <span />
          {Array.from({ length: columns }, (_, column) => <span key={column} style={INDEX}>c{column + 1}</span>)}
          {value.flatMap((row, rowIndex) => [
            <span key={`r${rowIndex}`} style={INDEX}>r{rowIndex + 1}</span>,
            ...row.map((cell, columnIndex) => {
              const message = cellIssue(name, rowIndex, columnIndex);
              return <label key={`${rowIndex}:${columnIndex}`} style={FIELD}><input type="number" step="any" data-plant-matrix-cell={`${name}:${rowIndex}:${columnIndex}`} aria-label={`${name.toUpperCase()} row ${rowIndex + 1} column ${columnIndex + 1}`} value={Number.isFinite(cell) ? cell : ""} aria-invalid={Boolean(message)} onChange={(event) => onChange(name, rowIndex, columnIndex, typedNumber(event.target.value))} style={INPUT} />{message && <span data-plant-state-space-error style={ERROR}>{message}</span>}</label>;
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
