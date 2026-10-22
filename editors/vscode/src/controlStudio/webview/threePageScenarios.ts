import { initEngineerScenarioEditor } from "./engineerScenarioEditor";

export interface ThreePageScenarioOptions {
  scenarioDefinitions: readonly Record<string, unknown>[];
}

export interface ThreePageScenarioEditor {
  /** The scenario list as the editor holds it, which a commit sends to the engine. */
  read(): readonly Record<string, unknown>[];
  /** Shows the engine's refusal of the scenario list, which it names under the `scenarios` field (B4.3). */
  refuse(sentence: string): void;
}

/** Mounts the existing engineer scenario editor inside the Requirements dialog. */
export function mountThreePageScenarios(section: HTMLElement, options: ThreePageScenarioOptions): ThreePageScenarioEditor {
  const document = section.ownerDocument;
  const container = document.createElement("details");
  container.dataset.studioScenarios = "true";
  const heading = document.createElement("summary"); heading.textContent = "Scenarios";
  const raw = document.createElement("textarea"); raw.dataset.engineerScenarios = ""; raw.hidden = true; raw.value = JSON.stringify(options.scenarioDefinitions, null, 2);
  const list = document.createElement("div"); list.dataset.engineerScenarioList = "";
  const error = document.createElement("p"); error.dataset.engineerScenarioError = ""; error.hidden = true;
  const add = document.createElement("button"); add.type = "button"; add.className = "trust-button"; add.textContent = "Add scenario"; add.dataset.engineerScenarioAdd = "";
  container.append(heading, raw, error, list, add);
  const form = section.querySelector<HTMLFormElement>("form");
  const actions = form?.querySelector<HTMLElement>(".trust-compact-editor-actions");
  // The scenarios belong to the Requirements test design, so they sit inside its More… section when the dialog has one.
  const more = section.querySelector<HTMLElement>(".trust-compact-editor-more-body");
  if (more) more.append(container);
  else if (actions) form!.insertBefore(container, actions);
  else section.append(container);
  // The engine refuses a check that ends after the validation duration and an event off the task's samples when the list changes (B4.3).
  initEngineerScenarioEditor(container);
  // Only the editor writes the carrier text, always as the JSON of the list it holds.
  const read = (): readonly Record<string, unknown>[] => JSON.parse(raw.value) as Record<string, unknown>[];
  return {
    read,
    refuse: sentence => {
      error.textContent = sentence; error.hidden = false;
      // The sentence sits inside the Scenarios section and the dialog's collapsed More… section: they open so that the engineer sees it.
      for (let details = error.closest("details"); details; details = details.parentElement?.closest("details") ?? null) details.open = true;
    },
  };
}
