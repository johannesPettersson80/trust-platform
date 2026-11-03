// What the switch, selector, mode selector and slider do when they are used on the runtime page
// (specification 39, §5.8). Each acts on the value it shows and sends that value with its command;
// the runtime decides what a switch writes, and refuses while the control shows no value.

import { sliderValueAt } from "../drawing/components/slider";
import type { HmiObjectModel, LiveValue } from "../drawing/model";
import { say, translate } from "../drawing/speech";

/** What a gesture needs from the page. */
export interface GestureContext {
  /** The live value of an object's bound property. */
  live(object: HmiObjectModel, property: string): LiveValue | undefined;
  /** Writes a value, after the control's question when it has one, saying what the control shows. */
  write(object: HmiObjectModel, value: unknown, shown: unknown): void;
  /** Toggles a switch, after the control's question when it has one, saying what it shows. */
  toggle(object: HmiObjectModel, shown: unknown): void;
  /** A new dialog in place of any open one. */
  dialog(part: string): HTMLElement;
  closeDialog(): void;
  /** Shows a value being entered on a control, or stops showing it. */
  draft(object: HmiObjectModel, value: number | undefined): void;
}

/** The value a control shows, null while it shows none (unknown or stale). */
function shownValue(object: HmiObjectModel, property: string, context: GestureContext): LiveValue["value"] | null {
  const live = context.live(object, property);
  if (!live || live.state === "unknown" || live.staleMs !== undefined) return null;
  return live.value;
}

/** A switch asks the runtime to write the opposite of the value it shows. */
export function toggleSwitch(object: HmiObjectModel, context: GestureContext): void {
  context.toggle(object, shownValue(object, "value", context));
}

/** A selector lists its options, the current one marked, and writes the one chosen. */
export function openSelector(object: HmiObjectModel, context: GestureContext): void {
  const shown = shownValue(object, "value", context);
  const options = object.options ?? [];
  const current = typeof shown === "number" ? options[shown] : shown === null ? undefined : String(shown);
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
      if (!chosen) context.write(object, option, shown);
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
  const shown = shownValue(object, "mode", context);
  if (shown !== null && String(shown).toLowerCase() === mode.toLowerCase()) return;
  context.write(object, mode, shown);
}

/** A slider being dragged: the draft follows the pointer and is written on release, with the value
 * the slider showed when the drag began. */
export class SliderDrag {
  private value: number;

  private constructor(
    private readonly object: HmiObjectModel,
    private readonly context: GestureContext,
    at: { x: number; y: number },
    private readonly shown: unknown,
  ) {
    this.value = sliderValueAt(object, at.x, at.y);
    context.draft(object, this.value);
  }

  /** A drag that starts at a point of the page. */
  static start(object: HmiObjectModel, at: { x: number; y: number }, context: GestureContext): SliderDrag {
    return new SliderDrag(object, context, at, shownValue(object, "value", context));
  }

  move(at: { x: number; y: number }): void {
    this.value = sliderValueAt(this.object, at.x, at.y);
    this.context.draft(this.object, this.value);
  }

  end(at: { x: number; y: number }): void {
    this.move(at);
    this.context.write(this.object, this.value, this.shown);
  }
}
