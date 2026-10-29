import React, { useCallback, useEffect, useRef, useState } from "react";

import type { PlantDraft } from "../plant/addFlow";
import type { PlantSignalCatalog } from "../plant/signalCatalog";
import {
  usePlantEditor,
  type PlantEditorIssue,
  type PlantEditorTables,
} from "../plant/editorSession";
import { PlantSignalPicker } from "./PlantSignalPicker";
import { PlantDynamicsEditor } from "./PlantDynamicsEditor";
import { PlantStartingConditionsEditor } from "./PlantStartingConditionsEditor";
import { PlantActuatorEditor } from "./PlantActuatorEditor";
import { PlantImperfectionsEditor } from "./PlantImperfectionsEditor";
import { PlantSignalEngineeringEditor } from "./PlantSignalEngineeringEditor";
import { PlantScenarioEventsEditor } from "./PlantScenarioEventsEditor";
import { RemoveBrickActions } from "./RemoveBrickActions";
import { PlantTemperatureExampleEditor } from "./PlantTemperatureExampleEditor";
import { PlantOptionalSettings, type PlantOptionalDestination } from "./PlantOptionalSettings";
import { PlantModelSchematic } from "./PlantModelSchematic";

type ValidationItem = {
  message: string;
  selector: string;
  destination?: PlantOptionalDestination;
  effect?: number;
};

/** Where the editor shows an issue Rust reported: the field to focus and its optional section. */
function validationItem(issue: PlantEditorIssue, startingState: "scalar" | "fields" | "vector" | undefined): ValidationItem {
  const target = issue.target;
  const item = (selector: string, destination?: PlantOptionalDestination, effect?: number): ValidationItem =>
    ({ message: issue.summary, selector, destination, effect });
  switch (target.kind) {
    case "identity":
      return item(target.field === "id" ? '[name="plant-id"]' : '[name="plant-display-name"]');
    case "port": {
      const base = target.direction === "command" ? "plant-command-source" : "plant-feedback-target";
      return item(`[name="${base}${target.index === 0 ? "" : `-${target.index}`}"]`);
    }
    case "dynamics":
      if (target.key === "initial_state" && startingState === "scalar") {
        return item('[data-plant-dynamics-field="starting_value"]');
      }
      if (target.key === "initial_state" && startingState === "fields") {
        return item(`[data-plant-starting-state="${target.index ?? 0}"]`);
      }
      return item(`[data-plant-dynamics-field="${target.key}"]`);
    case "input_delay":
      return item(`[data-plant-input-delay-cell="${target.column ?? 0}"]`);
    case "matrix":
      return item(target.matrix === "initial_state"
        ? `[data-plant-initial-state-cell="${target.column}"]`
        : `[data-plant-matrix-cell="${target.matrix}:${target.row}:${target.column}"]`);
    case "piecewise":
      return item(`[data-plant-piecewise-row="${target.row}"] [data-plant-piecewise-${target.field}]`);
    case "actuator":
      return item(`[data-plant-actuator-field="${target.key}"]`, "actuator");
    case "signal":
      return item(`[data-plant-engineering-field="${target.key === "step_ms" ? "step_ms" : `${target.key}.0`}"]`, "signals");
    case "effect":
      return item(`[data-plant-uncertainty-row="${target.row}"] [data-plant-uncertainty-field="${target.field}"]`, "realism", target.row);
    case "event":
      return item(
        `[data-plant-scenario-event-row="${target.row}"] [data-plant-scenario-event-field="${target.field === "at_ms" ? "at_s" : target.field}"]`,
        "scenarios"
      );
    case "temperature_units":
      return item('[name="plant-temperature-units-confirmed"]');
    case "configuration":
      return item("[data-plant-validation-summary]");
  }
}

