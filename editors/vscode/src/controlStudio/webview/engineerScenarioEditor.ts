type Scenario = Record<string, any>;
type EventRow = { atMs: number; symbol: string; value: number };
export interface EngineerScenarioEditorContext {
  processSymbol?: string;
  processMinimum?: number;
  processMaximum?: number;
  setpointSymbol?: string;
  durationSeconds?: number;
  samplePeriodMs?: number;
}

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const finite = (value: string): number | undefined => {
  if (!value.trim()) return undefined;
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
};
const object = (value: unknown): value is Record<string, any> => !!value && typeof value === "object" && !Array.isArray(value);

/** Event times must fall within the validation run; no limit applies until a positive duration is known. */
function eventTimeMaximum(context: EngineerScenarioEditorContext): number | undefined {
  const duration = context.durationSeconds;
  return typeof duration === "number" && Number.isFinite(duration) && duration > 0 ? duration * 1000 : undefined;
}

/** Re-applies the current duration to every scenario event time already on the page. */
export function applyEventTimeLimit(section: HTMLElement, context: EngineerScenarioEditorContext): void {
  const maximum = eventTimeMaximum(context);
  for (const input of Array.from(section.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-event-time]"))) {
    if (maximum === undefined) input.removeAttribute("max"); else input.max = String(maximum);
  }
}

