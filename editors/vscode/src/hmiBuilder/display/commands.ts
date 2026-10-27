import { commandReason } from "./commandReason";
import { withQuality } from "../drawing/values";
// The commands of one surface of the runtime display, the page or a popup (specification 39,
// §5.8, §12.5, §12.8): what a control does when it is used, each command's outcome, and the
// values being entered. A command is never sent again by itself.

import type { CommandState, DrawFrame, HmiObjectModel, HmiPageModel, LiveValue } from "../drawing/model";
import { withValueFormat } from "../drawing/valueFormats";
import { GestureContext, SliderDrag, chooseMode, openSelector, toggleSwitch } from "./gestures";
import { button, element, Overlay } from "./overlay";
import type { DisplayHost } from "./runtimeDisplay";
import type { SessionControl } from "./session";
import { formatValue } from "../drawing/format";
import { say, translate } from "../drawing/speech";
import { roleWord } from "../drawing/components/controls";
import { SentCommands } from "./sentCommands";

/** Controls that send a command when pressed. */
const BUTTONS = ["button", "pulse-button"];
/** Controls that ask for a number. */
const ENTRIES = ["setpoint", "numeric-input"];
/** Controls that act on the value they show (§5.8). */
const CHOOSERS = ["switch", "selector", "mode-selector"];
/** How long an applied or confirmed outcome stays on its control; a refusal stays until a later
 * command is accepted, an unknown or unconfirmed outcome until its control's next command (§12.5). */
const OUTCOME_FADE_MS = 4000;

/** What a command event on the value stream says. */
export interface CommandEvent {
  id: string;
  outcome: CommandState["outcome"];
  reason?: string;
  ended?: boolean;
}

export class CommandSurface {
  /** The latest outcome on each control, by object id. */
  private readonly outcomes = new Map<string, CommandState>();
  /** The control each of the newest commands came from, by command id. */
  private readonly sentFrom = new SentCommands<string>();
  /** Values being entered on controls, by object id, until they are sent (§5.8). */
  private readonly entered = new Map<string, number>();
  /** The slider being dragged. */
  private dragging: SliderDrag | undefined;

  constructor(
    private readonly host: DisplayHost,
    /** The page, or a popup named `popup:<name>` (§4.9), with the role each control needs. */
    private readonly surface: {
      name: string;
      model: HmiPageModel;
      roles?: Record<string, string | null>;
      formats?: DrawFrame["formats"];
    },
    private readonly bundle: string,
    private readonly session: SessionControl,
    private readonly overlay: Overlay,
    private readonly redraw: () => void,
    /** The live value at a bound path. */
    private readonly liveAt: (path: string) => LiveValue | undefined,
  ) {}

  /** The latest outcome on each control. */
  states(): Record<string, CommandState> {
    return Object.fromEntries(this.outcomes);
  }

  /** The values being entered on controls. */
  drafts(): Record<string, number> {
    return Object.fromEntries(this.entered);
  }

  /** The controls the session may not use: the role that may, or "none" (§5.9 item 5). The page
   * data names the role each control needs and the session the roles it may act as (§12.4 item 4),
   * both from the runtime. Without a session every control is read-only instead. */
  locked(): Record<string, string> {
    if (this.session.readOnly) return {};
    const actsAs = this.session.actsAs;
    const locked: Record<string, string> = {};
    for (const [id, needed] of Object.entries(this.surface.roles ?? {})) {
      if (needed === null) locked[id] = "none";
      else if (!actsAs.includes(needed)) locked[id] = needed;
    }
    return locked;
  }

  /** A later outcome from the stream; whether it was one of this surface's commands. */
  receive(command: CommandEvent): boolean {
    const from = this.sentFrom.from(command.id);
    if (!from) return false;
    this.setOutcome(from, { outcome: command.outcome, reason: command.reason });
    return true;
  }

  /** A click on a drawn object; `option` is the mode or option clicked inside it. */
  click(objectId: string, option?: string): void {
    const object = this.usable(objectId, [...BUTTONS, ...ENTRIES, ...CHOOSERS]);
    if (!object) return;
    if (BUTTONS.includes(object.type)) {
      const action = object.type === "pulse-button" ? "pulse" : (object.action ?? "set");
      this.asked(object, () => void this.send(object, action));
    } else if (object.type === "switch") {
      toggleSwitch(object, this.gestures);
    } else if (object.type === "selector") {
      openSelector(object, this.gestures);
    } else if (object.type === "mode-selector") {
      chooseMode(object, option, this.gestures);
    } else {
      this.showEntry(object);
    }
  }

  /** A press on a drawn object at a point of the page; whether it started dragging a slider. */
  press(objectId: string, at: { x: number; y: number }): boolean {
    const object = this.usable(objectId, ["slider"]);
    this.dragging = object ? SliderDrag.start(object, at, this.gestures) : undefined;
    return this.dragging !== undefined;
  }

