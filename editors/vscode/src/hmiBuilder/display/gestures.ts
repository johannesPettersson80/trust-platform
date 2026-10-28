// What the switch, selector, mode selector and slider do when they are used on the runtime page
// (specification 39, §5.8). Each acts on the value it shows, so while that value is unknown or
// stale it sends nothing and says so.

import { sliderValueAt } from "../drawing/components/slider";
import type { HmiObjectModel, LiveValue } from "../drawing/model";
import { say, translate } from "../drawing/speech";

/** What a gesture needs from the page. */
export interface GestureContext {
  /** The live value of an object's bound property. */
  live(object: HmiObjectModel, property: string): LiveValue | undefined;
  /** Writes a value, after the control's question when it has one. */
  write(object: HmiObjectModel, value: unknown): void;
  /** A short notice. */
  notice(text: string): void;
  /** A new dialog in place of any open one. */
  dialog(part: string): HTMLElement;
  closeDialog(): void;
  /** Shows a value being entered on a control, or stops showing it. */
  draft(object: HmiObjectModel, value: number | undefined): void;
}

const NOT_KNOWN = (): string => say("The value of this control is not known, so it sends nothing.");

/** The value a gesture acts on, or undefined (with a notice) while it is unknown or stale. */
function known(object: HmiObjectModel, property: string, context: GestureContext): LiveValue | undefined {
  const live = context.live(object, property);
  if (!live || live.state === "unknown" || live.staleMs !== undefined || live.value === null) {
    context.notice(NOT_KNOWN());
    return undefined;
  }
  return live;
}

/** A switch writes the opposite of the value it shows. */
export function toggleSwitch(object: HmiObjectModel, context: GestureContext): void {
  const live = known(object, "value", context);
  if (live) context.write(object, live.value !== true);
}

/** A selector lists its options, the current one marked, and writes the one chosen. */
export function openSelector(object: HmiObjectModel, context: GestureContext): void {
  const live = known(object, "value", context);
  if (!live) return;
  const options = object.options ?? [];
  const current = typeof live.value === "number" ? options[live.value] : String(live.value);
  const dialog = context.dialog("choices");
  const document = dialog.ownerDocument;
  const heading = document.createElement("h2");
  heading.textContent = object.label !== undefined ? translate(object.label) : say("Choose");
  const list = document.createElement("div");
  list.className = "hmi-choices";
  for (const option of options) {
    const choice = document.createElement("button");
    choice.type = "button";
    choice.textContent = option;
    choice.setAttribute("data-option", option);
    const chosen = option.toLowerCase() === current?.toLowerCase();
    choice.setAttribute("aria-pressed", String(chosen));
    choice.addEventListener("click", () => {
      context.closeDialog();
      if (!chosen) context.write(object, option);
    });
    list.append(choice);
  }
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.textContent = say("Cancel");
  cancel.setAttribute("data-action", "cancel");
  cancel.addEventListener("click", () => context.closeDialog());
  const actions = document.createElement("div");
  actions.className = "hmi-actions";
  actions.append(cancel);
  dialog.append(heading, list, actions);
}

/** A mode selector writes the mode clicked; the active one sends nothing. */
export function chooseMode(object: HmiObjectModel, mode: string | undefined, context: GestureContext): void {
  if (mode === undefined) return;
  const live = known(object, "mode", context);
  if (live && String(live.value).toLowerCase() !== mode.toLowerCase()) context.write(object, mode);
}

/** A slider being dragged: the draft follows the pointer and is written on release. */
export class SliderDrag {
  private value: number;

  private constructor(
    private readonly object: HmiObjectModel,
    private readonly context: GestureContext,
    at: { x: number; y: number },
  ) {
    this.value = sliderValueAt(object, at.x, at.y);
    context.draft(object, this.value);
  }

  /** A drag that starts at a point of the page, or none while the value is not known. */
  static start(object: HmiObjectModel, at: { x: number; y: number }, context: GestureContext): SliderDrag | undefined {
    return known(object, "value", context) ? new SliderDrag(object, context, at) : undefined;
  }

  move(at: { x: number; y: number }): void {
    this.value = sliderValueAt(this.object, at.x, at.y);
    this.context.draft(this.object, this.value);
  }

  end(at: { x: number; y: number }): void {
    this.move(at);
    this.context.write(this.object, this.value);
  }
}
