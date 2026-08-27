import React, { useEffect, useMemo, useRef, useState } from "react";

import type { PlantDraft } from "../plant/addFlow";
import {
  CUSTOM_STATE_SPACE_HELP,
  PLANT_ACTUATOR_FIELDS,
  PLANT_STARTING_POINTS,
  planPlantStartingPointChange,
  plantDynamicsFields,
  proposeCustomStateSpace,
  proposePlantStartingPoint,
  plantCandidateFromEditorDraft,
  type PlantStartingPoint,
  validatePlantDynamics,
  validatePlantActuator,
  validatePlantIdentity,
  validatePlantSignalEngineering,
} from "../plant/editorModel";
import {
  type PlantSignalCatalog,
  validateManualPlantSignalAddress,
} from "../plant/signalCatalog";
import { PlantSignalPicker } from "./PlantSignalPicker";
import { PlantDynamicsEditor } from "./PlantDynamicsEditor";
import { PlantActuatorEditor } from "./PlantActuatorEditor";
import { PlantImperfectionsEditor } from "./PlantImperfectionsEditor";
import { PlantSignalEngineeringEditor } from "./PlantSignalEngineeringEditor";
import { plantUncertaintyFieldLabel, validatePlantUncertainties } from "../plant/uncertaintyEditor";
import { validatePlantScenarioEvents } from "../plant/scenarioEventEditor";
import { PlantScenarioEventsEditor } from "./PlantScenarioEventsEditor";
import { RemoveBrickActions } from "./RemoveBrickActions";

