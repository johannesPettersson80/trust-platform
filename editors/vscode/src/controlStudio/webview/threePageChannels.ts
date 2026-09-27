import { displayEngineeringUnit } from "../chartContract";
import { groupChannelOwner } from "../controllerGroupPresentation";
import { assertPidGroupSignalMapping } from "../pidControllerGroup";
import type { ControlLoopDefinition } from "../loopDefinition";
import { loopSignal, validateLoopChannelBindings, type LoopChannels } from "../loopChannels";

/** Validate and clone a complete channel mapping before handing it to the host. */
export function applyControllerChannelMapping(loop: ControlLoopDefinition, draft: LoopChannels): LoopChannels {
  const next = structuredClone(draft);
  assertPidGroupSignalMapping(loop, next);
  validateLoopChannelBindings({ ...loop, channels: next });
  return next;
}

/** Edit the declared multivariable target mapping without inventing controller groups. */
export function editControllerChannels(loop: ControlLoopDefinition, commit: (next: LoopChannels) => void): HTMLDialogElement | undefined {
  const current = loop.channels;
  if (!current || (current.inputs.length < 2 && current.outputs.length < 2)) return undefined;
  document.querySelector("[data-studio-channel-mapping]")?.remove();
  const opener = document.activeElement;
  const draft = structuredClone(current);
  const dialog = document.createElement("dialog");
  dialog.dataset.studioChannelMapping = "true";
  dialog.className = "trust-compact-editor";
  const title = document.createElement("h2"); title.textContent = "Controller channels"; dialog.append(title);
  const form = document.createElement("form"); form.noValidate = true;
  const fields = new Map<string, HTMLSelectElement>();
  const select = (name: string, selected: string, choices: readonly string[]) => {
    const field = document.createElement("select"); field.className = "trust-input"; field.dataset.studioChannelField = name;
    field.id = `studio-channel-${fields.size}`; fields.set(name, field);
    for (const id of choices) { const signal = loopSignal(loop, id); const option = document.createElement("option"); option.value = id; option.textContent = `${signal.symbol} (${displayEngineeringUnit(signal.unit)})`; option.selected = id === selected; field.append(option); }
    return field;
  };
  for (const input of draft.inputs) {
    const fieldset = document.createElement("fieldset"); const legend = document.createElement("legend"); legend.textContent = `Command · ${groupChannelOwner(loop, "input", input.modelChannelId) ?? input.modelChannelId}`; fieldset.append(legend);
    fieldset.append(label("Plant signal", select(`input:${input.modelChannelId}:signalId`, input.signalId, draft.inputs.map(channel => channel.signalId))));
    fieldset.append(error(input.modelChannelId));
    form.append(fieldset);
  }
  for (const output of draft.outputs) {
    const fieldset = document.createElement("fieldset"); const legend = document.createElement("legend"); legend.textContent = `Feedback · ${groupChannelOwner(loop, "output", output.modelChannelId) ?? output.modelChannelId}`; fieldset.append(legend);
    fieldset.append(label("Plant signal", select(`output:${output.modelChannelId}:signalId`, output.signalId, draft.outputs.map(channel => channel.signalId))));
    fieldset.append(error(output.modelChannelId));
    form.append(fieldset);
  }
  const actions = document.createElement("div"); actions.className = "trust-compact-editor-actions"; const cancel = document.createElement("button"); cancel.className = "trust-button"; cancel.type = "button"; cancel.textContent = "Cancel"; cancel.dataset.studioEditorCancel = "true";
  const failure = document.createElement("p"); failure.hidden = true; failure.setAttribute("role", "alert"); form.append(failure);
  const done = document.createElement("button"); done.className = "trust-button trust-button--primary"; done.type = "submit"; done.textContent = "Done"; done.dataset.studioEditorDone = "true"; actions.append(cancel, done); form.append(actions); dialog.append(form); document.body.append(dialog);
  const close = () => { dialog.close(); };
  cancel.addEventListener("click", close);
  form.addEventListener("submit", event => {
    event.preventDefault();
    const next = structuredClone(draft);
    failure.hidden = true;
    let invalid: HTMLSelectElement | undefined;
    for (const [key, field] of fields) {
      const message = field.closest("fieldset")!.querySelector<HTMLElement>("[data-studio-channel-error]")!;
      message.hidden = true; field.removeAttribute("aria-invalid"); field.removeAttribute("aria-describedby");
      const kind = key.startsWith("input:") ? "input" : "output";
      const repeated = [...fields].filter(([otherKey, other]) => otherKey.startsWith(`${kind}:`) && other.value === field.value).length > 1;
      const original = (kind === "input" ? draft.inputs : draft.outputs).find(channel => key === `${kind}:${channel.modelChannelId}:signalId`);
      const unitChanged = original && loopSignal(loop, original.signalId).unit !== loopSignal(loop, field.value).unit;
      const groupOwner = original && groupChannelOwner(loop, kind, original.modelChannelId);
      const rewired = groupOwner && original && field.value !== original.signalId;
      if (!repeated && !unitChanged && !rewired) continue;
      message.textContent = repeated ? "Choose a different signal; this signal is already assigned to another channel."
        : rewired ? `${groupOwner} is bound to ${loopSignal(loop, original.signalId).symbol}. Keep its declared signal.`
          : "Choose a signal with the declared channel unit.";
      message.id = `${field.id}-error`; message.hidden = false;
      field.setAttribute("aria-invalid", "true"); field.setAttribute("aria-describedby", message.id);
      invalid ??= field;
    }
    if (invalid) { invalid.focus(); return; }
    for (const input of next.inputs) {
      input.signalId = fields.get(`input:${input.modelChannelId}:signalId`)!.value;
    }
    for (const output of next.outputs) {
      output.signalId = fields.get(`output:${output.modelChannelId}:signalId`)!.value;
    }
    try { commit(applyControllerChannelMapping(loop, next)); dialog.close(); } catch (error) {
      failure.textContent = error instanceof Error ? error.message : String(error); failure.hidden = false;
    }
  });
  dialog.addEventListener("close", () => { dialog.remove(); if (opener instanceof HTMLElement && opener.isConnected) opener.focus(); });
  dialog.showModal();
  return dialog;
}

function label(text: string, control: HTMLElement): HTMLElement { const wrapper = document.createElement("label"); wrapper.className = "trust-field"; wrapper.textContent = text; wrapper.append(control); return wrapper; }
function error(row: string): HTMLElement { const message = document.createElement("span"); message.dataset.studioChannelError = row; message.setAttribute("role", "alert"); message.hidden = true; return message; }