  /** The pointer moved while a slider is dragged. */
  drag(at: { x: number; y: number }): void {
    this.dragging?.move(at);
  }

  /** The pointer was released: a dragged slider writes its value. */
  release(at: { x: number; y: number }): void {
    const dragging = this.dragging;
    this.dragging = undefined;
    dragging?.end(at);
  }

  /** The control with this id when it is one of `types` and may be used; without a session it
   * asks to log in instead. */
  private usable(objectId: string, types: string[]): HmiObjectModel | undefined {
    const object = this.surface.model.object.find((candidate) => candidate.id === objectId);
    if (!object || !types.includes(object.type)) return undefined;
    if (this.session.readOnly) {
      this.session.showLogin(say("Log in to operate."));
      return undefined;
    }
    const locked = this.locked()[objectId];
    if (locked) {
      this.overlay.notice(
        locked === "none" ? say("Locked") : say("This needs the {role} role.", { role: roleWord(locked) }),
      );
      return undefined;
    }
    return object;
  }

  /** What the gestures of §5.8 use. */
  private readonly gestures: GestureContext = {
    live: (object, property) => {
      const binding = object.bind[property];
      const value = this.liveAt(binding?.path ?? "");
      return binding?.quality ? withQuality(value, this.liveAt(binding.quality)) : value;
    },
    write: (object, value) =>
      this.asked(
        object,
        () => void this.send(object, "write", value),
        () => this.dropDraft(object.id),
      ),
    notice: (text) => this.overlay.notice(text),
    dialog: (part) => this.overlay.dialog("div", part),
    closeDialog: () => this.overlay.closeDialog(),
    draft: (object, value) => {
      if (value === undefined) this.entered.delete(object.id);
      else this.entered.set(object.id, value);
      this.redraw();
    },
  };

  /** A control's latest outcome; an applied or confirmed one fades after a few seconds. A refusal
   * reads "Not accepted: <reason>" beside the control (§21.2 item 3). */
  private setOutcome(objectId: string, state: CommandState): void {
    if (state.reason) state = { ...state, reason: commandReason(state.outcome, state.reason) };
    this.outcomes.set(objectId, state);
    if (state.outcome !== "applied" && state.outcome !== "confirmed") return;
    this.host.later(OUTCOME_FADE_MS, () => {
      if (this.outcomes.get(objectId) !== state) return;
      this.outcomes.delete(objectId);
      this.redraw();
    });
  }

  /** Drops the refusal marks of the controls other than `except`: a refusal answered an earlier
   * click, and once a later command is accepted it no longer describes the equipment. Unknown and
   * timed-out outcomes stay, as those commands may still have acted. */
  private dropRefusals(except: string): void {
    for (const [id, state] of this.outcomes) {
      if (id !== except && state.outcome === "refused") this.outcomes.delete(id);
    }
  }

  private dropDraft(objectId: string): void {
    if (this.entered.delete(objectId)) this.redraw();
  }

  /** Runs `then` straight away, or after the control's question is answered yes; `cancelled`
   * when it is answered no. */
  private asked(object: HmiObjectModel, then: () => void, cancelled: () => void = () => undefined): void {
    if (!object.confirm) {
      then();
      return;
    }
    const dialog = this.overlay.dialog("div", "confirm");
    dialog.append(element(dialog, "h2", object.confirm));
    const actions = element(dialog, "div", "", "hmi-actions");
    const cancel = button(dialog, say("Cancel"), "cancel");
    const yes = button(dialog, say("Send"), "confirm");
    cancel.addEventListener("click", () => {
      this.overlay.closeDialog();
      cancelled();
    });
    yes.addEventListener("click", () => {
      this.overlay.closeDialog();
      then();
    });
    actions.append(cancel, yes);
    dialog.append(actions);
  }

