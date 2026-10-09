import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type { CommandState, DrawFrame, HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { drawnTexts } from "../../hmiBuilder/drawing/contrast";
import { textWidth } from "../../hmiBuilder/drawing/format";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";

// Specification 39, §5.2 and §12.4–12.5: the setpoint entry and the buttons, with their command
// outcomes.

function draw(object: HmiObjectModel, frame: Partial<DrawFrame> = {}): SvgNode {
  const svg = drawPage({ title: "Controls", object: [object], pipe: [] }, { size: [1280, 720], values: {}, ...frame });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function words(node: SvgNode): string {
  return textsOf(node).join(" ");
}

function part(node: SvgNode, name: string): SvgNode[] {
  return findAll(node, (child) => child.attrs["data-part"] === name);
}

/** The left and right edges of a text as the drawing estimates them, its spans included. */
function edges(node: SvgNode): [number, number] {
  const spans = node.children.filter((child) => child.tag === "tspan");
  const size = (span: SvgNode): number => Number(span.attrs["font-size"] ?? node.attrs["font-size"] ?? 16);
  const width =
    spans.length > 0
      ? spans.reduce((sum, span) => sum + Number(span.attrs.dx ?? 0) + textWidth(span.text ?? "", size(span)), 0)
      : textWidth(node.text ?? "", size(node));
  const x = Number(node.attrs.x ?? 0);
  const anchor = node.attrs["text-anchor"] ?? "start";
  const left = anchor === "middle" ? x - width / 2 : anchor === "end" ? x - width : x;
  return [left, left + width];
}

/** The box of a part placed by `translate(x y)`, sized by its first rect. */
function placedBox(node: SvgNode): { x: number; y: number; width: number; height: number } {
  const [, x, y] = /translate\((-?[\d.]+)[ ,]+(-?[\d.]+)\)/.exec(node.attrs.transform ?? "") ?? ["", "0", "0"];
  const rect = findAll(node, (child) => child.tag === "rect")[0];
  return {
    x: Number(x),
    y: Number(y),
    width: Number(rect?.attrs.width ?? 0) + 1,
    height: Number(rect?.attrs.height ?? 0) + 1,
  };
}

function setpoint(extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return {
    id: "sp",
    type: "setpoint",
    at: [0, 0],
    size: [240, 96],
    rotation: 0,
    label: "Setpoint",
    unit: "°C",
    min: 0,
    max: 100,
    decimals: 1,
    bind: { value: { path: "Loop.Setpoint", direction: "read-write" } },
    ...extra,
  };
}

function button(type = "button", extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return {
    id: "start",
    type,
    at: [0, 0],
    size: [140, 44],
    rotation: 0,
    text: "Start",
    bind: { target: { path: "Main.Start", direction: "write" } },
    ...extra,
  };
}

const VALUES: Record<string, LiveValue> = { "Loop.Setpoint": { value: 61.5, state: "confirmed" } };

suite("HMI drawing: controls", () => {
  test("a setpoint entry shows the program's value, its limits and that it can be changed", () => {
    const node = draw(setpoint(), { values: VALUES });
    assert.ok(words(node).includes("61.5"), words(node));
    assert.ok(words(node).includes("0 – 100 °C"), words(node));
    assert.strictEqual(part(node, "edit").length, 1, renderToString(node));
  });

  for (const type of ["setpoint", "numeric-input"]) {
    test(`a short ${type} puts its label and value on one line, apart, with its edit mark inside`, () => {
      const node = draw(setpoint({ type, size: [200, 48], label: "Feed pump P101", unit: "rpm" }), { values: VALUES });
      const texts = findAll(node, (child) => child.tag === "text");
      const label = texts.find((child) => (child.text ?? "").startsWith("Feed"));
      const number = texts.find((child) => textsOf(child).includes("61.5"));
      assert.ok(label && number, renderToString(node));
      assert.strictEqual(label.attrs.y, number.attrs.y, "one line");
      assert.ok(
        edges(label)[1] <= edges(number)[0] - 4,
        `the label ends at ${edges(label)[1]}, the number starts at ${edges(number)[0]}`,
      );
      const edit = placedBox(part(node, "edit")[0]);
      assert.ok(
        edit.y >= 0 && edit.y + edit.height <= 48,
        `the edit mark from ${edit.y} to ${edit.y + edit.height} in a 48 px card`,
      );
      assert.ok(edges(number)[1] <= edit.x - 4, `the number ends at ${edges(number)[1]}, the mark starts at ${edit.x}`);
      const small = drawnTexts(node).filter((drawn) => !drawn.sign && drawn.size < 14);
      assert.deepStrictEqual(
        small.map((drawn) => drawn.text),
        [],
        "no words below 14 px",
      );
    });
  }

  test("a setpoint entry's number stays below its label's line at every height (§14.5 item 6)", () => {
    for (const height of [80, 96, 120]) {
      const node = draw(setpoint({ size: [240, height] }), { values: VALUES });
      const texts = findAll(node, (child) => child.tag === "text");
      const label = texts.find((child) => child.text === "Setpoint");
      const number = texts.find((child) => child.children.some((span) => span.text === "61.5"));
      assert.ok(label && number, renderToString(node));
      // A text's box: about 0.97 em above its baseline and 0.25 em below (the HMI font's metrics).
      const labelBottom = Number(label.attrs.y) + 0.25 * Number(label.attrs["font-size"]);
      const numberTop = Number(number.attrs.y) - 0.97 * Number(number.attrs["font-size"]);
      assert.ok(
        numberTop >= labelBottom + 1,
        `${height} px: the number's box starts at ${numberTop}, the label's ends at ${labelBottom}`,
      );
    }
  });

  test("a short setpoint entry shows a refusal's reason in place of its label, at 14 px", () => {
    const node = draw(setpoint({ size: [400, 48], label: "Feed pump P101" }), {
      values: VALUES,
      commands: { sp: { outcome: "refused", reason: "Interlock: tank level high" } },
    });
    const reason = part(node, "reason")[0];
    assert.ok(reason, renderToString(node));
    assert.strictEqual(reason.attrs.fill, "var(--hmi-alarm)");
    assert.strictEqual(Number(reason.attrs["font-size"]), 14);
    assert.ok(!words(node).includes("Feed"), "the reason takes the label's place");
    const number = findAll(node, (child) => child.tag === "text" && textsOf(child).includes("61.5"))[0];
    assert.strictEqual(reason.attrs.y, number?.attrs.y, "one line");
    assert.ok(
      edges(reason)[1] <= edges(number)[0] - 4,
      `the reason ends at ${edges(reason)[1]}, the number starts at ${edges(number)[0]}`,
    );
  });

  test("a narrow entry leaves out a label it could show only a letter or two of, and says Refused when the reason does not fit", () => {
    const narrow = draw(setpoint({ type: "numeric-input", size: [160, 48], label: "Feed pump P101", unit: "rpm" }), {
      values: VALUES,
    });
    assert.ok(
      !findAll(narrow, (child) => child.tag === "text").some((child) => (child.text ?? "").startsWith("F")),
      words(narrow),
    );
    const refused = draw(setpoint({ size: [240, 48], label: "Feed pump P101", unit: "rpm" }), {
      values: VALUES,
      commands: { sp: { outcome: "refused", reason: "Interlock: tank level high" } },
    });
    const word = part(refused, "outcome-word")[0];
    assert.strictEqual(word?.text, "Refused", words(refused));
    assert.strictEqual(word?.attrs.fill, "var(--hmi-alarm)");
    assert.strictEqual(
      part(refused, "reason").length,
      0,
      "a cut reason says less than the word; the display's notice gives it in full",
    );
  });

  test("an entry too narrow for the outcome's word shows its sign in place of the edit mark", () => {
    const node = draw(setpoint({ type: "numeric-input", size: [160, 48], label: "Feed pump P101", unit: "rpm" }), {
      values: VALUES,
      commands: { sp: { outcome: "refused", reason: "Interlock: tank level high" } },
    });
    const sign = part(node, "outcome-sign")[0];
    assert.ok(sign, renderToString(node));
    assert.deepStrictEqual(textsOf(sign), ["!"]);
    assert.strictEqual(part(node, "edit").length, 0, "the sign takes the edit mark's place");
  });

  test("a button's outcome sign never covers its text", () => {
    const node = draw(button("button", { size: [120, 48], text: "Open trends" }), {
      commands: { start: { outcome: "refused", reason: "No" } },
    });
    const label = findAll(node, (child) => child.tag === "text" && (child.text ?? "").startsWith("Open"))[0];
    const circle = findAll(part(node, "outcome-sign")[0] ?? node, (child) => child.tag === "circle")[0];
    assert.ok(label && circle, renderToString(node));
    const signLeft = Number(circle.attrs.cx) - Number(circle.attrs.r);
    assert.ok(edges(label)[1] <= signLeft - 2, `the text ends at ${edges(label)[1]}, the sign starts at ${signLeft}`);
  });

  test("without a value the setpoint entry shows the unknown mark, never a number", () => {
    const node = draw(setpoint(), { values: { "Loop.Setpoint": { value: null, state: "unknown" } } });
    assert.ok(words(node).includes("?"), words(node));
  });

  test("a button and a pulse button show their text", () => {
    assert.ok(words(draw(button())).includes("Start"), words(draw(button())));
    const jog = draw(button("pulse-button", { text: "Jog" }));
    assert.ok(words(jog).includes("Jog"), words(jog));
  });

  test("each command outcome shows on its control, a refusal with its reason", () => {
    const shown: Record<CommandState["outcome"], string> = {
      pending: "Sending",
      applied: "Applied",
      confirmed: "Confirmed",
      refused: "Refused",
      failed: "Failed",
      "timed-out": "Not confirmed",
      unknown: "Unknown",
    };
    for (const [outcome, word] of Object.entries(shown) as Array<[CommandState["outcome"], string]>) {
      const node = draw(setpoint(), { values: VALUES, commands: { sp: { outcome } } });
      assert.strictEqual(node.attrs["data-outcome"], outcome);
      assert.ok(words(node).includes(word), `${outcome}: ${words(node)}`);
    }
    const refused = draw(setpoint(), {
      values: VALUES,
      commands: { sp: { outcome: "refused", reason: "150 is above the highest allowed value 100" } },
    });
    assert.ok(words(refused).includes("150 is above"), words(refused));
    const pressed = draw(button(), { commands: { start: { outcome: "confirmed" } } });
    assert.strictEqual(pressed.attrs["data-outcome"], "confirmed");
    assert.strictEqual(draw(button()).attrs["data-outcome"], undefined, "no command, no outcome");
  });

  test("on a read-only page the controls look unavailable", () => {
    assert.strictEqual(draw(setpoint(), { values: VALUES, readOnly: true }).attrs["data-readonly"], "true");
    assert.strictEqual(draw(button(), { readOnly: true }).attrs["data-readonly"], "true");
    assert.strictEqual(draw(setpoint(), { values: VALUES }).attrs["data-readonly"], undefined);
  });
  test("a button whose interlock permit reads FALSE is greyed with the permit's reason as its tooltip (§8.4 item 4)", () => {
    const start = button("button", {
      bind: {
        interlock: { path: "Main.P101.StartPermit", direction: "read" },
        "interlock-reason": { path: "Main.P101.CommandText", direction: "read" },
      },
    });
    const auto = {
      "Main.P101.StartPermit": { value: false, state: "confirmed" },
      "Main.P101.CommandText": { value: "Auto: switch to Manual", state: "confirmed" },
    } as Record<string, LiveValue>;
    const held = draw(start, { values: auto });
    assert.strictEqual(held.attrs["data-held"], "true");
    assert.strictEqual(part(held, "face")[0]?.attrs.opacity, "0.45", "the face is greyed");
    const tips = findAll(held, (node) => node.tag === "title");
    assert.deepStrictEqual(
      tips.map((node) => node.text),
      ["Auto: switch to Manual"],
      "one tooltip with the reason, as text",
    );
    const manual = { ...auto, "Main.P101.StartPermit": { value: true, state: "confirmed" } } as Record<
      string,
      LiveValue
    >;
    const free = draw(start, { values: manual });
    assert.strictEqual(free.attrs["data-held"], undefined);
    assert.strictEqual(part(free, "face")[0]?.attrs.opacity, undefined);
    const unknown = draw(start, { values: {} });
    assert.strictEqual(unknown.attrs["data-held"], undefined, "an unknown permit holds nothing: the runtime decides");
    const refused = draw(start, {
      values: auto,
      commands: { start: { outcome: "refused", reason: "Not accepted: Auto: switch to Manual" } },
    });
    assert.deepStrictEqual(
      findAll(refused, (node) => node.tag === "title").map((node) => node.text),
      ["Not accepted: Auto: switch to Manual"],
      "the outcome's tooltip replaces the reason",
    );
  });

  test("read-only and role-locked controls keep their labels and values at full opacity", () => {
    for (const model of [
      button(),
      setpoint(),
      control("switch", "value", "Loop.Setpoint"),
      control("slider", "value", "Loop.Setpoint"),
    ]) {
      const node = draw(model, { values: VALUES, readOnly: true, locked: { [model.id]: "engineer" } });
      const inspect = (node: SvgNode, parent = 1): void => {
        const opacity = parent * Number(node.attrs.opacity ?? 1);
        if (node.tag === "text" || node.tag === "tspan")
          assert.strictEqual(opacity, 1, `${model.type}: ${textsOf(node).join(" ")} must remain readable`);
        node.children.forEach((child) => inspect(child, opacity));
      };
      inspect(node);
      if (model.type === "button") {
        const caption = findAll(node, (child) => child.tag === "text" && child.text === model.text)[0];
        assert.strictEqual(caption?.attrs.fill, "var(--hmi-text)", "disabled buttons use readable theme ink");
      }
      assert.strictEqual(node.attrs["data-locked"], "engineer");
    }
  });
});

function control(type: string, property: string, path: string, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return {
    id: type,
    type,
    at: [0, 0],
    size: [240, 44],
    rotation: 0,
    bind: { [property]: { path, direction: "read-write" } },
    ...extra,
  };
}

function numberAttr(node: SvgNode, name: string): number {
  return Number(node.attrs[name]);
}

suite("HMI drawing: switch, selector, mode selector and slider", () => {
  test("a switch shows On or Off by its knob and the word, and ? when its value is unknown", () => {
    const on = draw(control("switch", "value", "Main.Fan", { size: [80, 40] }), {
      values: { "Main.Fan": { value: true, state: "confirmed" } },
    });
    assert.strictEqual(on.attrs["data-on"], "true");
    assert.ok(words(on).includes("On"), words(on));
    const knobOn = part(on, "knob")[0];
    assert.ok(numberAttr(knobOn, "cx") > 40, renderToString(knobOn));
    const off = draw(control("switch", "value", "Main.Fan", { size: [80, 40] }), {
      values: { "Main.Fan": { value: false, state: "confirmed" } },
    });
    assert.strictEqual(off.attrs["data-on"], "false");
    assert.ok(words(off).includes("Off"), words(off));
    assert.ok(numberAttr(part(off, "knob")[0], "cx") < 40);
    const unknown = draw(control("switch", "value", "Main.Fan", { size: [80, 40] }), {
      values: { "Main.Fan": { value: null, state: "unknown" } },
    });
    assert.ok(words(unknown).includes("?"), words(unknown));
    assert.strictEqual(part(unknown, "knob").length, 0, "no knob, never the last state");
  });

  test("a switch keeps a control's usual size beside its label in a tall object (§5.8 item 3)", () => {
    const node = draw(control("switch", "value", "Main.Fan", { size: [320, 56], label: "Low water" }), {
      values: { "Main.Fan": { value: false, state: "confirmed" } },
    });
    const track = part(node, "track")[0];
    assert.strictEqual(numberAttr(track, "height"), 31, "32 px high, less the stroke");
    assert.strictEqual(numberAttr(track, "y"), 12.5, "centred in the object");
    assert.strictEqual(numberAttr(track, "width"), 69, "70 px wide, less the stroke");
    assert.strictEqual(numberAttr(part(node, "knob")[0], "r"), 13);
  });

  test("a switch may name its two states: a loop's Manual switch reads Auto or Manual (§5.8 item 3)", () => {
    const loop = (manual: boolean) =>
      draw(
        control("switch", "value", "Main.TIC101_Manual", {
          size: [320, 48],
          label: "Mode",
          options: ["Auto", "Manual"],
        }),
        { values: { "Main.TIC101_Manual": { value: manual, state: "confirmed" } } },
      );
    const auto = loop(false);
    assert.ok(words(auto).includes("Auto") && !words(auto).includes("Off"), words(auto));
    const manual = loop(true);
    assert.ok(words(manual).includes("Manual") && !words(manual).includes("On"), words(manual));
    assert.ok(numberAttr(part(manual, "track")[0], "width") > 69, "the track widens to its longer word");
  });

  test("a selector's label stays clear of its choice", () => {
    const node = draw(
      control("selector", "value", "Main.Speed", {
        size: [200, 48],
        options: ["Low", "High"],
        label: "Feed pump P101",
      }),
      { values: { "Main.Speed": { value: "LOW", state: "confirmed" } } },
    );
    const label = findAll(node, (child) => child.tag === "text" && (child.text ?? "").startsWith("Feed"))[0];
    const choice = part(node, "choice")[0];
    assert.ok(label && choice, renderToString(node));
    assert.ok(
      edges(label)[1] <= edges(choice)[0] - 10,
      `the label ends at ${edges(label)[1]}, the choice starts at ${edges(choice)[0]}`,
    );
  });

  test("a selector shows the current choice by name or by its number, with the mark that it opens", () => {
    const named = draw(control("selector", "value", "Main.Speed", { options: ["Off", "Low", "High"], label: "Fan" }), {
      values: { "Main.Speed": { value: "LOW", state: "confirmed" } },
    });
    assert.ok(words(named).includes("Low"), words(named));
    assert.ok(words(named).includes("Fan"), words(named));
    assert.strictEqual(part(named, "chevron").length, 1);
    const numbered = draw(control("selector", "value", "Main.Stage", { options: ["Stop", "Slow", "Fast"] }), {
      values: { "Main.Stage": { value: 2, state: "confirmed" } },
    });
    assert.ok(words(numbered).includes("Fast"), words(numbered));
    const unknown = draw(control("selector", "value", "Main.Stage", { options: ["Stop"] }), {
      values: { "Main.Stage": { value: null, state: "unknown" } },
    });
    assert.ok(words(unknown).includes("?"), words(unknown));
  });

  test("a mode selector shows its modes side by side with the active one lit", () => {
    const node = draw(control("mode-selector", "mode", "Loop.Mode"), {
      values: { "Loop.Mode": { value: "Manual", state: "confirmed" } },
    });
    const modes = findAll(node, (child) => child.attrs["data-option"] !== undefined);
    assert.deepStrictEqual(
      modes.map((mode) => mode.attrs["data-option"]),
      ["Manual", "Auto", "Track"],
    );
    assert.deepStrictEqual(
      modes.map((mode) => mode.attrs["data-active"]),
      ["true", undefined, undefined],
    );
    const narrowed = draw(control("mode-selector", "mode", "Loop.Mode", { options: ["Manual", "Auto"] }), {
      values: { "Loop.Mode": { value: "auto", state: "confirmed" } },
    });
    const offered = findAll(narrowed, (child) => child.attrs["data-option"] !== undefined);
    assert.deepStrictEqual(
      offered.map((mode) => [mode.attrs["data-option"], mode.attrs["data-active"]]),
      [
        ["Manual", undefined],
        ["Auto", "true"],
      ],
    );
  });

  test("a mode selector shows an enumeration's value names as words in the display's language, and writes the names", () => {
    const device = control("mode-selector", "mode", "P101.Mode", {
      size: [464, 48],
      options: ["Auto", "Manual", "OutOfService"],
    });
    const node = draw(device, { values: { "P101.Mode": { value: "OutOfService", state: "confirmed" } } });
    const modes = findAll(node, (child) => child.attrs["data-option"] !== undefined);
    assert.deepStrictEqual(
      modes.map((mode) => mode.attrs["data-option"]),
      ["Auto", "Manual", "OutOfService"],
      "the names it writes",
    );
    assert.deepStrictEqual(
      modes.map((mode) => words(mode)),
      ["Auto", "Manual", "Out of service"],
    );
    assert.strictEqual(modes[2].attrs["data-active"], "true");
    const swedish = draw(device, {
      values: { "P101.Mode": { value: "Auto", state: "confirmed" } },
      texts: { Manual: "Hand", "Out of service": "Ur drift" },
      language: "sv",
    });
    assert.deepStrictEqual(
      findAll(swedish, (child) => child.attrs["data-option"] !== undefined).map((mode) => words(mode)),
      ["Auto", "Hand", "Ur drift"],
    );
  });

  test("a slider places its thumb by the value between its limits, and shows a draft while it is dragged", () => {
    const slider = control("slider", "value", "Main.Valve", { min: 0, max: 80, size: [240, 44] });
    const node = draw(slider, { values: { "Main.Valve": { value: 40, state: "confirmed" } } });
    const track = part(node, "track")[0];
    const start = numberAttr(track, "x1");
    const end = numberAttr(track, "x2");
    const thumb = part(node, "thumb")[0];
    assert.ok(Math.abs(numberAttr(thumb, "cx") - (start + (end - start) / 2)) < 0.5, renderToString(node));
    assert.ok(words(node).includes("40"), words(node));
    const dragged = draw(slider, {
      values: { "Main.Valve": { value: 40, state: "confirmed" } },
      drafts: { slider: 60 },
    });
    const moved = part(dragged, "thumb")[0];
    assert.ok(Math.abs(numberAttr(moved, "cx") - (start + (end - start) * 0.75)) < 0.5, renderToString(dragged));
    assert.strictEqual(dragged.attrs["data-draft"], "true");
    assert.ok(words(dragged).includes("60"), words(dragged));
    const vertical = draw(
      { ...slider, variant: "vertical", size: [60, 240] },
      { values: { "Main.Valve": { value: 80, state: "confirmed" } } },
    );
    const top = part(vertical, "thumb")[0];
    const column = part(vertical, "track")[0];
    assert.ok(
      Math.abs(numberAttr(top, "cy") - Math.min(numberAttr(column, "y1"), numberAttr(column, "y2"))) < 0.5,
      "the maximum at the top",
    );
  });
});

suite("HMI drawing: control labels and outlines", () => {
  test("a switch outlines its outcome around the track, never around its label", () => {
    const node = draw(control("switch", "value", "Main.Fan", { size: [200, 44], label: "Heater" }), {
      values: { "Main.Fan": { value: true, state: "confirmed" } },
      commands: { switch: { outcome: "refused", reason: "Door open" } },
    });
    const outline = part(node, "outcome")[0];
    const track = part(node, "track")[0];
    assert.ok(outline && track, renderToString(node));
    assert.ok(Number(outline.attrs.x) > 100, `the outline starts at the track: ${outline.attrs.x}`);
    assert.ok(Math.abs(Number(outline.attrs.x) - Number(track.attrs.x)) <= 2.5, renderToString(node));
  });

  test("a slider shows its label above the track", () => {
    const node = draw(control("slider", "value", "Main.Valve", { size: [360, 64], label: "Manual output" }), {
      values: { "Main.Valve": { value: 40, state: "confirmed" } },
    });
    assert.ok(words(node).includes("Manual output"), words(node));
    const label = findAll(node, (child) => child.tag === "text" && textsOf(child).join("") === "Manual output")[0];
    const track = part(node, "track")[0];
    assert.ok(Number(label.attrs.y) < Number(track.attrs.y1), "the label is above the track");
  });
});

suite("HMI drawing: outcome signs", () => {
  test("a button shows its outcome by a sign as well as its outline", () => {
    const signs: Array<[CommandState["outcome"], string]> = [
      ["pending", "…"],
      ["applied", "✓"],
      ["confirmed", "✓"],
      ["refused", "!"],
      ["timed-out", "!"],
      ["unknown", "?"],
    ];
    for (const [outcome, sign] of signs) {
      const node = draw(button(), { commands: { start: { outcome } } });
      const mark = part(node, "outcome-sign")[0];
      assert.ok(mark, `${outcome}: ${renderToString(node)}`);
      assert.strictEqual(textsOf(mark).join(""), sign, outcome);
    }
    assert.strictEqual(part(draw(button()), "outcome-sign").length, 0, "no sign without a command");
  });
});

suite("HMI drawing: control targets", () => {
  test("every control takes a press anywhere on its box, not only on its painted parts", () => {
    const controls: HmiObjectModel[] = [
      setpoint(),
      button(),
      button("pulse-button"),
      control("switch", "value", "Main.Fan", { size: [200, 44], label: "Heater" }),
      control("selector", "value", "Main.Speed", { options: ["Off"] }),
      control("mode-selector", "mode", "Loop.Mode"),
      control("slider", "value", "Main.Valve"),
    ];
    for (const object of controls) {
      const node = draw(object);
      const target = part(node, "target")[0];
      assert.ok(target, `${object.type}: ${renderToString(node)}`);
      assert.deepStrictEqual(
        [target.attrs.x, target.attrs.y, target.attrs.width, target.attrs.height, target.attrs.fill],
        ["0", "0", String(object.size[0]), String(object.size[1]), "transparent"],
        object.type,
      );
    }
  });
});
