/// <reference lib="dom" />

import { formatDurationSeconds } from "../controlStudio/webview/durationField";

export interface CompactNumberField {
  key: string;
  label: string;
  group?: string;
  signal?: string;
  /** The text shown in the Signal column, for example a role name; the symbol stays the hover text. */
  signalLabel?: string;
  /**
   * A number, or the text of a value: the engine's words for a duration, or what the engineer typed. A text is shown as it is, and a number
   * of a duration in its largest unit.
   */
  value?: number | string;
  /** A blank optional field is left out of the committed values; its placeholder names what applies instead. */
  optional?: boolean;
  placeholder?: string;
  /** A duration: typed as "90 s", "15 min", "1 h 30 min" or plain seconds. The engine reads the text and checks its range. */
  duration?: boolean;
  /** Shown only inside the editor's collapsed More… section. */
  more?: boolean;
}

export interface CompactEditorIssue {
  field: string;
  message: string;
}

/**
 * A small labelled editor using shared truST controls and native dialog focus handling. Done commits what was typed: a number for an entry
 * that reads as one, and the text for any other, which the engine reads and refuses under its field (`initialIssue` shows that refusal).
 */
export function openCompactNumberEditor(options: {
  title: string;
  fields: readonly CompactNumberField[];
  fieldAttribute: string;
  rowLayout?: "requirements";
  /** Requirements only: false hides the Signal column for a loop with one output. */
  showSignals?: boolean;
  moreLabel?: string;
  /** The engine's refusal of the last values, shown under the field it names. */
  initialIssue?: CompactEditorIssue;
  cancelled?: () => void;
  commit: (values: Readonly<Record<string, number | string>>) => void;
}): HTMLDialogElement {
  document.querySelector("[data-trust-compact-editor]")?.remove();
  const opener = document.activeElement;
  let committed = false;
  const dialog = document.createElement("dialog");
  dialog.className = "trust-compact-editor";
  dialog.dataset.trustCompactEditor = "true";
  const heading = document.createElement("h2");
  heading.id = `compact-editor-${crypto.randomUUID()}`;
  heading.textContent = options.title;
  dialog.setAttribute("aria-labelledby", heading.id);
  const form = document.createElement("form");
  form.noValidate = true;
  const showSignals = options.showSignals !== false;
  const inputs = new Map<string, HTMLInputElement>();
  const notices = new Map<string, HTMLElement>();
  const groups = new Map<string, HTMLElement>();
  /** A duration given as a number keeps it while its text is unchanged, so the rounding of the display never edits a stored value. */
  const originals = new Map<string, { text: string; seconds: number }>();
  if (options.rowLayout === "requirements") {
    form.classList.add("trust-compact-editor-requirements");
    if (!showSignals) form.classList.add("trust-compact-editor-no-signal");
    const headings = document.createElement("div"); headings.className = "trust-compact-editor-columns";
    for (const title of showSignals ? ["Requirement", "Signal", "Limit"] : ["Requirement", "Limit"]) {
      const cell = document.createElement("span"); cell.textContent = title; headings.append(cell);
    }
    form.append(headings);
  }
  // Fields marked more collect here; the collapsed section follows the visible rows once they are all built.
  const moreBody = document.createElement("div"); moreBody.className = "trust-compact-editor-more-body";
  for (const field of options.fields) {
    let parent: HTMLElement = field.more ? moreBody : form;
    if (field.group) {
      let group = groups.get(field.group);
      if (!group) {
        const fieldset = document.createElement("fieldset");
        const legend = document.createElement("legend"); legend.textContent = field.group;
        group = document.createElement("div"); group.className = "trust-compact-editor-grid";
        fieldset.append(legend, group); parent.append(fieldset); groups.set(field.group, group);
      }
      parent = group;
    }
    const row = document.createElement("div"); row.className = "trust-field";
    const label = document.createElement("label"); label.textContent = field.label;
    const input = document.createElement("input"); input.className = "trust-input";
    input.required = !field.optional;
    if (field.duration) {
      input.type = "text";
      input.placeholder = field.placeholder ?? "90 s, 15 min or 1 h 30 min";
      if (typeof field.value === "number") { input.value = formatDurationSeconds(field.value); originals.set(field.key, { text: input.value, seconds: field.value }); }
      // A text is shown as it is: the engine's words, or what the engineer typed, which goes back as typed.
      else if (field.value !== undefined) input.value = String(field.value);
    } else {
      input.type = "number"; input.step = "any";
      if (field.placeholder) input.placeholder = field.placeholder;
      if (field.value !== undefined) input.value = String(field.value);
    }
    input.id = `${heading.id}-${field.key}`; label.htmlFor = input.id;
    input.setAttribute(options.fieldAttribute, field.key);
    if (field.signal) input.dataset.studioSignal = field.signal;
    const notice = document.createElement("span"); notice.className = "trust-field__message trust-field__message--error";
    notice.id = `${input.id}-error`; notice.setAttribute("role", "alert"); notice.hidden = true;
    input.setAttribute("aria-describedby", notice.id);
    row.append(label);
    if (options.rowLayout === "requirements" && showSignals) {
      const signal = document.createElement("span"); signal.textContent = field.signalLabel ?? field.signal ?? "—";
      if (field.signal) signal.title = field.signal;
      row.append(signal);
    }
    row.append(input, notice); parent.append(row); inputs.set(field.key, input); notices.set(field.key, notice);
  }
  if (moreBody.childElementCount) {
    const more = document.createElement("details");
    more.className = "trust-compact-editor-more"; more.dataset.trustCompactMore = "true";
    const summary = document.createElement("summary"); summary.textContent = options.moreLabel ?? "More…";
    more.append(summary, moreBody); form.append(more);
  }
  const actions = document.createElement("div"); actions.className = "trust-compact-editor-actions";
  const cancel = document.createElement("button"); cancel.type = "button"; cancel.className = "trust-button";
  cancel.textContent = "Cancel"; cancel.dataset.studioEditorCancel = "true";
  const done = document.createElement("button"); done.type = "submit"; done.className = "trust-button trust-button--primary";
  done.textContent = "Done"; done.dataset.studioEditorDone = "true";
  actions.append(cancel, done); form.append(actions); dialog.append(heading, form); document.body.append(dialog);
  const mark = (field: string, message: string): void => {
    const input = inputs.get(field), notice = notices.get(field);
    if (!input || !notice) return;
    notice.textContent = message; notice.hidden = false; input.setAttribute("aria-invalid", "true");
    input.classList.add("trust-input--error");
    const section = input.closest("details"); if (section) section.open = true;
  };
  const clearMarks = (): void => {
    for (const [key, input] of inputs) {
      input.removeAttribute("aria-invalid"); input.classList.remove("trust-input--error"); notices.get(key)!.hidden = true;
    }
  };
  form.addEventListener("submit", event => {
    event.preventDefault();
    clearMarks();
    const values: Record<string, number | string> = {};
    for (const field of options.fields) {
      const input = inputs.get(field.key)!;
      const text = input.value.trim();
      if (field.optional && !text) continue;
      if (field.duration) {
        // The typed text goes to the engine, which reads it and refuses one it cannot read, a blank one included, under this field.
        const original = originals.get(field.key);
        values[field.key] = original && text === original.text ? original.seconds : text;
        continue;
      }
      // A number input holds no text it cannot read, so an entry that is no number is blank: the engine refuses the empty text under its field.
      values[field.key] = text && Number.isFinite(input.valueAsNumber) ? input.valueAsNumber : text;
    }
    options.commit(values); committed = true; dialog.close();
  });
  cancel.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => { if (!committed) options.cancelled?.(); dialog.remove(); if (opener instanceof HTMLElement && opener.isConnected) opener.focus(); });
  dialog.showModal();
  if (options.initialIssue) {
    mark(options.initialIssue.field, options.initialIssue.message);
    inputs.get(options.initialIssue.field)?.focus();
  }
  return dialog;
}