export function PlantDraftPanel({
  draft,
  existingPlantIds,
  usedFeedbackTargets,
  signalCatalog,
  onChange,
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
  existingPlantIds: string[];
  usedFeedbackTargets: string[];
  signalCatalog: PlantSignalCatalog;
  onChange: (draft: PlantDraft) => void;
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
  const [engineeringOpen, setEngineeringOpen] = useState(false);
  const [startingPointHelpOpen, setStartingPointHelpOpen] = useState(false);
  const [pendingStartingPoint, setPendingStartingPoint] = useState<PlantStartingPoint>();
  const startingPointTrigger = useRef<HTMLElement | null>(null);
  const editor = draft;
  const selectedStartingPoint = PLANT_STARTING_POINTS.find(
    (candidate) => candidate.id === editor.startingPoint
  );
  const startingPointHelp =
    editor.startingPoint === "custom_state_space"
      ? CUSTOM_STATE_SPACE_HELP
      : selectedStartingPoint;
  const startingPointLabel =
    editor.startingPoint === "custom_state_space"
      ? "Custom state space"
      : selectedStartingPoint?.label ?? "starting point";
  const identityIssues = useMemo(
    () =>
      validatePlantIdentity({
        id: editor.plantId,
        displayName: editor.displayName,
        existingPlantIds,
      }),
    [editor.plantId, editor.displayName, existingPlantIds]
  );
  const issueFor = (field: "id" | "displayName") =>
    identityIssues.find((issue) => issue.field === field);
  const dynamicsIssues = useMemo(() => validatePlantDynamics(editor), [editor]);
  const actuatorIssues = useMemo(() => validatePlantActuator(editor.actuator ?? {}), [editor.actuator]);
  const signalEngineeringIssues = useMemo(() => validatePlantSignalEngineering(editor), [editor]);
  const uncertaintyIssues = useMemo(
    () => validatePlantUncertainties(editor.uncertainties ?? [], Object.keys(editor.parameters)),
    [editor.uncertainties, editor.parameters]
  );
  const scenarioContext = useMemo(() => ({
    enabled: editor.enabled,
    stateCount: editor.initialState.length,
    outputCount: Math.max(1, editor.feedbackTargets.length),
    scalarActuator: editor.model !== "state_space" || editor.commandSources.length <= 1,
    parameters: Object.keys(editor.parameters),
  }), [editor.enabled, editor.initialState.length, editor.feedbackTargets.length, editor.model, editor.commandSources.length, editor.parameters]);
  const scenarioIssues = useMemo(
    () => validatePlantScenarioEvents(editor.scenarioEvents ?? [], scenarioContext),
    [editor.scenarioEvents, scenarioContext]
  );
  const commandValid = editor.commandSources.length === 1 &&
    validateManualPlantSignalAddress(signalCatalog, "command", editor.commandSources[0]).ok;
  const feedbackValid = editor.feedbackTargets.length === 1 &&
    validateManualPlantSignalAddress(signalCatalog, "feedback", editor.feedbackTargets[0], {
      usedFeedbackTargets,
    }).ok;
  const validationItems = useMemo(() => {
    const items: Array<{ message: string; selector: string; engineering?: boolean }> = [];
    for (const issue of identityIssues) {
      const label = issue.field === "id" ? "Plant ID" : "Display name";
      const value = issue.field === "id" ? editor.plantId : editor.displayName;
      items.push({
        message: `${label} "${value}": ${issue.message}`,
        selector: issue.field === "id" ? '[name="plant-id"]' : '[name="plant-display-name"]',
      });
    }
    if (!commandValid) {
      const result = editor.commandSources[0]
        ? validateManualPlantSignalAddress(signalCatalog, "command", editor.commandSources[0])
        : undefined;
      items.push({
        message: result && !result.ok ? result.message : "Choose the PLC output that commands the plant.",
        selector: '[name="plant-command-source"]',
      });
    }
    if (!feedbackValid) {
      const result = editor.feedbackTargets[0]
        ? validateManualPlantSignalAddress(signalCatalog, "feedback", editor.feedbackTargets[0], { usedFeedbackTargets })
        : undefined;
      items.push({
        message: result && !result.ok ? result.message : "Choose the PLC input that receives simulated feedback.",
        selector: '[name="plant-feedback-target"]',
      });
    }
    for (const issue of dynamicsIssues) {
      const key = issue.field.split(".")[0];
      const field = plantDynamicsFields(editor.model).find((candidate) => candidate.key === key);
      const value = key === "initial_state" ? editor.initialState : editor.parameters[key];
      items.push({
        message: `${field?.label ?? key} (${exactValue(value)}): ${issue.message}`,
        selector: `[data-plant-dynamics-field="${key}"]`,
      });
    }
    for (const issue of actuatorIssues) {
      const field = PLANT_ACTUATOR_FIELDS.find((candidate) => candidate.key === issue.field);
      const value = editor.actuator?.[issue.field as keyof NonNullable<typeof editor.actuator>];
      items.push({
        message: `${field?.label ?? issue.field} (${exactValue(value)}): ${issue.message}`,
        selector: `[data-plant-actuator-field="${issue.field}"]`,
        engineering: true,
      });
    }
    for (const issue of signalEngineeringIssues) {
      const field = issue.field === "step_ms" ? issue.field : `${issue.field}.0`;
      items.push({
        message: `${SIGNAL_ENGINEERING_LABELS[issue.field] ?? issue.field} (${exactValue(signalEngineeringValue(editor, issue.field))}): ${issue.message}`,
        selector: `[data-plant-engineering-field="${field}"]`,
        engineering: true,
      });
    }
    for (const issue of uncertaintyIssues) {
      const row = editor.uncertainties?.[issue.row];
      const value = row?.[issue.field as keyof NonNullable<typeof row>];
      items.push({
        message: `Effect ${issue.row + 1} (${row?.id ?? "unknown"}) · ${plantUncertaintyFieldLabel(issue.field)} (${exactValue(value)}): ${issue.message}`,
        selector: `[data-plant-uncertainty-row="${issue.row}"] [data-plant-uncertainty-field="${issue.field}"]`,
        engineering: true,
      });
    }
    for (const issue of scenarioIssues) {
      items.push({
        message: `Event ${issue.row + 1}: ${issue.message}`,
        selector: `[data-plant-scenario-event-row="${issue.row}"] [data-plant-scenario-event-field="${issue.field}"]`,
        engineering: true,
      });
    }
    return items;
  }, [identityIssues, commandValid, feedbackValid, editor.commandSources, editor.feedbackTargets, signalCatalog, usedFeedbackTargets, dynamicsIssues, actuatorIssues, signalEngineeringIssues, uncertaintyIssues, scenarioIssues]);
  const canSave =
    identityIssues.length === 0 &&
    dynamicsIssues.length === 0 &&
    actuatorIssues.length === 0 &&
    signalEngineeringIssues.length === 0 &&
    uncertaintyIssues.length === 0 &&
    scenarioIssues.length === 0 &&
    commandValid &&
    feedbackValid &&
    !saving;
  const pendingStartingPointPlan = pendingStartingPoint
    ? planPlantStartingPointChange({
        currentStartingPoint: editor.startingPoint,
        requestedStartingPoint: pendingStartingPoint,
        modelValuesTouched: true,
      })
    : undefined;
  const applyStartingPoint = (startingPoint: PlantStartingPoint) => {
    const proposal = startingPoint === "custom_state_space"
      ? proposeCustomStateSpace()
      : proposePlantStartingPoint(startingPoint);
    if (proposal) onChange({ ...editor, startingPoint, ...proposal, modelValuesTouched: false });
    setPendingStartingPoint(undefined);
  };
  const requestStartingPoint = (startingPoint: PlantStartingPoint, trigger: HTMLElement) => {
    startingPointTrigger.current = trigger;
    const plan = planPlantStartingPointChange({
      currentStartingPoint: editor.startingPoint,
      requestedStartingPoint: startingPoint,
      modelValuesTouched: editor.modelValuesTouched,
    });
    if (plan.kind === "confirm") setPendingStartingPoint(startingPoint);
    else applyStartingPoint(startingPoint);
  };
  const closeStartingPointConfirmation = () => {
    setPendingStartingPoint(undefined);
    requestAnimationFrame(() => startingPointTrigger.current?.focus());
  };
  const focusValidationItem = (item: { selector: string; engineering?: boolean }) => {
    if (item.engineering) setEngineeringOpen(true);
    requestAnimationFrame(() => {
      const target = document.querySelector<HTMLElement>(item.selector);
      target?.scrollIntoView({ block: "center" });
      target?.focus({ preventScroll: true });
    });
  };
  useEffect(() => sectionRef.current?.focus(), []);

  return (
    <aside
      className="trust-inspector"
      style={PANEL}
      aria-label="Plant properties"
      data-plant-dirty={String(editor.dirty)}
    >
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">
            {editor.persisted
              ? `Devices & Connections / Edit ${editor.plantId}`
              : "Devices & Connections / Add simulated plant"}
          </div>
          <div className="trust-inspector__title">Plant properties</div>
        </div>
        <button onClick={onCancel} aria-label="Cancel simulated plant draft" style={ICON_BUTTON}>✕</button>
      </div>
      <section
        ref={sectionRef}
        data-plant-section
        tabIndex={-1}
        className="trust-section"
        aria-labelledby="plant-draft-section-title"
        style={{ outline: "none" }}
      >
        <div id="plant-draft-section-title" className="trust-section__title">Plant</div>
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
            onChange={(event) => onChange({ ...editor, plantId: event.target.value })}
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
            onChange={(event) => onChange({ ...editor, displayName: event.target.value })}
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
            onChange={(event) => onChange({ ...editor, description: event.target.value })}
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
              onChange={(event) => onChange({ ...editor, enabled: event.target.checked })}
            />
            Enabled
          </label>
        </div>
        {editor.startingPoint === "custom_state_space" ? (
          <div style={{ ...FIELD, position: "relative" }}>
            <div style={HELP_LABEL_ROW}>
              <span style={FIELD_LABEL}>Starting point</span>
              <button
                type="button"
                data-plant-starting-point-help
                aria-label={`Help for ${startingPointLabel}`}
                aria-expanded={startingPointHelpOpen}
                onClick={() => setStartingPointHelpOpen((open) => !open)}
                style={HELP_BUTTON}
              >?</button>
            </div>
            {startingPointHelpOpen && startingPointHelp && (
              <div data-plant-help-popover role="dialog" aria-label={`Help for ${startingPointLabel}`} style={HELP_POPOVER}>
                <strong>Why it fits</strong><br />{startingPointHelp.whyReasonable}<br />
                <strong>Limitation</strong><br />{startingPointHelp.limitation}
              </div>
            )}
            <div data-plant-advanced-model style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{ flex: 1, fontSize: 12 }}>Custom state space</span>
              <span style={DIRTY_BADGE}>Advanced</span>
            </div>
            <button
              data-plant-familiar-starting-points
              className="trust-button"
              onClick={(event) => requestStartingPoint("temperature", event.currentTarget)}
              style={{ alignSelf: "flex-start" }}
            >
              Choose a familiar starting point
            </button>
          </div>
        ) : (
          <div style={{ ...FIELD, position: "relative" }}>
            <div style={HELP_LABEL_ROW}>
              <label htmlFor="plant-starting-point-select" style={FIELD_LABEL}>Starting point</label>
              <button
                type="button"
                data-plant-starting-point-help
                aria-label={`Help for ${startingPointLabel}`}
                aria-expanded={startingPointHelpOpen}
                onClick={() => setStartingPointHelpOpen((open) => !open)}
                style={HELP_BUTTON}
              >?</button>
            </div>
            {startingPointHelpOpen && startingPointHelp && (
              <div data-plant-help-popover role="dialog" aria-label={`Help for ${startingPointLabel}`} style={HELP_POPOVER}>
                <strong>Why it fits</strong><br />{startingPointHelp.whyReasonable}<br />
                <strong>Limitation</strong><br />{startingPointHelp.limitation}
              </div>
            )}
            <select
              id="plant-starting-point-select"
              name="plant-starting-point"
              value={editor.startingPoint}
              onChange={(event) => {
                const startingPoint = event.target.value as PlantStartingPoint;
                requestStartingPoint(startingPoint, event.currentTarget);
              }}
              style={INPUT}
            >
              {PLANT_STARTING_POINTS.map((startingPoint) => (
                <option key={startingPoint.id} value={startingPoint.id}>
                  {startingPoint.label}
                </option>
              ))}
            </select>
            <span data-plant-starting-point-note className="trust-help" style={{ marginTop: 1 }}>
              Editable simulation starting point — adapt it to the process you want to study.
            </span>
            <button
              type="button"
              data-plant-custom-state-space
              className="trust-button"
              onClick={(event) => requestStartingPoint("custom_state_space", event.currentTarget)}
              style={{ alignSelf: "flex-start" }}
            >
              Advanced: use custom state space…
            </button>
          </div>
        )}
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
            <strong id="plant-starting-point-confirmation-title">Replace edited dynamics?</strong>
            <span id="plant-starting-point-confirmation-message">
              {pendingStartingPointPlan?.kind === "confirm"
                ? pendingStartingPointPlan.message
                : "Replace the edited dynamics?"}
            </span>
            <span style={{ display: "flex", gap: 6, justifyContent: "flex-end" }}>
              <button className="trust-button" onClick={closeStartingPointConfirmation}>Keep current values</button>
              <button
                autoFocus
                className="trust-button trust-button--primary"
                onClick={() => applyStartingPoint(pendingStartingPoint)}
              >Replace values</button>
            </span>
          </div>
        )}
      </section>
      <section className="trust-section">
        <div className="trust-section__title">PLC signals</div>
        <PlantSignalPicker
          catalog={signalCatalog}
          usedFeedbackTargets={usedFeedbackTargets}
          commandSource={editor.commandSources[0]}
          feedbackTarget={editor.feedbackTargets[0]}
          onCommandSource={(address) =>
            onChange({
              ...editor,
              commandSources: address ? [address] : [],
              initialInput: address ? resizeScalar(editor.initialInput, 0) : [],
              inputScale: address ? resizeScalar(editor.inputScale, 1) : [],
              inputOffset: address ? resizeScalar(editor.inputOffset, 0) : [],
            })
          }
          onFeedbackTarget={(address) =>
            onChange({
              ...editor,
              feedbackTargets: address ? [address] : [],
              outputScale: address ? resizeScalar(editor.outputScale, 1) : [],
              outputOffset: address ? resizeScalar(editor.outputOffset, 0) : [],
            })
          }
        />
      </section>
      <PlantDynamicsEditor
        key={`${editor.startingPoint}:${editor.model}`}
        draft={editor}
        onChange={(next) => onChange({ ...editor, ...next })}
      />
      <section className="trust-section" style={{ paddingTop: 8, paddingBottom: 8 }}>
        <button
          className="trust-button"
          aria-expanded={engineeringOpen}
          onClick={() => setEngineeringOpen((open) => !open)}
          style={{ width: "100%", display: "flex", justifyContent: "space-between" }}
        >
          <span>{engineeringOpen ? "Hide" : "Show"} engineering details</span>
          <span aria-hidden="true">{engineeringOpen ? "−" : "+"}</span>
        </button>
      </section>
      {engineeringOpen && (
        <>
          <PlantActuatorEditor
            value={editor.actuator ?? {}}
            onChange={(actuator) => onChange({ ...editor, actuator })}
          />
          <PlantSignalEngineeringEditor
            draft={editor}
            onChange={(next) => onChange({ ...editor, ...next })}
          />
          <PlantImperfectionsEditor
            rows={editor.uncertainties ?? []}
            modelParameters={Object.keys(editor.parameters)}
            onChange={(uncertainties) => onChange({ ...editor, uncertainties })}
          />
          <PlantScenarioEventsEditor
            rows={editor.scenarioEvents ?? []}
            context={scenarioContext}
            onChange={(scenarioEvents) => onChange({ ...editor, scenarioEvents })}
          />
        </>
      )}
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
      <div data-plant-validation-summary role={saveError ? "alert" : "status"} style={VALIDATION_SUMMARY}>
        {saveError ? <span>{saveError}</span> : canSave ? <span>Ready to save.</span> : (
          <>
            <span>Fix {validationItems.length} item{validationItems.length === 1 ? "" : "s"} before saving.</span>
            {validationItems[0] && (
              <button
                type="button"
                className="trust-button"
                data-plant-focus-first-error
                onClick={() => focusValidationItem(validationItems[0])}
                style={{ marginTop: 5, justifySelf: "start" }}
              >
                {validationItems[0].message}
              </button>
            )}
          </>
        )}
      </div>
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
              onClick={() => onSave(plantCandidateFromEditorDraft(editor))}
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
              onClick={() => onSave(plantCandidateFromEditorDraft(editor))}
            >
              {saving ? "Saving…" : "Save"}
            </button>
          </>
        )}
      </footer>
    </aside>
  );
}