export function initEngineerScenarioEditor(section: HTMLElement, context: EngineerScenarioEditorContext = {}): void {
  const eventTimeLimit = () => { const maximum = eventTimeMaximum(context); return maximum === undefined ? "" : ` max="${maximum}"`; };
  const document = section.ownerDocument;
  const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]");
  const list = section.querySelector<HTMLElement>("[data-engineer-scenario-list]");
  const error = section.querySelector<HTMLElement>("[data-engineer-scenario-error]");
  if (!raw || !list || !error) return;
  let scenarios: Scenario[] = [];
  let syncing = false;

  const showError = (message: string) => { error.textContent = message; error.hidden = !message; };
  const write = () => {
    const persisted = clone(scenarios).map(scenario => {
      if (object(scenario.initialValues)) {
        scenario.initialValues = Object.fromEntries(Object.entries(scenario.initialValues).filter(([symbol]) => symbol.trim()));
      }
      return scenario;
    });
    raw.value = JSON.stringify(persisted, null, 2);
    syncing = true;
    raw.dispatchEvent(new document.defaultView!.Event("input", { bubbles: true }));
    raw.dispatchEvent(new document.defaultView!.Event("change", { bubbles: true }));
    syncing = false;
  };
  const read = (value: string): Scenario[] | undefined => {
    try {
      const parsed: unknown = JSON.parse(value);
      if (!Array.isArray(parsed) || parsed.some(item => !object(item))) throw new Error();
      raw.setCustomValidity(""); return clone(parsed);
    } catch { raw.setCustomValidity("Enter a valid JSON array of scenario objects."); showError("Enter a valid JSON array of scenario objects."); return undefined; }
  };
  const render = () => {
    const add = section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]"); if (add) add.disabled = scenarios.length >= 24;
    list.replaceChildren();
    scenarios.forEach((scenario, index) => renderScenario(scenario, index));
  };
  const update = (index: number, mutate: (scenario: Scenario) => void, redraw = false) => {
    mutate(scenarios[index]);
    write(); if (redraw) render();
  };
  const input = (label: string, value: string, attributes: string, onInput: (value: string) => void | string) => {
    const wrapper = document.createElement("label"); wrapper.className = "field";
    wrapper.append(document.createTextNode(label));
    const control = document.createElement("input");
    for (const match of attributes.matchAll(/([\w-]+)(?:="([^"]*)")?/g)) control.setAttribute(match[1], match[2] ?? "");
    if (control.type === "number" || /scenario-(id|initial-symbol|event-symbol)/.test(attributes)) control.required = true;
    control.value = value;
    control.addEventListener("input", () => {
      if (control.required && !control.value.trim()) { control.setCustomValidity(control.type === "number" ? "Enter a finite number." : "Enter a value."); showError("Complete the highlighted field."); return; }
      if (control.type === "number" && (!control.value.trim() || finite(control.value) === undefined)) {
        control.setCustomValidity("Enter a finite number."); showError("Enter a finite number in the highlighted field.");
      } else {
        control.setCustomValidity("");
        const rejection = onInput(control.value);
        if (typeof rejection === "string" && rejection) {
          control.setCustomValidity(rejection);
          showError(rejection);
        } else if (!section.querySelector("input:invalid,textarea:invalid")) showError("");
      }
    });
    wrapper.append(control);
    return wrapper;
  };
  const renderScenario = (scenario: Scenario, index: number) => {
    const card = document.createElement("article"); card.className = "summary-panel"; card.dataset.engineerScenario = String(index);
    card.setAttribute("aria-label", scenario.id || `Scenario ${index + 1}`);
    card.append(input("Name", String(scenario.id ?? ""), `type="text" data-engineer-scenario-id`, value => {
      const nextId = value.trim();
      if (scenarios.some((candidate, candidateIndex) => candidateIndex !== index && String(candidate.id ?? "").trim() === nextId)) {
        return "Scenario names must be unique.";
      }
      update(index, item => { item.id = nextId; card.setAttribute("aria-label", nextId || `Scenario ${index + 1}`); });
    }));
    const initial = document.createElement("details"); initial.innerHTML = "<summary>Initial values</summary>";
    initial.open = object(scenario.initialValues) && Object.keys(scenario.initialValues).length > 0;
    const values = object(scenario.initialValues) ? scenario.initialValues : {};
    Object.entries(values).forEach(([symbol, value]) => {
      let currentSymbol = symbol;
      let currentValue = typeof value === "number" && Number.isFinite(value) ? value : 0;
      const row = document.createElement("div"); row.className = "workspace-actions";
      row.append(input("Signal", symbol, `type="text" data-engineer-scenario-initial-symbol`, next => {
        const nextSymbol = next.trim();
        const currentValues = scenarios[index]?.initialValues;
        if (nextSymbol && nextSymbol !== currentSymbol && object(currentValues) && Object.prototype.hasOwnProperty.call(currentValues, nextSymbol)) {
          return `Signal ${nextSymbol} already has an initial value.`;
        }
        update(index, item => {
          const nextValues = { ...(item.initialValues ?? {}) };
          delete nextValues[currentSymbol];
          if (nextSymbol) nextValues[nextSymbol] = currentValue;
          currentSymbol = nextSymbol;
          item.initialValues = nextValues;
        });
      }));
      row.append(input("Value", String(currentValue), `type="number" step="any" data-engineer-scenario-initial`, next => update(index, item => {
        const nextValue = finite(next); if (nextValue === undefined) return;
        currentValue = nextValue;
        if (currentSymbol) item.initialValues = { ...(object(item.initialValues) ? item.initialValues : {}), [currentSymbol]: nextValue };
      })), button("Remove", "secondary", () => update(index, item => { const next = { ...(item.initialValues ?? {}) }; delete next[currentSymbol]; item.initialValues = next; }, true)));
      initial.append(row);
    });
    const addValue = button("Add value", "secondary", () => update(index, item => {
      const values = { ...(object(item.initialValues) ? item.initialValues : {}) };
      const preferred = context.setpointSymbol?.trim() ?? "";
      const symbol = preferred && !Object.prototype.hasOwnProperty.call(values, preferred) ? preferred : "";
      if (!Object.prototype.hasOwnProperty.call(values, symbol)) values[symbol] = 0;
      item.initialValues = values;
    }, true));
    addValue.dataset.engineerScenarioAddValue = ""; initial.append(addValue);
    card.append(initial);
    const events = document.createElement("details"); events.innerHTML = "<summary>Timed changes</summary>";
    events.open = Array.isArray(scenario.events) && scenario.events.length > 0;
    (Array.isArray(scenario.events) ? scenario.events : []).forEach((event: EventRow, eventIndex: number) => {
      const row = document.createElement("div"); row.className = "workspace-actions";
      let currentAtMs = typeof event.atMs === "number" && Number.isFinite(event.atMs) ? event.atMs : 0;
      row.append(input("Time (ms)", String(currentAtMs), `type="number" min="0" step="${context.samplePeriodMs ?? 1}"${eventTimeLimit()} data-engineer-scenario-event-time`, value => update(index, item => { const next = finite(value); if (next !== undefined) { currentAtMs = next; item.events[eventIndex].atMs = next; } })));
      row.append(input("Signal", String(event.symbol ?? ""), `type="text" data-engineer-scenario-event-symbol`, value => update(index, item => { item.events[eventIndex].symbol = value; })));
      let currentEventValue = typeof event.value === "number" && Number.isFinite(event.value) ? event.value : 0;
      row.append(input("Value", String(currentEventValue), `type="number" step="any" data-engineer-scenario-event-value`, value => update(index, item => { const next = finite(value); if (next !== undefined) { currentEventValue = next; item.events[eventIndex].value = next; } })));
      row.append(button("Remove", "secondary", () => update(index, item => { item.events.splice(eventIndex, 1); }, true)));
      events.append(row);
    });
    events.append(button("Add event", "secondary", () => update(index, item => { item.events = [...(Array.isArray(item.events) ? item.events : []), { atMs: 0, symbol: context.setpointSymbol ?? "", value: 0 }]; }, true))); card.append(events);
    const bounds = Array.isArray(scenario.requirements) ? scenario.requirements.find((requirement: Scenario) => requirement?.kind === "bounds") : undefined;
    if (bounds) {
      const limits = document.createElement("fieldset");
      const legend = document.createElement("legend"); legend.textContent = `Bounds: ${String(bounds.channel ?? "process")}`; limits.append(legend);
      const minimum = input("Minimum", String(bounds.minimum), `type="number" step="any" data-engineer-scenario-minimum`, value => update(index, item => {
        const next = finite(value); if (next !== undefined) { bounds.minimum = next; maximum.querySelector("input")!.min = String(next); }
      }));
      const maximum = input("Maximum", String(bounds.maximum), `type="number" step="any" data-engineer-scenario-maximum`, value => update(index, item => {
        const next = finite(value); if (next !== undefined) { bounds.maximum = next; minimum.querySelector("input")!.max = String(next); }
      }));
      minimum.querySelector("input")!.max = String(bounds.maximum); maximum.querySelector("input")!.min = String(bounds.minimum);
      const fields = document.createElement("div"); fields.className = "engineer-field-grid"; fields.append(minimum, maximum); limits.append(fields);
      card.append(limits);
    }
    const advanced = document.createElement("details"); advanced.innerHTML = "<summary>Advanced fields</summary><p>Additional requirements and plant effects.</p>";
    const advancedRaw = document.createElement("textarea"); advancedRaw.rows = 4; advancedRaw.spellcheck = false; advancedRaw.dataset.engineerScenarioAdvanced = "";
    const advancedObject = Object.fromEntries(Object.entries(scenario).filter(([key]) => !["id", "initialValues", "events"].includes(key)));
    advancedRaw.value = JSON.stringify(advancedObject, null, 2);
    advancedRaw.addEventListener("change", () => { try { const parsed = JSON.parse(advancedRaw.value); if (!object(parsed)) throw new Error(); advancedRaw.setCustomValidity(""); update(index, item => { for (const key of Object.keys(item)) if (!["id", "initialValues", "events"].includes(key)) delete item[key]; Object.assign(item, parsed); }, true); showError(""); } catch { advancedRaw.setCustomValidity("Advanced fields must be a valid JSON object."); showError("Advanced fields must be a valid JSON object."); } });
    advanced.append(advancedRaw); card.append(advanced);
    const removeScenario = button("Remove scenario", "secondary", () => { scenarios.splice(index, 1); write(); render(); });
    removeScenario.dataset.engineerScenarioRemove = "";
    card.append(removeScenario); list.append(card);
  };
  const button = (label: string, kind: string, action: () => void) => { const control = document.createElement("button"); control.type = "button"; control.className = `trust-button ${kind}`; control.textContent = label; control.addEventListener("click", action); return control; };
  const parsed = read(raw.value || "[]"); if (parsed) { scenarios = parsed; showError(""); render(); }
  section.querySelector<HTMLInputElement>("[data-engineer-duration]")?.addEventListener("input", () => applyEventTimeLimit(section, context));
  raw.addEventListener("input", () => { if (syncing) return; const parsed = read(raw.value); if (parsed) { scenarios = parsed; showError(""); render(); } });
  section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")?.addEventListener("click", () => {
    const durationSeconds = Number.isFinite(context.durationSeconds) ? context.durationSeconds! : 1;
    const minimum = Number.isFinite(context.processMinimum) ? context.processMinimum! : 0;
    const maximum = Number.isFinite(context.processMaximum) ? context.processMaximum! : 0;
    let nextId = 1; while (scenarios.some(scenario => scenario.id === `scenario-${nextId}`)) nextId++;
    scenarios.push({ id: `scenario-${nextId}`, initialValues: {}, events: [], requirements: [{ id: "requirement-1", kind: "bounds", channel: context.processSymbol ?? "", window: { start: 0, end: durationSeconds }, minimum, maximum }] });
    write(); render();
    const name = list.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-id]"); name.item(name.length - 1)?.focus(); name.item(name.length - 1)?.select();
  });
  document.defaultView?.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown> | undefined;
    if (!Array.isArray(payload?.scenarioDefinitions) || !["restored", "model-ready"].includes(String(payload.status))) return;
    if (JSON.stringify(payload.scenarioDefinitions) === JSON.stringify(scenarios)) return;
    const next = read(JSON.stringify(payload.scenarioDefinitions));
    if (next) { scenarios = next; showError(""); raw.value = JSON.stringify(scenarios, null, 2); render(); }
  });
}

export const engineerScenarioEditor = initEngineerScenarioEditor;
