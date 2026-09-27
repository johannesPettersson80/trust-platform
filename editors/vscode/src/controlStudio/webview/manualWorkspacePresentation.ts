const MANUAL_STAGES = ["Monitor", "Test", "Identify", "Tune", "Validate"] as const;

/**
 * Keeps the legacy engineering workspaces compact while leaving their existing
 * controls, handlers, safety approvals, and evidence nodes in place.
 */
export function initializeManualWorkspacePresentation(owner: Document = document): void {
  const root = owner.querySelector<HTMLElement>("[data-control-studio]");
  if (!root) return;
  root.classList.add("manual-workspace-presentation");
  for (const stage of MANUAL_STAGES) {
    const pane = root.querySelector<HTMLElement>(`[data-workflow-pane="${stage}"]`);
    if (!pane) continue;
    pane.classList.add("manual-workspace-pane");
    movePaneGuidanceIntoHelp(pane);
    markGroups(pane);
    compactSuppliedModelControls(pane);
    compactImportedRunControls(pane);
    const qualify = pane.querySelector<HTMLButtonElement>("[data-identify-run]");
    if (qualify && !pane.querySelector(".manual-prerequisite")) {
      qualify.textContent = "Check recording";
      const hint = pane.ownerDocument.createElement("p"); hint.className = "manual-prerequisite";
      hint.textContent = "Record a test, or import a saved recording.";
      qualify.parentElement?.append(hint);
      const update = () => { hint.hidden = qualify.hidden || !qualify.disabled; };
      const Observer = pane.ownerDocument.defaultView?.MutationObserver;
      if (Observer) new Observer(update).observe(qualify, { attributes: true, attributeFilter: ["disabled", "hidden"] });
      update();
    }
    const fit = pane.querySelector<HTMLButtonElement>("[data-identify-fit]"); if (fit) fit.textContent = "Fit model";
    const accept = pane.querySelector<HTMLButtonElement>("[data-identify-accept]"); if (accept) accept.textContent = "Use fitted model";
  }
  const plan = root.querySelector<HTMLElement>("[data-auto-tune-plan]");
  if (plan) {
    plan.classList.add("manual-workspace-pane");
    moveAutomaticTuneGuidanceIntoHelp(plan);
    markGroups(plan);
  }
}

function compactImportedRunControls(pane: HTMLElement): void {
  const section = pane.querySelector<HTMLElement>('[aria-label="Engineer-defined run validation"]');
  if (!section || section.closest("[data-manual-run-validation]")) return;
  const details = pane.ownerDocument.createElement("details");
  details.className = "advanced";
  details.dataset.manualRunValidation = "true";
  const summary = pane.ownerDocument.createElement("summary");
  summary.textContent = "Evaluate imported runs";
  details.append(summary, section);
  pane.append(details);
}

function movePaneGuidanceIntoHelp(pane: HTMLElement): void {
  if (pane.querySelector(":scope > details.manual-workspace-help")) return;
  const heading = pane.querySelector(":scope > h2");
  if (!heading || heading.tagName !== "H2") return;
  const guidance = heading.nextElementSibling;
  heading.remove();
  if (guidance?.tagName === "P") pane.prepend(createHelp(pane.ownerDocument, guidance as HTMLParagraphElement));
}

function moveAutomaticTuneGuidanceIntoHelp(plan: HTMLElement): void {
  if (plan.querySelector("details.manual-workspace-help")) return;
  const heading = plan.querySelector(":scope > .auto-header h2");
  const guidance = heading?.nextElementSibling;
  if (!heading || heading.tagName !== "H2" || !guidance || guidance.tagName !== "P") return;
  const help = createHelp(plan.ownerDocument, guidance as HTMLParagraphElement);
  heading.after(help);
}

function createHelp(owner: Document, guidance: HTMLParagraphElement): HTMLDetailsElement {
  const help = owner.createElement("details");
  help.className = "manual-workspace-help";
  const summary = owner.createElement("summary");
  summary.textContent = "Help";
  help.append(summary, guidance);
  return help;
}

function markGroups(pane: HTMLElement): void {
  pane.querySelectorAll<HTMLElement>(":scope > .workflow-grid, :scope > .workspace-actions, :scope > .summary-panel")
    .forEach(group => group.classList.add("manual-workspace-group"));
}

function compactSuppliedModelControls(pane: HTMLElement): void {
  const model = pane.querySelector<HTMLElement>("[aria-label=\"Supplied model frequency analysis\"]");
  if (!model) return;
  const explanation = model.querySelector(":scope > p:not([data-general-model-status])");
  if (explanation?.tagName === "P" && !model.querySelector(":scope > details.manual-model-help")) {
    const help = createHelp(model.ownerDocument, explanation as HTMLParagraphElement);
    help.classList.add("manual-model-help");
    const heading = model.querySelector(":scope > h3");
    if (heading) heading.after(help); else model.prepend(help);
  }

  const result = model.querySelector<HTMLElement>("[data-general-model-result]");
  if (!result) return;
  const metadata = ["data-general-model-provenance", "data-general-model-poles", "data-general-model-rga"]
    .map(attribute => result.querySelector<HTMLElement>(`[${attribute}]`))
    .filter((node): node is HTMLElement => Boolean(node));
  if (metadata.length && !result.querySelector(":scope > details.manual-model-metadata")) {
    const details = result.ownerDocument.createElement("details");
    details.className = "manual-model-metadata";
    const summary = result.ownerDocument.createElement("summary");
    summary.textContent = "Model details";
    details.append(summary, ...metadata);
    result.prepend(details);
  }

  const tableDetails = Array.from(result.querySelectorAll<HTMLDetailsElement>("details"))
    .find(details => details.querySelector("[data-general-model-table]"));
  if (tableDetails) {
    tableDetails.open = false;
    tableDetails.classList.add("manual-model-channel-response");
  }
}
