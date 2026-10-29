// Operating the runtime display (specification 39, §12.4–12.5, §12.8): the session, commands
// from the page's controls with their outcomes, and what the value stream says about them. The
// parts live in their own modules; this is the page's single entry point.

import type { DrawFrame, HmiPageModel, LiveValue, CommandState } from "../drawing/model";
import { AlarmActions } from "./alarmActions";
import { CommandEvent, CommandSurface } from "./commands";
import { Overlay } from "./overlay";
import type { DisplayHost } from "./runtimeDisplay";
import { Session, SessionControl } from "./session";

export { OPERATE_CSS } from "./overlay";
export type { Session } from "./session";

/** What the value stream says about the session and commands. */
export interface OperateMessage {
  session?: Session | null;
  /** The active bundle's version, after a deployment or rollback (§16.2.6). */
  bundle?: string;
  command?: CommandEvent;
}

import { EquipmentSettings } from "./settings";

export class Operation {
  readonly settings: EquipmentSettings;
  readonly overlay: Overlay;
  readonly session: SessionControl;
  /** Operator actions on alarms (specification 40, §4). */
  readonly alarms: AlarmActions;
  private readonly commands: CommandSurface;
  /** Popups' command surfaces, which hear the same command events (§4.9). */
  private readonly others = new Set<CommandSurface>();
  private readonly formats: DrawFrame["formats"];

  constructor(
    container: HTMLElement,
    private readonly host: DisplayHost,
    page: { name: string; model: HmiPageModel; formats?: DrawFrame["formats"] },
    private readonly bundle: string,
    private readonly redraw: () => void,
    /** The live value at a bound path. */
    liveAt: (path: string) => LiveValue | undefined,
    /** After a login or logout: the value stream carries the session it was opened with. */
    sessionChanged: () => void = () => undefined,
    /** Where the session bar goes: the top bar's slot, when the screen has a top bar (§4.8 item 8). */
    sessionSlot?: HTMLElement,
    /** SVG markup of an alarm priority's mark (§13.6, item 6). */
    alarmMark?: (priority: string) => string,
  ) {
    this.overlay = new Overlay(container, host);
    this.formats = page.formats;
    this.session = new SessionControl(
      host,
      this.overlay,
      () => {
        this.redraw();
        sessionChanged();
      },
      sessionSlot,
    );
    this.settings = new EquipmentSettings(host, this.overlay, this.session);
    this.commands = new CommandSurface(host, page, bundle, this.session, this.overlay, redraw, liveAt);
    this.alarms = new AlarmActions(host, this.session, this.overlay, alarmMark);
  }

  /** Reads the session the page opened with. */
  async start(): Promise<void> {
    await this.session.start();
  }

  /** Without a session the controls cannot be used (§12.4). */
  get readOnly(): boolean {
    return this.session.readOnly;
  }

  /** The latest outcome on each control. */
  states(): Record<string, CommandState> {
    return this.commands.states();
  }

  /** The values being entered on controls. */
  drafts(): Record<string, number> {
    return this.commands.drafts();
  }

  /** The page's controls the signed-in role may not use (§5.9 item 5). */
  locked(): Record<string, string> {
    return this.commands.locked();
  }

  /** A command surface for a popup, sharing the session and the overlay (§4.9). */
  popupSurface(
    popup: { name: string; model: HmiPageModel; roles?: Record<string, string | null>; formats?: DrawFrame["formats"] },
    redraw: () => void,
    liveAt: (path: string) => LiveValue | undefined,
  ): CommandSurface {
    // A popup's commands name it `popup:<name>`; a faceplate's, its own name (§8.7, item 5).
    const page = popup.name.startsWith("faceplate:") ? popup.name : `popup:${popup.name}`;
    const surface = new CommandSurface(
      this.host,
      { name: page, model: popup.model, roles: popup.roles, formats: popup.formats ?? this.formats },
      this.bundle,
      this.session,
      this.overlay,
      redraw,
      liveAt,
    );
    this.others.add(surface);
    return surface;
  }

  /** Forgets a closed popup's commands. */
  closePopupSurface(surface: CommandSurface): void {
    this.others.delete(surface);
  }

  /** A stream message about the session or a command; whether it was one. */
  receive(message: OperateMessage): boolean {
    let handled = false;
    if ("session" in message) {
      handled = true;
      this.session.receive(message.session);
    }
    if (typeof message.bundle === "string") {
      handled = true;
      if (message.bundle !== this.bundle) this.overlay.showReload();
    }
    if (message.command) {
      handled = true;
      const command = message.command;
      if (!this.commands.receive(command) && !this.alarms.receive(command)) {
        for (const other of this.others) if (other.receive(command)) break;
      }
    }
    if (handled) this.redraw();
    return handled;
  }

  /** A click on a drawn object; `option` is the mode or option clicked inside it. */
  click(objectId: string, option?: string): void {
    this.commands.click(objectId, option);
  }

  /** A press on a drawn object at a point of the page; whether it started dragging a slider. */
  press(objectId: string, at: { x: number; y: number }): boolean {
    return this.commands.press(objectId, at);
  }

  /** The pointer moved while a slider is dragged. */
  drag(at: { x: number; y: number }): void {
    this.commands.drag(at);
  }

  /** The pointer was released: a dragged slider writes its value. */
  release(at: { x: number; y: number }): void {
    this.commands.release(at);
  }
}
