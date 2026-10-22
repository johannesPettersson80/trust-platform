import { displayEngineeringUnit } from "../chartContract";
import type { ChannelRow, ChannelChoices } from "../generated/OpenResult";
import type { ChannelSelection } from "../generated/InputsRequest";

/**
 * The Channels editor (the Tune page's **Channels…**, specification 35, A8.1): it shows the rows the engine answered for the open loop,
 * one per model channel, each with the plant signals its side may take, and it hands the plant signal each row picks to `commit` when
 * the engineer presses Done. It holds no mapping rule: the engine checks the selection, saves it, and refuses one that breaks a rule,
 * with the next action that opens this editor again.
 */
export function editControllerChannels(choices: ChannelChoices, commit: (selection: ChannelSelection) => void): HTMLDialogElement {
  document.querySelector("[data-studio-channel-mapping]")?.remove();
  const opener = document.activeElement;
  const dialog = document.createElement("dialog");
  dialog.dataset.studioChannelMapping = "true";
  dialog.className = "trust-compact-editor";
  const title = document.createElement("h2");
  title.textContent = "Controller channels";
  dialog.append(title);
  const form = document.createElement("form");
  form.noValidate = true;
  const fields = new Map<string, HTMLSelectElement>();
  const sideFieldsets = (kind: "inputs" | "outputs", prefix: string): HTMLFieldSetElement[] => choices[kind].map(row => {
    const { element, field } = channelFieldset(prefix, row);
    fields.set(`${kind}:${row.modelChannelId}`, field);
    return element;
  });
  form.append(...sideFieldsets("inputs", "Command"), ...sideFieldsets("outputs", "Feedback"));
  const actions = document.createElement("div");
  actions.className = "trust-compact-editor-actions";
  const cancel = document.createElement("button");
  cancel.className = "trust-button";
  cancel.type = "button";
  cancel.textContent = "Cancel";
  cancel.dataset.studioEditorCancel = "true";
  const done = document.createElement("button");
  done.className = "trust-button trust-button--primary";
  done.type = "submit";
  done.textContent = "Done";
  done.dataset.studioEditorDone = "true";
  actions.append(cancel, done);
  form.append(actions);
  dialog.append(form);
  document.body.append(dialog);
  cancel.addEventListener("click", () => dialog.close());
  form.addEventListener("submit", event => {
    event.preventDefault();
    const picks = (kind: "inputs" | "outputs") => choices[kind].map(row => ({
      modelChannelId: row.modelChannelId,
      signalId: fields.get(`${kind}:${row.modelChannelId}`)?.value ?? row.signalId,
    }));
    commit({ inputs: picks("inputs"), outputs: picks("outputs") });
    dialog.close();
  });
  dialog.addEventListener("close", () => {
    dialog.remove();
    if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
  });
  dialog.showModal();
  return dialog;
}

/** One model channel's fieldset: its owner or identifier, and the select of the plant signals its side offers. */
function channelFieldset(prefix: string, row: ChannelRow): { element: HTMLFieldSetElement; field: HTMLSelectElement } {
  const element = document.createElement("fieldset");
  const legend = document.createElement("legend");
  legend.textContent = `${prefix} · ${row.owner ?? row.modelChannelId}`;
  element.append(legend);
  const field = document.createElement("select");
  field.className = "trust-input";
  field.dataset.studioChannelField = row.modelChannelId;
  for (const choice of row.choices) {
    const option = document.createElement("option");
    option.value = choice.signalId;
    option.textContent = `${choice.symbol} (${displayEngineeringUnit(choice.unit)})`;
    option.selected = choice.signalId === row.signalId;
    field.append(option);
  }
  element.append(label("Plant signal", field));
  return { element, field };
}

function label(text: string, control: HTMLElement): HTMLElement {
  const wrapper = document.createElement("label");
  wrapper.className = "trust-field";
  wrapper.textContent = text;
  wrapper.append(control);
  return wrapper;
}
