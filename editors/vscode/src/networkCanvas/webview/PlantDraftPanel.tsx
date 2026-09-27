import React, { useEffect, useMemo, useRef, useState } from "react";

import type { PlantDraft } from "../plant/addFlow";
import {
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
  plantPortLabels,
  validatePlantPortBindings,
  updatePlantPortBinding,
} from "../plant/editorModel";
import { plantModelDescriptors } from "../plant/modelDescriptors";
import type { PlantSignalCatalog } from "../plant/signalCatalog";
import { PlantSignalPicker } from "./PlantSignalPicker";
import { PlantDynamicsEditor } from "./PlantDynamicsEditor";
import { PlantStartingConditionsEditor } from "./PlantStartingConditionsEditor";
import { PlantActuatorEditor } from "./PlantActuatorEditor";
import { PlantImperfectionsEditor } from "./PlantImperfectionsEditor";
import { PlantSignalEngineeringEditor } from "./PlantSignalEngineeringEditor";
import { plantUncertaintyFieldLabel, validatePlantUncertainties } from "../plant/uncertaintyEditor";
import { validatePlantScenarioEvents } from "../plant/scenarioEventEditor";
import { PlantScenarioEventsEditor } from "./PlantScenarioEventsEditor";
import { RemoveBrickActions } from "./RemoveBrickActions";
import { PlantTemperatureExampleEditor } from "./PlantTemperatureExampleEditor";
import { applyTemperatureExample, temperatureExampleValues } from "../plant/temperatureExample";
import { plantStartingStateFields } from "../plant/startingConditions";
import { PlantOptionalSettings, type PlantOptionalDestination } from "./PlantOptionalSettings";
import { PlantModelSchematic } from "./PlantModelSchematic";

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
  const [optionalDestination, setOptionalDestination] = useState<PlantOptionalDestination>();
  const [activeEffectIndex, setActiveEffectIndex] = useState(0);
  const [technicalOpen, setTechnicalOpen] = useState(false);
  const [temperatureUnitsConfirmed, setTemperatureUnitsConfirmed] = useState(false);
  const [modelRevision, setModelRevision] = useState(0);
  const [pendingStartingPoint, setPendingStartingPoint] = useState<PlantStartingPoint>();
  const [rawInputIssues, setRawInputIssues] = useState<Record<string, string>>({});
  const startingPointTrigger = useRef<HTMLElement | null>(null);
  const editor = draft;
  const startingStateFields = plantStartingStateFields(editor.model);
  const temperatureView = editor.model === "first_order_dead_time" && editor.startingPoint === "temperature_example";
  const updateRawIssue = (field: string, message?: string) => setRawInputIssues((current) => {
    const next = { ...current };
    if (message) next[field] = message;
    else delete next[field];
    return next;
  });
  const modelDescriptors = plantModelDescriptors();
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
    stateLabels: startingStateFields.map((field) => {
      const quantity = field.label.replace(/^Starting /, "");
      return quantity.charAt(0).toUpperCase() + quantity.slice(1);
    }),
    outputLabels: plantPortLabels(editor).feedbacks,
  }), [editor.enabled, editor.initialState.length, editor.feedbackTargets.length, editor.model, editor.commandSources.length, editor.parameters, startingStateFields]);
  const scenarioIssues = useMemo(
    () => validatePlantScenarioEvents(editor.scenarioEvents ?? [], scenarioContext),
    [editor.scenarioEvents, scenarioContext]
  );
  const portLabels = plantPortLabels(editor);
  const portIssues = useMemo(() => validatePlantPortBindings(editor, signalCatalog, usedFeedbackTargets), [editor, signalCatalog, usedFeedbackTargets]);
  const validationItems = useMemo(() => {
    const items: Array<{ message: string; selector: string; destination?: PlantOptionalDestination; effect?: number }> = [];
    if (temperatureView && !temperatureUnitsConfirmed) {
      items.push({ message: "Confirm that the PLC signals use the displayed heater-percent and temperature units, or configure their conversion.", selector: '[name="plant-temperature-units-confirmed"]' });
    }
    for (const [field, message] of Object.entries(rawInputIssues)) {
      items.push({ message, selector: `[data-plant-dynamics-field="${field}"]` });
    }
    for (const issue of identityIssues) {
      const label = issue.field === "id" ? "Plant ID" : "Display name";
      const value = issue.field === "id" ? editor.plantId : editor.displayName;
      items.push({
        message: `${label} "${value}": ${issue.message}`,
        selector: issue.field === "id" ? '[name="plant-id"]' : '[name="plant-display-name"]',
      });
    }
    for (const issue of portIssues) {
      const base = issue.direction === "command" ? "plant-command-source" : "plant-feedback-target";
      items.push({
        message: issue.message,
        selector: `[name="${base}${issue.index === 0 ? "" : `-${issue.index}`}"]`,
      });
    }
    for (const issue of dynamicsIssues) {
      const key = issue.field.split(".")[0];
      const field = plantDynamicsFields(editor.model).find((candidate) => candidate.key === key);
      const value = key === "initial_state" ? editor.initialState : editor.parameters[key];
      items.push({
        message: `${field?.label ?? key} (${exactValue(value)}): ${issue.message}`,
        selector: key === "input_delays_s"
          ? `[data-plant-input-delay-cell="${issue.field.split(".")[1] ?? "0"}"]`
          : startingStateFields.length > 0 && key === "initial_state"
          ? '[data-plant-starting-state="0"]'
          : `[data-plant-dynamics-field="${key}"]`,
      });
    }
    for (const issue of actuatorIssues) {
      const field = PLANT_ACTUATOR_FIELDS.find((candidate) => candidate.key === issue.field);
      const value = editor.actuator?.[issue.field as keyof NonNullable<typeof editor.actuator>];
      items.push({
        message: `${field?.label ?? issue.field} (${exactValue(value)}): ${issue.message}`,
        selector: `[data-plant-actuator-field="${issue.field}"]`,
        destination: "actuator",
      });
    }
    for (const issue of signalEngineeringIssues) {
      const field = issue.field === "step_ms" ? issue.field : `${issue.field}.0`;
      items.push({
        message: `${SIGNAL_ENGINEERING_LABELS[issue.field] ?? issue.field} (${exactValue(signalEngineeringValue(editor, issue.field))}): ${issue.message}`,
        selector: `[data-plant-engineering-field="${field}"]`,
        destination: "signals",
      });
    }
    for (const issue of uncertaintyIssues) {
      const row = editor.uncertainties?.[issue.row];
      const value = row?.[issue.field as keyof NonNullable<typeof row>];
      items.push({
        message: `Effect ${issue.row + 1} (${row?.id ?? "unknown"}) · ${plantUncertaintyFieldLabel(issue.field)} (${exactValue(value)}): ${issue.message}`,
        selector: `[data-plant-uncertainty-row="${issue.row}"] [data-plant-uncertainty-field="${issue.field}"]`,
        destination: "realism",
        effect: issue.row,
      });
    }
    for (const issue of scenarioIssues) {
      items.push({
        message: `Event ${issue.row + 1}: ${issue.message}`,
        selector: `[data-plant-scenario-event-row="${issue.row}"] [data-plant-scenario-event-field="${issue.field === "at_ms" ? "at_s" : issue.field}"]`,
        destination: "scenarios",
      });
    }
    return items;
  }, [identityIssues, rawInputIssues, portIssues, editor, startingStateFields, dynamicsIssues, actuatorIssues, signalEngineeringIssues, uncertaintyIssues, scenarioIssues, temperatureView, temperatureUnitsConfirmed]);
  const canSave =
    identityIssues.length === 0 &&
    dynamicsIssues.length === 0 &&
    actuatorIssues.length === 0 &&
    signalEngineeringIssues.length === 0 &&
    uncertaintyIssues.length === 0 &&
    scenarioIssues.length === 0 &&
    Object.keys(rawInputIssues).length === 0 &&
    portIssues.length === 0 &&
    (!temperatureView || temperatureUnitsConfirmed) &&
    !saving;
  const pendingStartingPointPlan = pendingStartingPoint
    ? planPlantStartingPointChange({
        currentStartingPoint: editor.startingPoint,
        requestedStartingPoint: pendingStartingPoint,
        modelValuesTouched: true,
      })
    : undefined;
  const applyStartingPoint = (startingPoint: PlantStartingPoint) => {
    if (startingPoint === "temperature_example") {
      onChange(applyTemperatureExample(editor));
      setTemperatureUnitsConfirmed(false);
      setRawInputIssues({});
      setModelRevision((revision) => revision + 1);
      setPendingStartingPoint(undefined);
      return;
    }
    const proposal = startingPoint === "custom_state_space"
      ? proposeCustomStateSpace()
      : proposePlantStartingPoint(startingPoint);
    if (proposal) {
      onChange({ ...editor, startingPoint, ...proposal, modelValuesTouched: false });
      setRawInputIssues({});
      setModelRevision((revision) => revision + 1);
      setTemperatureUnitsConfirmed(false);
    }
    setPendingStartingPoint(undefined);
  };
  const requestStartingPoint = (startingPoint: PlantStartingPoint, trigger: HTMLElement) => {
    startingPointTrigger.current = trigger;
    const plan = planPlantStartingPointChange({
      currentStartingPoint: editor.startingPoint,
      requestedStartingPoint: startingPoint,
      modelValuesTouched: editor.modelValuesTouched,
      hasSavedConfiguration: editor.persisted,
    });
    if (plan.kind === "confirm" || startingPoint === "temperature_example") setPendingStartingPoint(startingPoint);
    else applyStartingPoint(startingPoint);
  };
  const closeStartingPointConfirmation = () => {
    setPendingStartingPoint(undefined);
    requestAnimationFrame(() => startingPointTrigger.current?.focus());
  };
  const focusValidationItem = (item: { selector: string; destination?: PlantOptionalDestination; effect?: number }) => {
    if (item.destination) setOptionalDestination(item.destination);
    if (item.effect !== undefined) setActiveEffectIndex(item.effect);
    if (/plant-id|plant-display-name/.test(item.selector)) setTechnicalOpen(true);
    requestAnimationFrame(() => {
      const target = document.querySelector<HTMLElement>(item.selector);
      target?.scrollIntoView({ block: "center" });
      target?.focus({ preventScroll: true });
    });
  };
  useEffect(() => sectionRef.current?.focus(), []);
  const modelChoice = (
    <label style={FIELD}>
      <span style={FIELD_LABEL}>Plant model</span>
      <select
        name="plant-model"
        data-plant-model-select
        value={editor.model}
        onChange={(event) => {
          const model = event.target.value as import("../plant/editorModel").PlantModelKind;
          const startingPoint = PLANT_STARTING_POINTS.find((candidate) => candidate.model === model);
          requestStartingPoint(startingPoint?.id ?? "custom_state_space", event.currentTarget);
        }}
        style={INPUT}
      >
        {modelDescriptors.map((descriptor) => (
          <option key={descriptor.kind} value={descriptor.kind}>{descriptor.productName}</option>
        ))}
      </select>
      <span data-plant-model-explanation className="trust-help" style={{ marginTop: 1 }}>
        {modelDescriptors.find((descriptor) => descriptor.kind === editor.model)?.purpose}
      </span>
      <PlantModelSchematic model={editor.model} />
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
          {editor.model === "first_order_dead_time" && <button
            type="button" className="trust-button" data-plant-temperature-example
            onClick={(event) => requestStartingPoint("temperature_example", event.currentTarget)}
            style={{ marginTop: 8 }}
          >Use temperature example</button>}
          {temperatureView && <PlantTemperatureExampleEditor
            key={modelRevision}
            draft={editor} inputStyle={INPUT}
            onChange={(next) => onChange({ ...editor, ...next })}
            onRawValidityChange={updateRawIssue}
          />}
        <PlantDynamicsEditor
          key={`${editor.startingPoint}:${editor.model}:${modelRevision}`}
          draft={editor}
          onChange={(next) => onChange({ ...editor, ...next })}
          onRawValidityChange={(field, message) => setRawInputIssues((current) => {
            const next = { ...current };
            if (message) next[field] = message;
            else delete next[field];
            return next;
          })}
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
            <strong id="plant-starting-point-confirmation-title">Replace {pendingStartingPoint === "temperature_example" ? "with temperature example" : "edited dynamics"}?</strong>
            <span id="plant-starting-point-confirmation-message">
              {pendingStartingPoint === "temperature_example"
                ? "Replace model values, starting state, actuator settings and signal conversions with the temperature teaching example? This sets 20 °C at rest and start, 100 °C full-power rise, 30 s response, 2 s delay, heater limits 0–100%, initial heater power 0%, and identity PLC conversions. Keep the existing name, runtime, bindings, validity, imperfections and scenarios; retained effects can change the nominal response."
                : pendingStartingPointPlan?.kind === "confirm"
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
      <div style={{ minWidth: 0 }}>
      <section className="trust-section" data-plant-setup-section="connections">
        <div className="trust-section__title">Connections</div>
        <p className="trust-help" style={{ marginTop: 0 }}>Choose a named PLC signal for each model port. Command ports read PLC outputs; feedback ports write PLC inputs.</p>
        {temperatureView && <div className="trust-help" data-plant-temperature-assumptions>
          <p>{(editor.inputScale?.[0] ?? 1) === 1 && (editor.inputOffset?.[0] ?? 0) === 0
            ? "PLC 0–100 = heater 0–100%"
            : `Heater power (%) = ${editor.inputScale?.[0] ?? 1} × PLC command + ${editor.inputOffset?.[0] ?? 0}`}</p>
          <p>{(editor.outputScale?.[0] ?? 1) === 1 && (editor.outputOffset?.[0] ?? 0) === 0
            ? "PLC feedback = temperature in °C"
            : `PLC feedback = ${editor.outputScale?.[0] ?? 1} × temperature (°C) + ${editor.outputOffset?.[0] ?? 0}`}</p>
          <label style={{ display: "flex", alignItems: "start", gap: 6 }}>
            <input type="checkbox" name="plant-temperature-units-confirmed" checked={temperatureUnitsConfirmed}
              onChange={(event) => setTemperatureUnitsConfirmed(event.target.checked)} />
            I confirm these units and conversions for the selected PLC signals.
          </label>
          <p>Raw-count signals need setup in Signal conversions. Signal names do not establish units.</p>
        </div>}
        <PlantSignalPicker
          catalog={signalCatalog}
          usedFeedbackTargets={usedFeedbackTargets}
          commandSources={editor.commandSources}
          feedbackTargets={editor.feedbackTargets}
          labels={portLabels}
          onCommandSource={(index, address) => {
            setTemperatureUnitsConfirmed(false);
            onChange(updatePlantPortBinding(editor, "command", index, address));
          }}
          onFeedbackTarget={(index, address) => {
            setTemperatureUnitsConfirmed(false);
            onChange(updatePlantPortBinding(editor, "feedback", index, address));
          }}
        />
      </section>
      <section className="trust-section" data-plant-setup-section="starting-conditions" style={{ paddingTop: 8, paddingBottom: 8 }}>
        <div className="trust-section__title">Starting conditions</div>
        {editor.model === "first_order_dead_time" ? (
          <PlantStartingConditionsEditor
            key={`${editor.model}:${editor.startingPoint}:${modelRevision}`}
            value={editor.initialState[0]}
            label={temperatureView ? "Starting temperature (°C)" : undefined}
            description={temperatureView ? "Absolute temperature when simulation starts or resets. This need not equal the resting temperature." : undefined}
            inputStyle={INPUT}
            onChange={(value) => onChange({ ...editor, initialState: [value], modelValuesTouched: true })}
            onValidityChange={(message) => setRawInputIssues((current) => {
              const next = { ...current };
              if (message) next.starting_value = message;
              else delete next.starting_value;
              return next;
            })}
          />
        ) : startingStateFields.length > 0 ? (
          <div style={{ display: "grid", gap: 12 }}>
            {startingStateFields.map(({ label, description }, index) => (
              <PlantStartingConditionsEditor
                key={`${editor.model}:${modelRevision}:${index}`}
                stateIndex={index}
                value={editor.initialState[index]}
                label={label}
                description={description}
                inputStyle={INPUT}
                onChange={(value) => {
                  const initialState = [...editor.initialState];
                  initialState[index] = value;
                  onChange({ ...editor, initialState, modelValuesTouched: true });
                }}
                onValidityChange={(message) => updateRawIssue(`starting_state.${index}`, message)}
              />
            ))}
          </div>
        ) : <p className="trust-help">Initial model state: {JSON.stringify(editor.initialState)}. Change it in Model &amp; behavior.</p>}
        {temperatureView && editor.initialState[0] !== temperatureExampleValues(editor).resting_temperature && <p className="trust-help">Non-equilibrium start: the starting temperature differs from the heater-off resting temperature. The configured input history remains visible in Model details.</p>}
        {temperatureView && <p className="trust-help">Heater range: {editor.actuator?.minimum ?? "unbounded"}–{editor.actuator?.maximum ?? "unbounded"}%. Adjust limits in Actuator &amp; command.</p>}
        <p className="trust-help">Plant step: {editor.stepMs === undefined ? "uses the project simulation setting" : `${editor.stepMs} ms`}. Saving does not start or restart simulation.</p>
      </section>
      <PlantOptionalSettings
        draft={editor}
        active={optionalDestination}
        onActiveChange={setOptionalDestination}
      />
      {optionalDestination && (
        <div data-plant-optional-editor={optionalDestination}>
          {optionalDestination === "actuator" && (
          <PlantActuatorEditor
            value={editor.actuator ?? {}}
            onChange={(actuator) => onChange({ ...editor, actuator })}
          />
          )}
          {optionalDestination === "signals" && (
          <PlantSignalEngineeringEditor
            draft={editor}
            onChange={(next) => {
              setTemperatureUnitsConfirmed(false);
              onChange({ ...editor, ...next });
            }}
          />
          )}
          {optionalDestination === "realism" && (
          <PlantImperfectionsEditor
            rows={editor.uncertainties ?? []}
            activeIndex={activeEffectIndex}
            onActiveIndexChange={setActiveEffectIndex}
            modelParameters={Object.keys(editor.parameters)}
            onChange={(uncertainties) => onChange({ ...editor, uncertainties })}
          />
          )}
          {optionalDestination === "scenarios" && (
          <PlantScenarioEventsEditor
            rows={editor.scenarioEvents ?? []}
            context={scenarioContext}
            onChange={(scenarioEvents) => onChange({ ...editor, scenarioEvents })}
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
                style={{ marginTop: 5, justifySelf: "start", maxWidth: "100%", whiteSpace: "normal", textAlign: "left", overflowWrap: "anywhere" }}
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
