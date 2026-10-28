import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as React from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

const { JSDOM } = require("jsdom") as {
  JSDOM: new (html: string, options: { url: string }) => any;
};
const { buildSync } = require("esbuild") as {
  buildSync(options: Record<string, unknown>): void;
};

// The form's actions, result notice and draft tile are drawn from the Rust view model and the host's
// `ActReply` (specification 25 §5.4, §5.9 and §5.13; design §4.5 D3). The expected texts and notices
// below are written by hand, next to the specification line they check; the golden forms are the
// Rust fixtures that `crates/trust-devices/src/panes/tests/golden.rs` writes, never read from the
// component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles one webview entry point and returns its named export, as the extension ships it. */
function loadComponent(component: "FormView" | "App"): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", `.trust-devices-form-actions-${component}-`),
  );
  const outfile = path.join(temp, `devices-form-actions-${component}.cjs`);
  try {
    buildSync({
      entryPoints: [
        path.join(extensionRoot, "src", "devicesPanel", "webview", `${component}.tsx`),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile)[component] as WebviewComponent;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

// The golden form fixtures, written by the Rust test and never by hand.
const fixtures = path.resolve(
  path.resolve(__dirname, "..", "..", ".."),
  "..",
  "..",
  "crates",
  "trust-devices",
  "tests",
  "fixtures",
  "devices_forms",
);

/** The form fixture `name`, as Rust serialized it. */
function form(name: string): any {
  return JSON.parse(fs.readFileSync(path.join(fixtures, `${name}.json`), "utf8"));
}

// JSDOM's elements stay untyped, as in devices-forms-render.test.ts: the extension's tsconfig has no
// DOM lib, so a `HTMLElement` here would not name a type.
interface Rendered {
  container: any;
  window: any;
  update(props: Record<string, unknown>): Promise<void>;
  unmount(): Promise<void>;
}

async function render(
  component: WebviewComponent,
  props: Record<string, unknown>,
): Promise<Rendered> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-form-actions.test/",
  });
  const globals = globalThis as any;
  const previousWindow = globals.window;
  const previousDocument = globals.document;
  const previousActEnvironment = globals.IS_REACT_ACT_ENVIRONMENT;
  globals.window = dom.window;
  globals.document = dom.window.document;
  globals.IS_REACT_ACT_ENVIRONMENT = true;
  const container = dom.window.document.getElementById("root");
  const root: Root = createRoot(container);
  await act(async () => {
    root.render(React.createElement(component, props));
  });
  return {
    container,
    window: dom.window,
    async update(next: Record<string, unknown>) {
      await act(async () => {
        root.render(React.createElement(component, next));
      });
    },
    async unmount() {
      await act(async () => {
        root.unmount();
      });
      globals.window = previousWindow;
      globals.document = previousDocument;
      globals.IS_REACT_ACT_ENVIRONMENT = previousActEnvironment;
      dom.window.close();
    },
  };
}

/** Types into a text input the way a user does, past React's value tracker. */
async function type(rendered: Rendered, input: any, value: string): Promise<void> {
  const setter = Object.getOwnPropertyDescriptor(
    rendered.window.HTMLInputElement.prototype,
    "value",
  )?.set;
  assert.ok(setter, "HTMLInputElement.value has a setter");
  await act(async () => {
    setter.call(input, value);
    input.dispatchEvent(new rendered.window.Event("input", { bubbles: true }));
  });
}

/** Clicks an element the way a user does. */
async function click(rendered: Rendered, element: any): Promise<void> {
  assert.ok(element, "the element to click is drawn");
  await act(async () => {
    element.click();
  });
}

/** The form field `id`'s block, wherever it sits in the sections. */
function fieldBlock(container: any, id: string): any {
  const block = container.querySelector(`[data-role="form-field"][data-field="${id}"]`);
  assert.ok(block, `the field ${id} is drawn`);
  return block;
}

// An action with the fields the panel draws (mod.rs `Action`); the tests override only what they
// are about.
function action(overrides: Record<string, unknown>): Record<string, unknown> {
  return { id: "connect", label: "Connect", enabled: true, emphasis: "primary", ...overrides };
}

/** The toolbar's three actions, as the shell needs them to draw. */
function toolbar(): Record<string, unknown> {
  return {
    add: action({ id: "add", label: "+ Add", emphasis: "primary" }),
    discover: action({ id: "discover", label: "Discover", emphasis: "secondary" }),
    add_host_slot: action({ id: "add_host", label: "Add host", emphasis: "secondary" }),
  };
}

