// Operator actions on alarms from the runtime display (specification 39, §5.7 item 4;
// specification 40, §4 and §5.1): what the alarm list's tabs, pages, buttons and rows do, the alarm
// dialog with the actions an alarm's state allows, and each action's outcome. An action is never
// sent again by itself.

import { alarmPriority, priorityName, rowStateName } from "../drawing/alarms";
import { timeOfDay } from "../drawing/format";
import type { AlarmColumn, AlarmFilter, AlarmRowModel, AlarmSummaryModel, AlarmTab } from "../drawing/model";
import type { AlarmView } from "./alarms";
import { commandReason } from "./commandReason";
import { CommandEvent, commandId } from "./commands";
import { button, element, Overlay } from "./overlay";
import type { DisplayHost } from "./runtimeDisplay";
import type { SessionControl } from "./session";
import { say, translate } from "../drawing/speech";
import type { Word } from "../drawing/words";
import { equipmentDestination } from "./equipmentNavigation";

/** A shelving duration, named in the language (§5.7, item 4). */
function durationName(seconds: number): string {
  return seconds < 3_600
    ? say("{minutes} min", { minutes: seconds / 60 })
    : say("{hours} h", { hours: seconds / 3_600 });
}

/** Each action's name, as buttons and notices say it. */
const ACTIONS: Record<string, Word> = {
  acknowledge: "Acknowledge",
  shelve: "Shelve",
  unshelve: "Unshelve",
  reset: "Reset",
  comment: "Comment",
  "remove-from-service": "Take out of service",
  "return-to-service": "Return to service",
};

/** An outcome's name, for a notice whose outcome gives no reason. */
const OUTCOMES: Record<string, Word> = {
  refused: "Not accepted",
  "timed-out": "Timed out",
  unknown: "Unknown",
};

/** What a notice says of an action's outcome: its reason, or the outcome's name. */
function outcomeSaid(outcome: string, reason?: string): string {
  if (reason) return commandReason(outcome, reason);
  const named = OUTCOMES[outcome];
  return named ? say(named) : outcome;
}

const TABS: AlarmTab[] = ["active", "shelved", "suppressed", "out_of_service"];

export class AlarmActions {
  /** What each action in flight was, by command id, for its later outcome. */
  private readonly sent = new Map<string, string>();

  constructor(
    private readonly host: DisplayHost,
    private readonly session: SessionControl,
    private readonly overlay: Overlay,
    /** SVG markup of a priority's mark, as the top bar draws it. */
    private readonly mark: (priority: string) => string = () => "",
  ) {}

  /**
   * A click in a drawn surface: a tab, a page, an action button or a row of the alarm list with
   * `objectId`. Whether it was one; `redraw` shows a changed tab or page.
   */
  click(clicked: Element | null, objectId: string | undefined, view: AlarmView, redraw: () => void): boolean {
    if (!clicked || !objectId) return false;
    // An object's equipment mark opens its alarm (§13.6, item 9).
    const equipment = clicked.closest('[data-part="equipment-alarm"]')?.getAttribute("data-alarm");
    if (equipment) {
      const row = view.summary?.active.find((candidate) => candidate.path === equipment);
      if (row) this.open(row, "active", view.summary);
      return true;
    }
    const tab = clicked.closest("[data-alarm-tab]")?.getAttribute("data-alarm-tab");
    if (tab && (TABS as string[]).includes(tab)) {
      view.views[objectId] = { ...view.views[objectId], tab: tab as AlarmTab, first: 0 };
      redraw();
      return true;
    }
    // The quick filters; the search and the equipment are asked in a dialog (§5.9 item 2).
    const chip = clicked.closest("[data-alarm-filter]");
    if (chip) {
      const current = view.views[objectId] ?? {};
      const filter: AlarmFilter = { ...current.filter };
      const kind = chip.getAttribute("data-alarm-filter");
      if (kind === "priority") {
        const value = chip.getAttribute("data-value") ?? "";
        const chosen = filter.priorities ?? [];
        filter.priorities = chosen.includes(value)
          ? chosen.filter((priority) => priority !== value)
          : [...chosen, value];
      } else if (kind === "unacknowledged") {
        filter.unacknowledged = !filter.unacknowledged;
      } else if (kind === "text") {
        this.search(objectId, view, redraw);
        return true;
      } else if (kind === "equipment") {
        this.chooseEquipment(objectId, view, redraw);
        return true;
      }
      view.views[objectId] = { ...current, first: 0, filter: kind === "clear" ? undefined : filter };
      redraw();
      return true;
    }
    // A heading sorts by its column; the same heading again reverses the order (§5.9 item 2).
    const column = clicked.closest("[data-alarm-sort]")?.getAttribute("data-alarm-sort") as
      AlarmColumn | null | undefined;
    if (column) {
      const current = view.views[objectId]?.sort;
      const reversed = current?.column === column ? !current.reversed : false;
      view.views[objectId] = { ...view.views[objectId], first: 0, sort: { column, reversed } };
      redraw();
      return true;
    }
    const page = clicked.closest("[data-page-first]")?.getAttribute("data-page-first");
    if (page !== null && page !== undefined) {
      view.views[objectId] = { ...view.views[objectId], first: Number(page) };
      redraw();
      return true;
    }
    if (clicked.closest("[data-disabled]")) return true;
    const action = clicked.closest("[data-alarm-action]");
    if (action) {
      const alarm = action.getAttribute("data-alarm") ?? "";
      const row = rowOf(view, objectId, alarm);
      void this.act(alarm, action.getAttribute("data-alarm-action") ?? "", undefined, row?.message);
      return true;
    }
    const picked = clicked.closest('[data-part="alarm-row"]')?.getAttribute("data-alarm");
    if (picked) {
      const row = rowOf(view, objectId, picked);
      if (row) this.open(row, view.views[objectId]?.tab ?? "active", view.summary);
      return true;
    }
    return false;
  }