function resizeScalar(current: number[] | undefined, fallback: number): number[] {
  return [current?.[0] ?? fallback];
}

function exactValue(value: unknown): string {
  return JSON.stringify(value) ?? String(value);
}

function signalEngineeringValue(editor: PlantDraft, field: string): unknown {
  switch (field) {
    case "step_ms": return editor.stepMs;
    case "initial_input": return editor.initialInput;
    case "input_scale": return editor.inputScale;
    case "input_offset": return editor.inputOffset;
    case "output_scale": return editor.outputScale;
    case "output_offset": return editor.outputOffset;
    default: return undefined;
  }
}

const SIGNAL_ENGINEERING_LABELS: Record<string, string> = {
  step_ms: "Plant step override (ms)",
  initial_input: "Initial input",
  input_scale: "Input scale",
  input_offset: "Input offset",
  output_scale: "Output scale",
  output_offset: "Output offset",
};

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 360,
  maxWidth: "100%",
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

const HELP_LABEL_ROW: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 8,
};

const HELP_BUTTON: React.CSSProperties = {
  width: 20,
  height: 20,
  border: "1px solid var(--trust-border)",
  borderRadius: 10,
  background: "transparent",
  color: "var(--trust-text-muted)",
  cursor: "pointer",
  fontSize: 11,
  padding: 0,
};

const HELP_POPOVER: React.CSSProperties = {
  position: "absolute",
  top: 23,
  left: 0,
  right: 0,
  zIndex: 4,
  border: "1px solid var(--trust-border)",
  borderRadius: 3,
  background: "var(--vscode-editorHoverWidget-background, var(--trust-surface))",
  color: "var(--trust-text)",
  fontSize: 11,
  lineHeight: 1.4,
  padding: "8px 9px",
  boxShadow: "0 3px 10px var(--vscode-widget-shadow)",
};
