import React, { useMemo, useRef, useState } from "react";

import {
  plantDynamicsFields,
  validatePlantDynamics,
  type PlantEditorDraft,
  type PlantParameterValue,
} from "../plant/editorModel";
import { plantModelDescriptor } from "../plant/modelDescriptors";
import { PlantPiecewiseEditor } from "./PlantPiecewiseEditor";
import { PlantStateSpaceEditor } from "./PlantStateSpaceEditor";

export function PlantDynamicsEditor({
  draft,
  onChange,
}: {
  draft: PlantEditorDraft;
  onChange: (draft: PlantEditorDraft) => void;
}) {
  const [parseErrors, setParseErrors] = useState<Record<string, string>>({});
  const [activeHelpKey, setActiveHelpKey] = useState<string>();
  const [summaryHelpKey, setSummaryHelpKey] = useState<string>();
  const helpButtons = useRef(new Map<string, HTMLButtonElement>());
  const fields = plantDynamicsFields(draft.model);
  const descriptor = plantModelDescriptor(draft.model);
  const shape = plantModelShape(draft, descriptor);
  const issues = useMemo(
    () => validatePlantDynamics(draft),
    [draft]
  );
  const issueFor = (key: string) =>
    parseErrors[key] ?? issues.find((issue) => issue.field === key || issue.field.startsWith(`${key}.`))?.message;

  const valueFor = (key: string): PlantParameterValue =>
    key === "initial_state" ? draft.initialState : draft.parameters[key];

  const update = (key: string, value: PlantParameterValue) => {
    setParseErrors((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });
    if (key === "initial_state") {
      onChange({ ...draft, initialState: value as number[], modelValuesTouched: true });
    } else {
      onChange({
        ...draft,
        parameters: { ...draft.parameters, [key]: value },
        modelValuesTouched: true,
      });
    }
  };

  const clearOptionalParameter = (key: string) => {
    setParseErrors((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    });
    const parameters = { ...draft.parameters };
    delete parameters[key];
    onChange({ ...draft, parameters, modelValuesTouched: true });
  };

  const updateStructured = (key: string, raw: string, matrix: boolean) => {
    try {
      const value = JSON.parse(raw) as unknown;
      const rows = matrix ? value : [value];
      if (
        !Array.isArray(value) ||
        !Array.isArray(rows) ||
        rows.flat(Infinity).length === 0 ||
        rows.flat(Infinity).some((entry) => typeof entry !== "number" || !Number.isFinite(entry)) ||
        (matrix && (value as unknown[]).some((row) => !Array.isArray(row)))
      ) {
        throw new Error();
      }
      update(key, value as number[] | number[][]);
    } catch {
      setParseErrors((current) => ({
        ...current,
        [key]: matrix
          ? "Enter a matrix such as [[0, 1], [-1, 0]]."
          : "Enter a list such as [0, 1, 2].",
      }));
    }
  };

  return (
    <section className="trust-section">
      <div className="trust-section__title">Dynamics</div>
      <p data-plant-model-name className="trust-help" style={{ marginTop: 7 }}>
        {descriptor.productName} · {descriptor.purpose} · {count(shape.states, "state")} · {count(shape.inputs, "input")} · {count(shape.outputs, "output")}
      </p>
      {draft.model === "piecewise_linear" ? (
        <PlantPiecewiseEditor draft={draft} onChange={onChange} />
      ) : draft.model === "state_space" ? (
        <PlantStateSpaceEditor draft={draft} onChange={onChange} />
      ) : (
      <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr)", gap: 8 }}>
        {fields.map((field) => {
          const issue = issueFor(field.key);
          const value = valueFor(field.key);
          const common = {
            name: `plant-dynamics-${field.key}`,
            "data-plant-dynamics-field": field.key,
            "aria-invalid": Boolean(issue),
            "aria-describedby": issue ? `plant-dynamics-error-${field.key}` : undefined,
          };
          return (
            <label key={field.key} style={FIELD}>
              <span style={HELP_LABEL}>
                <span style={LABEL}>{field.label}</span>
                <button
                  ref={(button) => {
                    if (button) helpButtons.current.set(field.key, button);
                    else helpButtons.current.delete(field.key);
                  }}
                  type="button"
                  data-plant-parameter-help={field.key}
                  aria-label={`Help for ${field.label}`}
                  aria-expanded={activeHelpKey === field.key}
                  aria-describedby={summaryHelpKey === field.key ? `plant-parameter-help-summary-${field.key}` : undefined}
                  title={`${field.plainTerm}: ${field.effect}`}
                  onFocus={() => setSummaryHelpKey(field.key)}
                  onBlur={() => setSummaryHelpKey(undefined)}
                  onMouseEnter={() => setSummaryHelpKey(field.key)}
                  onMouseLeave={(event) => {
                    if (document.activeElement !== event.currentTarget) setSummaryHelpKey(undefined);
                  }}
                  onKeyDown={(event) => {
                    if (event.key !== "Enter") return;
                    event.preventDefault();
                    setSummaryHelpKey(undefined);
                    setActiveHelpKey(field.key);
                  }}
                  onClick={(event) => {
                    event.preventDefault();
                    setSummaryHelpKey(undefined);
                    setActiveHelpKey((current) => current === field.key ? undefined : field.key);
                  }}
                  style={HELP_BUTTON}
                >?</button>
              </span>
              {summaryHelpKey === field.key && activeHelpKey !== field.key && (
                <span
                  id={`plant-parameter-help-summary-${field.key}`}
                  data-plant-parameter-help-summary={field.key}
                  role="tooltip"
                  style={HELP_SUMMARY}
                >
                  <strong>{field.plainTerm}</strong>
                  <span>{field.effect}</span>
                </span>
              )}
              {activeHelpKey === field.key && (
                <span
                  data-plant-parameter-help-popover={field.key}
                  role="dialog"
                  aria-label={`Help for ${field.label}`}
                  tabIndex={-1}
                  autoFocus
                  onKeyDown={(event) => {
                    if (event.key !== "Escape") return;
                    event.preventDefault();
                    setActiveHelpKey(undefined);
                    helpButtons.current.get(field.key)?.focus();
                  }}
                  style={HELP_POPOVER}
                >
                  <strong>{field.plainTerm}</strong>
                  <span>{field.controlTerm} · {field.unit}</span>
                  <span>{field.effect}</span>
                  <code>{field.storedKey}</code>
                </span>
              )}
              {field.valueKind === "number" ? (
                <input
                  {...common}
                  type="number"
                  step="any"
                  placeholder={field.optional ? "Unbounded" : undefined}
                  defaultValue={typeof value === "number" ? String(value) : ""}
                  onChange={(event) => {
                    if (field.optional && !event.target.value.trim()) {
                      clearOptionalParameter(field.key);
                      return;
                    }
                    const parsed = Number(event.target.value);
                    if (event.target.value.trim() && Number.isFinite(parsed)) {
                      update(field.key, parsed);
                    } else {
                      setParseErrors((current) => ({
                        ...current,
                        [field.key]: `${field.label} must be a finite number.`,
                      }));
                    }
                  }}
                  style={INPUT}
                />
              ) : (
                <textarea
                  {...common}
                  rows={field.valueKind === "matrix" ? 3 : 2}
                  defaultValue={JSON.stringify(value)}
                  onChange={(event) =>
                    updateStructured(field.key, event.target.value, field.valueKind === "matrix")
                  }
                  spellCheck={false}
                  style={{ ...INPUT, resize: "vertical", fontFamily: "var(--vscode-editor-font-family)" }}
                />
              )}
              {issue && (
                <span
                  id={`plant-dynamics-error-${field.key}`}
                  data-plant-dynamics-error={field.key}
                  style={ERROR}
                >
                  {issue}
                </span>
              )}
            </label>
          );
        })}
      </div>
      )}
    </section>
  );
}

