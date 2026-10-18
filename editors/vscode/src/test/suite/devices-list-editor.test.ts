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

// The list editor is drawn from the Rust view model (specification 25 §5.4 and §5.9, design §4.5
// D3). The expected values below are the literal texts of the golden form fixtures that
// `crates/trust-devices/src/panes/tests/golden.rs` writes; they are never read back from the
// component.

/** A webview component under test, bundled the way the extension ships it. */
type WebviewComponent = React.ComponentType<Record<string, unknown>>;

/** Bundles the list editor entry point and returns `ListEditor`, as the extension ships it. */
function loadListEditor(): WebviewComponent {
  const extensionRoot = path.resolve(__dirname, "..", "..", "..");
  const temp = fs.mkdtempSync(
    path.join(extensionRoot, "node_modules", ".trust-devices-list-editor-test-"),
  );
  const outfile = path.join(temp, "devices-list-editor.cjs");
  try {
    buildSync({
      entryPoints: [
        path.join(extensionRoot, "src", "devicesPanel", "webview", "ListEditor.tsx"),
      ],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).ListEditor as WebviewComponent;
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

/** The fixture's field with the value the test gives it. */
function withValue(listField: any, value: unknown): any {
  return { ...listField, value };
}

// JSDOM's elements stay untyped, as in devices-inspector-render.test.ts: the extension's tsconfig
// has no DOM lib, so a `HTMLElement` here would not name a type.
interface Rendered {
  container: any;
  window: any;
  unmount(): Promise<void>;
}

async function render(
  component: WebviewComponent,
  fieldValue: unknown,
  onChange: (value: unknown) => void,
): Promise<Rendered> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', {
    url: "https://devices-list-editor.test/",
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
    root.render(React.createElement(component, { field: fieldValue, onChange }));
  });
  return {
    container,
    window: dom.window,
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

suite("Devices & Connections list editor", () => {
  test("a_list_draws_each_item_under_its_numbered_title", async () => {
    const ListEditor = loadListEditor();
    // mesh_new `connect` is a list of peer addresses (specification 25 §5.4: json_array fields use
    // list editors; §5.9: the item texts come from the Rust catalog).
    const connect = field("mesh_new", "connect");
    const rendered = await render(
      ListEditor,
      withValue(connect, ["192.168.10.20:7447", "192.168.10.21:7447"]),
      () => undefined,
    );
    try {
      const titles = [...rendered.container.querySelectorAll('[data-role="list-item-title"]')].map(
        (title) => title.textContent,
      );
      assert.deepStrictEqual(
        titles,
        [`${connect.list.item_title} 1`, `${connect.list.item_title} 2`],
        "each item is drawn under the field's own item_title and its number from 1, never the item_noun or a number from 0",
      );
      const inputs = [...rendered.container.querySelectorAll('[data-role="list-item-input"]')].map(
        (input) => input.value,
      );
      assert.deepStrictEqual(
        inputs,
        ["192.168.10.20:7447", "192.168.10.21:7447"],
        "a scalar item is one input holding its value",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("an_empty_list_shows_its_empty_text", async () => {
    const ListEditor = loadListEditor();
    // The fixture's `connect` value is [] (specification 25 §5.9: an empty list shows its text).
    const connect = field("mesh_new", "connect");
    const rendered = await render(ListEditor, connect, () => undefined);
    try {
      const empty = rendered.container.querySelector('[data-role="list-empty"]');
      assert.strictEqual(
        empty.textContent,
        connect.list.empty_text,
        "the field's own empty_text shows while the list holds no items",
      );
      assert.strictEqual(
        rendered.container.querySelector('[data-role="list-item"]'),
        null,
        "no item is drawn while the list is empty",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("add_appends_the_item_template_and_remove_drops_the_item", async () => {
    const ListEditor = loadListEditor();
    const connect = field("mesh_new", "connect");
    // (Specification 25 §5.4: list editors add and remove items.) The values passed to onChange
    // are the assertion: add appends the field's item_template, remove drops exactly one item.
    const added: unknown[] = [];
    const adding = await render(ListEditor, withValue(connect, ["peer-a"]), (value) =>
      added.push(value),
    );
    try {
      await act(async () => {
        adding.container.querySelector('[data-role="list-add"]').click();
      });
      assert.deepStrictEqual(
        added,
        [["peer-a", ""]],
        "add appends the field's own item_template, never a copy of an existing item",
      );
    } finally {
      await adding.unmount();
    }

    const removed: unknown[] = [];
    const removing = await render(ListEditor, withValue(connect, ["peer-a", "peer-b"]), (value) =>
      removed.push(value),
    );
    try {
      const buttons = [...removing.container.querySelectorAll('[data-role="list-remove"]')];
      await act(async () => {
        buttons[1].click();
      });
      assert.deepStrictEqual(
        removed,
        [["peer-a"]],
        "the remove button of an item drops that item and no other",
      );
    } finally {
      await removing.unmount();
    }
  });

  test("an_object_item_draws_its_properties_and_option_labels", async () => {
    const ListEditor = loadListEditor();
    // ads_server_new `clients` is an object list whose items set ams_net_id and one of source_ip
    // and source_cidr (specification 25 §5.9).
    const clients = field("ads_server_new", "clients");
    const item = clients.list.item_template;
    const rendered = await render(ListEditor, withValue(clients, [item]), () => undefined);
    try {
      const labels = [
        ...rendered.container.querySelectorAll('[data-role="list-property-label"]'),
      ].map((label) => label.textContent);
      const expected = clients.list.properties
        .filter((property: any) => Object.prototype.hasOwnProperty.call(item, property.key))
        .map((property: any) => property.label);
      assert.deepStrictEqual(
        labels,
        expected,
        "an object item draws one input per property it sets, each under its own label",
      );
      const required = rendered.container.querySelector('[data-property="ams_net_id"]');
      assert.strictEqual(
        required.querySelector('[data-role="list-property-required"]').textContent,
        " *",
        "a required property is marked",
      );
      assert.strictEqual(
        rendered.container
          .querySelector('[data-property="source_ip"]')
          .querySelector('[data-role="list-property-required"]'),
        null,
        "an optional property carries no marker",
      );
      // The property's choices are drawn by their labels; this list's properties are text, and its
      // alternatives are drawn by their own labels beside the one the item sets.
      const alternative = rendered.container.querySelector('[data-role="list-alternative"]');
      const options = [...alternative.querySelectorAll("option")].map(
        (option: any) => option.textContent,
      );
      assert.deepStrictEqual(
        options,
        ["Source IP", "Source CIDR"],
        "the alternative is offered by its own label, never the raw key",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("an_item_offers_its_alternative_property_beside_the_one_it_sets", async () => {
    const ListEditor = loadListEditor();
    // (Specification 25 §5.9: the list editor offers each such property beside the one the item
    // starts with, as `Source CIDR` beside `Source IP`.)
    const clients = field("ads_server_new", "clients");
    const item = clients.list.item_template;
    const changes: unknown[] = [];
    const rendered = await render(ListEditor, withValue(clients, [item]), (value) =>
      changes.push(value),
    );
    try {
      const sourceIp = rendered.container.querySelector('[data-property="source_ip"]');
      assert.ok(sourceIp, "the item's Source IP property is drawn");
      const alternative = sourceIp.querySelector('[data-role="list-alternative"]');
      assert.ok(alternative, "Source IP offers the property it can set instead");
      const options = [...alternative.querySelectorAll("option")].map(
        (option: any) => option.textContent,
      );
      assert.deepStrictEqual(
        options,
        ["Source IP", "Source CIDR"],
        "the choice lists the properties by their own labels",
      );
      await choose(rendered, alternative, "source_cidr");
      assert.deepStrictEqual(
        changes,
        [[{ ams_net_id: "", source_cidr: "" }]],
        "choosing the alternative drops source_ip and sets source_cidr instead",
      );
    } finally {
      await rendered.unmount();
    }
  });

  test("add_is_disabled_when_the_list_cannot_grow", async () => {
    const ListEditor = loadListEditor();
    // opcua_unreachable `connections`: its items come from Discover or Browse, so can_add is false
    // (specification 25 §5.4: an action stays visible but neutral-disabled when it cannot act).
    const connections = field("opcua_unreachable", "connections");
    assert.strictEqual(connections.list.can_add, false, "the fixture's list cannot grow");
    const rendered = await render(ListEditor, connections, () => undefined);
    try {
      const add = rendered.container.querySelector('[data-role="list-add"]');
      assert.strictEqual(
        add.disabled,
        true,
        "the add button is disabled while the list cannot grow",
      );
      assert.strictEqual(
        add.textContent,
        connections.list.add_label,
        "the add button keeps the field's own add_label",
      );
    } finally {
      await rendered.unmount();
    }
  });
});
