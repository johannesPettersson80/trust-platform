// The Settings dialog of an equipment panel (specification 39, §21.4; Plant §10). Engineers only.
// It lists the component definition's settings in order, each with its plain title and unit, its
// value in its unit and a help line only where the definition gives one. Apply and Cancel stay at
// the bottom at every window size, with the result beside Apply: "Applied", or the runtime's
// refusal naming the field. Each changed field is sent as it holds its entry, and the runtime
// reads and checks it. A refusal keeps the draft. There is no import, export or defaults
// transaction.

import type { DisplayHost } from "./runtimeDisplay";
import type { SessionControl } from "./session";
import { Overlay, element, button } from "./overlay";
import { say } from "../drawing/speech";
import { settingLabel, settingText } from "./settingValues";
import type { Setting, SettingsReply, SettingValue } from "./settingValues";
export * from "./settingValues";

export const SETTINGS_CSS = `
.hmi-dialog.hmi-settings-form { width: min(560px, 92vw); max-width: none; max-height: min(86vh, 760px); display: flex; flex-direction: column; gap: 0; padding: 0; overflow: hidden; }
.hmi-settings-form > h2 { padding: 20px 24px 12px; }
.hmi-settings-fields { flex: 1 1 auto; min-height: 0; overflow: auto; display: grid; align-content: start; gap: 16px; padding: 4px 24px 18px; }
.hmi-settings-field { display: grid; gap: 6px; }
.hmi-settings-field span { font-weight: 600; font-size: 14px; color: var(--hmi-text); }
.hmi-settings-field small { color: var(--hmi-muted); font-size: 13px; line-height: 1.4; }
.hmi-settings-field input, .hmi-settings-field select { box-sizing: border-box; min-height: 44px; width: 100%; color: var(--hmi-text); background: var(--hmi-page); border: 1px solid var(--hmi-border); border-radius: 10px; padding: 8px 12px; font: 500 16px var(--hmi-font); }
.hmi-settings-field input[type=checkbox] { width: 22px; min-height: 22px; justify-self: start; }
.hmi-settings-field [aria-invalid=true] { border-color: var(--hmi-alarm); }
.hmi-settings-footer { position: sticky; bottom: 0; display: flex; align-items: center; gap: 10px; padding: 14px 24px; border-top: 1px solid var(--hmi-border); background: var(--hmi-surface); }
.hmi-settings-result { flex: 1; min-width: 0; margin: 0; color: var(--hmi-muted); font-size: 13px; }
.hmi-settings-result[data-refused] { color: var(--hmi-alarm); }
`;

type Field = HTMLInputElement | HTMLSelectElement;

/** A setting's field: a list of its choices, a checkbox for TRUE or FALSE, else a text field (a
 * number in the setting's unit for numbers and durations). */
function field(form: HTMLElement, setting: Setting): Field {
  const document = form.ownerDocument;
  if (setting.choices?.length) {
    const select = document.createElement("select");
    select.name = setting.member;
    const options = [...setting.choices];
    // A value none of the choices holds is shown as it is.
    if (setting.value !== null && !options.some((choice) => String(choice.value) === settingText(setting.value))) {
      options.push({ value: setting.value, title: settingText(setting.value) });
    }
    for (const choice of options) {
      const option = document.createElement("option");
      option.value = String(choice.value);
      option.textContent = choice.title;
      select.append(option);
    }
    select.value = settingText(setting.value);
    return select;
  }
  const input = document.createElement("input");
  input.name = setting.member;
  if (setting.kind === "bool") {
    input.type = "checkbox";
    input.checked = setting.value === true;
    return input;
  }
  input.type = "text";
  input.value = settingText(setting.value);
  input.setAttribute("autocomplete", "off");
  if (setting.kind === "integer") input.inputMode = "numeric";
  else if (setting.kind === "real" || setting.kind === "time") input.inputMode = "decimal";
  return input;
}

/** What a field holds now. */
function read(input: Field): string | boolean {
  return input.tagName === "INPUT" && (input as HTMLInputElement).type === "checkbox"
    ? (input as HTMLInputElement).checked
    : input.value;
}

/** Puts a kept entry back into a field. */
function write(input: Field, entered: string | boolean): void {
  if (typeof entered === "boolean") (input as HTMLInputElement).checked = entered;
  else input.value = entered;
}

export class EquipmentSettings {
  constructor(
    private readonly host: DisplayHost,
    private readonly overlay: Overlay,
    private readonly session: SessionControl,
  ) {}

