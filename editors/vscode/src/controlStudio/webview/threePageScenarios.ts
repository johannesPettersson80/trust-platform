import { applyEventTimeLimit, initEngineerScenarioEditor, type EngineerScenarioEditorContext } from "./engineerScenarioEditor";
import { latestScenarioCheck, validEngineerScenarios, type EngineerScenario } from "../scenarioContract";

export interface ThreePageScenarioOptions {
  scenarioDefinitions: readonly Record<string, unknown>[];
  processSymbol: string;
  setpointSymbol?: string;
  samplePeriodMs: number;
  durationSeconds: number;
  processMinimum?: number;
  processMaximum?: number;
}

export interface ThreePageScenarioEditor {
  read(): readonly Record<string, unknown>[] | undefined;
  validate(): string | undefined;
}

/** Mounts the existing engineer scenario editor inside the Requirements dialog. */
export function mountThreePageScenarios(section: HTMLElement, options: ThreePageScenarioOptions): ThreePageScenarioEditor {
  const document = section.ownerDocument;
  const container = document.createElement("details");
  container.dataset.studioScenarios = "true";
  const heading = document.createElement("summary"); heading.textContent = "Scenarios";
  const raw = document.createElement("textarea"); raw.dataset.engineerScenarios = ""; raw.hidden = true; raw.value = JSON.stringify(options.scenarioDefinitions, null, 2);
  const list = document.createElement("div"); list.dataset.engineerScenarioList = "";
  const error = document.createElement("p"); error.dataset.engineerScenarioError = ""; error.dataset.compactScenarioError = ""; error.hidden = true;
  const add = document.createElement("button"); add.type = "button"; add.className = "trust-button"; add.textContent = "Add scenario"; add.dataset.engineerScenarioAdd = "";
  container.append(heading, raw, error, list, add);
  const form = section.querySelector<HTMLFormElement>("form");
  const actions = form?.querySelector<HTMLElement>(".trust-compact-editor-actions");
  (actions ? form!.insertBefore(container, actions) : section.append(container));
  // The validation duration can change in the same dialog, so event-time limits follow its field.
  const durationField = section.querySelector<HTMLInputElement>("[data-studio-requirement='durationSeconds']");
  const context: EngineerScenarioEditorContext = {
    processSymbol: options.processSymbol, setpointSymbol: options.setpointSymbol,
    samplePeriodMs: options.samplePeriodMs, processMinimum: options.processMinimum, processMaximum: options.processMaximum,
    get durationSeconds() {
      const entered = durationField ? Number(durationField.value) : Number.NaN;
      return durationField && durationField.value.trim() !== "" && Number.isFinite(entered) ? entered : options.durationSeconds;
    },
  };
  initEngineerScenarioEditor(container, context);
  durationField?.addEventListener("input", () => applyEventTimeLimit(container, context));
  return {
    read: () => { try { const value: unknown = JSON.parse(raw.value); return Array.isArray(value) ? value as Record<string, unknown>[] : undefined; } catch { return undefined; } },
    validate: () => {
      const invalid = container.querySelector<HTMLInputElement | HTMLTextAreaElement>("input:invalid,textarea:invalid");
      if (invalid) { for (let details = invalid.closest("details"); details; details = details.parentElement?.closest("details") ?? null) details.open = true; error.textContent = invalid.validationMessage || "Complete the highlighted scenario field."; error.hidden = false; invalid.focus(); return error.textContent; }
      const parsed = (() => { try { return JSON.parse(raw.value) as unknown; } catch { return undefined; } })();
      if (!validEngineerScenarios(parsed)) {
        error.textContent = "Enter valid scenario definitions. Native validation will check requirements and effects before Tune."; error.hidden = false; return error.textContent;
      }
      const latest = latestScenarioCheck(parsed as EngineerScenario[]), duration = context.durationSeconds ?? 0;
      if (latest && duration > 0 && latest.endSeconds > duration + 1e-9) {
        error.textContent = `${latest.scenario} checks ${latest.requirement} until ${latest.endSeconds} s. Allow a validation duration of at least ${latest.endSeconds} s.`;
        error.hidden = false; return error.textContent;
      }
      error.textContent = ""; error.hidden = true; return undefined;
    },
  };
}