export function PlantDraftPanel({
  draft,
  signalCatalog,
  tables,
  post,
  onChange,
  onOpened,
  onCancel,
  onSave,
  onRemove,
  onOpenControlStudio,
  saving = false,
  saveError,
  saveConflict,
  onReloadConflict,
}: {
  draft: PlantDraft;
  signalCatalog: PlantSignalCatalog;
  tables?: PlantEditorTables;
  post: (message: unknown) => void;
  onChange: (draft: PlantDraft) => void;
  onOpened: (draft: PlantDraft) => void;
  onCancel: () => void;
  onSave: (candidate: Record<string, unknown>) => void;
  onRemove?: () => void;
  onOpenControlStudio?: () => void;
  saving?: boolean;
  saveError?: string;
  saveConflict?: { currentRevision: string; currentPlant: Record<string, unknown> | null };
  onReloadConflict?: () => void;
}) {
  const sectionRef = useRef<HTMLElement>(null);
  const [optionalDestination, setOptionalDestination] = useState<PlantOptionalDestination>();
  const [activeEffectIndex, setActiveEffectIndex] = useState(0);
  const [technicalOpen, setTechnicalOpen] = useState(false);
  const [modelRevision, setModelRevision] = useState(0);
  const [pendingStartingPoint, setPendingStartingPoint] = useState<{ id: string; message: string }>();
  const [rawInputIssues, setRawInputIssues] = useState<Record<string, string>>({});
  const startingPointTrigger = useRef<HTMLElement | null>(null);
  const editor = draft;
  const { answer, current, busy, failure, run, edit } = usePlantEditor({
    draft,
    post,
    signals: signalCatalog,
    onDraft: onChange,
    onOpened,
  });
  // Every field edit of this render, onto the draft as it is now.
  const change = (next: PlantDraft) => edit(next, editor);
  const view = answer?.view;
  // Only the answer for the draft as it is now has issues to show; an older one only lays out.
  const issues = current && answer ? answer.issues : [];
  const updateRawIssue = useCallback((field: string, message?: string) => setRawInputIssues((existing) => {
    const next = { ...existing };
    if (message) next[field] = message;
    else delete next[field];
    return next;
  }), []);
  const issueFor = (field: "id" | "displayName") =>
    issues.find((issue) => issue.target.kind === "identity" && issue.target.field === field);
  const startingIssue = issues.find((issue) =>
    issue.target.kind === "dynamics" && issue.target.key === "initial_state")?.message;
  // An issue of the whole starting state; one of a single value shows at its own field.
  const wholeStartingIssue = issues.find((issue) => issue.target.kind === "dynamics" &&
    issue.target.key === "initial_state" && issue.target.index === undefined)?.message;
  const validationItems: ValidationItem[] = [];
  for (const [field, message] of Object.entries(rawInputIssues)) {
    validationItems.push({ message, selector: `[data-plant-dynamics-field="${field}"]` });
  }
  for (const issue of issues) validationItems.push(validationItem(issue, view?.startingState.kind));
  const valid = current && !busy && answer?.can_save === true && Object.keys(rawInputIssues).length === 0;
  const canSave = valid && !saving;
  const resetModelState = () => {
    setRawInputIssues({});
    setModelRevision((revision) => revision + 1);
  };
  const applyStartingPoint = async (startingPoint: string) => {
    setPendingStartingPoint(undefined);
    const result = await run({ kind: "apply_starting_point", startingPoint });
    if (result.ok && result.answer.outcome?.kind === "applied") resetModelState();
  };
  const requestStartingPoint = async (startingPoint: string, trigger: HTMLElement) => {
    startingPointTrigger.current = trigger;
    const result = await run({ kind: "choose_starting_point", startingPoint });
    if (!result.ok) return;
    const outcome = result.answer.outcome;
    if (outcome?.kind === "confirm") setPendingStartingPoint({ id: startingPoint, message: outcome.message });
    else if (outcome?.kind === "applied") resetModelState();
  };
  const closeStartingPointConfirmation = () => {
    setPendingStartingPoint(undefined);
    requestAnimationFrame(() => startingPointTrigger.current?.focus());
  };
  const focusValidationItem = (item: ValidationItem) => {
    if (item.destination) setOptionalDestination(item.destination);
    if (item.effect !== undefined) setActiveEffectIndex(item.effect);
    if (/plant-id|plant-display-name/.test(item.selector)) setTechnicalOpen(true);
    requestAnimationFrame(() => {
      const target = document.querySelector<HTMLElement>(item.selector);
      target?.scrollIntoView({ block: "center" });
      target?.focus({ preventScroll: true });
    });
  };
  // The editor takes focus once Rust's first answer lets it show the draft.
  const ready = Boolean(tables && view);
  useEffect(() => {
    if (ready) sectionRef.current?.focus();
  }, [ready]);
  const models = tables?.models ?? [];
  const modelChoice = (
    <label style={FIELD}>
      <span style={FIELD_LABEL}>Plant model</span>
      <select
        name="plant-model"
        data-plant-model-select
        value={editor.model}
        onChange={(event) => {
          const choice = models.find((model) => model.kind === event.target.value);
          void requestStartingPoint(choice?.startingPoint ?? "custom_state_space", event.currentTarget);
        }}
        style={INPUT}
      >
        {models.map((model) => (
          <option key={model.kind} value={model.kind}>{model.productName}</option>
        ))}
      </select>
      <span data-plant-model-explanation className="trust-help" style={{ marginTop: 1 }}>
        {view?.model.purpose}
      </span>
      {view && <PlantModelSchematic model={editor.model} productName={view.model.productName} />}
    </label>
  );

  return (
    <aside
      className="trust-inspector trust-plant-draft-surface"
      style={PANEL}
      aria-label="Plant properties"
      data-plant-dirty={String(editor.dirty)}
      data-plant-layout="main-canvas"
    >
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">
            {editor.persisted
              ? `Devices & Connections / Edit ${editor.plantId}`
              : "Devices & Connections / Add simulated plant"}
          </div>
          <div className="trust-inspector__title">{editor.persisted ? "Configure plant" : "Add simulated plant"}</div>
        </div>
        <button onClick={onCancel} aria-label="Cancel simulated plant draft" style={ICON_BUTTON}>✕</button>
      </div>
      {(!tables || !view) && (
        <p role="status" className="trust-help" data-plant-editor-loading style={{ padding: 16 }}>
          {failure ?? (tables ? "Preparing the plant editor…" : "The plant editor is not available yet.")}
        </p>
      )}
      {tables && view && <>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 300px), 1fr))", gap: 16, padding: 16, alignItems: "start" }}>
      <section
        ref={sectionRef}
        data-plant-section
        tabIndex={-1}
        className="trust-section"
        aria-labelledby="plant-draft-section-title"
        style={{ outline: "none" }}
      >
        <section data-plant-setup-section="model-behavior" className="trust-section" style={{ padding: 0, border: 0 }}>
          <div id="plant-draft-section-title" className="trust-section__title">Model &amp; behavior</div>
          {modelChoice}
          {view.temperatureExampleAvailable && <button
            type="button" className="trust-button" data-plant-temperature-example
            onClick={(event) => void requestStartingPoint("temperature_example", event.currentTarget)}
            style={{ marginTop: 8 }}
          >Use temperature example</button>}
          {view.temperature && <PlantTemperatureExampleEditor
            key={modelRevision}
            temperature={view.temperature}
            inputStyle={INPUT}
            onSet={async (field, text) => {
              const result = await run({ kind: "set_temperature_value", field, text });
              if (!result.ok) return result.message;
              return result.answer.outcome?.kind === "rejected" ? result.answer.outcome.message : undefined;
            }}
            onRawValidityChange={updateRawIssue}
          />}
        <PlantDynamicsEditor
          key={`${editor.startingPoint}:${editor.model}:${modelRevision}`}
          draft={editor}
          view={view}
          issues={issues}
          onChange={(next) => change({ ...editor, ...next })}
          onAction={run}
        />
        </section>
        <details data-plant-technical-details open={technicalOpen} onToggle={(event) => setTechnicalOpen(event.currentTarget.open)} style={{ marginTop: 16 }}>
        <summary style={{ cursor: "pointer" }}>Plant details · {editor.displayName}</summary>
        <div style={{ marginTop: 9, display: "flex", alignItems: "center", gap: 8 }}>
          <div style={{ flex: 1, minWidth: 0, fontSize: 13, fontWeight: 650 }}>{editor.displayName}</div>
          {editor.dirty && <span style={DIRTY_BADGE}>Unsaved</span>}
        </div>
        <label style={FIELD}>
          <span style={FIELD_LABEL}>Plant ID</span>
          <input
            name="plant-id"
            value={editor.plantId}
            aria-invalid={Boolean(issueFor("id"))}
            aria-describedby={issueFor("id") ? "plant-id-error" : undefined}
            onChange={(event) => change({ ...editor, plantId: event.target.value })}
            style={INPUT}
          />
          {issueFor("id") && (
            <span id="plant-id-error" data-field-error="plant-id" style={FIELD_ERROR}>
              {issueFor("id")?.message}
            </span>
          )}
        </label>
        <label style={FIELD}>
          <span style={FIELD_LABEL}>Display name</span>
          <input
            name="plant-display-name"
            value={editor.displayName}
            aria-invalid={Boolean(issueFor("displayName"))}
            onChange={(event) => change({ ...editor, displayName: event.target.value })}
            style={INPUT}
          />
          {issueFor("displayName") && <span style={FIELD_ERROR}>{issueFor("displayName")?.message}</span>}
        </label>
        <label style={FIELD}>
          <span style={FIELD_LABEL}>Description <span style={{ fontWeight: 400 }}>(optional)</span></span>
          <textarea
            name="plant-description"
            value={editor.description}
            rows={2}
            onChange={(event) => change({ ...editor, description: event.target.value })}
            style={{ ...INPUT, resize: "vertical" }}
          />
        </label>
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 9 }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={FIELD_LABEL}>Runtime</div>
            <div data-plant-runtime-placement style={{ marginTop: 3, color: "var(--trust-text)", fontSize: 12 }}>
              {editor.runtimeName}
            </div>
          </div>
          <label style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 11.5 }}>
            <input
              name="plant-enabled"
              type="checkbox"
              checked={editor.enabled}
              onChange={(event) => change({ ...editor, enabled: event.target.checked })}
            />
            Enabled
          </label>
        </div>
        </details>
        {pendingStartingPoint && (
          <div
            data-plant-starting-point-confirmation
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="plant-starting-point-confirmation-title"
            aria-describedby="plant-starting-point-confirmation-message"
            onKeyDown={(event) => {
              if (event.key === "Escape") closeStartingPointConfirmation();
            }}
            style={CONFIRMATION}
          >
            <strong id="plant-starting-point-confirmation-title">Replace {pendingStartingPoint.id === "temperature_example" ? "with temperature example" : "edited dynamics"}?</strong>
            <span id="plant-starting-point-confirmation-message">{pendingStartingPoint.message}</span>
            <span style={{ display: "flex", gap: 6, justifyContent: "flex-end" }}>
              <button className="trust-button" onClick={closeStartingPointConfirmation}>Keep current values</button>
              <button
                autoFocus
                className="trust-button trust-button--primary"
                onClick={() => void applyStartingPoint(pendingStartingPoint.id)}
              >Replace values</button>
            </span>
          </div>
        )}
      </section>
      <div style={{ minWidth: 0 }}>
      <section className="trust-section" data-plant-setup-section="connections">
        <div className="trust-section__title">Connections</div>
        <p className="trust-help" style={{ marginTop: 0 }}>Choose a named PLC signal for each model port. Command ports read PLC outputs or memory; feedback ports write PLC inputs.</p>
        {view.temperature && <div className="trust-help" data-plant-temperature-assumptions>
          <p>{view.temperature.commandAssumption}</p>
          <p>{view.temperature.feedbackAssumption}</p>
          <label style={{ display: "flex", alignItems: "start", gap: 6 }}>
            <input type="checkbox" name="plant-temperature-units-confirmed" checked={view.temperature.unitsConfirmed}
              onChange={(event) => void run({ kind: "confirm_temperature_units", confirmed: event.target.checked })} />
            I confirm these units and conversions for the selected PLC signals.
          </label>
          <p>Raw-count signals need setup in Signal conversions. Signal names do not establish units.</p>
        </div>}
        <PlantSignalPicker
          catalog={signalCatalog}
          commandSources={editor.commandSources}
          feedbackTargets={editor.feedbackTargets}
          labels={view.ports}
          onBind={async (direction, index, address) => {
            const result = await run({ kind: "bind_port", direction, index, address });
            if (!result.ok) return result.message;
            return result.answer.outcome?.kind === "rejected" ? result.answer.outcome.message : undefined;
          }}
        />
      </section>
      <section className="trust-section" data-plant-setup-section="starting-conditions" style={{ paddingTop: 8, paddingBottom: 8 }}>
        <div className="trust-section__title">Starting conditions</div>
        {view.startingState.kind === "scalar" ? (
          <PlantStartingConditionsEditor
            key={`${editor.model}:${editor.startingPoint}:${modelRevision}`}
            value={Array.isArray(editor.initialState) ? editor.initialState[0] : undefined}
            label={view.startingState.fields[0]?.label}
            description={view.startingState.fields[0]?.description}
            issue={startingIssue}
            inputStyle={INPUT}
            onChange={(value) => change({ ...editor, initialState: [value], modelValuesTouched: true })}
          />
        ) : view.startingState.kind === "fields" ? (
          <div style={{ display: "grid", gap: 12 }}>
            {view.startingState.fields.map(({ label, description }, index) => (
              <PlantStartingConditionsEditor
                key={`${editor.model}:${modelRevision}:${index}`}
                stateIndex={index}
                value={Array.isArray(editor.initialState) ? editor.initialState[index] : undefined}
                label={label}
                description={description}
                issue={issues.find((issue) => issue.target.kind === "dynamics" &&
                  issue.target.key === "initial_state" && issue.target.index === index)?.message}
                inputStyle={INPUT}
                onChange={(value) => {
                  const initialState = Array.isArray(editor.initialState) ? [...editor.initialState] : [];
                  initialState[index] = value;
                  change({ ...editor, initialState, modelValuesTouched: true });
                }}
              />
            ))}
            {wholeStartingIssue && <span role="alert" style={FIELD_ERROR}>{wholeStartingIssue}</span>}
          </div>
        ) : <p className="trust-help">{view.startingState.text}</p>}
        {view.temperature?.nonEquilibrium && <p className="trust-help">Non-equilibrium start: the starting temperature differs from the heater-off resting temperature. The configured input history remains visible in Model details.</p>}
        {view.temperature && <p className="trust-help">{view.temperature.heaterRange}</p>}
        <p className="trust-help">{view.stepText}</p>
      </section>
      <PlantOptionalSettings
        summaries={view.summaries}
        active={optionalDestination}
        onActiveChange={setOptionalDestination}
      />
      {optionalDestination && (
        <div data-plant-optional-editor={optionalDestination}>
          {optionalDestination === "actuator" && (
          <PlantActuatorEditor
            value={editor.actuator ?? {}}
            fields={tables.actuatorFields}
            issues={issues}
            onChange={(actuator) => change({ ...editor, actuator })}
          />
          )}
          {optionalDestination === "signals" && (
          <PlantSignalEngineeringEditor
            draft={editor}
            rows={view.signals}
            issues={issues}
            onChange={(next) => change({ ...editor, ...next })}
          />
          )}
          {optionalDestination === "realism" && (
          <PlantImperfectionsEditor
            rows={editor.uncertainties ?? []}
            facts={view.effects}
            tables={tables}
            parameters={view.parameters}
            issues={issues}
            activeIndex={activeEffectIndex}
            onActiveIndexChange={setActiveEffectIndex}
            onChange={(uncertainties) => change({ ...editor, uncertainties })}
            onAction={run}
          />
          )}
          {optionalDestination === "scenarios" && (
          <PlantScenarioEventsEditor
            rows={editor.scenarioEvents ?? []}
            facts={view.events}
            kinds={tables.scenarioEventKinds}
            parameters={view.parameters}
            issues={issues}
            onChange={(scenarioEvents) => change({ ...editor, scenarioEvents })}
            onAction={run}
          />
          )}
        </div>
      )}
      </div>
      </div>
      {editor.persisted && onOpenControlStudio && (
        <section className="trust-section" style={{ paddingTop: 8, paddingBottom: 8 }}>
          <button
            className="trust-button"
            data-plant-open-control-studio
            onClick={onOpenControlStudio}
            style={{ width: "100%" }}
          >
            Open in Control Studio
          </button>
        </section>
      )}
      <div data-plant-validation-summary tabIndex={-1} role={saveError ? "alert" : "status"} style={VALIDATION_SUMMARY}>
        {saveError ? <span>{saveError}</span> : valid ? <span>Ready to save.</span> : !current && validationItems.length === 0 ? (
          <span>{failure ?? "Checking the plant…"}</span>
        ) : (
          <>
            <span>Fix {validationItems.length} item{validationItems.length === 1 ? "" : "s"} before saving.</span>
            {validationItems[0] && (
              <button
                type="button"
                className="trust-button"
                data-plant-focus-first-error
                onClick={() => focusValidationItem(validationItems[0])}
                style={{ marginTop: 5, justifySelf: "start", maxWidth: "100%", whiteSpace: "normal", textAlign: "left", overflowWrap: "anywhere" }}
              >
                {validationItems[0].message}
              </button>
            )}
          </>
        )}
      </div>
      </>}
      {saveConflict && (
        <section data-plant-revision-conflict role="alert" className="trust-section">
          <strong>Saved values changed</strong>
          <p className="trust-help">The saved plant changed since this editor was opened. Your local values are still intact.</p>
          <details style={{ marginBottom: 7 }}>
            <summary data-plant-conflict-review style={{ cursor: "pointer" }}>Review current values</summary>
            <dl className="trust-help" style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "3px 8px" }}>
              <dt>Revision</dt><dd>{saveConflict.currentRevision}</dd>
              <dt>Plant</dt><dd>{String(saveConflict.currentPlant?.display_name ?? saveConflict.currentPlant?.id ?? "Deleted")}</dd>
              <dt>Model</dt><dd>{String(saveConflict.currentPlant?.model ?? "Unavailable")}</dd>
            </dl>
          </details>
          <button type="button" className="trust-button" data-plant-conflict-reload onClick={onReloadConflict}>
            Reload saved values
          </button>
        </section>
      )}
      <footer className="trust-inspector__footer" style={{ flexWrap: "wrap" }}>
        {editor.persisted && onRemove ? (
          <RemoveBrickActions
            identity={editor.id}
            description="Remove this simulated plant from the project? This writes simulation.toml and takes effect after restart."
            onRemove={onRemove}
            busy={saving}
          >
            <button
              className="trust-button trust-button--primary"
              disabled={!canSave}
              title={canSave ? "Save simulated plant" : "Complete the required fields before saving"}
              onClick={() => answer && onSave(answer.candidate)}
            >
              {saving ? "Saving…" : "Save"}
            </button>
            <button className="trust-button" onClick={onCancel}>Cancel</button>
          </RemoveBrickActions>
        ) : (
          <>
            <button className="trust-button" onClick={onCancel}>Cancel</button>
            <button
              className="trust-button trust-button--primary"
              disabled={!canSave}
              title={canSave ? "Save simulated plant" : "Complete the required fields before saving"}
              onClick={() => answer && onSave(answer.candidate)}
            >
              {saving ? "Saving…" : "Save"}
            </button>
          </>
        )}
      </footer>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  left: 0,
  width: "auto",
  maxWidth: "none",
  overflow: "auto",
  background: "var(--vscode-editor-background, var(--trust-bg))",
  zIndex: 7,
};

