import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { click, fill, Host, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §13.6 items 2, 5, 7 and 8, and specification 40, §5.1: the alarm counter and
// the newest alarm in the runtime display's top bar.

type Priority = "high" | "medium" | "low";
type Sound = "continuous" | "once" | "none";

/** How the runtime sounds an unacknowledged active alarm of each priority (§5.9 item 2). */
const SOUNDS: Record<Priority, Sound> = { high: "continuous", medium: "once", low: "none" };

function row(path: string, message: string, priority: Priority, unacknowledged: boolean, sinceMs: number) {
  return {
    path,
    message,
    priority,
    state: unacknowledged ? "ActiveUnacknowledged" : "ActiveAcknowledged",
    active: true,
    unacknowledged,
    shelved: false,
    out_of_service: false,
    suppressed: false,
    first_out: false,
    annunciating: true,
    raise_count: 1,
    since_ms: sinceMs,
    // What the runtime sends an Operator for an alarm in the active list (§5.7 item 4, §5.9 item 2).
    sound: unacknowledged ? SOUNDS[priority] : ("none" as Sound),
    actions: unacknowledged ? ["acknowledge", "shelve", "comment"] : ["shelve", "comment"],
  };
}

type Row = ReturnType<typeof row>;

function summary(active: Row[], hidden: { shelved?: Row[]; suppressed?: Row[]; out_of_service?: Row[] } = {}) {
  const newest = active.reduce<Row | null>(
    (best, next) => (!best || next.since_ms > best.since_ms ? next : best),
    null,
  );
  return {
    active,
    shelved: hidden.shelved ?? [],
    suppressed: hidden.suppressed ?? [],
    out_of_service: hidden.out_of_service ?? [],
    unacknowledged: active.filter((alarm) => alarm.unacknowledged).length,
    annunciating: active.length,
    newest,
    shelve_for: [900, 3_600, 14_400, 28_800, 86_400],
    comment_max_chars: 120,
  };
}

const OVERFLOW = row("Tank.Overflow", "Tank overflow", "high", true, Date.UTC(2026, 9, 2, 12, 30, 5));
const LEVEL = row("Tank.High", "Tank level high", "medium", false, Date.UTC(2026, 9, 2, 12, 10, 0));

function data(alarms = true): PageData {
  return {
    screen: {
      size: [1280, 720],
      theme: "dark",
      top_bar: { title: "Feed station", clock: true, alarms, user: false, page_title: true },
    },
    pages: ["overview"],
    navigation: [{ name: "overview", title: "Overview" }],
    bundle: "v1",
    page: { name: "overview", model: { title: "Overview", object: [], pipe: [] } },
    errors: [],
  };
}

/** The display with a stream that has sent its period and a first update. */
async function opened(): Promise<{ host: Host; root: HTMLElement }> {
  const host = new Host(data());
  const { root } = await openedWith(host);
  host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
  host.sockets[0].send({ cycle: 1, values: {} });
  return { host, root };
}

async function alarms(host: Host, list: ReturnType<typeof summary>): Promise<void> {
  host.sockets[0].send({ alarms: list });
  await settle(host);
}

/** The page with the top bar's alarms, and its window for clicks. */
async function openedWithWindow(): Promise<{ host: Host; root: HTMLElement; window: any }> {
  const host = new Host(data());
  const { root, window } = await openedWith(host);
  host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
  host.sockets[0].send({ cycle: 1, values: {} });
  return { host, root, window };
}

function part(root: HTMLElement, name: string): Element {
  const found = root.querySelector(`[data-part="top-bar"] [data-part="${name}"]`);
  assert.ok(found, `the top bar has ${name}`);
  return found;
}

suite("HMI display: alarms in the top bar", () => {
  test("the counter says how many alarms annunciate, in the highest priority's mark", async () => {
    const { host, root } = await opened();
    await alarms(host, summary([OVERFLOW, LEVEL]));
    const count = part(root, "alarm-count");
    assert.ok(count.textContent?.includes("2 alarms"), count.textContent ?? "");
    assert.strictEqual(count.getAttribute("data-priority"), "high");
    const mark = count.querySelector('[data-part="priority-mark"]');
    assert.strictEqual(mark?.getAttribute("data-priority"), "high", "the High mark");
    await alarms(host, summary([LEVEL]));
    assert.ok(part(root, "alarm-count").textContent?.includes("1 alarm"), part(root, "alarm-count").textContent ?? "");
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-priority"), "medium");
  });

  test("the newest alarm shows its mark, message and time", async () => {
    const { host, root } = await opened();
    await alarms(host, summary([OVERFLOW, LEVEL]));
    const newest = part(root, "alarm-newest");
    assert.ok(newest.textContent?.includes("Tank overflow"), newest.textContent ?? "");
    assert.match(newest.textContent ?? "", /\d\d:\d\d:\d\d/);
    assert.strictEqual(newest.querySelector('[data-part="priority-mark"]')?.getAttribute("data-priority"), "high");
  });

  test("with no alarm annunciating it says No alarms, and hidden alarms leave a note with their counts", async () => {
    const { host, root } = await opened();
    const shelved = { ...LEVEL, shelved: true, annunciating: false };
    const away = { ...OVERFLOW, out_of_service: true, annunciating: false };
    await alarms(host, summary([], { shelved: [shelved], out_of_service: [away, { ...away, path: "Tank.Other" }] }));
    const count = part(root, "alarm-count");
    assert.ok(count.textContent?.includes("No alarms"), count.textContent ?? "");
    assert.strictEqual(count.getAttribute("data-priority"), null);
    assert.strictEqual(root.querySelector('[data-part="alarm-newest"]')?.textContent ?? "", "", "no newest alarm");
    assert.strictEqual(part(root, "alarm-hidden").textContent, "1 shelved · 2 out of service");
  });

  test("only the counter's priority mark blinks while unacknowledged; its number remains readable", async () => {
    const { host, root } = await opened();
    await alarms(host, summary([OVERFLOW, LEVEL]));
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-blink"), "true");
    const counter = part(root, "alarm-count");
    const style = root.ownerDocument.defaultView!.getComputedStyle.bind(root.ownerDocument.defaultView);
    assert.ok(!style(counter).animation.includes("hmi-blink"), "the whole badge must not fade");
    assert.ok(
      style(counter.querySelector(".hmi-alarm-mark")!).animation.includes("hmi-blink"),
      "the priority mark still blinks",
    );
    await alarms(host, summary([LEVEL]));
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-blink"), null);
  });

  test("while the stream is closed or its updates are stale nothing blinks and the alarms are marked stale", async () => {
    const { host, root } = await opened();
    await alarms(host, summary([OVERFLOW]));
    host.advance(1000);
    await settle(host);
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-stale"), "true", "no update for four periods");
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-blink"), null);
    host.sockets[0].send({ cycle: 2, values: {} });
    await settle(host);
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-blink"), "true", "fresh again");
    host.sockets[0].onclose?.();
    await settle(host);
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-stale"), "true", "the stream closed");
    assert.strictEqual(part(root, "alarm-newest").getAttribute("data-stale"), "true");
    assert.strictEqual(part(root, "alarm-count").getAttribute("data-blink"), null);
  });
});

// Specification 39, §5.7 item 4, and specification 40, §4 and §5.1: using the alarm list.

const ERIK = { ok: true, user: "erik", role: "engineer", session: "s2", idle_seconds: 900, expires_seconds: 43200 };

function listPage(): PageData {
  return {
    screen: { size: [1280, 720], theme: "dark" },
    pages: ["alarms"],
    navigation: [{ name: "alarms", title: "Alarms" }],
    bundle: "v1",
    page: {
      name: "alarms",
      model: {
        title: "Alarms",
        object: [{ id: "list", type: "alarm-list", at: [0, 0], size: [720, 376], rotation: 0, bind: {} }],
        pipe: [],
      },
    },
    errors: [],
  };
}

const SHELVED = {
  ...row("Pump.Dry", "Pump running dry", "low", false, Date.UTC(2026, 9, 2, 11, 0, 0)),
  shelved: true,
  annunciating: false,
  actions: ["unshelve", "comment"],
};
const LATCHED = {
  ...row("Tank.Trip", "Tank trip", "high", false, Date.UTC(2026, 9, 2, 12, 20, 0)),
  latching: true,
  actions: ["reset", "shelve", "comment"],
};

async function listOpened(session: unknown = undefined): Promise<{ host: Host; root: HTMLElement; window: any }> {
  const host = new Host(listPage());
  if (session !== undefined) host.session = { status: 200, body: session };
  const opened = await openedWith(host);
  host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
  host.sockets[0].send({ cycle: 1, values: {} });
  return opened;
}

function within(root: HTMLElement, selector: string): Element {
  const found = root.querySelector(selector);
  assert.ok(found, `${selector} is drawn`);
  return found;
}

function shownRows(root: HTMLElement): string[] {
  return Array.from(root.querySelectorAll('[data-part="alarm-row"]')).map(
    (element) => element.getAttribute("data-alarm") ?? "",
  );
}

function alarmPosts(host: Host): any[] {
  return host.posts.filter((post) => post.url === "/api/hmi/alarm").map((post) => post.body);
}

function dialog(root: HTMLElement): Element {
  return within(root, '[data-part="alarm"]');
}

function buttonNamed(scope: Element, name: string): HTMLButtonElement | undefined {
  return Array.from(scope.querySelectorAll("button")).find((candidate) => candidate.textContent === name) as
    HTMLButtonElement | undefined;
}

suite("HMI display: alarm sound", () => {
  test("a High alarm sounds until acknowledged or silenced, a Medium one once, a Low one never; Silence acknowledges nothing", async () => {
    // §5.9 item 2, EEMUA 191 practice.
    const { host, root, window } = await openedWithWindow();
    const at = Date.UTC(2026, 9, 2, 12, 0, 0);
    const low = row("Line.Low", "Strainer dirty", "low", true, at);
    await alarms(host, summary([low]));
    assert.deepStrictEqual(host.played, [], "a Low alarm does not sound");
    const medium = row("Line.Mid", "Flow low", "medium", true, at + 1000);
    await alarms(host, summary([medium, low]));
    await alarms(host, summary([medium, low]));
    assert.deepStrictEqual(host.played, ["medium"], "a Medium alarm sounds once");
    assert.ok(!root.querySelector('[data-action="silence"]:not([hidden])'), "nothing to silence");
    const high = row("Line.Top", "Tank overflow", "high", true, at + 2000);
    await alarms(host, summary([high, medium, low]));
    assert.strictEqual(host.sounding, true, "a High alarm sounds");
    click(window, within(root, '[data-part="top-bar"] [data-action="silence"]'));
    await settle(host);
    assert.strictEqual(host.sounding, false, "silenced");
    assert.strictEqual(
      host.posts.filter((post) => post.url.startsWith("/api/hmi/alarm")).length,
      0,
      "nothing acknowledged",
    );
    assert.ok(!root.querySelector('[data-action="silence"]:not([hidden])'), "Silence goes once quiet");
    const second = row("Line.Top2", "Pump overload", "high", true, at + 3000);
    await alarms(host, summary([second, high, medium, low]));
    assert.strictEqual(host.sounding, true, "a High alarm raised later sounds again");
    const acknowledged = (alarm: Row): Row => ({
      ...alarm,
      unacknowledged: false,
      state: "ActiveAcknowledged",
      sound: "none",
      actions: ["shelve", "comment"],
    });
    await alarms(host, summary([acknowledged(second), acknowledged(high), medium, low]));
    assert.strictEqual(host.sounding, false, "acknowledged, it stops");
  });
});

suite("HMI display: alarm list", () => {
  test("a tab shows its alarms and the pager moves through the pages", async () => {
    const { host, root, window } = await listOpened();
    const many = Array.from({ length: 9 }, (_, index) =>
      row(`Line.A${index + 1}`, `Alarm ${index + 1}`, "low", false, index),
    );
    await alarms(host, { ...summary(many), shelved: [SHELVED] });
    assert.strictEqual(shownRows(root).length, 5);
    click(window, within(root, '[data-alarm-page="next"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Line.A6", "Line.A7", "Line.A8", "Line.A9"]);
    click(window, within(root, '[data-alarm-tab="shelved"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Pump.Dry"], "a new tab starts at its first page");
  });

  test("a heading sorts the list by its column, a second click reverses it, and a tab keeps the order", async () => {
    // §5.9 item 2.
    const { host, root, window } = await listOpened();
    await alarms(host, { ...summary([LEVEL, OVERFLOW]), shelved: [SHELVED] });
    click(window, within(root, '[data-alarm-sort="time"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Tank.Overflow", "Tank.High"], "newest first");
    click(window, within(root, '[data-alarm-sort="time"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Tank.High", "Tank.Overflow"], "reversed");
    click(window, within(root, '[data-alarm-sort="priority"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Tank.Overflow", "Tank.High"], "highest first");
    click(window, within(root, '[data-alarm-tab="shelved"]'));
    await settle(host);
    assert.strictEqual(
      within(root, '[data-alarm-sort="priority"]').getAttribute("data-sorted"),
      "natural",
      "the order stays",
    );
  });

  test("the filter chips filter the list; Search and Equipment ask in a dialog; Clear shows every alarm again", async () => {
    // §5.9 item 2.
    const { host, root, window } = await listOpened();
    const pump = {
      ...row("Plant.P101.Dry", "Pump running dry", "low", false, Date.UTC(2026, 9, 2, 12, 20, 0)),
      equipment: "Plant.P101",
    };
    await alarms(host, summary([OVERFLOW, LEVEL, pump]));
    click(window, within(root, '[data-alarm-filter="unacknowledged"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Tank.Overflow"]);
    click(window, within(root, '[data-alarm-filter="unacknowledged"]'));
    click(window, within(root, '[data-alarm-filter="priority"][data-value="medium"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Tank.High"]);
    click(window, within(root, '[data-alarm-filter="priority"][data-value="medium"]'));
    click(window, within(root, '[data-alarm-filter="text"]'));
    await settle(host);
    const search = root.querySelector('[data-part="alarm-search"] input') as HTMLInputElement;
    assert.ok(search, "a search field");
    fill(window, search, "dry");
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Plant.P101.Dry"], "it filters while typing");
    fill(window, search, "");
    click(window, within(root, '[data-part="alarm-search"] [data-action="close"]'));
    click(window, within(root, '[data-alarm-filter="equipment"]'));
    await settle(host);
    click(window, within(root, '[data-part="alarm-equipment"] [data-equipment="Plant.P101"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Plant.P101.Dry"]);
    click(window, within(root, '[data-alarm-filter="clear"]'));
    await settle(host);
    assert.deepStrictEqual(shownRows(root), ["Tank.Overflow", "Tank.High", "Plant.P101.Dry"]);
  });

  test("Acknowledge sends the action for its alarm, without opening the alarm", async () => {
    const { host, root, window } = await listOpened();
    await alarms(host, summary([OVERFLOW, LEVEL]));
    host.reply("/api/hmi/alarm", { status: 200, body: { id: "x", outcome: "applied" } });
    click(window, within(root, '[data-alarm-action="acknowledge"]'));
    await settle(host);
    const [sent] = alarmPosts(host);
    assert.deepStrictEqual(
      { alarm: sent?.alarm, action: sent?.action },
      { alarm: "Tank.Overflow", action: "acknowledge" },
    );
    assert.match(sent.id, /^c-/);
    assert.strictEqual(root.querySelector('[data-part="alarm"]'), null);
  });

  test("without a session an action asks to log in and sends nothing", async () => {
    const { host, root, window } = await listOpened({ ok: true, session: null });
    await alarms(host, summary([OVERFLOW]));
    click(window, within(root, '[data-alarm-action="acknowledge"]'));
    await settle(host);
    assert.ok(root.querySelector('[data-part="login"]'), "the login dialog");
    assert.strictEqual(alarmPosts(host).length, 0);
  });

  test("a refused action and a later unconfirmed outcome tell why", async () => {
    const { host, root, window } = await listOpened();
    await alarms(host, summary([OVERFLOW]));
    host.reply("/api/hmi/alarm", {
      status: 200,
      body: { id: "x", outcome: "refused", reason: 'the program has no alarm "Tank.Overflow"' },
    });
    click(window, within(root, '[data-alarm-action="acknowledge"]'));
    await settle(host);
    assert.ok(within(root, '[data-part="notice"]').textContent?.includes("no alarm"), "the refusal's reason");
    host.reply("/api/hmi/alarm", { status: 200, body: { id: "y", outcome: "applied" } });
    click(window, within(root, '[data-alarm-action="acknowledge"]'));
    await settle(host);
    const id = alarmPosts(host)[1].id;
    host.sockets[0].send({ command: { id, outcome: "timed-out", reason: "not confirmed in time" } });
    await settle(host);
    assert.strictEqual(
      within(root, '[data-part="notice"]').textContent,
      "Acknowledge Tank overflow: not confirmed in time",
    );
  });

  test("selecting a row opens the alarm with the actions its state allows", async () => {
    const { host, root, window } = await listOpened();
    await alarms(host, summary([OVERFLOW, LATCHED]));
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Tank.Overflow"]'));
    await settle(host);
    const opened = dialog(root);
    assert.ok(opened.textContent?.includes("Tank overflow"), opened.textContent ?? "");
    assert.strictEqual(
      opened.querySelector('h2 [data-part="priority-mark"]')?.getAttribute("data-priority"),
      "high",
      "the title carries the mark",
    );
    for (const fact of ["Tank.Overflow", "High", "Unacknowledged"])
      assert.ok(opened.textContent?.includes(fact), `${fact}: ${opened.textContent}`);
    for (const name of ["Acknowledge", "15 min", "1 h", "4 h", "8 h", "24 h", "Add comment"])
      assert.ok(buttonNamed(opened, name), name);
    assert.strictEqual(buttonNamed(opened, "Reset"), undefined, "it does not latch");
    assert.strictEqual(buttonNamed(opened, "Take out of service"), undefined, "an Operator's session");
    click(window, buttonNamed(opened, "Close") as Element);
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Tank.Trip"]'));
    await settle(host);
    assert.ok(buttonNamed(dialog(root), "Reset"), "a latching alarm can be reset");
    assert.strictEqual(buttonNamed(dialog(root), "Acknowledge"), undefined, "it is acknowledged");
  });

  test("an Engineer can take an alarm out of service and return it; a shelved alarm can be unshelved", async () => {
    const { host, root, window } = await listOpened(ERIK);
    // What the runtime sends an Engineer's session (§5.7 item 4).
    const overflow = { ...OVERFLOW, actions: ["acknowledge", "remove-from-service", "shelve", "comment"] };
    const shelved = { ...SHELVED, actions: ["unshelve", "remove-from-service", "comment"] };
    const away = { ...LEVEL, out_of_service: true, annunciating: false, actions: ["return-to-service", "comment"] };
    await alarms(host, { ...summary([overflow]), shelved: [shelved], out_of_service: [away] });
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Tank.Overflow"]'));
    await settle(host);
    host.reply("/api/hmi/alarm", { status: 200, body: { id: "x", outcome: "applied" } });
    click(window, buttonNamed(dialog(root), "Take out of service") as Element);
    await settle(host);
    assert.deepStrictEqual(
      alarmPosts(host).map((sent) => sent.action),
      ["remove-from-service"],
    );
    click(window, within(root, '[data-alarm-tab="out_of_service"]'));
    await settle(host);
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Tank.High"]'));
    await settle(host);
    assert.ok(buttonNamed(dialog(root), "Return to service"));
    click(window, buttonNamed(dialog(root), "Close") as Element);
    click(window, within(root, '[data-alarm-tab="shelved"]'));
    await settle(host);
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Pump.Dry"]'));
    await settle(host);
    assert.ok(buttonNamed(dialog(root), "Unshelve"));
    assert.strictEqual(buttonNamed(dialog(root), "1 h"), undefined, "already shelved");
    assert.ok(dialog(root).textContent?.includes("Shelved, condition active"), dialog(root).textContent ?? "");
  });

  test("shelving for an hour sends 3600 seconds and a comment sends its text", async () => {
    const { host, root, window } = await listOpened();
    await alarms(host, summary([OVERFLOW]));
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Tank.Overflow"]'));
    await settle(host);
    host.reply(
      "/api/hmi/alarm",
      { status: 200, body: { id: "x", outcome: "applied" } },
      { status: 200, body: { id: "y", outcome: "applied" } },
    );
    click(window, buttonNamed(dialog(root), "1 h") as Element);
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="alarm"]'), null, "the dialog closes");
    click(window, within(root, '[data-part="alarm-row"][data-alarm="Tank.Overflow"]'));
    await settle(host);
    // The runtime checks a comment's length and answers with its reason (specification 40, §4).
    fill(window, dialog(root).querySelector("input") as HTMLInputElement, "Level gauge checked");
    click(window, buttonNamed(dialog(root), "Add comment") as Element);
    await settle(host);
    assert.deepStrictEqual(
      alarmPosts(host).map((sent) => [sent.action, sent.value]),
      [
        ["shelve", 3600],
        ["comment", "Level gauge checked"],
      ],
    );
  });
});

// Specification 39, §5.7 item 5: the event log on the runtime display.

function eventPage(): PageData {
  return {
    screen: { size: [1280, 720], theme: "dark" },
    pages: ["events"],
    navigation: [{ name: "events", title: "Events" }],
    bundle: "v1",
    page: {
      name: "events",
      model: {
        title: "Events",
        object: [{ id: "log", type: "event-log", at: [0, 0], size: [720, 376], rotation: 0, bind: {} }],
        pipe: [],
      },
    },
    errors: [],
  };
}

function record(seq: number, kind = "raised") {
  return {
    seq,
    time_ms: Date.UTC(2026, 9, 2, 12, 0, seq),
    path: "Tank.Overflow",
    kind,
    message: "Tank overflow",
    priority: "high",
  };
}

function shownRecords(root: HTMLElement): string[] {
  return Array.from(root.querySelectorAll('[data-part="event-row"]')).map(
    (element) => element.getAttribute("data-seq") ?? "",
  );
}

suite("HMI display: event log", () => {
  test("it shows the records the runtime keeps and those the stream adds, newest first", async () => {
    const host = new Host(eventPage());
    host.alarmsBody = { alarms: summary([]), events: [record(1), record(2, "acknowledged")] };
    const { root } = await openedWith(host);
    assert.ok(host.fetched.includes("/api/hmi/alarms"), host.fetched.join(", "));
    assert.deepStrictEqual(shownRecords(root), ["2", "1"]);
    host.sockets[0].send({ events: [record(2, "acknowledged"), record(3, "cleared")] });
    await settle(host);
    assert.deepStrictEqual(shownRecords(root), ["3", "2", "1"], "a record already shown is not repeated");
  });

  test("its pager moves through the pages", async () => {
    const host = new Host(eventPage());
    host.alarmsBody = { alarms: summary([]), events: Array.from({ length: 12 }, (_, index) => record(index + 1)) };
    const { root, window } = await openedWith(host);
    assert.deepStrictEqual(shownRecords(root), ["12", "11", "10", "9", "8", "7"]);
    click(window, within(root, '[data-event-page="next"]'));
    await settle(host);
    assert.deepStrictEqual(shownRecords(root), ["6", "5", "4", "3", "2", "1"]);
  });

  test("a page without an event log does not fetch the records", async () => {
    const host = new Host(listPage());
    await openedWith(host);
    assert.ok(!host.fetched.includes("/api/hmi/alarms"), host.fetched.join(", "));
  });
});

suite("HMI display: equipment alarms", () => {
  test("selecting an object's alarm mark opens that alarm", async () => {
    const page = listPage();
    page.page!.model.object.push({
      id: "pump",
      type: "status",
      at: [760, 0],
      size: [240, 48],
      rotation: 0,
      bind: { state: { path: "Tank.P1.Running", direction: "read" } },
    });
    const host = new Host(page);
    const { root, window } = await openedWith(host);
    host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
    host.sockets[0].send({ cycle: 1, values: {} });
    const failed = {
      ...row("Tank.P1.FailedToStart", "Feed pump failed to start", "high", true, Date.UTC(2026, 9, 2, 12, 0, 0)),
      equipment: "Tank.P1",
    };
    await alarms(host, summary([failed]));
    click(window, within(root, '[data-object="pump"] [data-part="equipment-alarm"]'));
    await settle(host);
    assert.ok(dialog(root).textContent?.includes("Feed pump failed to start"), dialog(root).textContent ?? "");
    assert.ok(buttonNamed(dialog(root), "Acknowledge"));
  });
});

// Specification 39, §5.7 item 1: a trend opens with its history from the runtime's historian.

suite("HMI display: trend history", () => {
  function trendPage(): PageData {
    return {
      screen: { size: [1280, 720], theme: "dark" },
      pages: ["trends"],
      navigation: [{ name: "trends", title: "Trends" }],
      bundle: "v1",
      page: {
        name: "trends",
        model: {
          title: "Trends",
          object: [
            {
              id: "loop",
              type: "trend",
              at: [0, 0],
              size: [480, 240],
              rotation: 0,
              span: 60,
              bind: { pv: { path: "Main.T", direction: "read" }, on: { path: "Main.Lamp", direction: "read" } },
            },
          ],
          pipe: [],
        },
      },
      errors: [],
    };
  }

  function penPoints(root: HTMLElement, pen: string): number {
    const line = root.querySelector(`polyline[data-pen="${pen}"]`);
    return (line?.getAttribute("points") ?? "").trim().split(/\s+/).filter(Boolean).length;
  }

  test("a trend asks for its pens' samples over its span and draws them before the stream's", async () => {
    const host = new Host(trendPage());
    host.clock = 100_000;
    host.historyBody = {
      enabled: true,
      samples: {
        "Main.T": [
          [70_000, 1],
          [80_000, 2],
          [90_000, 3],
        ],
        "Main.Lamp": [],
      },
    };
    const { root } = await openedWith(host);
    const asked = host.fetched.find((url) => url.startsWith("/api/hmi/history"));
    assert.strictEqual(asked, "/api/hmi/history?path=Main.T&path=Main.Lamp&span=60");
    host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
    host.sockets[0].send({ cycle: 1, values: { "Main.T": { value: 4 }, "Main.Lamp": { value: true } } });
    await settle(host);
    assert.strictEqual(penPoints(root, "pv"), 4, "three from the history, one from the stream");
  });

  test("a range chip asks for that span's history and shows it", async () => {
    // §5.9 item 1.
    const host = new Host(trendPage());
    host.clock = 10_000_000;
    host.historyBody = {
      enabled: true,
      samples: {
        "Main.T": [
          [10_000_000 - 3_000_000, 1],
          [10_000_000 - 1_000_000, 2],
        ],
        "Main.Lamp": [],
      },
    };
    const { root, window } = await openedWith(host);
    host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
    host.sockets[0].send({ cycle: 1, values: { "Main.T": { value: 4 }, "Main.Lamp": { value: true } } });
    await settle(host);
    host.fetched.length = 0;
    click(window, root.querySelector('[data-object="loop"] [data-trend-range="3600"]') as Element);
    await settle(host);
    assert.ok(host.fetched.includes("/api/hmi/history?path=Main.T&path=Main.Lamp&span=3600"), host.fetched.join(", "));
    assert.strictEqual(root.querySelector('[data-trend-range="3600"]')?.getAttribute("data-selected"), "true");
    assert.ok(penPoints(root, "pv") >= 3, "the hour's history is drawn");
  });

  /** The trend page, a design pixel being a screen pixel, at a clock of 10 000 s. */
  async function trendOpened(): Promise<{ host: Host; root: HTMLElement; window: any }> {
    const host = new Host(trendPage());
    host.clock = 10_000_000;
    host.historyBody = { enabled: true, samples: { "Main.T": [], "Main.Lamp": [] } };
    const view = await openedWith(host);
    view.window.SVGElement.prototype.getBoundingClientRect = () => ({
      left: 0,
      top: 0,
      right: 1280,
      bottom: 720,
      width: 1280,
      height: 720,
      x: 0,
      y: 0,
    });
    host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
    host.sockets[0].send({ cycle: 1, values: { "Main.T": { value: 4 }, "Main.Lamp": { value: true } } });
    await settle(host);
    return { host, root: view.root, window: view.window };
  }

  function plotBox(root: HTMLElement): { x: number; y: number; width: number; height: number; element: Element } {
    const element = root.querySelector('[data-object="loop"] [data-part="plot"]') as Element;
    const number = (name: string) => Number(element.getAttribute(name));
    return { x: number("x"), y: number("y"), width: number("width"), height: number("height"), element };
  }

  function windowOf(root: HTMLElement): { start: number; end: number; paused: boolean } {
    const object = root.querySelector('[data-object="loop"]') as Element;
    const trend = object.matches(".hmi-trend") ? object : (object.querySelector(".hmi-trend") as Element);
    return {
      start: Number(trend.getAttribute("data-window-start")),
      end: Number(trend.getAttribute("data-window-end")),
      paused: trend.getAttribute("data-paused") === "true",
    };
  }

  function pointer(window: any, target: EventTarget, type: string, x: number, y: number): void {
    target.dispatchEvent(
      new window.MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0 }),
    );
  }

  test("the wheel zooms the time axis at the pointer and pauses the trend; Back to live follows the values again", async () => {
    // §5.9 item 1.
    const { host, root, window } = await trendOpened();
    const plot = plotBox(root);
    assert.deepStrictEqual(windowOf(root), { start: 10_000_000 - 60_000, end: 10_000_000, paused: false });
    plot.element.dispatchEvent(
      new window.WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        clientX: plot.x + plot.width * 0.75,
        clientY: plot.y + plot.height / 2,
        deltaY: -100,
      }),
    );
    await settle(host);
    // The time under the pointer, 15 s before now, stays three quarters across a 48 s window.
    assert.deepStrictEqual(windowOf(root), { start: 10_000_000 - 51_000, end: 10_000_000 - 3_000, paused: true });
    click(window, root.querySelector('[data-object="loop"] [data-trend-action="live"]') as Element);
    await settle(host);
    assert.deepStrictEqual(
      windowOf(root),
      { start: 10_000_000 - 48_000, end: 10_000_000, paused: false },
      "live again, keeping the zoom",
    );
  });

  test("a box dragged on the plot zooms into its period; a drag along the time row pans", async () => {
    const { host, root, window } = await trendOpened();
    const plot = plotBox(root);
    const middle = plot.y + plot.height / 2;
    pointer(window, plot.element, "pointerdown", plot.x + plot.width * 0.25, middle);
    pointer(window, window, "pointermove", plot.x + plot.width * 0.5, middle);
    await settle(host);
    assert.ok(root.querySelector('[data-object="loop"] [data-part="trend-box"]'), "the box shows while it is dragged");
    pointer(window, window, "pointerup", plot.x + plot.width * 0.5, middle);
    await settle(host);
    assert.deepStrictEqual(windowOf(root), { start: 10_000_000 - 45_000, end: 10_000_000 - 30_000, paused: true });
    const below = plot.y + plot.height + 10;
    pointer(
      window,
      plot.element.ownerDocument.querySelector('[data-object="loop"] [data-part="trend-time"]') as Element,
      "pointerdown",
      plot.x + plot.width * 0.5,
      below,
    );
    pointer(window, window, "pointermove", plot.x + plot.width * 0.75, below);
    pointer(window, window, "pointerup", plot.x + plot.width * 0.75, below);
    await settle(host);
    // A quarter of the plot to the right shows a quarter of the 15 s window earlier.
    assert.deepStrictEqual(windowOf(root), { start: 10_000_000 - 48_750, end: 10_000_000 - 33_750, paused: true });
  });

  test("the cursor follows the pointer over the plot and goes when it leaves; a legend entry hides and shows its pen", async () => {
    const { host, root, window } = await trendOpened();
    const plot = plotBox(root);
    pointer(window, plot.element, "pointermove", plot.x + plot.width / 2, plot.y + plot.height / 2);
    await settle(host);
    assert.ok(root.querySelector('[data-object="loop"] [data-part="trend-cursor"]'), "a cursor");
    pointer(window, window, "pointermove", plot.x + plot.width / 2, plot.y + plot.height + 200);
    await settle(host);
    assert.ok(!root.querySelector('[data-object="loop"] [data-part="trend-cursor"]'), "gone off the plot");
    click(window, root.querySelector('[data-object="loop"] [data-part="legend"][data-pen="pv"]') as Element);
    await settle(host);
    assert.ok(!root.querySelector('[data-object="loop"] polyline[data-pen="pv"]'), "hidden");
    click(window, root.querySelector('[data-object="loop"] [data-part="legend"][data-pen="pv"]') as Element);
    await settle(host);
    assert.ok(root.querySelector('[data-object="loop"] polyline[data-pen="pv"]'), "shown again");
  });

  test("Export saves the visible window as CSV; Expand shows the trend over the page until it is closed", async () => {
    const host = new Host(trendPage());
    host.clock = 10_000_000;
    host.historyBody = {
      enabled: true,
      samples: {
        "Main.T": [
          [10_000_000 - 90_000, 1],
          [10_000_000 - 30_000, 2],
          [10_000_000 - 10_000, 3],
        ],
        "Main.Lamp": [[10_000_000 - 20_000, 1]],
      },
    };
    const { root, window } = await openedWith(host);
    host.sockets[0].send({ period_ms: 250, stale_ms: 750 });
    await settle(host);
    click(window, root.querySelector('[data-object="loop"] [data-trend-action="export"]') as Element);
    await settle(host);
    assert.strictEqual(host.downloads.length, 1);
    const [file] = host.downloads;
    assert.match(file.name, /^loop-\d{8}-\d{6}\.csv$/);
    const lines = file.text.trim().split("\n");
    assert.strictEqual(lines[0], "time,on,pv");
    // The window is the last 60 s: the samples 30, 20 and 10 s ago; each pen holds its last value.
    assert.deepStrictEqual(
      lines.slice(1).map((line) => line.split(",").slice(1).join(",")),
      [",2", "1,2", "1,3"],
    );
    assert.match(lines[1], /^1970-01-01T02:46:10\.000Z,/);
    click(window, root.querySelector('[data-object="loop"] [data-trend-action="expand"]') as Element);
    await settle(host);
    const expanded = root.querySelector('[data-part="trend-expanded"]');
    assert.ok(expanded?.querySelector('[data-object="loop"]'), "the trend over the page");
    click(window, expanded?.querySelector('[data-action="close"]') as Element);
    await settle(host);
    assert.ok(!root.querySelector('[data-part="trend-expanded"]'), "closed");
  });

  test("a page without a trend does not ask for history", async () => {
    const host = new Host(listPage());
    await openedWith(host);
    assert.ok(!host.fetched.some((url) => url.startsWith("/api/hmi/history")), host.fetched.join(", "));
  });
});
