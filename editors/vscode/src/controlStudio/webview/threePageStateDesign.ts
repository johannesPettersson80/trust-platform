import type { StateDesignDraft, StateDesignPreferences } from "../stateDesignPreferences";

/** One field of the design form as the engine lists it: the draft's key and the label the engine gives it (C4, D-B12). */
export interface StateDesignFieldView {
  readonly field: string;
  readonly label: string;
}

/** The design form of a state-feedback law. The engine lists the fields; the page keeps the text the engineer typed. */
export interface StateDesignEditor {
  /** Shows the fields the engine lists, in the answer's order; a field that stays listed keeps its control and its text. */
  setFields(fields: readonly StateDesignFieldView[]): void;
  /** The text the engineer typed, for the study: every field the page holds, with the solver's choice when the form lists the solver. */
  draft(): StateDesignDraft;
  /** Shows a draft the study holds, or the loop's declared values when there is none. */
  receive(draft?: StateDesignDraft): void;
  /** Shows the engine's sentence under a listed field and marks it invalid; false when the form does not list the field. */
  showError(field: string, sentence: string): boolean;
  /** Shows the engine's sentence of a refusal of the whole form at the form's head; false when there is no form. */
  showFormError(sentence: string): boolean;
  /** Removes every field message, the form's head message and the invalid marks. */
  clearErrors(): void;
}

const SOLVER_OPTIONS: ReadonlyArray<readonly [string, string]> = [
  ["infinite", "Infinite-horizon / steady-state"],
  ["finite", "Finite-horizon (fixed policy)"],
];

const NO_FORM: StateDesignEditor = {
  setFields: () => undefined,
  draft: () => ({}),
  receive: () => undefined,
  showError: () => false,
  showFormError: () => false,
  clearErrors: () => undefined,
};

/** The loop's declared design values as the text the fields show: each matrix or pole list as JSON. */
function declaredText(value?: StateDesignPreferences): Record<string, string> {
  const text: Record<string, string> = {};
  if (value?.finiteHorizon) {
    text.solverMode = "finite";
    text.horizonSteps = String(value.finiteHorizon.steps);
    text.terminalWeights = JSON.stringify(value.finiteHorizon.terminalWeights);
  }
  if (value?.desiredPoles) text.desiredPoles = JSON.stringify(value.desiredPoles);
  if (value?.stateWeights) text.stateWeights = JSON.stringify(value.stateWeights);
  if (value?.inputWeights) text.inputWeights = JSON.stringify(value.inputWeights);
  if (value?.processNoise) text.processNoise = JSON.stringify(value.processNoise);
  if (value?.measurementNoise) text.measurementNoise = JSON.stringify(value.measurementNoise);
  return text;
}

/** The text of a draft, field by field, without the fields it leaves out. */
function textOf(draft: StateDesignDraft): Record<string, string> {
  const text: Record<string, string> = {};
  for (const [field, value] of Object.entries(draft)) if (typeof value === "string") text[field] = value;
  return text;
}

/** The solver's choice: a finite horizon only when the text says so. */
function solverOf(text: Record<string, string>): string {
  return text.solverMode === "finite" ? "finite" : "infinite";
}

/** The entry of a listed field: the horizon as a whole number, every other field as the text the engineer types. */
function entryOf(doc: Document, field: string): HTMLInputElement | HTMLTextAreaElement {
  if (field !== "horizonSteps") return doc.createElement("textarea");
  const input = doc.createElement("input");
  input.type = "number";
  input.min = "1";
  return input;
}

/** The solver as the engine's two choices. */
function solverSelect(doc: Document): HTMLSelectElement {
  const select = doc.createElement("select");
  select.className = "trust-input";
  select.dataset.stateDesignMode = "true";
  for (const [value, words] of SOLVER_OPTIONS) {
    const option = doc.createElement("option");
    option.value = value;
    option.textContent = words;
    select.append(option);
  }
  return select;
}

