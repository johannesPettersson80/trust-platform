import type { RunRequirement } from "../generated/OpenResult";
import { openedOffers } from "./openedOffers";

type Scenario = Record<string, any>;
/** The kind of the engine's bounds requirement (`RunRequirement`), typed so that the compiler rejects the name when the engine's type no longer has it. */
const BOUNDS_KIND: RunRequirement["kind"] = "bounds";
/** An initial value as the engineer typed it: its signal and its value as text, one row each, so that a repeated signal reaches the engine. */
type InitialRow = { symbol: string; value: string };
/** A timed change as the engineer typed it: its time and its value as text, which the engine reads (B4.3). */
type EventRow = { atMs: string; symbol: string; value: string };

/** The fields the card shows as rows or as the typed advanced text: the advanced text shows the scenario's other fields. */
const SHOWN_AS_ROWS = ["id", "initialValues", "events", "advanced"];
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const object = (value: unknown): value is Record<string, any> => !!value && typeof value === "object" && !Array.isArray(value);
/** A value as the editor shows it and sends it back: a number as its text, a text as it is, anything else as blank. */
const typed = (value: unknown): string => typeof value === "number" || typeof value === "string" ? String(value) : "";

/**
 * A scenario as the page edits it (B4.3): its initial values and timed changes as typed rows, and the other fields as the engine gave them.
 * The advanced fields keep the text the engineer typed in `advanced`, which the engine reads. The page keeps what the engineer typed and
 * checks none of it: the engine refuses the list under `scenarios` when it changes.
 */
function editable(scenario: Scenario): Scenario {
  const initial: InitialRow[] = Array.isArray(scenario.initialValues)
    ? scenario.initialValues.map((row: Record<string, unknown>) => ({ symbol: typed(row?.symbol), value: typed(row?.value) }))
    : object(scenario.initialValues)
      ? Object.entries(scenario.initialValues).map(([symbol, value]) => ({ symbol, value: typed(value) }))
      : [];
  const events: EventRow[] = Array.isArray(scenario.events)
    ? scenario.events.map((event: Record<string, unknown>) => ({ atMs: typed(event?.atMs), symbol: typed(event?.symbol), value: typed(event?.value) }))
    : [];
  return { ...clone(scenario), initialValues: initial, events };
}