  private async send(object: HmiObjectModel, action: string, value?: unknown): Promise<void> {
    const id = commandId();
    this.sentFrom.add(id, object.id);
    this.setOutcome(object.id, { outcome: "pending" });
    this.redraw();
    const body: Record<string, unknown> = {
      id,
      bundle: this.bundle,
      page: this.surface.name,
      object: object.id,
      action: object.command ? "invoke" : action,
    };
    if (value !== undefined) body.value = value;
    let answer: { status: number; body: unknown };
    try {
      answer = await this.host.postJson("/api/hmi/command", body);
    } catch {
      // The outcome is unknown and never assumed; the command is not sent again (§12.5).
      this.setOutcome(object.id, {
        outcome: "unknown",
        reason: say("The connection was lost before the outcome was known."),
      });
      this.overlay.notice(say("The connection was lost before the outcome was known."));
      this.entered.delete(object.id);
      this.redraw();
      return;
    }
    // The control shows the program's value again once the command has its outcome (§5.8).
    this.entered.delete(object.id);
    const reply = answer.body as { outcome?: CommandState["outcome"]; reason?: string; error?: string } | null;
    if (answer.status === 200 && reply?.outcome) {
      this.setOutcome(object.id, { outcome: reply.outcome, reason: reply.reason });
      // An accepted command makes earlier refusals stale: their notice and their marks go.
      if (reply.outcome === "applied" || reply.outcome === "confirmed" || reply.outcome === "pending") {
        this.overlay.clearNotice();
        this.dropRefusals(object.id);
      }
      if ((reply.outcome === "refused" || reply.outcome === "failed") && reply.reason) {
        const owner = /^owner-login:([A-Za-z0-9_-]{1,128})$/.exec(reply.reason);
        if (owner)
          this.session.showLogin(
            say("Log in to {runtime} with a named account on that runtime. The command has not been sent.", {
              runtime: owner[1],
            }),
            owner[1],
          );
        else this.overlay.notice(commandReason(reply.outcome, reply.reason));
      }
    } else {
      const reason = reply?.error ?? say("The runtime answered {status}.", { status: answer.status });
      this.setOutcome(object.id, { outcome: "refused", reason });
      this.overlay.notice(reason);
    }
    this.redraw();
  }

  /** Asks for a number, shows the control's limits, and sends what is typed. */
  private showEntry(object: HmiObjectModel): void {
    const path = object.bind.value?.path ?? "";
    const live = this.liveAt(path);
    const unit = withValueFormat(object, { formats: this.surface.formats, values: live ? { [path]: live } : {} }).unit;
    const form = this.overlay.dialog("form", "entry");
    form.append(element(form, "h2", object.label !== undefined ? translate(object.label) : say("New value")));
    const field = element(form, "label", `${say("Value")}${unit ? ` (${unit})` : ""}`);
    const input = form.ownerDocument.createElement("input");
    input.name = "value";
    input.inputMode = "decimal";
    input.setAttribute("autocomplete", "off");
    const current = live?.value;
    input.value = typeof current === "number" ? String(current) : "";
    field.append(input);
    form.append(field);
    const limit = (value: number | undefined): string => (value === undefined ? say("any") : formatValue(value, {}));
    const limits =
      object.min !== undefined || object.max !== undefined
        ? `${say("Between {min} and {max}", { min: limit(object.min), max: limit(object.max) })}${unit ? ` ${unit}` : ""}`
        : "";
    if (limits) form.append(element(form, "p", limits, "hmi-hint"));
    // On a touch screen a large keypad types the number, and no device keyboard covers it.
    if (this.overlay.touched) {
      input.inputMode = "none";
      const keypad = element(form, "div", "", "hmi-keypad");
      keypad.setAttribute("data-part", "keypad");
      const keys: Array<[string, string]> = [
        ["7", "7"],
        ["8", "8"],
        ["9", "9"],
        ["4", "4"],
        ["5", "5"],
        ["6", "6"],
        ["1", "1"],
        ["2", "2"],
        ["3", "3"],
        ["sign", "±"],
        ["0", "0"],
        [".", "."],
        ["back", "⌫"],
      ];
      for (const [key, label] of keys) {
        const pressed = button(form, label);
        pressed.setAttribute("data-key", key);
        pressed.addEventListener("click", () => {
          if (key === "back") input.value = input.value.slice(0, -1);
          else if (key === "sign") input.value = input.value.startsWith("-") ? input.value.slice(1) : `-${input.value}`;
          else if (key !== "." || !input.value.includes(".")) input.value += key;
        });
        keypad.append(pressed);
      }
      form.append(keypad);
    }
    const actions = element(form, "div", "", "hmi-actions");
    const cancel = button(form, say("Cancel"), "cancel");
    cancel.type = "button";
    cancel.addEventListener("click", () => this.overlay.closeDialog());
    const submit = button(form, say("Send"));
    submit.type = "submit";
    actions.append(cancel, submit);
    form.append(actions);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      // The entry as typed: the runtime reads the number and checks its limits, and its refusal
      // names the reason (specification 39 §12.8).
      const entry = input.value.trim();
      this.overlay.closeDialog();
      this.asked(object, () => void this.send(object, "write", entry));
    });
    input.focus();
  }
}

/** A command id that is new each time. */
export function commandId(): string {
  const random = new Uint32Array(3);
  const crypto = (globalThis as { crypto?: { getRandomValues?: (array: Uint32Array) => Uint32Array } }).crypto;
  if (crypto?.getRandomValues) crypto.getRandomValues(random);
  else for (let index = 0; index < random.length; index += 1) random[index] = Math.floor(Math.random() * 2 ** 32);
  return `c-${Date.now().toString(36)}-${Array.from(random, (part) => part.toString(36)).join("")}`;
}
