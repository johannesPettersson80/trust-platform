import * as assert from "assert";

import type { PageData } from "../../hmiBuilder/display/runtimeDisplay";
import {
  ANNA,
  ERIK,
  click,
  commands,
  fill,
  Host as DisplayTestHost,
  object,
  openedWith,
  settle,
  submit,
} from "./hmiDisplayHost";

// Specification 39, §12.4–12.5 and §12.8: operating the runtime display: the session, logging in,
// commands with their outcomes, and what happens when the connection or the session ends.

const OPERATE: PageData = {
  screen: { size: [1280, 720], theme: "dark" },
  pages: ["operate"],
  bundle: "v1",
  page: {
    name: "operate",
    model: {
      title: "Operate",
      object: [
        {
          id: "sp",
          type: "setpoint",
          at: [40, 40],
          size: [240, 96],
          rotation: 0,
          label: "Setpoint",
          min: 0,
          max: 100,
          bind: { value: { path: "Loop.Setpoint", direction: "read-write" } },
        },
        {
          id: "start",
          type: "button",
          at: [40, 160],
          size: [140, 44],
          rotation: 0,
          text: "Start",
          bind: { target: { path: "Main.Start", direction: "write" } },
        },
        {
          id: "stop",
          type: "button",
          at: [200, 160],
          size: [140, 44],
          rotation: 0,
          text: "Stop",
          action: "reset",
          confirm: "Stop the pump?",
          bind: { target: { path: "Main.Run", direction: "write" } },
        },
      ],
      pipe: [],
    },
  },
  errors: [],
};

class Host extends DisplayTestHost {
  constructor() {
    super(OPERATE);
  }
}

async function opened(host = new Host()): Promise<{ host: Host; root: HTMLElement; window: any }> {
  const opened = await openedWith(host);
  return { ...opened, host };
}