export function initEngineerScenarioEditor(section: HTMLElement): void {
  const document = section.ownerDocument;
  const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]");
  const list = section.querySelector<HTMLElement>("[data-engineer-scenario-list]");
  if (!raw || !list) return;
  let scenarios: Scenario[] = [];

  /** The list goes to the engine as the page holds it: the typed text, with the blank rows, which the engine refuses under its field. */
  const write = () => {
    raw.value = JSON.stringify(scenarios, null, 2);
    raw.dispatchEvent(new document.defaultView!.Event("input", { bubbles: true }));
    raw.dispatchEvent(new document.defaultView!.Event("change", { bubbles: true }));
  };
  /** The scenarios the mount gave in the carrier text, as rows to edit: the engine's list of scenario objects (the loop file's, checked by the engine). */
  const read = (value: string): Scenario[] => (JSON.parse(value) as Scenario[]).map(editable);
  const render = () => {
    list.replaceChildren();
    scenarios.forEach((scenario, index) => renderScenario(scenario, index));
  };
  const update = (index: number, mutate: (scenario: Scenario) => void, redraw = false) => {
    mutate(scenarios[index]);
    write(); if (redraw) render();
  };
  const input = (label: string, value: string, attributes: string, onInput: (value: string) => void) => {
    const wrapper = document.createElement("label"); wrapper.className = "field";
    wrapper.append(document.createTextNode(label));
    const control = document.createElement("input");
    for (const match of attributes.matchAll(/([\w-]+)(?:="([^"]*)")?/g)) control.setAttribute(match[1], match[2] ?? "");
    control.value = value;
    control.addEventListener("input", () => onInput(control.value));
    wrapper.append(control);
    return wrapper;
  };
  const renderScenario = (scenario: Scenario, index: number) => {
    const card = document.createElement("article"); card.className = "summary-panel"; card.dataset.engineerScenario = String(index);
    card.setAttribute("aria-label", scenario.id || `Scenario ${index + 1}`);
    card.append(input("Name", String(scenario.id ?? ""), `type="text" placeholder="Named when saved" data-engineer-scenario-id`, value => {
      update(index, item => { item.id = value; card.setAttribute("aria-label", value.trim() || `Scenario ${index + 1}`); });
    }));
    const initial = document.createElement("details"); initial.innerHTML = "<summary>Initial values</summary>";
    initial.open = scenario.initialValues.length > 0;
    scenario.initialValues.forEach((row: InitialRow, rowIndex: number) => {
      const line = document.createElement("div"); line.className = "workspace-actions";
      line.append(input("Signal", row.symbol, `type="text" data-engineer-scenario-initial-symbol`, value => update(index, item => { item.initialValues[rowIndex].symbol = value; })));
      line.append(input("Value", row.value, `type="text" inputmode="decimal" data-engineer-scenario-initial`, value => update(index, item => { item.initialValues[rowIndex].value = value; })));
      line.append(button("Remove", "secondary", () => update(index, item => { item.initialValues.splice(rowIndex, 1); }, true)));
      initial.append(line);
    });
    // A new row starts at the signal the engine names (B4.3). A blank or repeated signal is the engine's to refuse.
    const addValue = button("Add value", "secondary", () => update(index, item => {
      item.initialValues.push({ symbol: openedOffers().scenarioRowSignal ?? "", value: "0" });
    }, true));
    addValue.dataset.engineerScenarioAddValue = ""; initial.append(addValue);
    card.append(initial);
    const events = document.createElement("details"); events.innerHTML = "<summary>Timed changes</summary>";
    events.open = scenario.events.length > 0;
    scenario.events.forEach((event: EventRow, eventIndex: number) => {
      const line = document.createElement("div"); line.className = "workspace-actions";
      line.append(input("Time (ms)", event.atMs, `type="text" inputmode="decimal" data-engineer-scenario-event-time`, value => update(index, item => { item.events[eventIndex].atMs = value; })));
      line.append(input("Signal", event.symbol, `type="text" data-engineer-scenario-event-symbol`, value => update(index, item => { item.events[eventIndex].symbol = value; })));
      line.append(input("Value", event.value, `type="text" inputmode="decimal" data-engineer-scenario-event-value`, value => update(index, item => { item.events[eventIndex].value = value; })));
      line.append(button("Remove", "secondary", () => update(index, item => { item.events.splice(eventIndex, 1); }, true)));
      events.append(line);
    });
    events.append(button("Add event", "secondary", () => update(index, item => { item.events.push({ atMs: "0", symbol: openedOffers().scenarioRowSignal ?? "", value: "0" }); }, true)));
    card.append(events);
    // The advanced text replaces the scenario's other fields, its bounds among them (B4.3): the bounds rows show until it is typed. The rows lay out
    // the two limits of the scenario's first requirement of the engine's `bounds` kind (`RunRequirement`, whose channel the engine always names): the
    // page keeps the text typed in them, and the engine reads it as a number and refuses what no number reads under `scenarios` (`scenario_form.rs`).
    const advancedTyped = typeof scenario.advanced === "string";
    const bounds = !advancedTyped && Array.isArray(scenario.requirements) ? scenario.requirements.find((requirement: Scenario) => requirement?.kind === BOUNDS_KIND) : undefined;
    if (bounds) {
      const limits = document.createElement("fieldset");
      const legend = document.createElement("legend"); legend.textContent = `Bounds: ${String(bounds.channel)}`; limits.append(legend);
      const minimum = input("Minimum", typed(bounds.minimum), `type="text" inputmode="decimal" data-engineer-scenario-minimum`, value => update(index, () => { bounds.minimum = value; }));
      const maximum = input("Maximum", typed(bounds.maximum), `type="text" inputmode="decimal" data-engineer-scenario-maximum`, value => update(index, () => { bounds.maximum = value; }));
      const fields = document.createElement("div"); fields.className = "engineer-field-grid"; fields.append(minimum, maximum); limits.append(fields);
      card.append(limits);
    }
    const advanced = document.createElement("details"); advanced.innerHTML = "<summary>Advanced fields</summary><p>Additional requirements and plant effects.</p>";
    advanced.open = advancedTyped;
    const advancedRaw = document.createElement("textarea"); advancedRaw.rows = 4; advancedRaw.spellcheck = false; advancedRaw.dataset.engineerScenarioAdvanced = "";
    // The text shows what the engineer typed, else the scenario's other fields as the engine gave them, formatted.
    advancedRaw.value = advancedTyped ? scenario.advanced : JSON.stringify(Object.fromEntries(Object.entries(scenario).filter(([key]) => !SHOWN_AS_ROWS.includes(key))), null, 2);
    // The text goes to the engine as typed: the engine reads it, and refuses one that is not a JSON object when the list changes.
    advancedRaw.addEventListener("change", () => update(index, item => { item.advanced = advancedRaw.value; }, true));
    advanced.append(advancedRaw); card.append(advanced);
    const removeScenario = button("Remove scenario", "secondary", () => { scenarios.splice(index, 1); write(); render(); });
    removeScenario.dataset.engineerScenarioRemove = "";
    card.append(removeScenario); list.append(card);
  };
  const button = (label: string, kind: string, action: () => void) => { const control = document.createElement("button"); control.type = "button"; control.className = `trust-button ${kind}`; control.textContent = label; control.addEventListener("click", action); return control; };
  scenarios = read(raw.value || "[]"); render();
  section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")?.addEventListener("click", () => {
    // The new scenario is the engine's template as it is (B4.3). It has no name: the engine names it when the list changes.
    const template = openedOffers().scenarioTemplate;
    if (!template) return;
    scenarios.push(editable(clone(template)));
    write(); render();
    const name = list.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-id]"); name.item(name.length - 1)?.focus(); name.item(name.length - 1)?.select();
  });
}
