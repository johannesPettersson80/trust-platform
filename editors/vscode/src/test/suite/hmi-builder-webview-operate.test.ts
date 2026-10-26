import * as assert from "assert";
import { Webview } from "./hmiBuilderWebview";
import type { AlarmRowModel, AlarmSummaryModel, HmiPageModel } from "../../hmiBuilder/drawing/model";

const wait = (): Promise<void> => new Promise((done) => setTimeout(done, 100));
/** The tank example's alarm page: an alarm list at its catalogue size (§5.7 item 4). */
const ALARM_PAGE: HmiPageModel = {
  title: "Alarms",
  pipe: [],
  object: [{ id: "alarm-list-1", type: "alarm-list", at: [40, 40], size: [720, 376], rotation: 0, bind: {} }],
};
/** The tank example's high level alarm, active and unacknowledged, as the runtime lists it
 * (specification 40, §5.1). */
const LEVEL_HIGH: AlarmRowModel = {
  path: "Tank.Vessel.High",
  message: "T-101 level high",
  priority: "medium",
  state: "ActiveUnacknowledged",
  active: true,
  unacknowledged: true,
  shelved: false,
  out_of_service: false,
  suppressed: false,
  first_out: false,
  annunciating: true,
  raise_count: 1,
  since_ms: Date.UTC(2026, 9, 26, 12, 0, 0),
  equipment: "Tank.Vessel",
};
const ALARMS: AlarmSummaryModel = {
  active: [LEVEL_HIGH],
  shelved: [],
  suppressed: [],
  out_of_service: [],
  unacknowledged: 1,
  annunciating: 1,
  newest: LEVEL_HIGH,
};

/** Live mode opened on the alarm page; the id of the operating connection it asks the host for. */
async function openLive(view: Webview): Promise<string> {
  await view.loaded(ALARM_PAGE);
  view.send({ type: "pages", pages: [{ name: "alarms", title: "Alarms", hidden: false }], current: "alarms" });
  view.query('[data-action="live"]').click();
  await wait();
  const open = view.posted.find((message) => message.type === "operateOpen");
  assert.ok(open, "Live requests a named operating connection for this page");
  return open.id as string;
}
const PAGE: HmiPageModel = {
  title: "Cell",
  pipe: [],
  object: [
    {
      id: "start",
      type: "button",
      at: [40, 40],
      size: [140, 44],
      rotation: 0,
      text: "Start",
      confirm: "Start the pump?",
      bind: { target: { path: "Main.Run", direction: "write" } },
    },
    {
      id: "photo",
      type: "image",
      at: [300, 40],
      size: [100, 100],
      rotation: 0,
      image: "hmi/assets/cell.png",
      bind: {},
    },
  ],
};

