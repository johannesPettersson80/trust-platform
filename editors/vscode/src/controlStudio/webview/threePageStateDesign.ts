import type { StateDesignDraft } from "../generated/InputsResult";
import { openedOffers } from "./openedOffers";

/** One option of a field that is a choice, with the engine's words for it (C4, D-B12). */
export interface StateDesignChoiceView {
  readonly value: string;
  readonly label: string;
}

/**
 * One field of the design form as the engine lists it: the draft's key and the label the engine gives it, and for a field that is a choice its
 * options with their words and the option that stands now (C4, D-B12). The page lists the options as given and invents no default.
 */
export interface StateDesignFieldView {
  readonly field: string;
  readonly label: string;
  readonly choices?: readonly StateDesignChoiceView[] | null;
  readonly selected?: string | null;
}

/** The design form of a state-feedback law. The engine lists the fields; the page keeps the text the engineer typed. */
export interface StateDesignEditor {
  /** Shows the fields the engine lists, in the answer's order; a field that stays listed keeps its control and its text. */
  setFields(fields: readonly StateDesignFieldView[]): void;
  /** The text the engineer typed, for the study: every field the page holds, with the option each choice shows (the engineer's pick, else the one the engine says stands). */
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

const NO_FORM: StateDesignEditor = {
  setFields: () => undefined,
  draft: () => ({}),
  receive: () => undefined,
  showError: () => false,
  showFormError: () => false,
  clearErrors: () => undefined,
};

/** The text the fields show for the loop's declared design, as the engine gives it on open (`stateDeclaredText`, C4). */
function declaredText(): Record<string, string> {
  return { ...(openedOffers().stateDeclaredText ?? {}) };
}

/** The text of a draft, field by field, without the fields it leaves out. */
function textOf(draft: StateDesignDraft): Record<string, string> {
  const text: Record<string, string> = {};
  for (const [field, value] of Object.entries(draft)) if (typeof value === "string") text[field] = value;
  return text;
}

/** The entry of a listed field: the horizon as a whole number, every other field as the text the engineer types. */
function entryOf(doc: Document, field: string): HTMLInputElement | HTMLTextAreaElement {
  if (field !== "horizonSteps") return doc.createElement("textarea");
  const input = doc.createElement("input");
  input.type = "number";
  input.min = "1";
  return input;
}

/** A field that is a choice, as a select listing the engine's options with the engine's words. */
function choiceSelect(doc: Document, choices: readonly StateDesignChoiceView[]): HTMLSelectElement {
  const select = doc.createElement("select");
  select.className = "trust-input";
  select.dataset.stateDesignMode = "true";
  for (const { value, label } of choices) {
    const option = doc.createElement("option");
    option.value = value;
    option.textContent = label;
    select.append(option);
  }
  return select;
}

/**
 * Builds the design form of the Tune page. The loop's own design values, shown until the study holds a draft, are the engine's (`stateDeclaredText`);
 * `changed` runs after each edit, so that the study receives the text as the engineer typed it.
 */
export function initializeThreePageStateDesign(root: HTMLElement, changed: () => void): StateDesignEditor {
  const host = root.querySelector<HTMLElement>("[data-studio-state-design]");
  if (!host) return NO_FORM;
  const doc = root.ownerDocument;
  /** The text as typed, by field: the draft the study receives. */
  let text: Record<string, string> = declaredText();
  /** The option the engine says stands for each field that is a choice, until the engineer picks another. */
  const standing = new Map<string, string>();
  /** The fields that are a choice: their control is a select. */
  const choiceFields = new Set<string>();
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
  const build = (view: StateDesignFieldView): HTMLElement => {
    const { field, label } = view;
    const wrapper = doc.createElement("label");
    wrapper.className = "studio-field";
    wrapper.textContent = label;
    const error = doc.createElement("span");
    error.className = "trust-field__message trust-field__message--error";
    error.hidden = true;
    error.dataset.stateDesignError = field;
    const choices = view.choices ?? undefined;
    const control = choices ? choiceSelect(doc, choices) : entryOf(doc, field);
    if (choices) choiceFields.add(field);
    if (view.selected) standing.set(field, view.selected);
    control.dataset.stateDesignField = field;
    control.value = text[field] ?? standing.get(field) ?? "";
    const edit = (): void => {
      text[field] = control.value;
      clear(field);
      changed();
    };
    control.addEventListener(choices ? "change" : "input", edit);
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
        choiceFields.delete(field);
        standing.delete(field);
      }
      // A field keeps its wrapper and its control, and a wrapper moves only when the answer puts it elsewhere, so the control the
      // engineer changed keeps its focus.
      let previous: Element = head;
      for (const view of fields) {
        const { field, label } = view;
        const existing = wrappers.get(field);
        const wrapper = existing ?? build(view);
        if (existing?.firstChild) existing.firstChild.textContent = label;
        // The option that stands is the engine's until the engineer picks another: an answer that changes it moves a control the engineer left alone.
        if (existing && view.selected) {
          standing.set(field, view.selected);
          const control = controls.get(field);
          if (control && text[field] === undefined) control.value = view.selected;
        }
        if (previous.nextElementSibling !== wrapper) host.insertBefore(wrapper, previous.nextElementSibling);
        previous = wrapper;
      }
    },
    draft() {
      const value: Record<string, string> = { ...text };
      // A choice goes as the option its control shows: the engineer's pick, else the one the engine says stands.
      for (const field of choiceFields) {
        const control = controls.get(field);
        if (control) value[field] = control.value;
      }
      return value;
    },
    receive(draft) {
      text = draft ? textOf(draft) : declaredText();
      for (const [field, control] of controls) {
        control.value = text[field] ?? standing.get(field) ?? "";
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
