import { validateControlPidSettings } from "../pidSettingsValidation";

export interface PidSettingsControlsDependencies {
  readonly owner: Document;
  readonly post: (kind: string, payload: Record<string, unknown>) => void;
  readonly setText: (selector: string, value: string) => void;
  readonly readManual: () => Record<string, unknown>;
  readonly imported: () => Record<string, unknown> | undefined;
}

/** Binds the visible Manual/Imported Generate action and keeps malformed input local to Tune. */
export function initializePidSettingsControls(dependencies: PidSettingsControlsDependencies): void {
  const generate = dependencies.owner.querySelector<HTMLButtonElement>("[data-tune-generate]");
  const method = dependencies.owner.querySelector<HTMLSelectElement>("#tune-method");
  if (!generate || !method) return;
  generate.addEventListener("click", () => {
    const selected = method.value;
    if (selected === "SIMC" || selected === "AMIGO") {
      const objective = selected === "SIMC"
        ? dependencies.owner.querySelector<HTMLSelectElement>("#tune-objective")?.value ?? "balanced" : "balanced";
      dependencies.post("control-studio.tune.run", { method: selected, objective });
      dependencies.setText("[data-tune-state]", "Generating and checking the controller candidate…");
      return;
    }
    if (selected !== "Manual" && selected !== "Imported") return;
    const settings = selected === "Manual" ? dependencies.readManual() : dependencies.imported();
    if (!settings) {
      dependencies.setText("[data-tune-state]", "Select a Control.PID JSON file first.");
      return;
    }
    const validation = validateControlPidSettings(settings);
    if (!validation.ok) {
      dependencies.setText("[data-tune-state]", validation.reason);
      return;
    }
    dependencies.post("control-studio.tune.run", { method: selected, settings: validation.value });
    dependencies.setText("[data-tune-state]", "Generating and checking the controller candidate…");
  });
}