function plantModelShape(
  draft: PlantEditorDraft,
  descriptor: ReturnType<typeof plantModelDescriptor>
): { states: number; inputs: number; outputs: number } {
  if (draft.model !== "state_space") return descriptor;
  const b = draft.parameters.b;
  const c = draft.parameters.c;
  return {
    states: draft.initialState.length,
    inputs: Array.isArray(b) && Array.isArray(b[0]) ? b[0].length : descriptor.inputs,
    outputs: Array.isArray(c) ? c.length : descriptor.outputs,
  };
}

function count(value: number, label: string): string {
  return `${value} ${label}${value === 1 ? "" : "s"}`;
}

const FIELD: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
};

const LABEL: React.CSSProperties = {
  color: "var(--trust-text-muted)",
  fontSize: 10.5,
  fontWeight: 600,
};

const HELP_LABEL: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 8,
};

const HELP_BUTTON: React.CSSProperties = {
  width: 18,
  height: 18,
  padding: 0,
  border: "1px solid var(--vscode-contrastBorder, var(--trust-border))",
  borderRadius: "50%",
  background: "transparent",
  color: "var(--vscode-descriptionForeground)",
  font: "inherit",
  fontSize: 10,
  lineHeight: "16px",
};

const HELP_POPOVER: React.CSSProperties = {
  display: "grid",
  gap: 4,
  padding: "8px 9px",
  border: "1px solid var(--vscode-widget-border, var(--trust-border))",
  borderRadius: 3,
  background: "var(--vscode-editorHoverWidget-background, var(--vscode-editor-background))",
  color: "var(--vscode-editorHoverWidget-foreground, var(--vscode-editor-foreground))",
  boxShadow: "0 2px 8px var(--vscode-widget-shadow)",
  fontSize: 11,
  lineHeight: 1.4,
};

const HELP_SUMMARY: React.CSSProperties = {
  display: "grid",
  gap: 3,
  padding: "6px 8px",
  border: "1px solid var(--vscode-widget-border, var(--trust-border))",
  borderRadius: 3,
  background: "var(--vscode-editorHoverWidget-background, var(--vscode-editor-background))",
  color: "var(--vscode-editorHoverWidget-foreground, var(--vscode-editor-foreground))",
  fontSize: 10.5,
  lineHeight: 1.35,
};

const INPUT: React.CSSProperties = {
  boxSizing: "border-box",
  width: "100%",
  border: "1px solid var(--vscode-input-border, var(--trust-border))",
  borderRadius: 2,
  background: "var(--vscode-input-background)",
  color: "var(--vscode-input-foreground)",
  font: "inherit",
  fontSize: 12,
  padding: "5px 7px",
};

const ERROR: React.CSSProperties = {
  color: "var(--trust-danger)",
  fontSize: 10.5,
  lineHeight: 1.35,
};
