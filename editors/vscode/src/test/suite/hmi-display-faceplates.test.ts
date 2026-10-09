import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { ANNA, click, commands, Host, object, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §8.7 item 5: a click on an object with a faceplate opens it over the page,
// named by the page and the object, with its description under its title; its stream and its
// commands name it the same way.

const PAGE: PageData = {
  screen: { size: [1280, 720], theme: "dark" },
  pages: ["overview"],
  navigation: [{ name: "overview", title: "Overview" }],
  bundle: "v1",
  page: {
    name: "overview",
    model: {
      title: "Overview",
      object: [
        {
          id: "p-101",
          type: "pump",
          at: [40, 40],
          size: [128, 108],
          rotation: 0,
          tag: "P-101",
          bind: { running: { path: "Main.P101.Running", direction: "read" } },
          click: { faceplate: "pump" },
        },
      ],
      pipe: [],
    },
  },
  errors: [],
};

const FACEPLATE = {
  popup: {
    name: "faceplate:overview/p-101",
    model: {
      title: "Pump",
      description: "Transfer pump",
      size: [400, 300],
      object: [
        {
          id: "speed",
          type: "value",
          at: [24, 24],
          size: [200, 48],
          rotation: 0,
          bind: { value: { path: "Main.P101.ActualSpeed", direction: "read" } },
        },
        {
          id: "start",
          type: "button",
          at: [24, 100],
          size: [120, 44],
          rotation: 0,
          text: "Start",
          bind: { target: { path: "Main.P101.StartCmd", direction: "write" } },
        },
      ],
      pipe: [],
      fields: { Speed: { accepts: ["number"] }, StartCmd: { accepts: ["boolean"] } },
    },
  },
  errors: [],
  bundle: "v1",
};

suite("HMI display: faceplates", () => {
  /** The pump's panel as the runtime serves it: titled by its equipment, with settings. */
  const equipment = (): unknown => ({
    ...FACEPLATE,
    popup: {
      ...FACEPLATE.popup,
      model: {
        ...FACEPLATE.popup.model,
        title: "P101 — Pump",
        description: undefined,
        settings: true,
        settings_instance: "Main.P101",
      },
    },
  });

  test("an equipment faceplate is titled by its tag and component as the runtime serves it", async () => {
    // §8.4 item 1: "P101 — Pump".
    const host = new Host(PAGE);
    host.popups["faceplate:overview/p-101"] = equipment();
    const { root, window } = await openedWith(host);
    click(window, object(root, "p-101"));
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="popup-title"]')?.textContent, "P101 — Pump");
    assert.strictEqual(root.querySelector('[data-part="popup"]')?.getAttribute("aria-label"), "P101 — Pump");
  });

  test("the Settings button is shown to an Engineer and never to an Operator", async () => {
    // §8.4 item 6, §21.4 item 1.
    const operator = new Host(PAGE);
    operator.popups["faceplate:overview/p-101"] = equipment();
    const asOperator = await openedWith(operator);
    click(asOperator.window, object(asOperator.root, "p-101"));
    await settle(operator);
    assert.ok(asOperator.root.querySelector('[data-part="popup"]'), "the panel opens");
    assert.strictEqual(
      asOperator.root.querySelector('[data-part="popup"] [data-action="settings"]'),
      null,
      "no Settings for an Operator",
    );
    const engineer = new Host(PAGE);
    engineer.session = { status: 200, body: { ...ANNA, user: "erik", role: "engineer" } };
    engineer.popups["faceplate:overview/p-101"] = equipment();
    engineer.reply("/api/hmi/settings", { status: 200, body: { ok: true, instance: "Main.P101", settings: [] } });
    const asEngineer = await openedWith(engineer);
    click(asEngineer.window, object(asEngineer.root, "p-101"));
    await settle(engineer);
    const settings = asEngineer.root.querySelector('[data-part="popup"] [data-action="settings"]');
    assert.ok(settings, "Settings in the header for an Engineer");
    click(asEngineer.window, settings);
    await settle(engineer);
    assert.deepStrictEqual(engineer.posts.find((post) => post.url === "/api/hmi/settings")?.body, {
      faceplate: "faceplate:overview/p-101",
      action: "read",
    });
    assert.ok(
      asEngineer.root.querySelector('[data-part="equipment-settings"]')?.textContent?.includes("P101 — Pump"),
      "the dialog names the equipment",
    );
  });
  test("a faceplate draws with its served project format", async () => {
    const host = new Host(PAGE);
    host.popups["faceplate:overview/p-101"] = {
      ...FACEPLATE,
      formats: { "main.p101.actualspeed": { unit: "rpm", decimals: 1 } },
    };
    const { root, window } = await openedWith(host);
    click(window, object(root, "p-101"));
    await settle(host);
    host.sockets[1].send({ cycle: 1, values: { "Main.P101.ActualSpeed": { value: 1460 } } });
    await settle(host);
    const text = root.querySelector('[data-part="popup"] [data-object="speed"]')?.textContent ?? "";
    assert.ok(text.includes("1460.0") && text.includes("rpm"), text);
  });
  test("a click on an object with a faceplate opens it named by the page and the object; its stream and commands name it so", async () => {
    const host = new Host(PAGE);
    host.popups["faceplate:overview/p-101"] = FACEPLATE;
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
    const { root, window } = await openedWith(host);
    click(window, object(root, "p-101"));
    await settle(host);
    assert.ok(host.fetched.includes("/api/hmi/popup?faceplate=overview%2Fp-101"), host.fetched.join(", "));
    const popup = root.querySelector('[data-part="popup"]');
    assert.ok(popup, "the faceplate opens over the page");
    assert.strictEqual(popup.querySelector('[data-part="popup-title"]')?.textContent, "Pump");
    assert.strictEqual(popup.querySelector('[data-part="popup-description"]')?.textContent, "Transfer pump");
    assert.match(host.socketUrls[host.socketUrls.length - 1], /\/ws\/hmi\?faceplate=overview%2Fp-101$/);
    host.sockets[host.sockets.length - 1].send({ cycle: 1, values: { "Main.P101.ActualSpeed": { value: 1460 } } });
    await settle(host);
    assert.match(popup.querySelector('[data-object="speed"]')?.textContent ?? "", /1460/);
    click(window, popup.querySelector('[data-object="start"]') as Element);
    await settle(host);
    assert.deepStrictEqual(
      commands(host).map((command) => [command.page, command.object]),
      [["faceplate:overview/p-101", "start"]],
    );
  });
  test("live faceplate updates retain the control being clicked", async () => {
    const host = new Host(PAGE);
    host.popups["faceplate:overview/p-101"] = FACEPLATE;
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
    const { root, window } = await openedWith(host);
    click(window, object(root, "p-101"));
    await settle(host);
    const popup = root.querySelector('[data-part="popup"]')!;
    const start = popup.querySelector('[data-object="start"]')!;
    for (let cycle = 1; cycle <= 8; cycle++) {
      host.sockets[1].send({ cycle, values: { "Main.P101.ActualSpeed": { value: 1400 + cycle } } });
      await settle(host);
      assert.strictEqual(
        popup.querySelector('[data-object="start"]'),
        start,
        "a value update must not detach a control between pointer down and click",
      );
    }
    assert.match(popup.querySelector('[data-object="speed"]')?.textContent ?? "", /1408/);
    click(window, start);
    await settle(host);
    assert.strictEqual(commands(host).length, 1);
  });
  test("ending the display also closes its faceplates and their streams", async () => {
    const host = new Host(PAGE);
    host.popups["faceplate:overview/p-101"] = FACEPLATE;
    const lifetime = new AbortController();
    const { root, window } = await openedWith(host, lifetime.signal);
    click(window, object(root, "p-101"));
    await settle(host);
    assert.strictEqual(host.sockets.length, 2);
    lifetime.abort();
    assert.ok(
      host.sockets.every((socket) => socket.closed),
      "all page and faceplate streams close",
    );
    assert.strictEqual(root.querySelector('[data-part="popup"]'), null);
    host.advance(5000);
    await settle(host);
    assert.strictEqual(host.sockets.length, 2, "no popup reconnects");
  });
});