  /** The search: the list keeps the rows holding the text while it is typed. */
  private search(objectId: string, view: AlarmView, redraw: () => void): void {
    const dialog = this.overlay.dialog("div", "alarm-search");
    dialog.append(element(dialog, "h2", say("Search")));
    const input = dialog.ownerDocument.createElement("input");
    input.type = "search";
    input.name = "search";
    input.value = view.views[objectId]?.filter?.text ?? "";
    input.addEventListener("input", () => {
      const current = view.views[objectId] ?? {};
      view.views[objectId] = { ...current, first: 0, filter: { ...current.filter, text: input.value } };
      redraw();
    });
    const close = button(dialog, say("Close"), "close");
    close.addEventListener("click", () => this.overlay.closeDialog());
    const actions = element(dialog, "div", "", "hmi-actions");
    actions.append(close);
    dialog.append(input, actions);
    input.focus();
  }

  /** The equipment of the open tab's alarms to choose from, by tag. */
  private chooseEquipment(objectId: string, view: AlarmView, redraw: () => void): void {
    const current = view.views[objectId] ?? {};
    const rows = view.summary?.[current.tab ?? "active"] ?? [];
    const equipment = [
      ...new Set(rows.map((row) => row.equipment).filter((path): path is string => Boolean(path))),
    ].sort();
    const dialog = this.overlay.dialog("div", "alarm-equipment");
    dialog.append(element(dialog, "h2", say("Equipment")));
    const choices = element(dialog, "div", "", "hmi-actions");
    for (const path of equipment) {
      const choice = button(dialog, path.split(".").pop() ?? path);
      choice.setAttribute("data-equipment", path);
      if (current.filter?.equipment === path) choice.setAttribute("aria-pressed", "true");
      choice.addEventListener("click", () => {
        this.overlay.closeDialog();
        view.views[objectId] = { ...current, first: 0, filter: { ...current.filter, equipment: path } };
        redraw();
      });
      choices.append(choice);
    }
    const close = button(dialog, say("Close"), "close");
    close.addEventListener("click", () => this.overlay.closeDialog());
    choices.append(close);
    dialog.append(choices);
  }

  /** A later outcome from the stream; whether it was one of the actions sent here. */
  receive(command: CommandEvent): boolean {
    const what = this.sent.get(command.id);
    if (what === undefined) return false;
    if (command.outcome !== "applied" && command.outcome !== "pending") this.sent.delete(command.id);
    if (command.outcome === "timed-out" || command.outcome === "unknown" || command.outcome === "refused") {
      this.overlay.notice(`${what}: ${outcomeSaid(command.outcome, command.reason)}`);
    }
    return true;
  }

  /** Sends an action for an alarm; without a session it asks to log in instead (§12.4). */
  async act(alarm: string, action: string, value?: unknown, message?: string): Promise<void> {
    if (this.session.readOnly) {
      this.session.showLogin(say("Log in to operate."));
      return;
    }
    const id = commandId();
    const named = ACTIONS[action];
    const what = `${named ? say(named) : action} ${translate(message || alarm)}`;
    this.sent.set(id, what);
    const body: Record<string, unknown> = { id, alarm, action };
    if (value !== undefined) body.value = value;
    let answer: { status: number; body: unknown };
    try {
      answer = await this.host.postJson("/api/hmi/alarm", body);
    } catch {
      // The outcome is unknown and never assumed; the action is not sent again (§12.5).
      this.sent.delete(id);
      this.overlay.notice(`${what}: ${say("The connection was lost before the outcome was known.")}`);
      return;
    }
    const reply = answer.body as { outcome?: string; reason?: string; error?: string } | null;
    if (answer.status !== 200 || !reply?.outcome) {
      this.sent.delete(id);
      this.overlay.notice(
        `${what}: ${reply?.error ?? say("The runtime answered {status}.", { status: answer.status })}`,
      );
    } else if (reply.outcome === "refused" || reply.outcome === "timed-out") {
      this.sent.delete(id);
      this.overlay.notice(`${what}: ${outcomeSaid(reply.outcome, reply.reason)}`);
    }
  }

