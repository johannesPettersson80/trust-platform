/// <reference lib="dom" />

export interface CompactNumberField {
  key: string;
  label: string;
  group?: string;
  signal?: string;
  value?: number;
  minimum?: number;
  maximum?: number;
  unit?: string;
  /** A blank optional field is left out of the committed values; its placeholder names what applies instead. */
  optional?: boolean;
  placeholder?: string;
}

/** A small labelled editor using shared truST controls and native dialog focus handling. */
export function openCompactNumberEditor(options: {
  title: string;
  fields: readonly CompactNumberField[];
  fieldAttribute: string;
  rowLayout?: "requirements";
  initialIssue?: { field: string; message: string };
  cancelled?: () => void;
  validate?: (values: Readonly<Record<string, number>>) => { field: string; message: string } | undefined;
  beforeCommit?: () => string | undefined;
  commit: (values: Readonly<Record<string, number>>) => void;
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
  const inputs = new Map<string, HTMLInputElement>();
  const notices = new Map<string, HTMLElement>();
  const groups = new Map<string, HTMLElement>();
  if (options.rowLayout === "requirements") {
    form.classList.add("trust-compact-editor-requirements");
    const headings = document.createElement("div"); headings.className = "trust-compact-editor-columns";
    for (const title of ["Requirement", "Signal", "Limit"]) { const cell = document.createElement("span"); cell.textContent = title; headings.append(cell); }
    form.append(headings);
  }
  for (const field of options.fields) {
    let parent: HTMLElement = form;
    if (field.group) {
      let group = groups.get(field.group);
      if (!group) {
        const fieldset = document.createElement("fieldset");
        const legend = document.createElement("legend"); legend.textContent = field.group;
        group = document.createElement("div"); group.className = "trust-compact-editor-grid";
        fieldset.append(legend, group); form.append(fieldset); groups.set(field.group, group);
      }
      parent = group;
    }
    const row = document.createElement("div"); row.className = "trust-field";
    const label = document.createElement("label"); label.textContent = field.label;
    const input = document.createElement("input"); input.className = "trust-input";
    input.type = "number"; input.step = "any"; input.required = !field.optional;
    if (field.placeholder) input.placeholder = field.placeholder;
    input.id = `${heading.id}-${field.key}`; label.htmlFor = input.id;
    input.setAttribute(options.fieldAttribute, field.key);
    if (field.signal) input.dataset.studioSignal = field.signal;
    if (field.value !== undefined) input.value = String(field.value);
    if (field.minimum !== undefined) input.min = String(field.minimum);
    if (field.maximum !== undefined) input.max = String(field.maximum);
    const notice = document.createElement("span"); notice.className = "trust-field__message trust-field__message--error";
    notice.id = `${input.id}-error`; notice.setAttribute("role", "alert"); notice.hidden = true;
    input.setAttribute("aria-describedby", notice.id);
    row.append(label);
    if (options.rowLayout === "requirements") { const signal = document.createElement("span"); signal.textContent = field.signal ?? "—"; row.append(signal); }
    row.append(input, notice); parent.append(row); inputs.set(field.key, input); notices.set(field.key, notice);
  }
  const actions = document.createElement("div"); actions.className = "trust-compact-editor-actions";
  const cancel = document.createElement("button"); cancel.type = "button"; cancel.className = "trust-button";
  cancel.textContent = "Cancel"; cancel.dataset.studioEditorCancel = "true";
  const done = document.createElement("button"); done.type = "submit"; done.className = "trust-button trust-button--primary";
  done.textContent = "Done"; done.dataset.studioEditorDone = "true";
  actions.append(cancel, done); form.append(actions); dialog.append(heading, form); document.body.append(dialog);
  const invalid = (field: string, message: string) => {
    const input = inputs.get(field), notice = notices.get(field);
    if (!input || !notice) return;
    notice.textContent = message; notice.hidden = false; input.setAttribute("aria-invalid", "true");
    input.classList.add("trust-input--error"); input.focus();
  };
  form.addEventListener("submit", event => {
    event.preventDefault();
    const values: Record<string, number> = {};
    for (const field of options.fields) {
      const input = inputs.get(field.key)!;
      input.removeAttribute("aria-invalid"); input.classList.remove("trust-input--error"); notices.get(field.key)!.hidden = true;
      if (field.optional && !input.value.trim()) continue;
      if (!input.value.trim() || !Number.isFinite(input.valueAsNumber)) { invalid(field.key, `Enter ${field.label.toLowerCase()}.`); return; }
      const value = input.valueAsNumber;
      if (field.minimum !== undefined && value < field.minimum) { invalid(field.key, `Use ${field.minimum} or greater.`); return; }
      if (field.maximum !== undefined && value > field.maximum) { invalid(field.key, `Use ${field.maximum} or less.`); return; }
      values[field.key] = value;
    }
    const issue = options.validate?.(values);
    if (issue) { invalid(issue.field, issue.message); return; }
    const scenarioIssue = options.beforeCommit?.();
    if (scenarioIssue) { const notice = form.querySelector<HTMLElement>("[data-compact-scenario-error]"); if (notice) { notice.textContent = scenarioIssue; notice.hidden = false; } return; }
    options.commit(values); committed = true; dialog.close();
  });
  cancel.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => { if (!committed) options.cancelled?.(); dialog.remove(); if (opener instanceof HTMLElement && opener.isConnected) opener.focus(); });
  dialog.showModal();
  if (options.initialIssue) invalid(options.initialIssue.field, options.initialIssue.message);
  return dialog;
}
