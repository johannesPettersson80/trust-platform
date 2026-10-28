// The overlay of the runtime display (specification 39, §12, §16.2.6): dialogs, short notices
// and the reload notice, in one layer over the drawing, and their styles.

import type { DisplayHost } from "./runtimeDisplay";
import { say } from "../drawing/speech";

/** How long a notice stays, in milliseconds. */
const NOTICE_MS = 6000;

export const OPERATE_CSS = `
.hmi-keypad { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin: 14px 0 4px; }
.hmi-dialog .hmi-keypad button { height: 56px; padding: 0; border-radius: 12px; border: 1px solid var(--hmi-border); background: var(--hmi-raised); color: var(--hmi-text); font: 600 24px var(--hmi-font); cursor: pointer; }
.hmi-dialog .hmi-keypad button[data-key="back"] { grid-column: span 3; font-size: 22px; }
.hmi-dialog .hmi-keypad ~ .hmi-actions button { min-height: 52px; padding: 12px 28px; font-size: 17px; }
.hmi-session { position: fixed; top: 12px; right: 12px; z-index: 10; display: flex; align-items: center; gap: 10px; padding: 5px 6px 5px 14px; border-radius: 999px; background: var(--hmi-raised); border: 1px solid var(--hmi-border); box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35); font: 500 13px var(--hmi-font); color: var(--hmi-text); }
.hmi-session .hmi-role { padding: 2px 9px; border-radius: 999px; background: color-mix(in srgb, var(--hmi-accent) 18%, transparent); color: var(--hmi-accent); font-size: 12px; font-weight: 600; }
.hmi-session button, .hmi-dialog button { box-sizing: border-box; min-width: 44px; min-height: 44px; font: 600 14px var(--hmi-font); border-radius: 999px; padding: 6px 14px; cursor: pointer; border: 1px solid var(--hmi-border); background: var(--hmi-surface); color: var(--hmi-text); }
.hmi-dialog button[type="submit"], .hmi-dialog [data-action="confirm"] { background: var(--hmi-accent); border-color: var(--hmi-accent); color: var(--hmi-page); }
.hmi-backdrop { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; background: rgba(0, 0, 0, 0.55); backdrop-filter: blur(3px); }
.hmi-dialog { min-width: 320px; max-width: 440px; display: grid; gap: 14px; padding: 24px; border-radius: 16px; background: var(--hmi-surface); border: 1px solid var(--hmi-border); box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5); font: 14px/1.45 var(--hmi-font); color: var(--hmi-text); }
.hmi-dialog h2 { margin: 0; font: 650 18px var(--hmi-font); }
.hmi-dialog label { display: grid; gap: 6px; color: var(--hmi-muted); font-size: 13px; }
.hmi-dialog input { box-sizing: border-box; min-height: 44px; font: 500 16px var(--hmi-font); padding: 10px 12px; border-radius: 10px; border: 1px solid var(--hmi-border); background: var(--hmi-page); color: var(--hmi-text); }
.hmi-dialog .hmi-actions { display: flex; justify-content: flex-end; gap: 10px; }
.hmi-dialog .hmi-hint { margin: 0; color: var(--hmi-muted); font-size: 13px; }
.hmi-dialog .hmi-error { margin: 0; color: var(--hmi-alarm); font-size: 13px; }
.hmi-notice { position: fixed; left: 50%; bottom: 24px; z-index: 30; transform: translateX(-50%); max-width: min(560px, 90vw); padding: 10px 16px; border-radius: 12px; background: var(--hmi-raised); border: 1px solid var(--hmi-alarm); box-shadow: 0 12px 34px rgba(0, 0, 0, 0.45); font: 500 14px var(--hmi-font); color: var(--hmi-text); }
.hmi-reload { display: flex; align-items: center; gap: 14px; top: 72px; bottom: auto; border-color: var(--hmi-accent); }
.hmi-choices { display: grid; gap: 8px; }
.hmi-choices button { text-align: left; font: 600 15px var(--hmi-font); padding: 12px 16px; min-height: 44px; border-radius: 12px; cursor: pointer; border: 1px solid var(--hmi-border); background: var(--hmi-page); color: var(--hmi-text); }
.hmi-choices button[aria-pressed="true"] { border-color: var(--hmi-accent); color: var(--hmi-accent); box-shadow: inset 3px 0 0 var(--hmi-accent); }
.hmi-dialog .hmi-alarm-title { display: flex; align-items: center; gap: 10px; }
.hmi-dialog .hmi-alarm-mark { display: inline-flex; flex: none; }
.hmi-dialog .hmi-facts { display: grid; grid-template-columns: auto 1fr; gap: 6px 18px; margin: 0; }
.hmi-dialog .hmi-facts dt { color: var(--hmi-muted); font-size: 13px; }
.hmi-dialog .hmi-facts dd { margin: 0; font-weight: 600; overflow-wrap: anywhere; }
.hmi-dialog .hmi-alarm-actions, .hmi-dialog .hmi-shelve { justify-content: flex-start; flex-wrap: wrap; }
.hmi-dialog .hmi-alarm-actions button, .hmi-dialog .hmi-shelve button { min-height: 44px; padding: 8px 16px; }
.hmi-dialog [data-action="acknowledge"] { background: var(--hmi-accent); border-color: var(--hmi-accent); color: var(--hmi-page); }
.hmi-reload button { box-sizing: border-box; min-width: 44px; min-height: 44px; font: 600 14px var(--hmi-font); border-radius: 999px; padding: 6px 14px; cursor: pointer; border: 1px solid var(--hmi-accent); background: var(--hmi-accent); color: var(--hmi-page); }
`;