suite("HMI Builder editor: named Live operation", function () {
  this.timeout(15000);
  test("Live uses the runtime login, confirmation and command path, and ends when Design returns", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "pages", pages: [{ name: "overview", title: "Cell", hidden: false }], current: "overview" });
    view.query('[data-action="live"]').click();
    await wait();
    const open = view.posted.find((message) => message.type === "operateOpen");
    assert.ok(open, "Live requests a named operating connection for this page");
    const id = open.id as string;
    view.send({ type: "live", state: "connected", target: "Cell runtime" });
    view.send({ type: "operateReady", id, target: "Cell runtime" });
    const handled = new Set<number>();
    let user: string | undefined;
    const answer = async (): Promise<void> => {
      for (let round = 0; round < 5; round++) {
        await wait();
        for (const message of view.posted.filter((entry) => entry.type === "operateRequest")) {
          const request = message.request as number;
          if (handled.has(request)) continue;
          handled.add(request);
          const path = message.path as string;
          if (path === "/api/hmi/login") user = "anna";
          const body = path.startsWith("/api/hmi/page")
            ? {
                screen: { size: [960, 640], top_bar: { title: "Cell", user: true, logo: "hmi/assets/logo.svg" } },
                page: {
                  name: new URL(path, "http://live.invalid").searchParams.get("name") ?? "overview",
                  model: PAGE,
                },
                pages: ["overview", "detail"],
                navigation: [
                  { name: "overview", title: "Cell" },
                  { name: "detail", title: "Detail" },
                ],
                bundle: "v1",
                roles: { start: "operator" },
                errors: [],
              }
            : path === "/api/hmi/command"
              ? { outcome: "applied" }
              : user
                ? { ok: true, user, role: "operator" }
                : { ok: true, session: null };
          view.send({ type: "operateReply", id, request, status: 200, body });
        }
      }
    };
    await answer();
    const root = view.query('[data-part="live-operating"]');
    assert.ok(root, "the runtime display replaces the design canvas");
    assert.ok(!view.query(".hb-zoom"), "design zoom is not offered for the runtime layout");
    assert.doesNotMatch(
      view.query('[data-part="live-status"]').textContent,
      /operate in the runtime display/i,
      "the status describes in-editor operation",
    );
    assert.strictEqual(
      root.querySelector('[data-object="photo"] image').getAttribute("href"),
      "https://assets.test/hmi/assets/cell.png",
      "Live draws project images from the editor asset root",
    );
    assert.strictEqual(
      root.querySelector(".hmi-brand img").src,
      "https://assets.test/hmi/assets/logo.svg",
      "the runtime logo uses the same project assets",
    );
    root.querySelector('[data-object="start"]').dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
    await wait();
    const form = root.querySelector('[data-part="login"]');
    assert.ok(form, "operating asks for a named login");
    form.querySelector('[name="user"]').value = "anna";
    form.querySelector('[name="password"]').value = "test-only";
    form.dispatchEvent(new view.window.Event("submit", { bubbles: true, cancelable: true }));
    await answer();
    assert.match(root.textContent, /anna/);
    root.querySelector('[data-object="start"]').dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
    await wait();
    assert.match(root.querySelector('[data-part="confirm"]').textContent, /Start the pump/);
    assert.ok(
      !view.posted.some((message) => message.path === "/api/hmi/command"),
      "nothing is sent before confirmation",
    );
    root.querySelector('[data-action="confirm"]').click();
    await answer();
    const commands = view.posted.filter((message) => message.path === "/api/hmi/command");
    assert.strictEqual(commands.length, 1);
    assert.strictEqual(root.querySelector('[data-object="start"]').getAttribute("data-outcome"), "applied");
    assert.strictEqual((commands[0].body as { bundle: string }).bundle, "v1");
    root.querySelector('[data-action="theme"]').click();
    assert.ok(root.querySelector(".hmi-theme-dark"));
    root.querySelector('a[href="/hmi/detail"]').click();
    await answer();
    assert.strictEqual(
      view.posted.filter((message) => message.type === "operateOpen").length,
      1,
      "navigation keeps the same named connection",
    );
    assert.ok(
      view.posted.some((message) => typeof message.path === "string" && message.path.includes("name=detail")),
      "navigation loads the selected runtime page inside Live mode",
    );
    assert.match(view.query('[data-part="live-operating"]').textContent, /anna/);
    assert.ok(
      view.query('[data-part="live-operating"] .hmi-theme-dark'),
      "the chosen theme lasts across page navigation",
    );
    const streams = view.posted.filter((message) => message.type === "operateSocket");
    view.send({
      type: "operateStream",
      id,
      socket: streams[streams.length - 1].socket as number,
      data: JSON.stringify({ bundle: "v2" }),
    });
    await wait();
    const reads = () =>
      view.posted.filter((message) => message.type === "operateRequest" && String(message.path).includes("name=detail"))
        .length;
    const beforeReload = reads();
    view.query('[data-part="live-operating"] [data-action="reload"]').click();
    await answer();
    assert.strictEqual(reads(), beforeReload + 1, "Reload loads the current address again");
    view.query('[data-action="design"]').click();
    await wait();
    assert.ok(view.posted.some((message) => message.type === "operateClose" && message.id === id));
    assert.strictEqual(view.query('[data-part="live-operating"]'), null);
  });

  test("live alarm list on the simulator says where alarms show instead of zero counts", async () => {
    // §11 item 6: the simulator has no HMI web server, so Live mode has no alarm source. Caught:
    // "Active 0" and "No active alarms" while the program's alarm is active.
    const view = new Webview();
    const id = await openLive(view);
    const reason =
      "The simulator has no HMI web server. Operate the page, with its alarms, on a runtime: select or connect one in Devices & Connections.";
    view.send({ type: "live", state: "connected", target: "Simulator" });
    view.send({ type: "operateReady", id, reason });
    view.send({ type: "liveValues", update: { cycle: 1, values: {}, missing: [] } });
    await wait();
    const list = view.query('svg [data-object="alarm-list-1"]');
    assert.ok(list, "the canvas draws the page's alarm list");
    assert.strictEqual(
      list.querySelector('[data-part="alarm-unavailable"]')?.textContent,
      "Alarms show in the runtime display of a runtime with its HMI web server.",
    );
    assert.strictEqual(
      list.querySelectorAll('[data-part="alarm-tab"], [data-part="alarm-filter"], [data-part="alarm-heading"]').length,
      0,
      "no tabs with counts, no filters and no headings that suggest data",
    );
    assert.doesNotMatch(list.textContent, /Active 0|No active alarms/);
    assert.strictEqual(view.query(".hb-live-reason")?.textContent, `Read-only · ${reason}`);
  });

  test("live alarm list on a runtime with an HMI web server is the runtime display's list", async () => {
    // §11 item 6 and §13.6 item 3: the runtime's web server is the alarm source. Caught: the
    // sentence for a target without one shown in place of the alarms the runtime sends.
    const view = new Webview();
    const id = await openLive(view);
    view.send({ type: "live", state: "connected", target: "Cell runtime" });
    view.send({ type: "operateReady", id, target: "Cell runtime" });
    // The runtime display reads the page and the session, then opens its value stream (§11.1).
    const answered = new Set<number>();
    for (let round = 0; round < 5; round++) {
      await wait();
      for (const message of view.posted.filter((entry) => entry.type === "operateRequest")) {
        const request = message.request as number;
        if (answered.has(request)) continue;
        answered.add(request);
        const body = String(message.path).startsWith("/api/hmi/page")
          ? {
              screen: { size: [960, 640] },
              page: { name: "alarms", model: ALARM_PAGE },
              pages: ["alarms"],
              bundle: "v1",
              errors: [],
            }
          : { ok: true, session: null };
        view.send({ type: "operateReply", id, request, status: 200, body });
      }
    }
    const stream = view.posted.filter((message) => message.type === "operateSocket").pop();
    assert.ok(stream, "the runtime display opens its value stream");
    view.send({ type: "operateStream", id, socket: stream.socket as number, data: JSON.stringify({ alarms: ALARMS }) });
    await wait();
    const list = view.query('[data-part="live-operating"] [data-object="alarm-list-1"]');
    assert.ok(list, "the runtime display draws the page's alarm list");
    assert.deepStrictEqual(
      Array.from(list.querySelectorAll('[data-part="alarm-tab"]'), (tab: any) => tab.textContent),
      ["Active 1", "Shelved 0", "Suppressed 0", "Out of service 0"],
    );
    assert.match(list.querySelector('[data-part="alarm-row"]')?.textContent ?? "", /T-101 level high/);
    assert.strictEqual(list.querySelector('[data-part="alarm-unavailable"]'), null);
  });
});
