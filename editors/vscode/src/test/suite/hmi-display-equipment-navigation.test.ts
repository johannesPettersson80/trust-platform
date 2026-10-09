import * as assert from "assert";
import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { click, Host, openedWith, settle } from "./hmiDisplayHost";

const ALARM = {
  path: "Main.Tank.High",
  equipment: "Main.Tank",
  message: "High level",
  priority: "high",
  state: "ActiveUnacknowledged",
  active: true,
  unacknowledged: true,
  shelved: false,
  suppressed: false,
  out_of_service: false,
  first_out: false,
  annunciating: true,
  raise_count: 1,
  since_ms: 0,
};

function page(name: "alarms" | "overview"): PageData {
  return {
    screen: { size: [1280, 720], theme: "dark" },
    pages: ["alarms", "overview"],
    bundle: "v1",
    errors: [],
    page: {
      name,
      model: {
        title: name,
        pipe: [],
        object:
          name === "alarms"
            ? [{ id: "list", type: "alarm-list", at: [0, 0], size: [1000, 600], rotation: 0, bind: {} }]
            : [
                {
                  id: "tank",
                  type: "tank",
                  at: [40, 40],
                  size: [160, 240],
                  rotation: 0,
                  bind: { level: { path: "Main.Tank.Level", direction: "read" } },
                  click: { faceplate: "tank", instance: "Main.Tank" },
                },
              ],
      },
    },
  };
}

async function alarmPage(equipment = "Main.Tank") {
  const host = new Host(page("alarms"));
  host.location.pathname = "/hmi/alarms";
  const fetch = host.fetchJson.bind(host);
  host.fetchJson = async (url) =>
    url.includes("name=overview") ? { status: 200, body: page("overview") } : fetch(url);
  const opened = await openedWith(host);
  const alarm = { ...ALARM, equipment };
  host.sockets[0].send({
    alarms: {
      active: [alarm],
      shelved: [],
      suppressed: [],
      out_of_service: [],
      unacknowledged: 1,
      annunciating: 1,
      newest: alarm,
    },
  });
  await settle(host);
  click(opened.window, opened.root.querySelector('[data-part="alarm-row"]')!);
  await settle(host);
  return opened;
}

suite("HMI display: Show equipment", () => {
  test("alarm details stay open until Show equipment navigates to the bound symbol without a command", async () => {
    const { host, root, window } = await alarmPage();
    try {
      assert.ok(root.querySelector('[data-part="alarm"]'), "row click still opens details");
      const button = root.querySelector<HTMLButtonElement>('[data-action="show-equipment"]');
      assert.ok(button, "alarm details offer Show equipment");
      assert.strictEqual(button.disabled, false);
      assert.deepStrictEqual(host.navigations, []);
      click(window, button);
      assert.deepStrictEqual(host.navigations, ["/hmi/overview?focus=tank"]);
      assert.deepStrictEqual(host.posts, [], "navigation never acknowledges or writes");
    } finally {
      window.close();
    }
  });

  test("an equipment instance not shown on a page has a disabled action with a reason", async () => {
    const { root, window } = await alarmPage("Main.Missing");
    try {
      const button = root.querySelector<HTMLButtonElement>('[data-action="show-equipment"]');
      assert.ok(button, "the action remains discoverable");
      assert.ok(button.disabled);
      assert.ok(root.querySelector('[data-part="alarm"]')?.textContent?.includes("Equipment is not shown on a page."));
    } finally {
      window.close();
    }
  });

  test("the destination highlights the requested object temporarily", async () => {
    const host = new Host(page("overview"));
    Object.assign(host.location, { pathname: "/hmi/overview", search: "?focus=tank" });
    const { root, window } = await openedWith(host);
    try {
      assert.ok(
        root.querySelector('[data-object="tank"][data-focused="true"]'),
        "the requested equipment is highlighted",
      );
      host.advance(5000);
      await settle(host);
      assert.ok(!root.querySelector('[data-focused="true"]'), "the highlight clears");
    } finally {
      window.close();
    }
  });
});