  /** Opens the Settings dialog of a panel popup, `faceplate:<page>/<object id>`, titled by its
   * equipment ("P101 — Pump"). */
  async open(faceplate: string, title: string): Promise<void> {
    if (this.session.readOnly) {
      this.session.showLogin(say("Log in to change settings."));
      return;
    }
    if (!this.session.mayChangeSettings) {
      this.overlay.notice(say("This needs the {role} role.", { role: say("Engineer") }));
      return;
    }
    const form = this.overlay.dialog("form", "equipment-settings");
    form.classList.add("hmi-settings-form");
    form.append(element(form, "h2", `${title} · ${say("Settings")}`));
    const fields = element(form, "div", "", "hmi-settings-fields");
    fields.setAttribute("data-part", "settings-fields");
    fields.append(element(fields, "p", say("Loading settings…"), "hmi-hint"));
    const footer = element(form, "div", "", "hmi-settings-footer");
    footer.setAttribute("data-part", "settings-footer");
    const result = element(footer, "p", "", "hmi-settings-result");
    result.setAttribute("data-part", "settings-result");
    result.setAttribute("role", "status");
    const cancel = button(footer, say("Cancel"), "cancel");
    cancel.addEventListener("click", () => this.overlay.closeDialog());
    // An edit after Applied is a draft again: closing would drop it.
    const drafting = (): void => {
      cancel.textContent = say("Cancel");
    };
    form.addEventListener("input", drafting);
    form.addEventListener("change", drafting);
    const apply = button(footer, say("Apply"), "apply");
    apply.type = "submit";
    apply.disabled = true;
    // In the HMI Builder's Live mode the values the PLC has can be kept in the project (§21.4).
    const keep = this.host.keepSettings ? button(footer, say("Keep in project"), "keep") : undefined;
    if (keep) keep.disabled = true;
    footer.append(result, cancel, ...(keep ? [keep] : []), apply);
    form.append(fields, footer);

    let shown: Setting[] = [];
    let instance: string | undefined;
    const inputs = new Map<string, Field>();
    /** What each field held when it showed the runtime's value; a field that differs is edited. */
    const showing = new Map<string, string | boolean>();
    let busy = false;
    const report = (text: string, refused = false): void => {
      result.textContent = text;
      if (refused) result.setAttribute("data-refused", "true");
      else result.removeAttribute("data-refused");
    };
    const send = async (body: Record<string, unknown>): Promise<SettingsReply> => {
      const answer = await this.host.postJson("/api/hmi/settings", { faceplate, ...body });
      const reply = answer.body as SettingsReply | null;
      if (!reply || typeof reply !== "object") throw new Error("no answer");
      return reply;
    };
    /** Shows the settings; `draft` keeps what was entered in fields that were edited. */
    const render = (settings: Setting[], draft: Map<string, string | boolean>): void => {
      shown = settings;
      inputs.clear();
      showing.clear();
      fields.replaceChildren();
      for (const setting of settings) {
        const label = element(fields, "label", "", "hmi-settings-field");
        label.setAttribute("data-member", setting.member);
        label.append(element(label, "span", settingLabel(setting)));
        const input = field(form, setting);
        showing.set(setting.member, read(input));
        const kept = draft.get(setting.member);
        if (kept !== undefined) write(input, kept);
        label.append(input);
        if (setting.help) label.append(element(label, "small", setting.help));
        fields.append(label);
        inputs.set(setting.member, input);
      }
      apply.disabled = settings.length === 0;
      if (keep) keep.disabled = settings.length === 0;
    };
    /** What was entered in the fields that were edited: those that no longer hold what they
     * showed. */
    const drafted = (): Map<string, string | boolean> => {
      const draft = new Map<string, string | boolean>();
      for (const setting of shown) {
        const input = inputs.get(setting.member);
        if (!input) continue;
        const entered = read(input);
        if (entered !== showing.get(setting.member)) draft.set(setting.member, entered);
      }
      return draft;
    };
    const load = async (draft: Map<string, string | boolean>): Promise<void> => {
      const reply = await send({ action: "read" });
      if (!form.isConnected) return;
      if (!reply.ok || !reply.settings) {
        report(reply.error ?? say("The settings cannot be read."), true);
        return;
      }
      instance = reply.instance;
      render(reply.settings, draft);
    };
    // The values the PLC has, as last read or applied; an edit not yet applied is not kept.
    keep?.addEventListener("click", () => {
      const keepSettings = this.host.keepSettings?.bind(this.host);
      if (busy || !instance || !keepSettings) return;
      const values = Object.fromEntries(shown.map((setting) => [setting.member, setting.value]));
      busy = true;
      keep.disabled = true;
      report(say("Keeping in the project…"));
      void (async () => {
        try {
          const answer = (await keepSettings(instance!, values)).body as {
            ok?: boolean;
            kept?: string[];
            error?: string;
          } | null;
          if (answer?.ok)
            report(answer.kept?.length ? say("Kept in the project.") : say("The project already has these values."));
          else report(answer?.error ?? say("The values could not be kept in the project."), true);
        } catch {
          report(say("The values could not be kept in the project."), true);
        } finally {
          busy = false;
          keep.disabled = shown.length === 0;
        }
      })();
    });
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      if (busy || !form.isConnected) return;
      // Each edited field as it holds its entry, with the value it replaces; the runtime reads
      // and checks the entries and names the field it refuses (§21.4 item 4).
      const changes: Record<string, string | boolean> = {};
      const expected: Record<string, SettingValue | null> = {};
      inputs.forEach((input) => input.removeAttribute("aria-invalid"));
      const draft = drafted();
      for (const setting of shown) {
        const entered = draft.get(setting.member);
        if (entered === undefined) continue;
        changes[setting.member] = entered;
        expected[setting.member] = setting.value;
      }
      if (Object.keys(changes).length === 0) {
        report(say("There are no changes to apply."));
        return;
      }
      busy = true;
      apply.disabled = true;
      report(say("Applying…"));
      void (async () => {
        try {
          const reply = await send({ action: "apply", changes, expected });
          if (!form.isConnected) return;
          if (reply.ok && reply.settings) {
            instance = reply.instance ?? instance;
            render(reply.settings, new Map());
            report(say("Applied"));
            // Nothing is left to apply: the dialog only closes now.
            cancel.textContent = say("Close");
            return;
          }
          // Changed meanwhile: the fields not edited show the values now; the edits stay.
          if (reply.changed) await load(drafted());
          const input = reply.field ? inputs.get(reply.field) : undefined;
          input?.setAttribute("aria-invalid", "true");
          input?.focus();
          report(reply.error ?? say("The settings were not applied."), true);
        } catch {
          report(`${say("The runtime cannot be reached.")} ${say("Check the values before another change.")}`, true);
        } finally {
          busy = false;
          apply.disabled = shown.length === 0;
        }
      })();
    });
    try {
      await load(new Map());
    } catch {
      report(say("The runtime cannot be reached."), true);
    }
  }
}