export class Overlay {
  /** The last press on the page or a popup was a touch: number entries open the on-screen keypad
   * (specification 39, §18 phase 5 item 6). */
  touched = false;
  readonly layer: HTMLElement;
  private noticeTimer = 0;

  constructor(
    container: HTMLElement,
    private readonly host: DisplayHost,
  ) {
    this.layer = container.ownerDocument.createElement("div");
    this.layer.className = "hmi-operate";
    container.append(this.layer);
  }

  /** A short notice at the bottom that goes after a few seconds. */
  notice(text: string): void {
    this.layer.querySelector('[data-part="notice"]')?.remove();
    const note = element(this.layer, "div", text, "hmi-notice");
    note.setAttribute("data-part", "notice");
    note.setAttribute("role", "status");
    this.layer.append(note);
    const mine = (this.noticeTimer += 1);
    this.host.later(NOTICE_MS, () => {
      if (mine === this.noticeTimer) note.remove();
    });
  }

  /** Takes away the notice now: a later command was accepted, so an earlier refusal is past. */
  clearNotice(): void {
    this.noticeTimer += 1;
    this.layer.querySelector('[data-part="notice"]')?.remove();
  }

  /** A newer bundle is active: the page tells its user to reload; its commands would be refused
   * as out of date (§16.2.6). */
  showReload(): void {
    if (this.layer.querySelector('[data-part="reload"]')) return;
    const note = element(this.layer, "div", "", "hmi-notice hmi-reload");
    note.setAttribute("data-part", "reload");
    note.setAttribute("role", "alert");
    note.append(element(note, "span", say("This page has changed. Reload to see the new version.")));
    const again = button(note, say("Reload"), "reload");
    again.addEventListener("click", () => this.host.reload());
    note.append(again);
    this.layer.append(note);
  }

  /** A new dialog in place of any open one. */
  dialog<K extends "div" | "form">(tag: K, part: string): HTMLElementTagNameMap[K] {
    this.closeDialog();
    const document = this.layer.ownerDocument;
    const backdrop = document.createElement("div");
    backdrop.className = "hmi-backdrop";
    backdrop.setAttribute("data-part", "backdrop");
    const dialog = document.createElement(tag);
    dialog.className = "hmi-dialog";
    dialog.setAttribute("data-part", part);
    dialog.setAttribute("role", "dialog");
    backdrop.append(dialog);
    this.layer.append(backdrop);
    return dialog;
  }

  closeDialog(): void {
    this.layer.querySelector('[data-part="backdrop"]')?.remove();
  }
}

export function element(parent: Element, tag: string, text: string, className?: string): HTMLElement {
  const created = parent.ownerDocument.createElement(tag);
  if (text) created.textContent = text;
  if (className) created.className = className;
  return created;
}

export function button(parent: Element, text: string, action?: string): HTMLButtonElement {
  const created = parent.ownerDocument.createElement("button");
  created.type = "button";
  created.textContent = text;
  if (action) created.setAttribute("data-action", action);
  return created;
}

export function labelledInput(form: HTMLElement, text: string, name: string, autocomplete: string): HTMLInputElement {
  const label = element(form, "label", text);
  const input = form.ownerDocument.createElement("input");
  input.name = name;
  input.setAttribute("autocomplete", autocomplete);
  label.append(input);
  form.append(label);
  return input;
}
