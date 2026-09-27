import { initializeEngineerInspectors } from "./engineerInspectors";
import { initEngineerScenarioEditor } from "./engineerScenarioEditor";
import { initializeEngineerEvidencePresentation } from "./engineerEvidencePresentation";
import { showEngineerInputError } from "./engineerNotices";
import { controllerPinControls } from "./controllerPinControls";
import { initializeEngineerNavigation } from "./engineerNavigation";
import { renderEngineerStageEvidence } from "./engineerStageEvidence";
import { initializeEngineerCandidateHeader } from "./engineerCandidateHeader";
import { formatEngineeringNumber } from "../chartContract";
import { initializeEngineerCampaignHistory, initializeEngineerVerificationHistory } from "./engineerCampaignHistory";
import { EngineerRecordedResponses } from "./engineerRecordedResponses";
import { initializeEngineerStudyControls } from "./engineerStudyControls";
import type { CommissioningResult } from "../proportionalCommissioning";

export function initializeEngineerWorkflow(post: (kind: string, payload?: Record<string, unknown>) => void): void {
  const section = document.querySelector<HTMLElement>("[data-engineer-workflow]");
  if (!section) return;
  const settingPins = controllerPinControls(section);
  initializeEngineerCandidateHeader();
  initializeEngineerCampaignHistory();
  initializeEngineerVerificationHistory();
  initializeEngineerStudyControls();
  const query = <T extends Element>(name: string) => section.querySelector<T>(`[data-engineer-${name}]`)!;
  query("view-results").addEventListener("click", () => { query<HTMLDetailsElement>("results").open = true; });
  const action = (name: string, payload: Record<string, unknown> = {}) => post(`control-studio.engineer.${name}`, payload);
  const tuningPreferences = (): Record<string, unknown> => {
    const method = query<HTMLSelectElement>("tuning-method").value;
    return method === "automatic" ? {} : { tuningMethod: method,
      ...(method === "SIMC" ? { tuningObjective: query<HTMLSelectElement>("tuning-objective").value } : {}) };
  };
  const currentWorkflowInputs = (): Record<string, unknown> => {
    const number = (name: string) => {
      const value = query<HTMLInputElement>(name).valueAsNumber;
      return Number.isFinite(value) ? value : undefined;
    };
    let scenarioDefinitions: unknown;
    try { scenarioDefinitions = JSON.parse(query<HTMLTextAreaElement>("scenarios").value); } catch { /* Tune reports malformed JSON. */ }
    let pinnedValues: Record<string, number> | null = null;
    let pinsValid = false;
    try { pinnedValues = settingPins.read(); pinsValid = true; } catch { /* Preserve the edit; Tune will show the pin error. */ }
    const acquisition = query<HTMLSelectElement>("acquisition-method");
    const acquisitionSettings = {
      method: acquisition.value,
      ...(number("acquisition-duration") === undefined ? {} : { durationSeconds: number("acquisition-duration") }),
      ...(number("acquisition-amplitude") === undefined ? {} : { amplitude: number("acquisition-amplitude") }),
      reuseRecordings: query<HTMLInputElement>("reuse-recordings").checked,
      ...(query<HTMLSelectElement>("relay-direction").value ? { relayDirection: query<HTMLSelectElement>("relay-direction").value } : {}),
    };
    return {
      family: query<HTMLSelectElement>("family").value,
      ...(scenarioDefinitions === undefined ? {} : { scenarioDefinitions }),
      ...(pinsValid ? { pinnedValues } : {}),
      acquisitionSettings,
      ...(number("reference") === undefined ? {} : { referenceChange: number("reference") }),
      ...(number("error") === undefined ? {} : { maximumError: number("error") }),
      ...(number("band") === undefined ? {} : { settlingBand: number("band") }),
      ...(number("settling") === undefined ? {} : { settlingTimeSeconds: number("settling") }),
      ...(number("duration") === undefined ? {} : { durationSeconds: number("duration") }),
      ...(number("transfer-tolerance") === undefined ? {} : { transferTolerance: number("transfer-tolerance") }),
      ...(number("disturbance") === undefined ? {} : { constantInputDisturbance: number("disturbance") }),
      ...(number("noise") === undefined ? {} : { measurementNoiseBand: number("noise") }),
      ...(number("uncertainty") === undefined ? {} : { gainUncertaintyFraction: number("uncertainty") }),
      ...tuningPreferences(),
      applicationMode: query<HTMLSelectElement>("application-mode").value,
    };
  };
  const responses = new EngineerRecordedResponses(section, () => action("verify", {
    applicationMode: query<HTMLSelectElement>("application-mode").value
  }));
  const renderEvidence = initializeEngineerEvidencePresentation(query("evidence"), responses);
  initEngineerScenarioEditor(section, {
    processSymbol: decodeURIComponent(section.dataset.processSymbol ?? ""),
    setpointSymbol: decodeURIComponent(section.dataset.setpointSymbol ?? ""),
    samplePeriodMs: Number(section.dataset.samplePeriod), processMinimum: Number(section.dataset.processMinimum), processMaximum: Number(section.dataset.processMaximum),
    get durationSeconds() { return Number(query<HTMLInputElement>("duration").value) || 4; },
  });
  const relayField = query<HTMLElement>("relay-field");
  const updateRelayField = () => {
    const method = query<HTMLSelectElement>("acquisition-method").value;
    relayField.hidden = !(method === "relay" || method === "configured" && section.dataset.experimentKind === "relay");
    query<HTMLElement>("amplitude-label").textContent = method === "setpoint_step" ? `Setpoint change (${section.dataset.engineerProcessUnit})`
      : ["mv_step", "mv_pulse", "relay", "prbs", "multisine"].includes(method) ? `Output change (${section.dataset.engineerOutputUnit})` : "Amplitude (chosen method)";
  };
  query("acquisition-method").addEventListener("change", updateRelayField); updateRelayField();
  let manualAmplitude = Number(query<HTMLInputElement>("acquisition-amplitude").value) || 1;
  const presentAmplitude = () => {
    const field = query<HTMLInputElement>("acquisition-amplitude");
    const automatic = Number(field.value) === 0;
    query<HTMLInputElement>("auto-amplitude").checked = automatic;
    query<HTMLElement>("amplitude-field").hidden = automatic;
  };
  query("auto-amplitude").addEventListener("change", () => {
    const field = query<HTMLInputElement>("acquisition-amplitude");
    if (query<HTMLInputElement>("auto-amplitude").checked) { manualAmplitude = Number(field.value) || manualAmplitude; field.value = "0"; }
    else field.value = String(manualAmplitude);
    presentAmplitude();
  });
  presentAmplitude();
  const renderNavigation = initializeEngineerNavigation(section, post);
  initializeEngineerInspectors(section);
  const commissioningControls = new Map<HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement, boolean>();
  const lockCommissioningControls = (reviewAllowed = false) => {
    for (const control of section.querySelectorAll<HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("button:not([data-engineer-cancel]):not([data-engineer-stage-tab]):not([data-engineer-return]):not([data-engineer-inspector-close]),input,select,textarea")) {
      if (!commissioningControls.has(control)) commissioningControls.set(control, control.disabled);
      control.disabled = !(reviewAllowed && control.matches("[data-engineer-rollback],[data-engineer-confirm],[data-engineer-retry-recovery],[data-engineer-review-runtime],[data-engineer-inspect]"));
    }
  };
  let operatingModel: { operatingPoint?: { inputs: number[]; outputs: number[] }; inputs: { unit: string }[]; outputs: { unit: string }[] } | undefined;
  const showOperatingPoint = () => {
    const display = query<HTMLElement>("operating-point");
    display.hidden = !operatingModel;
    if (!operatingModel) { display.textContent = ""; return; }
    if (operatingModel.inputs.length !== 1 || operatingModel.outputs.length !== 1) {
      display.textContent = `This model has ${operatingModel.inputs.length} inputs and ${operatingModel.outputs.length} outputs. Targets are evaluated per bound channel.`; return;
    }
    const input = operatingModel.operatingPoint?.inputs[0] ?? 0;
    const output = operatingModel.operatingPoint?.outputs[0] ?? 0;
    const target = output + query<HTMLInputElement>("reference").valueAsNumber;
    display.textContent = `Starting MV ${formatEngineeringNumber(input)} ${operatingModel.inputs[0].unit}; PV ${formatEngineeringNumber(output)} ${operatingModel.outputs[0].unit}; absolute target ${formatEngineeringNumber(target)} ${operatingModel.outputs[0].unit}.`;
  };
  const renderTaskSummary = (visible: boolean) => {
    const summary = query<HTMLElement>("task-summary");
    if (!visible) { summary.hidden = true; summary.textContent = ""; return; }
    const number = (name: string) => query<HTMLInputElement>(name).valueAsNumber;
    const processUnit = section.dataset.engineerProcessUnit ?? "engineering unit";
    summary.textContent = `Target ${number("reference") >= 0 ? "+" : ""}${formatEngineeringNumber(number("reference"))} ${processUnit} · Error ≤${formatEngineeringNumber(number("error"))} ${processUnit} · Response ${formatEngineeringNumber(number("settling"))} s.`;
    summary.hidden = false;
  };
  query("reference").addEventListener("input", showOperatingPoint);
  const tools = query<HTMLButtonElement>("tools");
  const showTools = (visible: boolean) => {
    const studio = document.querySelector<HTMLElement>("[data-control-studio]");
    if (studio) studio.dataset.manualWorkspace = String(visible);
    tools.setAttribute("aria-pressed", String(visible));
    query<HTMLElement>("manual-toolbar").hidden = !visible;
    tools.textContent = visible ? "Return to workflow" : "Open advanced tools";
    if (!visible) section.querySelector<HTMLButtonElement>(`[data-engineer-stage-tab=${section.dataset.selectedStage ?? "model"}]`)?.click();
  };
  tools.addEventListener("click", () => showTools(tools.getAttribute("aria-pressed") !== "true"));
  query("return").addEventListener("click", () => showTools(false));
  query("supply").addEventListener("click", () => {
    showTools(false);
    document.querySelector<HTMLInputElement>("#general-model-file")?.click();
  });
  query("manual-staging").addEventListener("click", () => {
    query<HTMLSelectElement>("application-mode").value = "manual";
    action("verify", { applicationMode: "manual" });
  });
  query("tune").addEventListener("click", () => {
    const invalid = section.querySelector<HTMLInputElement | HTMLTextAreaElement>("[data-engineer-goal-fields] input:invalid,[data-engineer-settings-stage=tune] input:invalid,[data-engineer-settings-stage=verify] input:invalid,[data-engineer-settings-stage=verify] textarea:invalid");
    if (invalid) {
      const owner = invalid.closest<HTMLElement>("[data-engineer-settings-stage]")?.dataset.engineerSettingsStage;
      if (owner) section.querySelector<HTMLButtonElement>(`[data-engineer-stage-tab=${owner}]`)?.click();
      for (let details = invalid.closest("details"); details; details = details.parentElement?.closest("details") ?? null) details.open = true;
      invalid.reportValidity(); showEngineerInputError(section, "Correct the highlighted value before tuning."); return;
    }
    const value = (name: string) => Number(query<HTMLInputElement>(name).value);
    let pinnedValues: Record<string, number> | null;
    try { pinnedValues = settingPins.read(); }
    catch (error) { showEngineerInputError(section, error instanceof Error ? error.message : String(error)); return; }
    let scenarioDefinitions: unknown;
    try { scenarioDefinitions = JSON.parse(query<HTMLTextAreaElement>("scenarios").value); }
    catch { showEngineerInputError(section, "Correct the scenario JSON before tuning."); return; }
    action("tune", { scenarioDefinitions, constantInputDisturbance: value("disturbance"), measurementNoiseBand: value("noise"), gainUncertaintyFraction: value("uncertainty"), pinnedValues, ...tuningPreferences(), family: query<HTMLSelectElement>("family").value, referenceChange: value("reference"),
      maximumError: value("error"), settlingBand: value("band"), settlingTimeSeconds: value("settling"), durationSeconds: value("duration"),
      ...(query<HTMLInputElement>("transfer-tolerance").value.trim() ? { transferTolerance: value("transfer-tolerance") } : {}),
      gainMode: query<HTMLSelectElement>("gain-mode").value, proportionalGain: query<HTMLInputElement>("gain-value").valueAsNumber,
      applicationMode: query<HTMLSelectElement>("application-mode").value });
  });
  query("gain-mode").addEventListener("change", () => { query<HTMLInputElement>("gain-value").disabled = query<HTMLSelectElement>("gain-mode").value !== "manual"; });
  for (const name of ["verify", "apply", "rollback", "confirm", "cancel", "inspect"]) query(name).addEventListener("click", () => action(name,
    { applicationMode: query<HTMLSelectElement>("application-mode").value }));
  query("configure-acquisition").addEventListener("click", () => {
    section.querySelector<HTMLDetailsElement>("[data-engineer-options]")!.open = true;
    query<HTMLSelectElement>("acquisition-method").focus();
  });
  query("configure-scenarios").addEventListener("click", () => {
    section.querySelector<HTMLDetailsElement>("[data-engineer-options]")!.open = true;
    section.querySelector<HTMLDetailsElement>("[data-engineer-scenario-controls]")!.open = true;
    query<HTMLTextAreaElement>("scenarios").focus();
  });
  query("review-runtime").addEventListener("click", () => action("review-runtime"));
  query("retry-recovery").addEventListener("click", () => action("retry-recovery"));
  query("find").addEventListener("click", () => action("find", { acquisitionSettings: {
    method: query<HTMLSelectElement>("acquisition-method").value, durationSeconds: query<HTMLInputElement>("acquisition-duration").valueAsNumber,
    amplitude: query<HTMLInputElement>("acquisition-amplitude").valueAsNumber, reuseRecordings: query<HTMLInputElement>("reuse-recordings").checked,
    ...(query<HTMLSelectElement>("relay-direction").value ? { relayDirection: query<HTMLSelectElement>("relay-direction").value } : {}),
  } }));
  const findButton = query<HTMLButtonElement>("find");
  const tuneButton = query<HTMLButtonElement>("tune");
  const primaryDefaults = new Map([findButton, tuneButton, query<HTMLButtonElement>("verify"), query<HTMLButtonElement>("apply"), query<HTMLButtonElement>("confirm")].map(button => [button, button.title]));
  const restorePrimary = (control: HTMLButtonElement) => {
    control.classList.remove("trust-button--primary");
    control.title = primaryDefaults.get(control) ?? "";
  };
  const promotePrimary = (control: HTMLButtonElement, label: string) => {
    control.classList.add("trust-button--primary");
    control.title = `Next action: ${label}.`;
  };
  query("application-mode").addEventListener("change", () => action("clear-review", { applicationMode: query<HTMLSelectElement>("application-mode").value }));
  section.addEventListener("change", event => {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement && !event.target.matches("[data-engineer-application-mode]")) {
      action("invalidate", currentWorkflowInputs());
    }
  });
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown>;
    const status = String(payload.status);
    if (payload.task && typeof payload.task === "object") {
      const task = payload.task as { tuningMethod?: string; tuningObjective?: string };
      query<HTMLSelectElement>("tuning-method").value = task.tuningMethod ?? "automatic";
      query<HTMLSelectElement>("tuning-objective").value = task.tuningObjective ?? "balanced";
    }
    if (Array.isArray(payload.scenarioDefinitions) && ["restored", "model-ready"].includes(status)) query<HTMLTextAreaElement>("scenarios").value = JSON.stringify(payload.scenarioDefinitions, null, 2);
    query<HTMLButtonElement>("configure-acquisition").hidden = (payload.failure as { nextAction?: string } | undefined)?.nextAction !== "configure-acquisition";
    query<HTMLButtonElement>("configure-scenarios").hidden = (payload.failure as { nextAction?: string } | undefined)?.nextAction !== "configure-scenarios";
    if (status === "restored" && payload.acquisitionSettings) {
      const acquisition = payload.acquisitionSettings as { method: string; durationSeconds: number; amplitude: number; relayDirection?: string; reuseRecordings: boolean };
      query<HTMLSelectElement>("acquisition-method").value = acquisition.method;
      query<HTMLInputElement>("acquisition-duration").value = String(acquisition.durationSeconds);
      query<HTMLInputElement>("acquisition-amplitude").value = String(acquisition.amplitude);
      query<HTMLSelectElement>("relay-direction").value = acquisition.relayDirection ?? "";
      query<HTMLInputElement>("reuse-recordings").checked = acquisition.reuseRecordings;
    }
    renderTaskSummary(["model-ready", "restored", "candidate", "verified", "review"].includes(status));
    settingPins.render(payload);
    if (typeof payload.controllerChoice === "string") query<HTMLSelectElement>("family").value = payload.controllerChoice;
    if (Array.isArray(payload.availableFamilies)) {
      section.dataset.pidAvailable = String(payload.availableFamilies.some(family => ["P", "PI", "PD", "PID"].includes(String(family))));
      const select = query<HTMLSelectElement>("family");
      for (const family of payload.availableFamilies) if (typeof family === "string" && ![...select.options].some(option => option.value === family)) {
        const option = document.createElement("option"); option.value = family; option.textContent = family; select.append(option);
      }
      if (typeof payload.controllerChoice === "string") select.value = payload.controllerChoice;
      for (const option of select.options) option.disabled = option.value !== "automatic" && !payload.availableFamilies.includes(option.value);
    }
    if (!["review-cleared", "busy"].includes(status)) section.dataset.stage = status;

    const baseline = payload.baseline as Parameters<EngineerRecordedResponses["render"]>[0] | undefined;
    const candidate = payload.candidate as Parameters<EngineerRecordedResponses["render"]>[1] | undefined;
    const attempts = Array.isArray(payload.verificationAttempts) ? payload.verificationAttempts as Array<{ status?: unknown; reason?: unknown; run?: unknown }> : [];
    const interrupted = ["cancelled", "failed", "restored"].includes(status) && !baseline && !candidate
      ? [...attempts].reverse().find(attempt => (attempt.status === "cancelled" || attempt.status === "failed") && attempt.run)
      : undefined;
    if (interrupted) responses.renderInterrupted({ status: String(interrupted.status),
      run: interrupted.run as Parameters<EngineerRecordedResponses["renderInterrupted"]>[0]["run"],
      scope: `${typeof interrupted.reason === "string" ? interrupted.reason : "The isolated run ended before its planned response was complete."} The chart contains only received ST-executed prefix samples; no baseline or candidate response is inferred.`,
      eventDescription: "This run ended after its last received ST sample; no terminal point or comparison was inferred." });
    if (status === "commissioning") {
      lockCommissioningControls();
    } else if ((["commissioned", "failed", "cancelled", "applied"].includes(status) || payload.recoveryResolved === true)
      && (payload.commissioning as { recoveryConfirmed?: boolean } | undefined)?.recoveryConfirmed !== false) {
      for (const [control, disabled] of commissioningControls) control.disabled = disabled;
      commissioningControls.clear();
    }
    if (status === "recovery-required" || status === "restored" && (payload.commissioning as CommissioningResult | undefined)?.recoveryConfirmed === false) {
      queueMicrotask(() => lockCommissioningControls(true));
    }
    if (status === "restored") query<HTMLSelectElement>("application-mode").value = payload.applicationMode === "manual" ? "manual" : "auto";
    renderEngineerStageEvidence(payload.stageEvidence);
    if (status === "restored" || status === "model-ready") operatingModel = payload.model as typeof operatingModel;
    if (status === "unavailable") operatingModel = undefined;
    showOperatingPoint();
    if (["restored", "model-ready", "candidate"].includes(status)) {
      const choice = payload.gainChoice as { mode: string; pin?: { value: number } } | undefined;
      query<HTMLSelectElement>("gain-mode").value = choice?.mode ?? "automatic";
      query<HTMLInputElement>("gain-value").disabled = choice?.mode !== "manual";
      if (choice?.pin) query<HTMLInputElement>("gain-value").value = String(choice.pin.value);
    }
    query<HTMLElement>("status").textContent = String(payload.reason ?? status);
    if (payload.cancellationUnconfirmed === true) {
      for (const name of ["find", "tune", "verify", "apply"]) query<HTMLButtonElement>(name).disabled = true;
    } else if (status === "cancelled" || status === "edited") {
      if (typeof payload.canIdentify === "boolean") query<HTMLButtonElement>("find").disabled = !payload.canIdentify;
      if (typeof payload.canTune === "boolean") query<HTMLButtonElement>("tune").disabled = !payload.canTune;
    }
    query<HTMLButtonElement>("confirm").hidden = status !== "review";
    if (status === "review") query<HTMLButtonElement>("confirm").textContent = payload.rollback ? "Restore previous settings" : query<HTMLSelectElement>("application-mode").value === "auto" ? "Apply and start Auto" : "Apply settings";
    query<HTMLButtonElement>("review-runtime").hidden = payload.commissioningCheckpoint !== true;
    query<HTMLButtonElement>("retry-recovery").hidden = !(status === "recovery-required" && payload.retryManualRecovery === true);
    const failure = payload.failure as { nextAction?: unknown } | undefined;
    const nextAction = typeof failure?.nextAction === "string" ? failure.nextAction : undefined;
    for (const button of primaryDefaults.keys()) restorePrimary(button);
    const inspectButton = query<HTMLButtonElement>("inspect");
    const manualStaging = query<HTMLButtonElement>("manual-staging");
    manualStaging.hidden = nextAction !== "choose-application-mode";
    manualStaging.classList.toggle("trust-button--primary", nextAction === "choose-application-mode");
    inspectButton.hidden = nextAction !== "review-implementation";
    inspectButton.classList.toggle("trust-button--primary", nextAction === "review-implementation");
    const normalNext = status === "candidate" ? "verify" : status === "verified" ? "apply" : status === "review" ? "confirm" : ["model-ready", "restored", "edited"].includes(status) ? "tune" : undefined;
    if (normalNext) promotePrimary(query<HTMLButtonElement>(normalNext), normalNext);
    query<HTMLButtonElement>("cancel").hidden = !["running", "commissioning", "busy"].includes(status) && payload.cancellationUnconfirmed !== true;
    if (["recovery-required"].includes(status) || (payload.commissioning as { recoveryConfirmed?: boolean } | undefined)?.recoveryConfirmed === false) {
      const options = query<HTMLDetailsElement>("options");
      options.open = true;
    }
    query<HTMLSelectElement>("family").title = "";
    query<HTMLInputElement>("reference").title = "";
    if (nextAction === "find-model") promotePrimary(findButton, "Find model");
    else if (nextAction === "retune") promotePrimary(tuneButton, "Tune again");
    else if (nextAction === "choose-controller") query<HTMLSelectElement>("family").title = "Next action: Choose controller.";
    else if (nextAction === "revise-requirements") {
      const details = section.querySelector("details");
      if (details) details.open = true;
      query<HTMLInputElement>("reference").title = "Next action: Revise requirements or equipment.";
    }
    if (["model-ready", "edited", "cancelled", "unavailable", "candidate", "verification-failed", "failed"].includes(status)) query<HTMLButtonElement>("apply").disabled = true;
    if (status === "model-ready") query<HTMLButtonElement>("tune").disabled = false;
    if (["model-ready", "edited", "cancelled", "unavailable"].includes(status)) query<HTMLButtonElement>("verify").disabled = true;
    if (status === "candidate") query<HTMLButtonElement>("verify").disabled = false;
    if (status === "verified") query<HTMLButtonElement>("apply").disabled = false;
    if (["edited", "cancelled", "model-ready", "candidate", "unavailable", "failed"].includes(status) && !interrupted) responses.clear();
    renderEvidence(payload, status);
    updateRelayField();
    presentAmplitude();
    renderNavigation(payload);
  });
  section.dataset.ready = "true";
}
