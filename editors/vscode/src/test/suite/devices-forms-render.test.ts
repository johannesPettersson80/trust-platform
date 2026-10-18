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

// The form pane is drawn from the Rust view model (specification 25 §5.4, §5.9 and §5.13, design
// §4.5 D3). The expected values below are the literal texts of the golden form fixtures that
// `crates/trust-devices/src/panes/tests/golden.rs` writes; they are never read back from the
// component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles the form entry point and returns `FormView`, as the extension ships it. */
function loadFormView(): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-devices-form-view-test-"),
  );
  const outfile = path.join(temp, "devices-form-view.cjs");
  try {
    buildSync({
      entryPoints: [
        path.join(extensionRoot, "src", "devicesPanel", "webview", "FormView.tsx"),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).FormView as WebviewComponent;
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

/** The field `id` of a form fixture, wherever it sits in the sections. */
function field(name: string, id: string): any {
  for (const section of form(name).sections) {
    for (const candidate of section.fields) {
      if (candidate.id === id) {
        return candidate;
      }
    }
  }
  throw new Error(`${name} holds a field ${id}`);
}

// JSDOM's elements stay untyped, as in devices-list-editor.test.ts: the extension's tsconfig has no
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
    url: "https://devices-forms.test/",
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

/** Sets a `<select>`'s value the way a user does, past React's value tracker. */
async function choose(rendered: Rendered, select: any, value: string): Promise<void> {
  const setter = Object.getOwnPropertyDescriptor(
    rendered.window.HTMLSelectElement.prototype,
    "value",
  )?.set;
  assert.ok(setter, "HTMLSelectElement.value has a setter");
  await act(async () => {
    setter.call(select, value);
    select.dispatchEvent(new rendered.window.Event("change", { bubbles: true }));
  });
}

suite("Devices & Connections form view", () => {
  test("a_form_draws_its_header_and_sections_in_order", async () => {
    const FormView = loadFormView();
    // mqtt_new: the header texts and the Advanced section (specification 25 §5.13: an edit view
    // shows advanced fields under an `Advanced` heading, not collapsed).
    const rendered = await render(FormView, {
      form: form("mqtt_new"),
      onValues: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="form-title"]').textContent,
        "Add MQTT",
        "the title is drawn as Rust gives it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-breadcrumb"]').textContent,
        "Devices & Connections / Add device or connection",
        "the breadcrumb is drawn as Rust gives it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-owner"]').textContent,
        "on Simulator",
        "the owner line is drawn as Rust gives it",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-purpose"]').textContent,
        "Publish and subscribe process I/O through a broker.",
        "the purpose is drawn as Rust gives it",
      );

      const sections = [...container.querySelectorAll('[data-role="form-section"]')];
      assert.strictEqual(sections.length, 2, "both sections are drawn");
      assert.strictEqual(
        sections[0].querySelector('[data-role="form-section-heading"]'),
        null,
        "the first section carries no heading",
      );
      assert.deepStrictEqual(
        [...container.querySelectorAll('[data-role="form-section-heading"]')].map(
          (heading) => heading.textContent,
        ),
        ["Advanced"],
        "the Advanced heading comes after the first section, never before it or once per section",
      );
      assert.ok(
        sections[0].querySelector('[data-field="broker"]'),
        "the first section draws its own fields",
      );
      assert.ok(
        sections[1].querySelector('[data-field="tls_ca_path"]'),
        "the Advanced section is open: its fields are drawn, never collapsed",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("each_field_kind_draws_its_input", async () => {
    const FormView = loadFormView();
    // mqtt_new: a text, a number, a boolean, a choice and a list field (specification 25 §5.4:
    // boolean fields use On/Off checkboxes; §5.9: a choice is drawn by its option label).
    const rendered = await render(FormView, {
      form: form("mqtt_new"),
      onValues: () => undefined,
    });
    try {
      const container = rendered.container;
      const input = (id: string) =>
        container.querySelector(`[data-role="field-input"][data-field="${id}"]`);

      assert.strictEqual(input("broker").type, "text", "a text field is a text input");
      assert.strictEqual(
        input("broker").value,
        "127.0.0.1:1883",
        "a text field shows the field's own value",
      );
      assert.strictEqual(
        input("reconnect_ms").type,
        "text",
        "a number field is typed as text; Rust converts it on submit",
      );
      assert.strictEqual(
        input("reconnect_ms").value,
        "500",
        "a number field shows its value as text",
      );
      assert.strictEqual(
        input("tls").type,
        "checkbox",
        "a boolean field is an On/Off checkbox",
      );
      assert.strictEqual(input("tls").checked, false, "the checkbox shows the field's own off value");

      const onError = input("on_error");
      assert.strictEqual(onError.tagName, "SELECT", "a choice field is a select");
      assert.deepStrictEqual(
        [...onError.querySelectorAll("option")].map((option: any) => option.textContent),
        ["Stop with fault", "Warn and continue", "Ignore"],
        "each choice is drawn by its own option label, never the raw value",
      );
      assert.deepStrictEqual(
        [...onError.querySelectorAll("option")].map((option: any) => option.value),
        ["fault", "warn", "ignore"],
        "each choice is sent by its own option value",
      );
      assert.strictEqual(onError.value, "fault", "the choice starts at the field's own value");

      assert.ok(
        container.querySelector('[data-role="list-editor"][data-field="tls_alpn"]'),
        "a list field is drawn by the shared list editor",
      );

      const brokerField = container.querySelector('[data-role="form-field"][data-field="broker"]');
      assert.strictEqual(
        brokerField.querySelector('[data-role="field-label"]').textContent,
        "Broker",
        "the field's own label is drawn",
      );
      assert.ok(
        brokerField.querySelector('[data-role="field-required"]'),
        "a required field carries its mark",
      );
      assert.strictEqual(
        brokerField.querySelector('[data-role="field-help"]').textContent,
        "MQTT broker host and port.",
        "the field's own help is drawn",
      );
      assert.strictEqual(
        container
          .querySelector('[data-role="form-field"][data-field="client_id"]')
          .querySelector('[data-role="field-required"]'),
        null,
        "an optional field carries no mark",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_secret_field_never_shows_a_value", async () => {
    const FormView = loadFormView();
    // mqtt_new password: a secret starts empty and never shows a stored value (specification 25
    // §5.4: secret help states saved secrets are not shown).
    const password = field("mqtt_new", "password");
    assert.strictEqual(password.value, null, "the fixture stores no displayable password");

    const rendered = await render(FormView, {
      form: form("mqtt_new"),
      onValues: () => undefined,
    });
    try {
      const input = rendered.container.querySelector(
        '[data-role="field-input"][data-field="password"]',
      );
      assert.strictEqual(input.type, "password", "a secret field is a password input");
      assert.strictEqual(input.value, "", "the secret field starts empty");
    } finally {
      await rendered.unmount();
    }

    // Even a value Rust might carry is never shown: the field starts empty.
    const withStored = form("mqtt_new");
    for (const candidate of withStored.sections[0].fields) {
      if (candidate.id === "password") {
        candidate.value = "stored-secret";
      }
    }
    const second = await render(FormView, {
      form: withStored,
      onValues: () => undefined,
    });
    try {
      const input = second.container.querySelector(
        '[data-role="field-input"][data-field="password"]',
      );
      assert.strictEqual(input.value, "", "a stored secret is never shown");
    } finally {
      await second.unmount();
    }
  });

  test("a_field_appears_while_its_visible_when_matches", async () => {
    const FormView = loadFormView();
    // gpio_new: `chip` is shown while Backend is libgpiod and `sysfs_base` while it is sysfs
    // (specification 25 §5.9: the panel uses the declared predicate to show or hide the field while
    // the user types).
    const changes: any[] = [];
    const rendered = await render(FormView, {
      form: form("gpio_new"),
      onValues: (values: unknown) => changes.push(values),
    });
    try {
      const container = rendered.container;
      const drawn = (id: string) =>
        container.querySelector(`[data-role="form-field"][data-field="${id}"]`);
      assert.ok(drawn("chip"), "the field of the matching branch is shown");
      assert.strictEqual(drawn("sysfs_base"), null, "the field of the other branch is hidden");

      const backend = container.querySelector('[data-role="field-input"][data-field="backend"]');
      await choose(rendered, backend, "sysfs");
      assert.strictEqual(
        drawn("chip"),
        null,
        "changing the named field hides the dependent field as the user types",
      );
      assert.ok(drawn("sysfs_base"), "the other branch appears");
      assert.strictEqual(
        changes[changes.length - 1].chip,
        "/dev/gpiochip0",
        "a hidden field keeps its value",
      );

      await choose(rendered, backend, "libgpiod");
      assert.ok(drawn("chip"), "changing the named field back shows the dependent field again");
      assert.strictEqual(drawn("sysfs_base"), null, "the other branch hides again");
      assert.strictEqual(
        changes[changes.length - 1].chip,
        "/dev/gpiochip0",
        "the hidden field's value survived the round trip",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("typed_values_survive_a_new_form_with_the_same_id_and_reset_with_another_id", async () => {
    const FormView = loadFormView();
    // J07: the values are kept until the form's id changes.
    const changes: any[] = [];
    const onValues = (values: unknown) => changes.push(values);
    const rendered = await render(FormView, { form: form("mqtt_new"), onValues });
    try {
      const broker = () =>
        rendered.container.querySelector('[data-role="field-input"][data-field="broker"]');
      await type(rendered, broker(), "broker.example:1883");
      assert.strictEqual(
        changes[changes.length - 1].broker,
        "broker.example:1883",
        "a typed text is reported as the string the user typed",
      );
      assert.strictEqual(
        typeof changes[changes.length - 1].broker,
        "string",
        "a text stays a string, never coerced",
      );
      assert.deepStrictEqual(
        changes[changes.length - 1].tls_alpn,
        [],
        "a list stays its array, never stringified",
      );

      const sameId = form("mqtt_new");
      for (const candidate of sameId.sections[0].fields) {
        if (candidate.id === "broker") {
          candidate.value = "form-default:1883";
        }
      }
      await rendered.update({ form: sameId, onValues });
      assert.strictEqual(
        broker().value,
        "broker.example:1883",
        "a new form with the same id keeps what the user typed, never a fresh default",
      );

      const otherId = form("mqtt_new");
      otherId.id = "mqtt@runtime:other";
      for (const candidate of otherId.sections[0].fields) {
        if (candidate.id === "broker") {
          candidate.value = "other:1883";
        }
      }
      await rendered.update({ form: otherId, onValues });
      assert.strictEqual(
        broker().value,
        "other:1883",
        "another id starts again from its own values, never the typed ones",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("an_edit_form_starts_from_the_connections_values", async () => {
    const FormView = loadFormView();
    // modbus_tcp_edit: an edit form starts from the connection's values, and its note is drawn
    // (specification 25 §5.13: the edit view keeps §5.4).
    const rendered = await render(FormView, {
      form: form("modbus_tcp_edit"),
      onValues: () => undefined,
    });
    try {
      const container = rendered.container;
      assert.strictEqual(
        container.querySelector('[data-role="form-title"]').textContent,
        "Modbus TCP",
        "the edit form's title is drawn as Rust gives it",
      );
      const input = (id: string) =>
        container.querySelector(`[data-role="field-input"][data-field="${id}"]`);
      assert.strictEqual(
        input("address").value,
        "192.168.1.50:502",
        "a text field starts from the connection's value, never an empty default",
      );
      assert.strictEqual(
        input("unit_id").value,
        "1",
        "a number field starts from the connection's value, as text",
      );
      assert.strictEqual(
        input("on_error").value,
        "warn",
        "a choice starts at the connection's value",
      );
      assert.strictEqual(
        container.querySelector('[data-role="form-note"]').textContent,
        "This endpoint is disabled. Use Enable to turn it back on; restart the runtime to apply the change.",
        "the form's note is drawn as Rust gives it",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_rust_button_posts_its_values_once_and_keeps_the_mqtt_password", async () => {
    const FormView = loadFormView();
    // mqtt_new: Save is a Rust action, so it posts the typed values once (specification 25 §5.4; design D5b
    // R7). The password is not one that Rust makes the form forget (design D5c R12), so it stays after the
    // post. A wrong implementation empties every secret after a post, or posts the values twice.
    const posted: unknown[][] = [];
    const rendered = await render(FormView, {
      form: form("mqtt_new"),
      onValues: () => undefined,
      onAction: (...args: unknown[]) => posted.push(args),
    });
    try {
      const container = rendered.container;
      await type(
        rendered,
        container.querySelector('[data-role="field-input"][data-field="password"]'),
        "s3cret",
      );
      const save = [...container.querySelectorAll('[data-role="action-button"]')].find(
        (button: any) => button.textContent === "Save",
      );
      await act(async () => {
        save.click();
      });
      assert.strictEqual(posted.length, 1, "Save posts once per click");
      assert.deepStrictEqual(posted[0][0], form("mqtt_new").buttons[0], "Save posts its own action");
      assert.strictEqual(
        (posted[0][1] as any).password,
        "s3cret",
        "the typed secret goes with the Rust action",
      );
      assert.strictEqual(
        container.querySelector('[data-role="field-input"][data-field="password"]').value,
        "s3cret",
        "the password stays in the form after the post",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_refused_save_keeps_the_mqtt_password", async () => {
    const FormView = loadFormView();
    // mqtt_new: Rust refuses the Save and answers with a notice in the form's own pane. The password the user
    // typed is still in the form, so the user can correct the settings and Save again (design D5c R12). A
    // wrong implementation empties the password after the post.
    const rendered = await render(FormView, {
      form: form("mqtt_new"),
      onValues: () => undefined,
      onAction: () => undefined,
    });
    try {
      const container = rendered.container;
      await type(
        rendered,
        container.querySelector('[data-role="field-input"][data-field="password"]'),
        "s3cret",
      );
      const save = [...container.querySelectorAll('[data-role="action-button"]')].find(
        (button: any) => button.textContent === "Save",
      );
      await act(async () => {
        save.click();
      });
      await rendered.update({
        form: form("mqtt_new"),
        onValues: () => undefined,
        onAction: () => undefined,
        notice: {
          pane: "form:mqtt_new",
          tone: "error",
          text: "The broker refused these settings.",
          field_errors: [],
          actions: [],
        },
      });
      assert.strictEqual(
        container.querySelector('[data-role="field-input"][data-field="password"]').value,
        "s3cret",
        "a refused Save keeps the password",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("a_ui_button_posts_no_values", async () => {
    const FormView = loadFormView();
    // Cancel is a `ui` op: the shell carries it out and posts no values (design D5b R7). A wrong
    // implementation sends the typed values with it.
    const posted: unknown[][] = [];
    const rendered = await render(FormView, {
      form: form("mqtt_new"),
      onValues: () => undefined,
      onAction: (...args: unknown[]) => posted.push(args),
    });
    try {
      const cancel = [...rendered.container.querySelectorAll('[data-role="action-button"]')].find(
        (button: any) => button.textContent === "Cancel",
      );
      await act(async () => {
        cancel.click();
      });
      assert.deepStrictEqual(
        posted,
        [[form("mqtt_new").buttons[1]]],
        "Cancel is posted alone, with no values",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
