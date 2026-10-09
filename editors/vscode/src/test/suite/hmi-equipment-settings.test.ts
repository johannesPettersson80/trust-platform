import * as assert from "assert";
import {
  EquipmentSettings,
  SETTINGS_CSS,
  enteredValue,
  mayChangeSettings,
  settingLabel,
} from "../../hmiBuilder/display/settings";
import type { Setting } from "../../hmiBuilder/display/settings";
import { Overlay } from "../../hmiBuilder/display/overlay";
import { SessionControl } from "../../hmiBuilder/display/session";
import { ANNA, Host, openedWith, settle, fill, submit } from "./hmiDisplayHost";

// Specification 39, §21.4 and Plant §10: the Settings dialog of an equipment panel. Engineers only;
// the definition's settings in order with plain titles and units, a help line only where the
// definition gives one, Apply and Cancel always at the bottom with the result beside Apply, and a
// draft that survives a refusal.

const PANEL = "faceplate:overview/pump";

const START_TIMEOUT: Setting = {
  member: "StartTimeout",
  title: "Start timeout",
  unit: "s",
  min: 1,
  max: 600,
  key: true,
  help: "Running feedback must arrive within this time.",
  kind: "time",
  value: 5,
};
const STOP_TIMEOUT: Setting = {
  member: "StopTimeout",
  title: "Stop timeout",
  unit: "s",
  min: 1,
  max: 600,
  kind: "time",
  value: 5,
};
const SERVICE: Setting = {
  member: "ServiceInterval",
  title: "Service interval",
  unit: "h",
  min: 0,
  max: 100000,
  kind: "real",
  value: 4000,
};
const READ = {
  status: 200,
  body: { ok: true, instance: "Main.P101", settings: [START_TIMEOUT, STOP_TIMEOUT, SERVICE] },
};

const PAGE = {
  screen: { size: [1280, 720] as [number, number] },
  pages: ["overview"],
  bundle: "v1",
  page: { name: "overview", model: { title: "Overview", object: [], pipe: [] } },
  errors: [],
};

/** The dialog for a session of `role`, opened on the pump's panel. */
async function dialog(
  role: string,
  ...replies: Array<{ status: number; body: unknown }>
): Promise<{ host: Host; root: HTMLElement; window: any; form: HTMLFormElement | null }> {
  const host = new Host(PAGE);
  host.session = { status: 200, body: { ...ANNA, user: role === "engineer" ? "erik" : "anna", role } };
  const { root, window } = await openedWith(host);
  const overlay = new Overlay(root, host);
  const session = new SessionControl(host, overlay, () => undefined);
  await session.start();
  host.reply("/api/hmi/settings", ...replies);
  await new EquipmentSettings(host, overlay, session).open(PANEL, "P101 — Pump");
  await settle(host);
  return { host, root, window, form: root.querySelector('[data-part="equipment-settings"]') as HTMLFormElement | null };
}

function input(form: HTMLFormElement, member: string): HTMLInputElement {
  const found = form.querySelector(`[name="${member}"]`) as HTMLInputElement | null;
  assert.ok(found, `a field for ${member}`);
  return found;
}

function applied(host: Host): any[] {
  return host.posts
    .filter((post) => post.url === "/api/hmi/settings" && post.body.action === "apply")
    .map((post) => post.body);
}

