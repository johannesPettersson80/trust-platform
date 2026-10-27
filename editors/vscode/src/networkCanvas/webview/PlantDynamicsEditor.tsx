import React, { useEffect, useRef, useState } from "react";

import type { PlantDraft } from "../plant/addFlow";
import type { PlantParameterValue } from "../plant/editorModel";
import {
  numberText,
  type PlantEditAction,
  type PlantEditorIssue,
  type PlantEditorView,
  type PlantEditResult,
  type PlantFieldView,
} from "../plant/editorSession";
import { PlantPiecewiseEditor } from "./PlantPiecewiseEditor";
import { PlantStateSpaceEditor } from "./PlantStateSpaceEditor";

/**
 * The model's dynamics fields as Rust lays them out (specification 47 §9). What the user types goes
 * into the draft as the value it spells; Rust's checks say whether it is valid.
 */
export function PlantDynamicsEditor({
  draft,
  view,
  issues,
  onChange,
  onAction,
}: {
  draft: PlantDraft;
  view: PlantEditorView;
  issues: readonly PlantEditorIssue[];
  onChange: (draft: PlantDraft) => void;
  onAction: (action: PlantEditAction) => Promise<PlantEditResult>;
}) {
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [modelDetailsOpen, setModelDetailsOpen] = useState(false);
  const [activeHelpKey, setActiveHelpKey] = useState<string>();
  const [summaryHelpKey, setSummaryHelpKey] = useState<string>();
  const helpButtons = useRef(new Map<string, HTMLButtonElement>());
  const temperatureView = Boolean(view.temperature);
  const { fields, details } = view.dynamics;
  const issueFor = (key: string) =>
    issues.find((issue) => issue.target.kind === "dynamics" && issue.target.key === key)?.message;
  const valueFor = (key: string): unknown =>
    key === "initial_state" ? draft.initialState : draft.parameters[key];
  const write = (key: string, value: unknown) => {
    if (key === "initial_state") {
      onChange({ ...draft, initialState: value as number[], modelValuesTouched: true });
      return;
    }
    const parameters = { ...draft.parameters };
    if (value === undefined) delete parameters[key];
    else parameters[key] = value as PlantParameterValue;
    onChange({ ...draft, parameters, modelValuesTouched: true });
  };
  // An empty field holds no value (Rust says whether one is needed); a number field holds the
  // number its text spells, a list or matrix field what its text reads as, or the text itself.
  const enter = (field: PlantFieldView, text: string) => {
    setTexts((current) => ({ ...current, [field.key]: text }));
    write(field.key, spelled(field, text));
  };
  // The field being typed in keeps its text; any other field shows the draft's value once the draft
  // holds another value than its text spells, as after an edit of the temperature example's fields.
  const [focused, setFocused] = useState<string>();
  useEffect(() => {
    setTexts((current) => {
      const kept = Object.entries(current).filter(([key, text]) => {
        const field = [...fields, ...details].find((candidate) => candidate.key === key);
        const value = key === "initial_state" ? draft.initialState : draft.parameters[key];
        return key === focused || !field || JSON.stringify(spelled(field, text)) === JSON.stringify(value);
      });
      return kept.length === Object.keys(current).length ? current : Object.fromEntries(kept);
    });
  }, [draft, fields, details, focused]);
  const textFor = (field: PlantFieldView) => {
    const value = valueFor(field.key);
    return texts[field.key] ??
      (field.valueKind === "number" ? numberText(value) : value === undefined ? "" : JSON.stringify(value));
  };
  const input = (field: PlantFieldView, issue: string | undefined) => {
    const common = {
      name: `plant-dynamics-${field.key}`,
      "data-plant-dynamics-field": field.key,
      "aria-invalid": Boolean(issue),
      "aria-describedby": issue ? `plant-dynamics-error-${field.key}` : undefined,
      value: textFor(field),
      onFocus: () => setFocused(field.key),
      onBlur: () => setFocused((current) => (current === field.key ? undefined : current)),
    };
    return field.valueKind === "number" ? (
      <input
        {...common}
        type="text"
        inputMode="decimal"
        placeholder={field.defaultValue !== undefined ? `Default: ${field.defaultValue}`
          : field.optional ? "Unbounded" : undefined}
        onChange={(event) => enter(field, event.target.value)}
        style={INPUT}
      />
    ) : (
      <textarea
        {...common}
        rows={field.valueKind === "matrix" ? 3 : 2}
        onChange={(event) => enter(field, event.target.value)}
        spellCheck={false}
        style={{ ...INPUT, resize: "vertical", fontFamily: "var(--vscode-editor-font-family)" }}
      />
    );
  };
  const error = (key: string, issue: string | undefined) => issue && (
    <span id={`plant-dynamics-error-${key}`} data-plant-dynamics-error={key} style={ERROR}>
      {issue}
    </span>
  );

  return (
    <section className="trust-section">
      {!temperatureView && <div className="trust-section__title">Dynamics</div>}
      {!temperatureView && <p data-plant-model-name className="trust-help" style={{ marginTop: 7 }}>
        {count(view.model.states, "state")} · {count(view.model.inputs, "input")} · {count(view.model.outputs, "output")}
      </p>}
      {view.piecewise ? (
        <PlantPiecewiseEditor draft={draft} facts={view.piecewise} issues={issues} onChange={onChange} onAction={onAction} />
      ) : view.stateSpace ? (
        <PlantStateSpaceEditor draft={draft} facts={view.stateSpace} issues={issues} onChange={onChange} onAction={onAction} />
      ) : (
      <>
      <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr)", gap: 8 }}>
        {fields.map((field) => {
          const issue = issueFor(field.key);
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
              {field.help && <span className="trust-help">{field.help}</span>}
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
              {input(field, issue)}
              {error(field.key, issue)}
            </label>
          );
        })}
      </div>
      {details.length > 0 && (
        <div style={{ marginTop: 8 }}>
          <button type="button" className="trust-button" data-plant-model-details aria-expanded={modelDetailsOpen} onClick={() => setModelDetailsOpen((open) => !open)}>Model details</button>
          {modelDetailsOpen && <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr)", gap: 8, marginTop: 8 }}>
            {view.temperature && <div data-plant-temperature-equation className="trust-help" style={{ overflowWrap: "anywhere" }}>
              {view.temperature.equation.map((line) => <div key={line}>{line}</div>)}
            </div>}
            {details.map((field) => {
              const issue = issueFor(field.key);
              return <label key={field.key} style={FIELD}>
                <span style={LABEL}>{field.label}</span>
                {input(field, issue)}
                {error(field.key, issue)}
              </label>;
            })}
          </div>}
        </div>
      )}
      </>
      )}
    </section>
  );
}

/** What a list or matrix field's text reads as: its JSON value, or the text itself. */
/** The value a field's text spells, as it goes into the draft. */
function spelled(field: PlantFieldView, text: string): unknown {
  if (!text.trim()) return field.key === "initial_state" ? [] : undefined;
  return field.valueKind === "number" ? Number(text) : readList(text);
}

function readList(text: string): unknown {
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
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
