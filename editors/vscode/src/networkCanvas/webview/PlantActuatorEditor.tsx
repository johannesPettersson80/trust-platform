import React, { useRef, useState } from "react";

import {
  PLANT_ACTUATOR_FIELDS,
  validatePlantActuator,
  type PlantActuatorSettings,
} from "../plant/editorModel";

export function PlantActuatorEditor({
  value,
  onChange,
}: {
  value: PlantActuatorSettings;
  onChange: (value: PlantActuatorSettings) => void;
}) {
  const issues = validatePlantActuator(value);
  const [activeHelpKey, setActiveHelpKey] = useState<string>();
  const [summaryHelpKey, setSummaryHelpKey] = useState<string>();
  const helpButtons = useRef(new Map<string, HTMLButtonElement>());
  return (
    <section className="trust-section">
      <div className="trust-section__title">Actuator</div>
      <p className="trust-help" style={{ marginTop: 7 }}>
        Optional limits and dynamics between the PLC command and the process. Empty rates are unbounded; zero prevents movement in that direction.
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 8 }}>
        {PLANT_ACTUATOR_FIELDS.map((field) => {
          const issue = issues.find((candidate) => candidate.field === field.key);
          return (
            <label key={field.key} style={{ display: "grid", gap: 4 }}>
              <span style={HELP_LABEL}>
                <span style={LABEL}>{field.label}</span>
                <button
                  ref={(button) => {
                    if (button) helpButtons.current.set(field.key, button);
                    else helpButtons.current.delete(field.key);
                  }}
                  type="button"
                  data-plant-actuator-help={field.key}
                  aria-label={`Help for ${field.label}`}
                  aria-expanded={activeHelpKey === field.key}
                  aria-describedby={summaryHelpKey === field.key ? `plant-actuator-help-summary-${field.key}` : undefined}
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
                <span id={`plant-actuator-help-summary-${field.key}`} role="tooltip" style={HELP_SUMMARY}>
                  <strong>{field.plainTerm}</strong>
                  <span>{field.effect}</span>
                </span>
              )}
              {activeHelpKey === field.key && (
                <span
                  data-plant-actuator-help-popover={field.key}
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
              <input
                type="number"
                step="any"
                data-plant-actuator-field={field.key}
                value={value[field.key] ?? ""}
                aria-invalid={Boolean(issue)}
                aria-describedby={issue ? `plant-actuator-error-${field.key}` : undefined}
                placeholder={field.blankMeaning}
                onChange={(event) => {
                  const next = { ...value };
                  if (event.target.value.trim() === "") delete next[field.key];
                  else {
                    const parsed = Number(event.target.value);
                    if (Number.isFinite(parsed)) next[field.key] = parsed;
                  }
                  onChange(next);
                }}
                style={INPUT}
              />
              <span data-plant-actuator-blank-meaning={field.key} style={BLANK_MEANING}>
                Empty: {field.blankMeaning}
              </span>
              {issue && (
                <span
                  id={`plant-actuator-error-${field.key}`}
                  data-plant-actuator-error={field.key}
                  style={ERROR}
                >
                  {issue.message}
                </span>
              )}
            </label>
          );
        })}
      </div>
    </section>
  );
}

const LABEL: React.CSSProperties = {
  color: "var(--trust-text-muted)",
  fontSize: 10.5,
  fontWeight: 600,
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
  gridColumn: "1 / -1",
};
const BLANK_MEANING: React.CSSProperties = {
  color: "var(--trust-text-muted)",
  fontSize: 10,
  lineHeight: 1.25,
};
const HELP_LABEL: React.CSSProperties = { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 6 };
const HELP_BUTTON: React.CSSProperties = { width: 20, height: 20, border: "1px solid var(--trust-border)", borderRadius: 10, background: "transparent", color: "var(--trust-text-muted)", cursor: "pointer", fontSize: 11, padding: 0 };
const HELP_SUMMARY: React.CSSProperties = { display: "grid", gap: 3, border: "1px solid var(--trust-border)", borderRadius: 3, background: "var(--vscode-editorHoverWidget-background, var(--trust-surface))", color: "var(--trust-text)", fontSize: 10.5, lineHeight: 1.35, padding: "6px 7px" };
const HELP_POPOVER: React.CSSProperties = { display: "grid", gap: 4, border: "1px solid var(--trust-border)", borderRadius: 3, background: "var(--vscode-editorHoverWidget-background, var(--trust-surface))", color: "var(--trust-text)", fontSize: 10.5, lineHeight: 1.4, padding: "7px 8px", boxShadow: "0 3px 10px var(--vscode-widget-shadow)" };