suite("HMI display: operating", () => {
  test("owner login identifies its runtime, preserves the local user and never resends a refused command", async () => {
    const host = new Host();
    host.replies["/api/hmi/command"] = [{ status: 200, body: { outcome: "refused", reason: "owner-login:Owner" } }];
    host.replies["/api/hmi/login"] = [
      { status: 200, body: { ok: true, user: "owner-user", role: "engineer", runtime: "Owner" } },
    ];
    const { root, window } = await opened(host);
    click(window, object(root, "start"));
    await settle(host);
    const form = root.querySelector('[data-part="login"]') as HTMLFormElement;
    assert.ok(form.textContent?.includes("Owner"));
    fill(window, form.querySelector('input[name="user"]') as HTMLInputElement, "owner-user");
    fill(window, form.querySelector('input[name="password"]') as HTMLInputElement, "secret");
    submit(window, form);
    await settle(host);
    const login = host.posts.find((post) => post.url === "/api/hmi/login");
    assert.strictEqual(login?.body.peer, "Owner");
    assert.strictEqual(commands(host).length, 1, "login must not retry the command");
    assert.ok(
      root.querySelector('[data-part="session"]')?.textContent?.includes("anna"),
      "local session identity stays unchanged",
    );
  });
  // The runtime reads the entry and checks the authored limits (specification 39 §12.8;
  // web_hmi_commands.rs, a_denied_write_is_refused_with_its_reason); the display shows them and
  // sends the entry as typed. Catches a hint that takes the equipment's wider range.
  test("numeric entry shows inherited units and the authored limits without rounding the value", async () => {
    for (const equipment of [false, true]) {
      const host = new DisplayTestHost({ ...OPERATE, formats: { "loop.setpoint": { unit: "m", decimals: 2 } } });
      const { root, window } = await openedWith(host);
      host.sockets[0].send({
        cycle: 1,
        values: {
          "Loop.Setpoint": { value: 1.2345, ...(equipment ? { presentation: { unit: "°C", range: [0, 200] } } : {}) },
        },
      });
      await settle(host);
      click(window, object(root, "sp"));
      await settle(host);
      const form = root.querySelector('[data-part="entry"]') as HTMLFormElement;
      assert.ok(form.textContent?.includes(equipment ? "°C" : "m"), "the entry identifies the value's inherited unit");
      assert.strictEqual(
        (form.querySelector('input[name="value"]') as HTMLInputElement).value,
        "1.2345",
        "editing keeps full precision",
      );
      assert.ok(
        form.textContent?.includes("Between 0 and 100"),
        "engineering display metadata cannot widen the authored 0..100 write range",
      );
      fill(window, form.querySelector('input[name="value"]') as HTMLInputElement, "150");
      submit(window, form);
      await settle(host);
      assert.deepStrictEqual(
        commands(host).map((command) => command.value),
        ["150"],
        "the entry as typed",
      );
    }
  });
  test("the top bar shows the user and role, and without a session a login button", async () => {
    const { root } = await opened();
    const session = root.querySelector('[data-part="session"]');
    assert.ok(session, "a session bar");
    assert.ok(
      session.textContent?.includes("anna") && session.textContent.includes("Operator"),
      session.textContent ?? "",
    );
    assert.ok(root.querySelector('[data-action="logout"]'));
    const host = new Host();
    host.session = { status: 401, body: { ok: false, error: "no session" } };
    const signedOut = await opened(host);
    assert.ok(signedOut.root.querySelector('[data-action="login"]'), "a login button");
    assert.strictEqual(
      object(signedOut.root, "start").getAttribute("data-readonly"),
      "true",
      "the controls cannot be used",
    );
  });

  test("a control the signed-in role may not use is dimmed with a lock, names its role and cannot be pressed", async () => {
    // §5.9 item 5: the page says which role each control needs.
    const roles = { sp: "engineer", start: "operator", stop: null };
    const host = new DisplayTestHost({ ...OPERATE, roles });
    const { root, window } = await opened(host as Host);
    const sp = object(root, "sp");
    assert.strictEqual(sp.getAttribute("data-locked"), "engineer");
    assert.ok(sp.querySelector('[data-part="lock"]'), "a lock");
    assert.ok(sp.textContent?.includes("Engineer"), sp.textContent ?? "");
    assert.strictEqual(object(root, "stop").getAttribute("data-locked"), "none", "no role may use it");
    assert.ok(object(root, "stop").textContent?.includes("Locked"), object(root, "stop").textContent ?? "");
    assert.strictEqual(object(root, "start").getAttribute("data-locked"), null, "an Operator may press Start");
    click(window, sp);
    await settle(host);
    assert.strictEqual(commands(host).length, 0, "nothing is sent");
    assert.ok(root.querySelector('[data-part="notice"]')?.textContent?.includes("Engineer"), "it says who may");
    const engineer = new DisplayTestHost({ ...OPERATE, roles });
    engineer.session = { status: 200, body: ERIK };
    const asEngineer = await opened(engineer as Host);
    assert.strictEqual(object(asEngineer.root, "sp").getAttribute("data-locked"), null, "an Engineer may use it");
    // The roles the runtime says the session acts as decide, not the role's name (§12.4 item 4).
    const listed = new DisplayTestHost({ ...OPERATE, roles });
    listed.session = { status: 200, body: { ...ERIK, acts_as: ["viewer", "operator"] } };
    const asListed = await opened(listed as Host);
    assert.strictEqual(object(asListed.root, "sp").getAttribute("data-locked"), "engineer", "the page ranks no roles");
  });

  test("on a touch screen a number entry opens a large keypad with the allowed range; with a mouse it does not", async () => {
    // §18 phase 5 item 6.
    const { host, root, window } = await opened();
    const press = (kind: string): void => {
      const event = new window.MouseEvent("pointerdown", { bubbles: true, cancelable: true });
      Object.defineProperty(event, "pointerType", { value: kind });
      object(root, "sp").dispatchEvent(event);
    };
    press("touch");
    click(window, object(root, "sp"));
    await settle(host);
    const keypad = root.querySelector('[data-part="entry"] [data-part="keypad"]');
    assert.ok(keypad, "a keypad");
    const input = root.querySelector('[data-part="entry"] input[name="value"]') as HTMLInputElement;
    assert.strictEqual(input.inputMode, "none", "no device keyboard over it");
    input.value = "";
    const tap = (key: string) => click(window, keypad?.querySelector(`[data-key="${key}"]`) as Element);
    for (const key of ["4", "2", ".", "7", "back", "5"]) tap(key);
    assert.strictEqual(input.value, "42.5");
    assert.ok(
      root.querySelector('[data-part="entry"]')?.textContent?.includes("Between 0 and 100"),
      "the allowed range",
    );
    submit(window, root.querySelector('[data-part="entry"]') as Element);
    await settle(host);
    assert.deepStrictEqual(
      commands(host).map((command) => [command.object, command.value]),
      [["sp", "42.5"]],
      "the entry as typed; the runtime reads the number (§12.8)",
    );
    press("mouse");
    click(window, object(root, "sp"));
    await settle(host);
    assert.ok(root.querySelector('[data-part="entry"]'), "the entry opens");
    assert.ok(!root.querySelector('[data-part="keypad"]'), "without a keypad");
  });

  test("logging in starts the session and makes the controls usable", async () => {
    const host = new Host();
    host.session = { status: 401, body: { ok: false } };
    host.reply("/api/hmi/login", { status: 200, body: ANNA });
    const { root, window } = await opened(host);
    click(window, root.querySelector('[data-action="login"]') as Element);
    await settle(host);
    const form = root.querySelector('[data-part="login"]');
    assert.ok(form, "a login form");
    fill(window, form.querySelector('input[name="user"]') as HTMLInputElement, "anna");
    fill(window, form.querySelector('input[name="password"]') as HTMLInputElement, "secret");
    submit(window, form);
    await settle(host);
    assert.deepStrictEqual(host.posts[0], { url: "/api/hmi/login", body: { user: "anna", password: "secret" } });
    assert.ok(root.querySelector('[data-part="session"]')?.textContent?.includes("anna"));
    assert.strictEqual(object(root, "start").getAttribute("data-readonly"), null);
    // The stream carries the session it was opened with, so it is opened again (§12.8).
    assert.strictEqual(host.sockets.length, 2, "a new stream with the session");
    assert.ok(host.sockets[0].closed, "the old stream is closed");
  });

  test("a button sends its command with an id and the page version, and shows each outcome", async () => {
    const { host, root, window } = await opened();
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
    click(window, object(root, "start"));
    await settle(host);
    const sent = commands(host);
    assert.strictEqual(sent.length, 1);
    assert.ok(typeof sent[0].id === "string" && sent[0].id.length >= 8, JSON.stringify(sent[0]));
    assert.deepStrictEqual(
      { ...sent[0], id: "?" },
      { id: "?", bundle: "v1", page: "operate", object: "start", action: "set" },
    );
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "applied");
    host.sockets[0].send({ command: { id: sent[0].id, outcome: "confirmed" } });
    await settle(host);
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "confirmed");
  });

  test("a control that calls a method sends invoke with its identity, never the method or its inputs", async () => {
    const data = JSON.parse(JSON.stringify(OPERATE)) as PageData;
    const start = data.page!.model.object.find((item) => item.id === "start")!;
    start.command = { interface: "Plant.IRunDevice", method: "Start", instance: "Main.P101", arguments: {} };
    start.bind = {};
    const host = new DisplayTestHost(data);
    const { root, window } = await openedWith(host);
    host.reply("/api/hmi/command", { status: 200, body: { outcome: "applied" } });
    click(window, object(root, "start"));
    await settle(host);
    const sent = commands(host);
    assert.strictEqual(sent.length, 1);
    assert.deepStrictEqual(
      { ...sent[0], id: "?" },
      { id: "?", bundle: "v1", page: "operate", object: "start", action: "invoke" },
    );
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "applied");
  });

  test("a stop button is drawn unlike a start button", async () => {
    // §8.4 item 4: Stop uses the `stop` variant of the button, in the drawing's alarm colour.
    const data = JSON.parse(JSON.stringify(OPERATE)) as PageData;
    data.page!.model.object.find((item) => item.id === "stop")!.variant = "stop";
    const { root } = await openedWith(new DisplayTestHost(data));
    const stop = object(root, "stop");
    const start = object(root, "start");
    assert.ok(
      (stop.getAttribute("class") ?? "").split(" ").includes("hmi-button-stop"),
      stop.getAttribute("class") ?? "",
    );
    assert.ok(!(start.getAttribute("class") ?? "").includes("hmi-button-stop"), start.getAttribute("class") ?? "");
    const face = (element: Element): Element | null => element.querySelector('[data-part="face"]');
    assert.strictEqual(face(stop)?.getAttribute("fill"), "var(--hmi-raised)");
    assert.strictEqual(face(stop)?.getAttribute("stroke"), "var(--hmi-alarm)");
    assert.notStrictEqual(
      face(stop)?.getAttribute("fill"),
      face(start)?.getAttribute("fill"),
      "Start keeps its accent face",
    );
  });

  test("a control with a question asks before it sends", async () => {
    const { host, root, window } = await opened();
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "applied" } });
    click(window, object(root, "stop"));
    await settle(host);
    const question = root.querySelector('[data-part="confirm"]');
    assert.ok(question, "a question");
    assert.ok(question.textContent?.includes("Stop the pump?"), question.textContent ?? "");
    click(window, question.querySelector('[data-action="cancel"]') as Element);
    await settle(host);
    assert.strictEqual(commands(host).length, 0, "cancelled");
    click(window, object(root, "stop"));
    await settle(host);
    click(window, root.querySelector('[data-part="confirm"] [data-action="confirm"]') as Element);
    await settle(host);
    assert.strictEqual(commands(host)[0]?.action, "reset");
  });

  // Specification 39 §12.8: the runtime checks the limits and its refusal names the reason.
  // Catches a display that drops an entry outside the limits without the runtime's word, or shows
  // a refusal without its reason.
  test("a setpoint shows its limits, sends the entry as typed and shows the runtime's refusal", async () => {
    const { host, root, window } = await opened();
    host.reply(
      "/api/hmi/command",
      { status: 200, body: { id: "x", outcome: "refused", reason: "150 is above the highest allowed value 100" } },
      { status: 200, body: { id: "x", outcome: "applied", readback: 61.5 } },
    );
    const enter = async (typed: string): Promise<void> => {
      click(window, object(root, "sp"));
      await settle(host);
      const entry = root.querySelector('[data-part="entry"]');
      assert.ok(entry, "an entry");
      assert.ok(entry.textContent?.includes("Between 0 and 100"), entry.textContent ?? "");
      fill(window, entry.querySelector('input[name="value"]') as HTMLInputElement, typed);
      submit(window, entry);
      await settle(host);
    };
    await enter("150");
    assert.strictEqual(object(root, "sp").getAttribute("data-outcome"), "refused");
    assert.ok(root.querySelector('[data-part="notice"]')?.textContent?.includes("above the highest allowed value 100"));
    await enter("61.5");
    assert.deepStrictEqual(
      commands(host).map((command) => ({ ...command, id: "?" })),
      [
        { id: "?", bundle: "v1", page: "operate", object: "sp", action: "write", value: "150" },
        { id: "?", bundle: "v1", page: "operate", object: "sp", action: "write", value: "61.5" },
      ],
    );
  });

  test("a refused command shows its reason", async () => {
    const { host, root, window } = await opened();
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "refused", reason: "Door open" } });
    click(window, object(root, "start"));
    await settle(host);
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "refused");
    assert.ok(root.querySelector('[data-part="notice"]')?.textContent?.includes("Door open"));
  });

  test("a refusal's notice and mark go once a later command is accepted", async () => {
    const { host, root, window } = await opened();
    host.reply("/api/hmi/command", { status: 200, body: { id: "x", outcome: "refused", reason: "Door open" } });
    click(window, object(root, "start"));
    await settle(host);
    assert.ok(root.querySelector('[data-part="notice"]'), "the refusal's notice");
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "refused");
    // Another control's accepted command, as Manual is chosen after "Auto: switch to Manual".
    host.reply("/api/hmi/command", { status: 200, body: { id: "y", outcome: "applied" } });
    click(window, object(root, "stop"));
    await settle(host);
    click(window, root.querySelector('[data-part="confirm"] [data-action="confirm"]') as Element);
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="notice"]'), null, "the refusal is past");
    assert.notStrictEqual(object(root, "start").getAttribute("data-outcome"), "refused", "and so is its mark");
  });

  test("a command the equipment does not accept says Not accepted with its reason and is never resent", async () => {
    // §21.2 item 3: the device's CommandText is the reason.
    const { host, root, window } = await opened();
    host.reply(
      "/api/hmi/command",
      { status: 200, body: { outcome: "refused", reason: "Auto: switch to Manual" } },
      { status: 200, body: { outcome: "refused", reason: "not accepted" } },
    );
    click(window, object(root, "start"));
    await settle(host);
    const notice = root.querySelector('[data-part="notice"]')?.textContent ?? "";
    assert.strictEqual(notice, "Not accepted: Auto: switch to Manual");
    assert.ok(!/take control|owner/i.test(notice), notice);
    assert.strictEqual(commands(host).length, 1, "never sent again");
    const start = object(root, "start");
    assert.strictEqual(start.getAttribute("data-outcome"), "refused");
    assert.strictEqual(
      start.querySelector("title")?.textContent,
      "Not accepted: Auto: switch to Manual",
      "the reason beside the control",
    );
    click(window, object(root, "start"));
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="notice"]')?.textContent, "Not accepted", "no reason of its own");
  });

  test("a disconnect during a command ends as unknown and is not sent again", async () => {
    const { host, root, window } = await opened();
    host.reply("/api/hmi/command", "fail");
    click(window, object(root, "start"));
    await settle(host);
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "unknown");
    host.sockets[0].onclose?.();
    await settle(host);
    assert.strictEqual(commands(host).length, 1, "never sent again");
  });

  test("a page opened without a session does not ask to log in until the user acts", async () => {
    const host = new Host();
    host.session = { status: 401, body: { ok: false } };
    const { root } = await opened(host);
    host.sockets[0].send({ session: null });
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="login"]'), null, "nothing ended, so nothing to say");
    assert.strictEqual(object(root, "start").getAttribute("data-readonly"), "true");
  });

  test("a confirmed or applied outcome fades after a few seconds; a refusal stays", async () => {
    const { host, root, window } = await opened();
    host.reply(
      "/api/hmi/command",
      { status: 200, body: { id: "x", outcome: "applied" } },
      { status: 200, body: { id: "y", outcome: "refused", reason: "Door open" } },
    );
    click(window, object(root, "start"));
    await settle(host);
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), "applied");
    host.advance(5000);
    await settle(host);
    assert.strictEqual(object(root, "start").getAttribute("data-outcome"), null, "the outcome has faded");
    click(window, object(root, "start"));
    await settle(host);
    host.advance(30000);
    await settle(host);
    assert.strictEqual(
      object(root, "start").getAttribute("data-outcome"),
      "refused",
      "a refusal stays until the next command",
    );
  });

  test("a page drawn from an older bundle is told to reload", async () => {
    const { host, root, window } = await opened();
    host.sockets[0].send({ bundle: "v1" });
    await settle(host);
    assert.strictEqual(root.querySelector('[data-part="reload"]'), null, "the same version");
    host.sockets[0].send({ bundle: "v2" });
    await settle(host);
    const notice = root.querySelector('[data-part="reload"]');
    assert.ok(notice, "a reload notice");
    assert.ok(notice.textContent?.includes("changed"), notice.textContent ?? "");
    click(window, notice.querySelector('[data-action="reload"]') as Element);
    assert.strictEqual(host.reloads, 1);
  });

  test("an ended session makes the page read-only at once and asks to log in", async () => {
    const { host, root, window } = await opened();
    host.sockets[0].send({ session: null });
    await settle(host);
    assert.strictEqual(object(root, "start").getAttribute("data-readonly"), "true");
    assert.ok(root.querySelector('[data-part="login"]'), "a login prompt");
    click(window, object(root, "start"));
    await settle(host);
    assert.strictEqual(commands(host).length, 0);
  });
});
