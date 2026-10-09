import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { ANNA, click, commands, Host, object, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §4.6–4.9: the top bar, the navigation, page buttons and popups on the runtime
// display.

const NAVIGATION = [
  { name: "overview", title: "Overview", icon: "overview" },
  { name: "alarms", title: "Alarms", icon: "alarm" },
  { name: "utilities", title: "Utilities", group: "Plant" },
  { name: "zeta", title: "Zeta" },
];

function data(screen: Partial<NonNullable<PageData["screen"]>> = {}, navigation = NAVIGATION): PageData {
  return {
    screen: { size: [1280, 720], theme: "dark", ...screen },
    pages: navigation.map((page) => page.name),
    navigation,
    bundle: "v1",
    page: {
      name: "alarms",
      model: {
        title: "Alarms",
        object: [
          {
            id: "more",
            type: "page-button",
            at: [40, 40],
            size: [160, 44],
            rotation: 0,
            text: "Utilities",
            bind: {},
            click: { page: "utilities" },
          },
          {
            id: "pump",
            type: "value",
            at: [40, 120],
            size: [200, 96],
            rotation: 0,
            label: "Pump",
            bind: { value: { path: "Main.P101.Speed", direction: "read" } },
            click: { popup: "pump" },
          },
        ],
        pipe: [],
      },
    },
    errors: [],
  };
}

const PUMP = {
  popup: {
    name: "pump",
    model: {
      title: "Pump P101",
      size: [400, 240],
      pinnable: true,
      object: [
        {
          id: "speed",
          type: "value",
          at: [24, 24],
          size: [200, 96],
          rotation: 0,
          bind: { value: { path: "Main.P101.Speed", direction: "read" } },
        },
        {
          id: "start",
          type: "button",
          at: [24, 140],
          size: [120, 44],
          rotation: 0,
          text: "Start",
          bind: { target: { path: "Main.Start", direction: "write" } },
        },
      ],
      pipe: [],
    },
  },
  errors: [],
  bundle: "v1",
};

async function opened(page: PageData): Promise<{ host: Host; root: HTMLElement; window: any }> {
  const host = new Host(page);
  host.popups.pump = PUMP;
  return openedWith(host);
}

suite("HMI display: top bar, navigation and popups", () => {
  test("the top bar shows the title, the page title, the clock and the session", async () => {
    const { root } = await opened(
      data({ top_bar: { title: "Feed station", clock: true, alarms: false, user: true, page_title: true } }),
    );
    const bar = root.querySelector('[data-part="top-bar"]');
    assert.ok(bar, "a top bar");
    assert.ok(bar.textContent?.includes("Feed station"), bar.textContent ?? "");
    assert.strictEqual(bar.querySelector('[data-part="page-title"]')?.textContent, "Alarms");
    assert.match(bar.querySelector('[data-part="clock"]')?.textContent ?? "", /^\d\d:\d\d:\d\d$/);
    assert.ok(
      bar.querySelector('[data-part="session"]')?.textContent?.includes(ANNA.user),
      "the session is in the top bar",
    );
  });

  test("the session in the top bar sits in the bar's flow, not floating over the clock", async () => {
    const { root, window } = await opened(
      data({ top_bar: { title: "Feed station", clock: true, alarms: true, user: true, page_title: true } }),
    );
    const session = root.querySelector('[data-part="top-bar"] [data-part="session"]') as Element;
    assert.strictEqual(window.getComputedStyle(session).position, "static");
  });

  test("the side bar lists the pages in order with their group, marks the open one and links to them", async () => {
    const { root } = await opened(data({ navigation: { style: "side", position: "right" } }));
    const side = root.querySelector('[data-part="navigation"]');
    assert.ok(side, "a side bar");
    assert.strictEqual(side.getAttribute("data-style"), "side");
    assert.strictEqual(side.getAttribute("data-position"), "right");
    const links = [...side.querySelectorAll("a")];
    assert.deepStrictEqual(
      links.map((link) => link.getAttribute("href")),
      ["/hmi/overview", "/hmi/alarms", "/hmi/zeta", "/hmi/utilities"],
      "the group's pages come together after the others",
    );
    assert.deepStrictEqual(
      links.map((link) => link.getAttribute("aria-current")),
      [null, "page", null, null],
    );
    assert.ok(links[0].querySelector('svg[data-icon="overview"]'), "a page's icon");
    const heading = side.querySelector('[data-part="group"]');
    assert.strictEqual(heading?.textContent, "Plant");
  });

  test("tabs list the pages under the top bar, and one page alone needs no navigation", async () => {
    const tabs = await opened(data({ navigation: { style: "tabs", position: "left" } }));
    const navigation = tabs.root.querySelector('[data-part="navigation"]');
    assert.strictEqual(navigation?.getAttribute("data-style"), "tabs");
    assert.strictEqual(navigation?.querySelectorAll("a").length, 4);
    const alone = await opened(data({ navigation: { style: "side", position: "left" } }, [NAVIGATION[1]]));
    assert.strictEqual(alone.root.querySelector('[data-part="navigation"]'), null);
  });

  test("a page button goes to its page", async () => {
    const { host, root, window } = await opened(data());
    click(window, object(root, "more"));
    assert.deepStrictEqual(host.navigations, ["/hmi/utilities"]);
  });

  test("a popup opens over the page with its own values and closes with Escape", async () => {
    const { host, root, window } = await opened(data());
    click(window, object(root, "pump"));
    await settle(host);
    const popup = root.querySelector('[data-part="popup"]');
    assert.ok(popup, "the popup");
    assert.strictEqual(popup.querySelector('[data-part="popup-title"]')?.textContent, "Pump P101");
    assert.ok(root.querySelector('[data-part="popup-backdrop"]'), "modal: the page is dimmed");
    assert.match(host.socketUrls[host.socketUrls.length - 1], /\/ws\/hmi\?popup=pump$/);
    host.sockets[host.sockets.length - 1].send({ cycle: 1, values: { "Main.P101.Speed": { value: 1460 } } });
    await settle(host);
    assert.match(popup.querySelector('[data-object="speed"]')?.textContent ?? "", /1460/);
    window.document.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="popup"]'), null, "closed");
    assert.ok(host.sockets[host.sockets.length - 1].closed, "its stream is closed");
  });

  test("a popup's command names the popup, and a pinned popup lets the page be used", async () => {
    const { host, root, window } = await opened(data());
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
    click(window, object(root, "pump"));
    await settle(host);
    const popup = root.querySelector('[data-part="popup"]') as Element;
    click(window, popup.querySelector('[data-object="start"]') as Element);
    await settle(host);
    assert.deepStrictEqual(
      commands(host).map((command) => [command.page, command.object, command.action]),
      [["popup:pump", "start", "set"]],
    );
    click(window, popup.querySelector('[data-action="pin"]') as Element);
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="popup-backdrop"]'), null, "pinned: the page is not dimmed");
    assert.strictEqual(popup.getAttribute("data-pinned"), "true");
    click(window, popup.querySelector('[data-action="close"]') as Element);
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="popup"]'), null, "closed by its button");
  });
});