/**
 * Builds the design form of the Tune page. `declared` holds the loop's own design values, shown until the study holds a draft;
 * `changed` runs after each edit, so that the study receives the text as the engineer typed it.
 */
export function initializeThreePageStateDesign(root: HTMLElement, declared: StateDesignPreferences | undefined,
  changed: () => void): StateDesignEditor {
  const host = root.querySelector<HTMLElement>("[data-studio-state-design]");
  if (!host) return NO_FORM;
  const doc = root.ownerDocument;
  /** The text as typed, by field: the draft the study receives. */
  let text: Record<string, string> = declaredText(declared);
  /** The control, its message and its wrapper of each field the form shows. */
  const controls = new Map<string, HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>();
  const messages = new Map<string, HTMLElement>();
  const wrappers = new Map<string, HTMLElement>();
  /** The form's head, above its fields: where the engine's refusal of the whole form is shown (C4, D-B12). */
  const head = doc.createElement("p");
  head.className = "trust-field__message trust-field__message--error";
  head.hidden = true;
  head.dataset.stateDesignFormError = "";
  host.prepend(head);

  const clear = (field: string): void => {
    controls.get(field)?.removeAttribute("aria-invalid");
    const message = messages.get(field);
    if (message) {
      message.textContent = "";
      message.hidden = true;
    }
  };

  /** Creates a field's wrapper, which setFields places in the answer's order. */
  const build = (field: string, label: string): HTMLElement => {
    const wrapper = doc.createElement("label");
    wrapper.className = "studio-field";
    wrapper.textContent = label;
    const error = doc.createElement("span");
    error.className = "trust-field__message trust-field__message--error";
    error.hidden = true;
    error.dataset.stateDesignError = field;
    const solver = field === "solverMode";
    const control = solver ? solverSelect(doc) : entryOf(doc, field);
    control.dataset.stateDesignField = field;
    control.value = solver ? solverOf(text) : text[field] ?? "";
    const edit = (): void => {
      text[field] = control.value;
      clear(field);
      changed();
    };
    control.addEventListener(solver ? "change" : "input", edit);
    wrapper.append(control, error);
    controls.set(field, control);
    messages.set(field, error);
    wrappers.set(field, wrapper);
    return wrapper;
  };

  return {
    setFields(fields) {
      const listed = new Set(fields.map(item => item.field));
      for (const [field, wrapper] of wrappers) {
        if (listed.has(field)) continue;
        wrapper.remove();
        wrappers.delete(field);
        controls.delete(field);
        messages.delete(field);
      }
      // A field keeps its wrapper and its control, and a wrapper moves only when the answer puts it elsewhere, so the control the
      // engineer changed keeps its focus.
      let previous: Element = head;
      for (const { field, label } of fields) {
        const existing = wrappers.get(field);
        const wrapper = existing ?? build(field, label);
        if (existing?.firstChild) existing.firstChild.textContent = label;
        if (previous.nextElementSibling !== wrapper) host.insertBefore(wrapper, previous.nextElementSibling);
        previous = wrapper;
      }
    },
    draft() {
      const value: Record<string, string> = { ...text };
      if (controls.has("solverMode")) value.solverMode = solverOf(text);
      return value;
    },
    receive(draft) {
      text = draft ? textOf(draft) : declaredText(declared);
      for (const [field, control] of controls) {
        control.value = field === "solverMode" ? solverOf(text) : text[field] ?? "";
        clear(field);
      }
    },
    showError(field, sentence) {
      const control = controls.get(field);
      const message = messages.get(field);
      if (!control || !message) return false;
      control.setAttribute("aria-invalid", "true");
      message.textContent = sentence;
      message.hidden = false;
      return true;
    },
    showFormError(sentence) {
      head.textContent = sentence;
      head.hidden = false;
      return true;
    },
    clearErrors() {
      head.textContent = "";
      head.hidden = true;
      for (const field of controls.keys()) clear(field);
    },
  };
}