/** A shell view holding one runtime, so `App` can draw its canvas around the form pane. */
function view(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    toolbar: toolbar(),
    computers: [
      {
        id: "host-1",
        heading: "This computer",
        reachability: { word: "Reachable", tone: "ok" },
        runtimes: [
          { id: "runtime-1", name: "Line runtime", state: { word: "Running", tone: "ok" } },
        ],
      },
    ],
    ...overrides,
  };
}

/** The reply `form.open` gives, which `App` opens in its right-hand pane (design §1.4.1). */
function formReply(name: string): Record<string, unknown> {
  return {
    ok: true,
    effects: [],
    close_pane: false,
    refresh: "none",
    form: form(name),
  };
}

suite("Devices & Connections form actions", () => {
  test("field_errors_appear_under_their_fields", async () => {
    const FormView = loadComponent("FormView");
    // modbus_tcp_edit: a rejected save is a notice whose field_errors name their fields
    // (specification 25 §5.9: a value that does not fit its type is a field error of that field;
    // §5.13: the result shows in the pane that started it).
    const edit = form("modbus_tcp_edit");
    const notice = {
      pane: "form:modbus_tcp@runtime:local#endpoint:meter",
      tone: "error",
      text: "The connection was not saved.",
      detail: "Fix the highlighted fields and save again.",
      field_errors: [
        { field: "unit_id", message: "Enter a whole number of 0 or more." },
        { field: "timeout_ms", message: "Enter a timeout of at least 1 ms." },
      ],
      actions: [],
    };
    const rendered = await render(FormView, {
      form: edit,
      notice,
      onValues: () => undefined,
      onAction: () => undefined,
    });
    try {
      const container = rendered.container;
      // The notice itself is drawn by its own pane, with its text, tone and detail.
      assert.strictEqual(
        container.querySelector('[data-role="form-notice"]').getAttribute("data-tone"),
        "error",
        "the notice is drawn in its own tone",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-notice-text"]').textContent,
        "The connection was not saved.",
        "the notice's text is drawn as given",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-notice-detail"]').textContent,
        "Fix the highlighted fields and save again.",
        "the notice's detail is drawn as given",
      );
      // Each refused field carries its own message, under that field.
      assert.strictEqual(
        fieldBlock(container, "unit_id").querySelector('[data-role="field-error"]').textContent,
        "Enter a whole number of 0 or more.",
        "the unit_id field error is drawn under unit_id",
      );
      assert.strictEqual(
        fieldBlock(container, "timeout_ms").querySelector('[data-role="field-error"]').textContent,
        "Enter a timeout of at least 1 ms.",
        "the timeout_ms field error is drawn under timeout_ms",
      );
      assert.strictEqual(
        fieldBlock(container, "address").querySelector('[data-role="field-error"]'),
        null,
        "a field the notice did not refuse carries no error",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("an_edit_hides_the_stale_notice_until_the_next_one", async () => {
    const FormView = loadComponent("FormView");
    // modbus_tcp_edit: editing a rejected form hides only its stale apply fault, and the next notice
    // shows again (specification 25 §5.9; inventory J09: `edited since the last notice` is view
    // state).
    const edit = form("modbus_tcp_edit");
    const stale = {
      pane: "form:modbus_tcp@runtime:local#endpoint:meter",
      tone: "error",
      text: "The Unit ID was refused.",
      field_errors: [{ field: "unit_id", message: "Enter a whole number of 0 or more." }],
      actions: [],
    };
    const onValues = () => undefined;
    const rendered = await render(FormView, { form: edit, notice: stale, onValues });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="form-notice-text"]').textContent,
        "The Unit ID was refused.",
        "the notice is drawn before any edit",
      );
      assert.ok(
        fieldBlock(container, "unit_id").querySelector('[data-role="field-error"]'),
        "its field error is drawn before any edit",
      );

      const address = container.querySelector('[data-role="field-input"][data-field="address"]');
      await type(rendered, address, "10.0.0.9:502");
      assert.strictEqual(
        container.querySelector('[data-role="form-notice"]'),
        null,
        "editing a field hides the stale notice",
      );
      assert.strictEqual(
        fieldBlock(container, "unit_id").querySelector('[data-role="field-error"]'),
        null,
        "editing a field hides the stale field errors too",
      );

      const next = {
        pane: "form:modbus_tcp@runtime:local#endpoint:meter",
        tone: "ok",
        text: "I/O configuration saved. Restart the runtime to apply it.",
        field_errors: [],
        actions: [],
      };
      await rendered.update({ form: edit, notice: next, onValues });
      assert.strictEqual(
        container.querySelector('[data-role="form-notice-text"]').textContent,
        "I/O configuration saved. Restart the runtime to apply it.",
        "the next notice shows again",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-notice"]').getAttribute("data-tone"),
        "ok",
        "the next notice is drawn in its own tone",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("buttons_draw_by_emphasis_and_a_disabled_one_shows_its_reason", async () => {
    const FormView = loadComponent("FormView");
    // opcua_unreachable: Save, Cancel, Test and Apply, in Rust's order, each by its own emphasis; a
    // disabled action stays visible with its reason as text and tooltip (specification 25 §5.4:
    // add/write actions remain visible but neutral-disabled with a reason; §5.1).
    const rendered = await render(FormView, {
      form: form("opcua_unreachable"),
      onValues: () => undefined,
      onAction: () => undefined,
    });
    try {
      const container = rendered.container;
      const cells = [...container.querySelectorAll('[data-role="form-buttons"] [data-action]')];
      assert.deepStrictEqual(
        cells.map((cell: any) => cell.getAttribute("data-action")),
        ["save", "cancel", "test", "apply_live"],
        "the form's buttons are drawn in Rust's own order",
      );
      assert.deepStrictEqual(
        cells.map((cell: any) => cell.getAttribute("data-emphasis")),
        ["primary", "secondary", "quiet", "quiet"],
        "each button is drawn by its own emphasis",
      );
      assert.deepStrictEqual(
        cells.map((cell: any) => cell.querySelector('[data-role="action-button"]').textContent),
        ["Save", "Cancel", "Test", "Apply to running runtime"],
        "each button is drawn with its own label",
      );

      const testCell = cells.find((cell: any) => cell.getAttribute("data-action") === "test");
      const testButton = testCell.querySelector('[data-role="action-button"]');
      assert.strictEqual(testButton.disabled, true, "a disabled button is not clickable");
      assert.strictEqual(
        testButton.getAttribute("title"),
        "Select a reachable online runtime before testing Communication setup.",
        "the disabled button's reason is its tooltip",
      );
      assert.strictEqual(
        testCell.querySelector('[data-role="action-reason"]').textContent,
        "Select a reachable online runtime before testing Communication setup.",
        "the disabled button's reason is shown as text too",
      );

      const applyCell = cells.find(
        (cell: any) => cell.getAttribute("data-action") === "apply_live",
      );
      assert.strictEqual(
        applyCell.querySelector('[data-role="action-button"]').getAttribute("title"),
        "Select a reachable online runtime before applying Communication setup.",
        "the other disabled button carries its own reason",
      );

      const saveCell = cells.find((cell: any) => cell.getAttribute("data-action") === "save");
      assert.strictEqual(
        saveCell.querySelector('[data-role="action-button"]').disabled,
        false,
        "an enabled button stays clickable",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("remove_posts_only_after_its_confirmation", async () => {
    const App = loadComponent("App");
    // modbus_tcp_edit: Remove is a two-step action; nothing is posted until its confirming button
    // (specification 25 §5.4 and §5.13; inventory J10: a save/remove takes effect only when the
    // user confirms).
    const posted: any[] = [];
    const rendered = await render(App, {
      view: view(),
      post: (message: unknown) => {
        posted.push(message);
      },
      reply: formReply("modbus_tcp_edit"),
    });
    try {
      const container = rendered.container;
      assert.ok(container.querySelector('[data-role="form"]'), "the form pane is open");

      // Query the button again each time: the cell swaps the button for the confirmation and back.
      const removeButton = () =>
        container.querySelector('[data-action="remove"] [data-role="action-button"]');
      await click(rendered, removeButton());
      assert.strictEqual(posted.length, 0, "choosing Remove posts nothing yet");
      assert.strictEqual(
        container.querySelector('[data-role="confirm-text"]').textContent,
        "Remove this endpoint from the project? This writes the config file and takes effect after restart.",
        "the confirmation text is drawn as Rust gives it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="confirm-yes"]').textContent,
        "Confirm remove",
        "the confirming button is drawn as Rust gives it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="confirm-cancel"]').textContent,
        "Cancel",
        "the cancelling button is drawn as Rust gives it",
      );

      await click(rendered, container.querySelector('[data-role="confirm-cancel"]'));
      assert.strictEqual(posted.length, 0, "cancelling posts nothing");
      assert.strictEqual(
        container.querySelector('[data-role="confirm-text"]'),
        null,
        "cancelling closes the confirmation",
      );

      await click(rendered, removeButton());
      await click(rendered, container.querySelector('[data-role="confirm-yes"]'));
      assert.strictEqual(posted.length, 1, "confirming posts the action once");
      assert.strictEqual(posted[0].type, "action", "the confirming button posts in the shell's envelope");
      assert.strictEqual(
        posted[0].action.id,
        "remove",
        "the posted action is the button Rust gave, unchanged",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_button_posts_the_typed_values", async () => {
    const App = loadComponent("App");
    // modbus_tcp_edit: a chosen button posts the values the user typed, unchanged (specification 25
    // §5.4: Rust converts each submitted value by its schema type; §5.9).
    const posted: any[] = [];
    const rendered = await render(App, {
      view: view(),
      post: (message: unknown) => {
        posted.push(message);
      },
      reply: formReply("modbus_tcp_edit"),
    });
    try {
      const container = rendered.container;
      const address = container.querySelector('[data-role="field-input"][data-field="address"]');
      await type(rendered, address, "10.0.0.9:502");
      await click(
        rendered,
        container.querySelector('[data-action="test"] [data-role="action-button"]'),
      );
      assert.strictEqual(posted.length, 1, "the chosen button posts once");
      assert.strictEqual(posted[0].type, "action", "it posts in the shell's envelope");
      assert.strictEqual(posted[0].action.id, "test", "the chosen action is the one Rust gave");
      assert.deepStrictEqual(
        posted[0].values,
        {
          address: "10.0.0.9:502",
          unit_id: 1,
          input_start: 0,
          output_start: 0,
          timeout_ms: 500,
          on_error: "warn",
          // The advanced fields of the expected identity start empty; Rust drops a blank one when it writes.
          vendor_name: "",
          product_code: "",
          revision: "",
        },
        "the typed address is posted with every other value the form started from",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_reply_with_a_form_opens_the_form_pane_and_close_pane_closes_it", async () => {
    const App = loadComponent("App");
    // The host hands each `ActReply` to `App`: a `form` opens its pane, `close_pane` closes the pane
    // that started the action (design §1.4.1; specification 25 §5.11: one right-hand pane at a
    // time; §5.13).
    const rendered = await render(App, { view: view(), post: () => undefined });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="form"]'),
        null,
        "no form is open before a form reply",
      );

      await rendered.update({
        view: view(),
        post: () => undefined,
        reply: formReply("modbus_tcp_prefilled"),
      });
      assert.ok(
        container.querySelector('[data-role="form-pane"]'),
        "a form reply opens the form pane",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-title"]').textContent,
        "Add Modbus TCP",
        "the opened form is Rust's own form",
      );

      await rendered.update({
        view: view(),
        post: () => undefined,
        reply: {
          ok: true,
          notice: {
            pane: "form:modbus_tcp@runtime:local~f36970d12c317300",
            tone: "ok",
            text: "I/O configuration saved. Restart the runtime to apply it.",
            field_errors: [],
            actions: [],
          },
          effects: [],
          close_pane: true,
          refresh: "now",
        },
      });
      assert.strictEqual(
        container.querySelector('[data-role="form"]'),
        null,
        "close_pane closes the pane that started the action",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-pane"]'),
        null,
        "the form pane is gone",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("the_ghost_tile_is_drawn_while_its_form_is_open", async () => {
    const App = loadComponent("App");
    // modbus_tcp_prefilled: while a form with a ghost is open, the canvas draws the draft tile after
    // the devices, and it goes when the pane closes (design §4.5 D3; specification 25 §5.13).
    const withDevice = view({
      devices: [
        {
          id: "device-1",
          kind: "external",
          name: "Modbus meter",
          presents: "Modbus device",
        },
      ],
    });
    const rendered = await render(App, {
      view: withDevice,
      post: () => undefined,
      reply: formReply("modbus_tcp_prefilled"),
    });
    try {
      const container = rendered.container;
      const ghost = container.querySelector('[data-role="ghost"]');
      assert.ok(ghost, "the ghost tile is drawn while its form is open");
      assert.strictEqual(
        ghost.querySelector('[data-role="ghost-name"]').textContent,
        "New device",
        "the ghost's name is drawn as Rust gives it",
      );
      assert.strictEqual(
        ghost.querySelector('[data-role="ghost-detail"]').textContent,
        "Draft: configure and Apply",
        "the ghost's detail is drawn as Rust gives it",
      );
      const drawn = [...container.querySelectorAll('[data-role="device-card"], [data-role="ghost"]')];
      assert.deepStrictEqual(
        drawn.map((card: any) => card.getAttribute("data-role")),
        ["device-card", "ghost"],
        "the ghost is drawn after the devices",
      );

      await rendered.update({
        view: withDevice,
        post: () => undefined,
        reply: {
          ok: true,
          notice: {
            pane: "form:modbus_tcp@runtime:local~f36970d12c317300",
            tone: "ok",
            text: "I/O configuration saved. Restart the runtime to apply it.",
            field_errors: [],
            actions: [],
          },
          effects: [],
          close_pane: true,
          refresh: "now",
        },
      });
      assert.strictEqual(
        container.querySelector('[data-role="ghost"]'),
        null,
        "the ghost tile goes when its form closes",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
