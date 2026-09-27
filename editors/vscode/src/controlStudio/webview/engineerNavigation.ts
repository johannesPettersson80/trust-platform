import { presentEngineerNotice } from "./engineerNotices";
import { ENGINEER_STAGES, isEngineerStage, type EngineerStage } from "../engineerNavigationStorage";

type Evidence = { hasModel?: boolean; hasCandidate?: boolean; validationHasResult?: boolean; validationPassed?: boolean };
/** Navigation changes presentation only; existing handlers retain operation authority. */
export function initializeEngineerNavigation(section: HTMLElement, post: (kind: string, payload?: Record<string, unknown>) => void) {
  let selected: EngineerStage = "model";
  let navigationStarted = false;
  let status = "start";
  let latest: Record<string, unknown> = {};
  let reviewAutoOpened = false;
  let modelSource = "";
  let validationError: number | undefined;
  let rollbackAvailable = false;
  let rollbackReview = false;
  let evidence: Evidence = {};
  const root = section.closest<HTMLElement>("[data-control-studio]")!;
  const query = <T extends HTMLElement>(name: string) => section.querySelector<T>(`[data-engineer-${name}]`)!;
  const actions = query("actions");
  const secondary = section.ownerDocument.createElement("div");
  secondary.className = "engineer-secondary";
  secondary.append(query("options"), query("results"));
  section.append(secondary);
  // The actions keep their original identity and handlers, but never move into Settings.
  for (const name of ["find", "supply", "rollback"]) actions.append(query(name));
  function select(stage: EngineerStage, persist = true) {
    if (persist) navigationStarted = true;
    selected = stage;
    root.dataset.manualWorkspace = "false";
    query("manual-toolbar").hidden = true;
    query("tools").setAttribute("aria-pressed", "false");
    query("tools").textContent = "Open advanced tools";
    render();
    if (persist) post("control-studio.navigation.select", { stage });
  }
  query("prerequisite-action").addEventListener("click", () => { const stage = query("prerequisite-action").dataset.targetStage; if (isEngineerStage(stage)) select(stage); });
  query("use-model").addEventListener("click", () => select("tune"));
  query("verify").addEventListener("click", () => select("verify"));
  query("apply").addEventListener("click", () => select("apply"));
  query("gain-mode").addEventListener("change", render);
  const tabs = Array.from(section.querySelectorAll<HTMLButtonElement>("[data-engineer-stage-tab]"));
  for (const [index, tab] of tabs.entries()) {
    tab.addEventListener("click", () => select(tab.dataset.engineerStageTab as EngineerStage));
    tab.addEventListener("keydown", event => {
      const position = event.key === "ArrowRight" ? (index + 1) % tabs.length : event.key === "ArrowLeft" ? (index + tabs.length - 1) % tabs.length
        : event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : undefined;
      if (position === undefined) return;
      event.preventDefault(); tabs[position].focus(); tabs[position].click();
    });
  }
  window.addEventListener("message", event => {
    const message = event.data;
    if (navigationStarted || root.dataset.manualWorkspace === "true") return;
    if (message?.kind !== "control-studio.navigation.restore" || message.projectId !== root.dataset.projectId ||
      message.editorSessionId !== root.dataset.editorSessionId || message.loopId !== root.dataset.loopId ||
      message.loopRevision !== root.dataset.loopRevision || !isEngineerStage(message.stage)) return;
    select(message.stage, false);
  });
  function render() {
    presentEngineerNotice(section, latest);
    const details = query<HTMLDetailsElement>("results");
    if (status === "review" && !details.open) { details.open = true; reviewAutoOpened = true; }
    else if (status !== "review" && reviewAutoOpened) { details.open = false; reviewAutoOpened = false; }
    section.dataset.selectedStage = selected;
    actions.setAttribute("aria-labelledby", `engineer-stage-${selected}`);
    const busy = ["running", "busy", "commissioning"].includes(status);
    if (busy) query<HTMLButtonElement>("apply").disabled = true;
    const recovering = (latest.commissioning as { recoveryConfirmed?: boolean } | undefined)?.recoveryConfirmed === false || status === "recovery-required" || latest.commissioningCheckpoint === true || latest.cancellationUnconfirmed === true;
    query<HTMLButtonElement>("tune").disabled = !evidence.hasModel || busy || recovering;
    query<HTMLButtonElement>("verify").disabled = !evidence.hasCandidate || busy || recovering;
    query<HTMLButtonElement>("apply").disabled = !evidence.validationPassed || busy || recovering || status !== "verified";
    query<HTMLButtonElement>("use-model").disabled = busy || recovering;
    const completed = [Boolean(evidence.hasModel), Boolean(evidence.hasCandidate), Boolean(evidence.validationPassed), ["applied", "commissioned"].includes(status)];
    const saved = [false, Boolean(latest.draft || latest.familyEvaluations), Boolean(latest.baseline || latest.candidate), Boolean(latest.commissioning)];
    for (const [index, tab] of tabs.entries()) {
      const active = tab.dataset.engineerStageTab === selected;
      tab.disabled = false;
      tab.setAttribute("aria-selected", String(active)); tab.tabIndex = active ? 0 : -1;
      const result = section.querySelector<HTMLElement>(`[data-engineer-stage-result="${ENGINEER_STAGES[index]}"]`)!;
      result.textContent = completed[index] ? "✓" : status === "restored" && saved[index] ? "Saved" : "";
      tab.setAttribute("aria-label", `${ENGINEER_STAGES[index]}, ${completed[index] ? "complete" : status === "restored" && saved[index] ? "saved, needs review" : "not complete"}`);
    }
    const stageActions: Record<EngineerStage, string[]> = { model: ["find", "supply"], tune: ["tune"], verify: ["verify"], apply: ["apply", "rollback"] };
    for (const name of ["find", "supply", "tune", "verify", "apply", "rollback"]) query<HTMLButtonElement>(name).hidden = !stageActions[selected].includes(name);
    const attempts = Array.isArray(latest.verificationAttempts) ? latest.verificationAttempts : [];
    const lastAttempt = attempts[attempts.length - 1] as { run?: { samples?: unknown[] } } | undefined;
    if (status === "failed" && lastAttempt?.run?.samples?.length === 0) query("verify").hidden = true;
    query("use-model").hidden = selected !== "model" || !evidence.hasModel;
    const rollback = query<HTMLButtonElement>("rollback");
    const recoveryAccess = query("rollback-access");
    if (rollbackAvailable || recovering) actions.append(rollback); else recoveryAccess.append(rollback);
    recoveryAccess.hidden = selected !== "apply" || rollbackAvailable || recovering;
    rollback.hidden = status === "review" || selected !== "apply" && !recovering;
    query<HTMLButtonElement>("view-results").hidden = selected !== "apply" || !["applied", "commissioned"].includes(status);
    if (["applied", "commissioned", "review"].includes(status)) query<HTMLButtonElement>("apply").hidden = true;
    query("controller-choice").hidden = selected !== "tune";
    query<HTMLButtonElement>("find").textContent = evidence.hasModel ? "Identify again" : "Identify model";
    query("task-summary").hidden = selected === "model" || !evidence.hasModel;
    const completedApplication = ["applied", "commissioned"].includes(status);
    const recoveryAction = (latest.failure as { nextAction?: string } | undefined)?.nextAction;
    const tuneRecovery = ["retune", "choose-controller", "revise-requirements"].includes(recoveryAction ?? "");
    const targetStage = tuneRecovery && selected === "verify" ? "tune" : completedApplication || selected === "model" ? undefined : !evidence.hasModel ? "model"
      : selected !== "tune" && !evidence.hasCandidate ? "tune" : selected === "apply" && !evidence.validationPassed ? "verify" : undefined;
    const prerequisite = tuneRecovery && targetStage === "tune" ? "Review the failed response, then adjust the controller or response target." : targetStage === "model" ? "Accept a model first. Open Model to find or supply one."
      : targetStage === "tune" ? "Tune a controller first. Saved results remain available in Details."
      : targetStage === "verify" ? "Pass verification for the current controller before applying." : "";
    const go = query<HTMLButtonElement>("prerequisite-action");
    go.hidden = !targetStage || busy || recovering || status === "review" || Boolean(latest.failure) && !tuneRecovery;
    if (targetStage) { go.dataset.targetStage = targetStage; go.textContent = tuneRecovery ? "Adjust tuning" : `Go to ${targetStage[0].toUpperCase()}${targetStage.slice(1)}`; }
    query("prerequisite").textContent = prerequisite;
    if (prerequisite) query<HTMLButtonElement>(selected === "model" ? "find" : selected).disabled = true;
    for (const group of Array.from(section.querySelectorAll<HTMLElement>("[data-engineer-settings-stage]"))) group.hidden = group.dataset.engineerSettingsStage !== selected;
    const choice = query<HTMLSelectElement>("family").value;
    const builtinTuning = section.dataset.engineerBuiltinTuning !== "false";
    const availableFamilies = Array.from(query<HTMLSelectElement>("family").options)
      .filter(option => !option.disabled).map(option => option.value);
    const pidFamiliesAvailable = builtinTuning && availableFamilies.some(family => ["PI", "PD", "PID"].includes(family));
    const pidOptions = builtinTuning && (choice === "automatic" ? pidFamiliesAvailable : ["PI", "PD", "PID"].includes(choice));
    for (const group of Array.from(section.querySelectorAll<HTMLElement>("[data-engineer-applicability=pid]"))) group.hidden = !pidOptions;
    const objectiveGroup = query("tuning-objective").closest<HTMLElement>("[data-engineer-applicability=pid]");
    if (objectiveGroup) objectiveGroup.hidden = !pidOptions || query<HTMLSelectElement>("tuning-method").value !== "SIMC";
    const gainControls = query<HTMLElement>("gain-controls");
    gainControls.hidden = !builtinTuning || !availableFamilies.includes("P") || !(choice === "P" || choice === "automatic" && !pidFamiliesAvailable);
    const gainMode = query<HTMLSelectElement>("gain-mode");
    gainMode.disabled = gainControls.hidden;
    query<HTMLInputElement>("gain-value").disabled = gainControls.hidden || gainMode.value !== "manual";
    query<HTMLInputElement>("gain-value").closest("label")!.hidden = gainControls.hidden || gainMode.value !== "manual";
    const goals = query<HTMLFieldSetElement>("goal-fields");
    goals.hidden = selected !== "tune";
    goals.disabled = !evidence.hasModel || busy || recovering;
    query("task-summary").hidden = selected === "model" || selected === "tune" || !evidence.hasModel;
    query("readiness").textContent = recovering ? "Recovery needed" : latest.failure || ["failed", "verification-failed", "unavailable"].includes(status) ? "Needs attention" : busy ? "Working…" : targetStage === "model" ? "Model needed" : targetStage === "tune" ? "Tune first" : targetStage === "verify" ? "Verify first" : selected === "model" ? evidence.hasModel ? modelSource === "provided" ? "Imported model ready" : modelSource === "fitted" ? `Identified model ready${validationError === undefined ? "" : ` · Validation error ${(validationError * 100).toFixed(1)}%`}` : "Model ready" : "No model selected"
      : selected === "tune" ? evidence.hasCandidate ? "Controller tuned" : evidence.hasModel ? "Ready to tune" : "Model needed"
      : selected === "verify" ? evidence.validationPassed ? "Verification passed" : evidence.hasCandidate ? "Ready to verify" : "Tune first"
      : status === "commissioned" ? "Auto control observed" : status === "applied" ? rollbackReview ? "Previous settings restored" : "Settings applied" : status === "review" ? rollbackReview ? "Ready to restore" : "Ready to apply" : evidence.validationPassed ? "Ready to review" : "Verification needed";
    for (const group of Array.from(section.querySelectorAll<HTMLElement>("[data-engineer-evidence-stage]"))) group.hidden = group.dataset.engineerEvidenceStage !== selected;
    for (const button of Array.from(section.querySelectorAll(".trust-button--primary"))) button.classList.remove("trust-button--primary");
    const next = (latest.failure as { nextAction?: string } | undefined)?.nextAction;
    let primary: string = targetStage && !go.hidden ? "prerequisite-action" : selected === "model" ? evidence.hasModel ? "use-model" : "find" : selected;
    if (!targetStage && selected === "tune" && evidence.hasCandidate && !tuneRecovery) primary = "verify";
    if (!targetStage && selected === "verify" && evidence.validationPassed && status === "verified") primary = "apply";
    if (completedApplication && selected === "apply") primary = "view-results";
    if (busy || latest.cancellationUnconfirmed === true) primary = "cancel";
    else if (recovering) primary = latest.retryManualRecovery === true ? "retry-recovery" : "rollback";
    else if (status === "review") primary = "confirm";
    else if (next === "configure-acquisition") primary = "configure-acquisition";
    else if (next === "configure-scenarios") primary = "configure-scenarios";
    else if (next === "review-implementation") primary = "inspect";
    else if (next === "choose-application-mode") primary = "manual-staging";
    const button = query<HTMLButtonElement>(primary);
    button.hidden = false; button.classList.add("trust-button--primary");
    // Plot navigation has no runtime commands and cannot invalidate recorded results.
    window.dispatchEvent(new CustomEvent("trust-control-studio-engineer-stage", { detail: { stage: selected } }));
  }
  render();
  return (payload: Record<string, unknown>) => {
    const previous = status;
    status = String(payload.status ?? status);
    latest = payload;
    if (status === "review") rollbackReview = payload.rollback === true;
    if (status === "applied") rollbackAvailable = !rollbackReview;
    if (status === "commissioned") rollbackAvailable = true;
    if ((payload.commissioning as { status?: string } | undefined)?.status === "passed") rollbackAvailable = true;
    if (payload.model && ["model-ready", "restored"].includes(status)) validationError = undefined;
    if (payload.identificationPresentation) validationError = (payload.identificationPresentation as { validation?: { normalizedError?: number } }).validation?.normalizedError;
    const model = payload.model as { source?: string } | undefined; if (model?.source) modelSource = model.source;
    if (payload.stageEvidence) evidence = payload.stageEvidence as Evidence;
    if (payload.model && (payload.stageEvidence as Evidence | undefined)?.hasModel === undefined
        && ["model-ready", "restored"].includes(status)) evidence = { ...evidence, hasModel: true };
    if (["cancelled", "edited", "restored", "unavailable", "model-ready"].includes(status)) evidence = { ...evidence, hasCandidate: false, validationPassed: false };
    if (["failed", "verification-failed"].includes(status)) evidence = { ...evidence, validationPassed: false };
    if (status !== previous) {
      const next = (payload.failure as { nextAction?: string } | undefined)?.nextAction;
      const destination = status === "verification-failed" && ["retune", "choose-controller", "revise-requirements"].includes(next ?? "") ? "verify" : next === "review-implementation" && Array.isArray(payload.verificationAttempts) && payload.verificationAttempts.some((attempt: { status?: string }) => attempt.status === "failed") ? "verify"
        : next === "find-model" || next === "configure-acquisition" ? "model"
        : next === "retune" || next === "choose-controller" || next === "revise-requirements" ? "tune"
        : next === "configure-scenarios" ? "verify"
        : next === "choose-application-mode" ? "apply" : status === "model-ready" ? "model"
        : ["review", "commissioned", "applied", "recovery-required"].includes(status) ? "apply" : undefined;
      if (destination) { select(destination); return; }
    }
    render();
  };
}
