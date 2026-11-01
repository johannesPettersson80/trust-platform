import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import { click, commands, Host, object, openedWith, settle } from "./hmiDisplayHost";

// Specification 39, §5.8: the switch, selector, mode selector and slider on the runtime page.

const CONTROLS: PageData = {
  screen: { size: [1280, 720], theme: "dark" },
  pages: ["controls"],
  bundle: "v1",
  page: {
    name: "controls",
    model: {
      title: "Controls",
      object: [
        {
          id: "fan",
          type: "switch",
          at: [40, 40],
          size: [80, 40],
          rotation: 0,
          bind: { value: { path: "Main.Fan", direction: "read-write" } },
        },
        {
          id: "speed",
          type: "selector",
          at: [40, 100],
          size: [200, 44],
          rotation: 0,
          label: "Fan speed",
          options: ["Off", "Low", "High"],
          bind: { value: { path: "Main.Speed", direction: "read-write" } },
        },
        {
          id: "mode",
          type: "mode-selector",
          at: [40, 160],
          size: [240, 44],
          rotation: 0,
          bind: { mode: { path: "Loop.Mode", direction: "read-write" } },
        },
        {
          id: "valve",
          type: "slider",
          at: [40, 300],
          size: [240, 44],
          rotation: 0,
          min: 0,
          max: 80,
          bind: { value: { path: "Main.Valve", direction: "read-write" } },
        },
      ],
      pipe: [],
    },
  },
  errors: [],
};

const VALUES = {
  cycle: 1,
  values: {
    "Main.Fan": { value: false },
    "Main.Speed": { value: "Low" },
    "Loop.Mode": { value: "Auto" },
    "Main.Valve": { value: 40 },
  },
};

async function opened(values: unknown = VALUES): Promise<{ host: Host; root: HTMLElement; window: any }> {
  const host = new Host(CONTROLS);
  host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
  const view = await openedWith(host);
  // The page fills the design size, so a design pixel is a screen pixel.
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
  host.sockets[0].send(values);
  await settle(host);
  return view;
}

/** Each command sent: its control, its action, its value and what the control showed (§5.8). */
function written(host: Host): unknown[] {
  return commands(host).map((command) => [command.object, command.action, command.value, command.shown]);
}

function pointer(window: any, target: EventTarget, type: string, x: number, y: number): void {
  target.dispatchEvent(new window.MouseEvent(type, { bubbles: true, clientX: x, clientY: y }));
}

suite("HMI display: switch, selector, mode selector and slider", () => {
  // The runtime writes the opposite of the value a switch shows (decided by the owner on 1 November
  // 2026); the page sends that value with `toggle`. Catches a page that decides the value itself.
  test("a switch asks the runtime to write the opposite of the value it shows", async () => {
    const { host, root, window } = await opened();
    click(window, object(root, "fan"));
    await settle(host);
    assert.deepStrictEqual(written(host), [["fan", "toggle", undefined, false]]);
  });

  // While a switch shows no value, the runtime refuses, records the refusal and says why; the page
  // shows the runtime's reason in its language. Catches a page that decides the refusal itself and a
  // reason that is not shown.
  test("a switch whose value is unknown or stale sends what it shows and shows the runtime's refusal", async () => {
    const { host, root, window } = await opened({ cycle: 1, values: { "Main.Fan": null } });
    host.replies["/api/hmi/command"] = [
      {
        status: 200,
        body: { id: "x", outcome: "refused", reason: "The value of this control is not known, so it sends nothing." },
      },
    ];
    click(window, object(root, "fan"));
    await settle(host);
    assert.deepStrictEqual(written(host), [["fan", "toggle", undefined, null]]);
    assert.match(root.querySelector('[data-part="notice"]')?.textContent ?? "", /Not accepted: .*not known/);
  });

  test("a selector lists its options with the current one marked, and writes the chosen one", async () => {
    const { host, root, window } = await opened();
    click(window, object(root, "speed"));
    await settle(host);
    const choices = root.querySelector('[data-part="choices"]');
    assert.ok(choices, "the list of options");
    const listed = [...choices.querySelectorAll("[data-option]")];
    assert.deepStrictEqual(
      listed.map((option) => option.getAttribute("data-option")),
      ["Off", "Low", "High"],
    );
    assert.deepStrictEqual(
      listed.map((option) => option.getAttribute("aria-pressed")),
      ["false", "true", "false"],
    );
    click(window, listed[2]);
    await settle(host);
    assert.deepStrictEqual(written(host), [["speed", "write", "High", "Low"]]);
    assert.strictEqual(root.querySelector('[data-part="choices"]'), null, "the list closes");
  });

  test("a mode selector writes the mode clicked, and nothing for the active one", async () => {
    const { host, root, window } = await opened();
    click(window, object(root, "mode").querySelector('[data-option="Auto"]') as Element);
    await settle(host);
    assert.deepStrictEqual(written(host), []);
    click(window, object(root, "mode").querySelector('[data-option="Manual"]') as Element);
    await settle(host);
    assert.deepStrictEqual(written(host), [["mode", "write", "Manual", "Auto"]]);
  });

  test("a slider shows a draft while it is dragged and writes the value where it is released", async () => {
    const { host, root, window } = await opened();
    // The track runs from x = 40 + 15 to x = 40 + 165 at y = 322: 0 to 80.
    pointer(window, object(root, "valve"), "pointerdown", 130, 322);
    pointer(window, window.document, "pointermove", 167.6, 322);
    await settle(host);
    const dragged = object(root, "valve");
    assert.strictEqual(dragged.getAttribute("data-draft"), "true");
    assert.match(dragged.textContent ?? "", /\b60\b/);
    assert.deepStrictEqual(written(host), [], "nothing is sent while dragging");
    pointer(window, window.document, "pointerup", 167.6, 322);
    await settle(host);
    assert.deepStrictEqual(written(host), [["valve", "write", 60, 40]]);
    assert.strictEqual(
      object(root, "valve").getAttribute("data-draft"),
      null,
      "the program's value again once the command ended",
    );
  });
});