const CONFIRMATION: React.CSSProperties = {
  display: "grid",
  gap: 8,
  marginTop: 10,
  padding: 10,
  border: "1px solid var(--vscode-widget-border, var(--trust-border))",
  borderRadius: 3,
  background: "var(--vscode-editorWidget-background, var(--vscode-editor-background))",
  color: "var(--vscode-editorWidget-foreground, var(--vscode-editor-foreground))",
  boxShadow: "0 2px 8px var(--vscode-widget-shadow)",
  fontSize: 11.5,
  lineHeight: 1.4,
};

const ICON_BUTTON: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--trust-text-muted)",
  fontSize: 14,
  cursor: "pointer",
  padding: 0,
};

const DIRTY_BADGE: React.CSSProperties = {
  flex: "none",
  border: "1px solid var(--trust-border)",
  borderRadius: 999,
  color: "var(--trust-text-muted)",
  fontSize: 10,
  lineHeight: 1,
  padding: "3px 6px",
};

const VALIDATION_SUMMARY: React.CSSProperties = {
  margin: "auto 12px 10px",
  borderLeft: "2px solid var(--trust-warn)",
  color: "var(--trust-text-muted)",
  fontSize: 11,
  lineHeight: 1.35,
  padding: "6px 8px",
};

const FIELD: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
  marginTop: 9,
};

const FIELD_LABEL: React.CSSProperties = {
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

const FIELD_ERROR: React.CSSProperties = {
  color: "var(--trust-danger)",
  fontSize: 10.5,
  lineHeight: 1.35,
};