suite("HMI equipment settings", () => {
  test("only an Engineer may open the dialog", async () => {
    assert.strictEqual(mayChangeSettings("engineer"), true);
    assert.strictEqual(mayChangeSettings("operator"), false);
    assert.strictEqual(mayChangeSettings(undefined), false);
    const { host, root, form } = await dialog("operator", READ);
    assert.strictEqual(form, null, "no dialog for an Operator");
    assert.ok(root.querySelector('[data-part="notice"]')?.textContent?.includes("Engineer"));
    assert.strictEqual(
      host.posts.filter((post) => post.url === "/api/hmi/settings").length,
      0,
      "nothing is asked of the runtime",
    );
  });

  test("the settings are listed in order with plain titles, units and a help line only where given", async () => {
    const { form } = await dialog("engineer", READ);
    assert.ok(form, "the dialog opens");
    assert.ok(form.querySelector("h2")?.textContent?.includes("P101 — Pump"));
    const labels = Array.from(form.querySelectorAll(".hmi-settings-field > span")).map((span) => span.textContent);
    assert.deepStrictEqual(labels, ["Start timeout (s)", "Stop timeout (s)", "Service interval (h)"]);
    assert.strictEqual(input(form, "StartTimeout").value, "5", "a duration in its unit");
    assert.strictEqual(input(form, "ServiceInterval").value, "4000");
    const help = Array.from(form.querySelectorAll(".hmi-settings-field small")).map((line) => line.textContent);
    assert.deepStrictEqual(help, [START_TIMEOUT.help], "one help line, only where the definition gives one");
    assert.strictEqual(settingLabel({ ...SERVICE, unit: undefined }), "Service interval");
  });

  test("Apply and Cancel stay in a sticky footer with the result beside Apply", async () => {
    const after = {
      ...READ,
      body: { ...READ.body, settings: [{ ...START_TIMEOUT, value: 10 }, STOP_TIMEOUT, { ...SERVICE, value: 2000 }] },
    };
    const { host, window, form } = await dialog("engineer", READ, after);
    assert.ok(form, "the dialog opens");
    const footer = form.querySelector('[data-part="settings-footer"]');
    assert.ok(footer, "a footer");
    assert.strictEqual(footer.parentElement, form, "outside the scrolling fields");
    assert.ok(!form.querySelector('[data-part="settings-fields"]')?.contains(footer));
    const order = Array.from(footer.children).map(
      (child) => child.getAttribute("data-part") ?? child.getAttribute("data-action"),
    );
    assert.deepStrictEqual(order, ["settings-result", "cancel", "apply"], "the result beside Apply");
    assert.match(SETTINGS_CSS, /\.hmi-settings-footer \{[^}]*position: sticky; bottom: 0;/);
    assert.match(SETTINGS_CSS, /\.hmi-settings-fields \{[^}]*overflow: auto;/);
    fill(window, input(form, "StartTimeout"), "10");
    fill(window, input(form, "ServiceInterval"), "2000");
    submit(window, form);
    await settle(host);
    assert.deepStrictEqual(
      applied(host),
      [
        {
          faceplate: PANEL,
          action: "apply",
          changes: { StartTimeout: 10, ServiceInterval: 2000 },
          expected: { StartTimeout: 5, ServiceInterval: 4000 },
        },
      ],
      "only the changed settings, with the values they replace",
    );
    assert.strictEqual(form.querySelector('[data-part="settings-result"]')?.textContent, "Applied");
    assert.strictEqual(input(form, "StartTimeout").value, "10", "the values read back");
    const cancel = form.querySelector('[data-action="cancel"]');
    assert.strictEqual(cancel?.textContent, "Close", "nothing is left to apply: the dialog only closes");
    fill(window, input(form, "StartTimeout"), "12");
    assert.strictEqual(cancel?.textContent, "Cancel", "an edit after Applied is a draft again");
  });

  test("in the HMI Builder's Live mode, Keep in project keeps the values the PLC has; a browser has no such button", async () => {
    const browser = await dialog("engineer", READ);
    assert.strictEqual(browser.form?.querySelector('[data-action="keep"]'), null, "nothing to keep in a browser");
    const host = new Host(PAGE);
    host.session = { status: 200, body: { ...ANNA, user: "erik", role: "engineer" } };
    const kept: Array<{ instance: string; settings: Record<string, unknown> }> = [];
    const answers = [
      { ok: true, kept: ["StartTimeout"] },
      { ok: true, kept: [] },
      { ok: false, error: "This equipment is declared in your PLC code: copy the values there." },
    ];
    (host as any).keepSettings = async (instance: string, settings: Record<string, unknown>) => {
      kept.push({ instance, settings });
      return { status: 200, body: answers.shift() };
    };
    const { root, window } = await openedWith(host);
    const overlay = new Overlay(root, host);
    const session = new SessionControl(host, overlay, () => undefined);
    await session.start();
    host.reply("/api/hmi/settings", READ);
    await new EquipmentSettings(host, overlay, session).open(PANEL, "P101 — Pump");
    await settle(host);
    const form = root.querySelector('[data-part="equipment-settings"]') as HTMLFormElement;
    const footer = form.querySelector('[data-part="settings-footer"]')!;
    assert.deepStrictEqual(
      Array.from(footer.children).map((child) => child.getAttribute("data-part") ?? child.getAttribute("data-action")),
      ["settings-result", "cancel", "keep", "apply"],
    );
    const keep = form.querySelector('[data-action="keep"]') as HTMLButtonElement;
    assert.strictEqual(keep.textContent, "Keep in project");
    fill(window, input(form, "StartTimeout"), "9");
    keep.click();
    await settle(host);
    assert.deepStrictEqual(
      kept,
      [{ instance: "Main.P101", settings: { StartTimeout: 5, StopTimeout: 5, ServiceInterval: 4000 } }],
      "the values the PLC has, not an edit that is not applied",
    );
    const result = () => form.querySelector('[data-part="settings-result"]');
    assert.strictEqual(result()?.textContent, "Kept in the project.");
    keep.click();
    await settle(host);
    assert.strictEqual(result()?.textContent, "The project already has these values.");
    keep.click();
    await settle(host);
    assert.strictEqual(result()?.textContent, "This equipment is declared in your PLC code: copy the values there.");
    assert.strictEqual(result()?.hasAttribute("data-refused"), true);
  });

  test("a refusal names the field beside Apply and keeps the draft", async () => {
    const refused = {
      status: 200,
      body: { ok: false, error: "Start timeout: 700 is above the maximum 600 s", field: "StartTimeout" },
    };
    const { host, window, form } = await dialog("engineer", READ, refused);
    assert.ok(form, "the dialog opens");
    // A value outside the definition's limits is named before it is sent.
    fill(window, input(form, "StartTimeout"), "700");
    submit(window, form);
    await settle(host);
    assert.strictEqual(applied(host).length, 0, "not sent");
    const result = form.querySelector('[data-part="settings-result"]');
    assert.ok(result?.textContent?.includes("Start timeout"), result?.textContent ?? "");
    assert.strictEqual(result?.getAttribute("data-refused"), "true");
    assert.strictEqual(input(form, "StartTimeout").getAttribute("aria-invalid"), "true");
    // The runtime's own refusal.
    fill(window, input(form, "StartTimeout"), "60");
    submit(window, form);
    await settle(host);
    assert.strictEqual(applied(host).length, 1);
    assert.strictEqual(result?.textContent, "Start timeout: 700 is above the maximum 600 s");
    assert.strictEqual(input(form, "StartTimeout").value, "60", "the draft stays");
    assert.strictEqual(input(form, "StartTimeout").getAttribute("aria-invalid"), "true");
  });

  test("a value changed meanwhile refreshes the fields not edited and keeps the edits", async () => {
    const changed = {
      status: 200,
      body: { ok: false, error: "Changed meanwhile: refresh", field: "StartTimeout", changed: true },
    };
    const now = {
      ...READ,
      body: { ...READ.body, settings: [{ ...START_TIMEOUT, value: 7 }, STOP_TIMEOUT, { ...SERVICE, value: 3000 }] },
    };
    const { host, window, form } = await dialog("engineer", READ, changed, now);
    assert.ok(form, "the dialog opens");
    fill(window, input(form, "StartTimeout"), "10");
    submit(window, form);
    await settle(host);
    assert.strictEqual(form.querySelector('[data-part="settings-result"]')?.textContent, "Changed meanwhile: refresh");
    assert.strictEqual(input(form, "StartTimeout").value, "10", "the edit stays");
    assert.strictEqual(input(form, "ServiceInterval").value, "3000", "a field not edited shows the value now");
  });

  test("an entered value is a number in the setting's unit within its limits, or a choice's value", () => {
    assert.strictEqual(enteredValue(START_TIMEOUT, "2,5"), 2.5);
    assert.throws(() => enteredValue(START_TIMEOUT, "0.5"), /Start timeout: 0.5 is below the minimum 1 s/);
    assert.throws(() => enteredValue(START_TIMEOUT, "soon"), /enter a number/);
    assert.throws(() => enteredValue({ ...SERVICE, kind: "integer" }, "1.5"), /whole number/);
    const per: Setting = {
      member: "TotalPer",
      title: "Total per",
      kind: "time",
      value: "T#1h",
      choices: [
        { value: "T#1m", title: "minute" },
        { value: "T#1h", title: "hour" },
      ],
    };
    assert.strictEqual(enteredValue(per, "T#1m"), "T#1m");
    assert.throws(() => enteredValue(per, "T#1d"), /choose one of its values/);
    assert.strictEqual(enteredValue({ member: "Heating", title: "Heating", kind: "bool", value: true }, false), false);
  });
});
