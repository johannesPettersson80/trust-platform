interface AutomaticTuneControlsDependencies {
  post(kind: string, payload?: Record<string, unknown>): void;
  showError(): void;
}

/** Owns the small plan/review controls for automatic controller tuning. */
export function initializeAutomaticTuneControls(dependencies: AutomaticTuneControlsDependencies): void {
  const root = document.querySelector<HTMLElement>("[data-control-studio]");
  const launcher = document.querySelector<HTMLButtonElement>("[data-control-studio-automatic-tune]");
  const plan = document.querySelector<HTMLElement>("[data-auto-tune-plan]");
  const placeholder = plan ? document.createComment("automatic-tune-plan-placeholder") : undefined;
  if (plan && placeholder) plan.parentNode?.insertBefore(placeholder, plan);
  const closePlan = (returnFocus = true): void => {
    if (!plan) return;
    plan.hidden = true;
    if (placeholder?.parentNode && plan.parentNode !== placeholder.parentNode) {
      placeholder.parentNode.insertBefore(plan, placeholder.nextSibling);
    }
    if (returnFocus) launcher?.focus();
  };
  const openPlan = (): void => {
    if (!plan) return;
    if (root && plan.parentNode !== root) root.append(plan);
    plan.hidden = false;
    plan.querySelector<HTMLButtonElement>("[data-auto-tune-close]")?.focus();
  };
  document.querySelector("[data-control-studio-configure-plant]")?.addEventListener("click", () =>
    dependencies.post("control-studio.navigate-plant"));
  launcher?.addEventListener("click", openPlan);
  document.querySelector("[data-auto-tune-close]")?.addEventListener("click", () => closePlan());
  document.addEventListener("keydown", event => {
    if (event.key === "Escape" && plan && !plan.hidden) {
      event.preventDefault();
      closePlan();
    }
  });
  for (const button of Array.from(document.querySelectorAll("[data-auto-tune-apply]"))) {
    button.addEventListener("click", () => {
      closePlan(false);
    });
  }
  document.querySelector("[data-auto-tune-approve]")?.addEventListener("click", (event) => {
    const button = event.currentTarget as HTMLButtonElement;
    const objective = document.querySelector<HTMLSelectElement>("[data-auto-tune-objective]")?.value ?? "balanced";
    button.disabled = true; button.textContent = "Running…";
    const previous = document.querySelector<HTMLElement>("[data-auto-tune-result]");
    if (previous) previous.hidden = true;
    dependencies.showError();
    dependencies.post("control-studio.automatic.approve", { objective });
  });
}