  /** The alarm with its facts and the actions its state allows, as the runtime sends them (§5.7, item 4). */
  private open(row: AlarmRowModel, tab: AlarmTab, summary: AlarmSummaryModel | undefined): void {
    const dialog = this.overlay.dialog("div", "alarm");
    dialog.setAttribute("aria-label", row.message || row.path);
    const title = element(dialog, "h2", "", "hmi-alarm-title");
    const mark = element(title, "span", "", "hmi-alarm-mark");
    // The drawing module's own markup for a normalised priority; no program text goes in.
    mark.innerHTML = this.mark(row.priority);
    title.append(mark, element(title, "span", row.message || row.path));
    dialog.append(title);
    const facts = element(dialog, "dl", "", "hmi-facts");
    const fact = (name: string, content: string): void => {
      facts.append(element(facts, "dt", name), element(facts, "dd", content));
    };
    fact(say("Alarm"), row.path);
    fact(say("Priority"), priorityName(alarmPriority(row.priority)));
    fact(say("State"), rowStateName(row, tab));
    fact(say("Since"), timeOfDay(row.since_ms));
    fact(say("First out"), row.first_out ? say("Yes") : say("No"));
    fact(say("Raised"), String(row.raise_count));
    dialog.append(facts);

    const equipment = button(dialog, say("Show equipment"), "show-equipment");
    equipment.disabled = true;
    const equipmentReason = element(dialog, "p", say("Finding equipment…"), "hmi-hint");
    void equipmentDestination(this.host, row.equipment ?? "")
      .then((url) => {
        if (!dialog.isConnected) return;
        equipment.disabled = !url;
        equipmentReason.textContent = url ? "" : say("Equipment is not shown on a page.");
        equipmentReason.hidden = Boolean(url);
        if (url) equipment.addEventListener("click", () => this.host.navigate(url));
      })
      .catch(() => {
        if (dialog.isConnected) equipmentReason.textContent = say("Unable to find equipment.");
      });

    const actions = element(dialog, "div", "", "hmi-actions hmi-alarm-actions");
    actions.append(equipment);
    const offer = (
      action: string,
      value?: unknown,
      label = ACTIONS[action] ? say(ACTIONS[action]) : action,
    ): HTMLButtonElement => {
      const offered = button(dialog, label, action);
      offered.addEventListener("click", () => {
        this.overlay.closeDialog();
        void this.act(row.path, action, value, row.message);
      });
      return offered;
    };
    // The runtime decides which actions the alarm's state allows this session.
    const allowed = row.actions ?? [];
    for (const action of allowed) {
      if (action !== "shelve" && action !== "comment") actions.append(offer(action));
    }
    if (actions.childElementCount > 0) dialog.append(actions);
    dialog.append(equipmentReason);
    if (allowed.includes("shelve")) {
      dialog.append(element(dialog, "p", say("Shelve for"), "hmi-hint"));
      const shelve = element(dialog, "div", "", "hmi-actions hmi-shelve");
      for (const seconds of summary?.shelve_for ?? []) {
        const offered = offer("shelve", seconds, durationName(seconds));
        offered.setAttribute("data-shelve", String(seconds));
        shelve.append(offered);
      }
      dialog.append(shelve);
    }

    const footer = element(dialog, "div", "", "hmi-actions");
    const close = button(dialog, say("Close"), "cancel");
    close.addEventListener("click", () => this.overlay.closeDialog());
    footer.append(close);
    if (allowed.includes("comment")) {
      // The runtime checks the comment and answers with its reason when it refuses one.
      const comment = element(dialog, "label", say("Comment"));
      const input = dialog.ownerDocument.createElement("input");
      input.name = "comment";
      if (summary?.comment_max_chars) input.maxLength = summary.comment_max_chars;
      input.setAttribute("autocomplete", "off");
      comment.append(input);
      dialog.append(comment);
      const add = button(dialog, say("Add comment"), "comment");
      add.addEventListener("click", () => {
        this.overlay.closeDialog();
        void this.act(row.path, "comment", input.value.trim(), row.message);
      });
      footer.append(add);
    }
    dialog.append(footer);
  }
}

/** The alarm at `path` in the list's open tab. */
function rowOf(view: AlarmView, objectId: string, path: string): AlarmRowModel | undefined {
  const tab = view.views[objectId]?.tab ?? "active";
  return view.summary?.[tab].find((row) => row.path === path);
}
