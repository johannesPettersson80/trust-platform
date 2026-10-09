import type { Catalogue } from "../editor/catalogue";
import type { AlarmFrame, DrawFrame, HmiBindingModel, HmiObjectModel, HmiPageModel } from "../drawing/model";
import { designValues } from "../editor/designSamples";
import { DESIGN_NOW_MS } from "../editor/designHistory";
import { portPoint } from "../drawing/pipes";
import type { CompositionPattern } from "./patterns";

export interface PatternReference {
  origin: "synthetic";
  size: [number, number];
  model: HmiPageModel;
  values: DrawFrame["values"];
  alarms: AlarmFrame;
  missingComponents: string[];
  imageIndex?: number;
}
const read = (path: string): HmiBindingModel => ({ path: `Example.${path}`, direction: "read" });
const write = (path: string): HmiBindingModel => ({ path: `Example.${path}`, direction: "read-write" });

/** Explicit example topology and data, never inferred project facts. Every element uses the
 * normal builder model and shipped symbols; these examples do not add a drawing vocabulary. */
export function patternReference(pattern: CompositionPattern, catalogue: Catalogue): PatternReference {
  const model: HmiPageModel = { title: pattern.title, object: [], pipe: [] };
  const add = (
    id: string,
    type: string,
    at: [number, number],
    size: [number, number],
    properties: Partial<HmiObjectModel> = {},
  ): void => {
    model.object.push({ id, type, at, size, rotation: 0, bind: {}, ...properties });
  };
  const reading = (
    id: string,
    at: [number, number],
    width: number,
    label: string,
    unit: string,
    maximum: number,
    path = id,
  ): void => {
    add(id, "value", at, [width, 96], { label, unit, min: 0, max: maximum, decimals: 1, bind: { value: read(path) } });
  };
  const trend = (at: [number, number], size: [number, number]): void =>
    add("history", "trend", at, size, {
      label: "Tank level and setpoint",
      unit: "m",
      min: 0,
      max: 3,
      span: 300,
      bind: { Level: read("Level"), Setpoint: read("Setpoint") },
    });
  add("heading", "text", [32, 24], [1216, 40], { variant: "heading", text: pattern.title });
  add("example-note", "text", [32, 72], [1216, 28], {
    variant: "caption",
    text: "Synthetic design reference · Replace Example bindings with inspected project paths",
  });
  if (pattern.id === "process-overview") {
    add("process-area", "group", [32, 120], [792, 400], { label: "Transfer line · declared example branch" });
    add("vessel", "tank", [88, 192], [200, 260], {
      tag: "TK-101",
      unit: "m",
      min: 0,
      max: 3,
      bind: { level: read("Level") },
    });
    for (const [index, y] of [184, 360].entries())
      add(`pump-${index}`, "pump", [504, y], [160, 120], {
        tag: `P-10${index + 1}`,
        bind: { running: read(`Pump${index}.Running`), fault: read(`Pump${index}.Fault`) },
      });
    // The builder joins only free ports, so the declared branch runs through a junction on the
    // vessel outlet's line, as the canvas makes one when a pipe is dropped onto another.
    const outlet = portPoint(
      model.object.find((object) => object.id === "vessel")!,
      "outlet",
    )!.at;
    add("branch", "junction", [408, outlet[1] - 8], [16, 16]);
    model.pipe = [
      { from: "vessel.outlet", to: "branch.left" },
      { from: "branch.top", to: "pump-0.inlet" },
      { from: "branch.bottom", to: "pump-1.inlet" },
    ];
    reading("level", [856, 120], 392, "Tank level", "m", 3, "Level");
    reading("flow", [856, 240], 392, "Transfer flow", "m³/h", 80, "Flow");
    reading("pressure", [856, 360], 392, "Discharge pressure", "bar", 6, "Pressure");
    trend([32, 552], [600, 216]);
    add("alarms", "alarm-list", [656, 552], [592, 216]);
  } else if (pattern.id === "equipment-detail") {
    add("equipment-area", "group", [32, 120], [792, 400], { label: "TK-101 · storage vessel" });
    add("vessel", "tank", [96, 184], [240, 312], {
      tag: "TK-101",
      unit: "m",
      min: 0,
      max: 3,
      bind: { level: read("Level") },
    });
    reading("level", [416, 184], 368, "Process value", "m", 3, "Level");
    reading("output", [416, 312], 368, "Controller output", "%", 100, "Output");
    add("setpoint", "setpoint", [856, 120], [392, 112], {
      label: "Level setpoint",
      unit: "m",
      min: 0,
      max: 3,
      decimals: 1,
      bind: { value: write("Setpoint") },
    });
    add("mode", "mode-selector", [856, 264], [392, 64], { options: ["Manual", "Auto"], bind: { mode: write("Mode") } });
    add("control-note", "text", [856, 368], [392, 96], { text: "Named operator login required" });
    trend([32, 552], [1216, 216]);
  } else if (pattern.id === "multi-unit") {
    for (let index = 0; index < 3; index++) {
      const x = 32 + index * 416,
        prefix = `Unit${index}`;
      add(`card-${index}`, "group", [x, 120], [384, 400], { label: `P-10${index + 1} · transfer pump` });
      add(`pump-${index}`, "pump", [x + 96, 184], [192, 144], {
        tag: `P-10${index + 1}`,
        bind: { running: read(`${prefix}.Running`), fault: read(`${prefix}.Fault`) },
      });
      reading(`flow-${index}`, [x + 24, 384], 336, "Outlet flow", "m³/h", 80, `${prefix}.Flow`);
    }
    trend([32, 552], [1216, 216]);
  } else if (pattern.id === "investigation") {
    trend([32, 120], [1216, 288]);
    add("alarms", "alarm-list", [32, 440], [1216, 328]);
  } else {
    add("settings-area", "group", [32, 120], [584, 648], { label: "Process settings" });
    add("readback-area", "group", [648, 120], [600, 648], { label: "Readback and response" });
    add("setpoint", "setpoint", [64, 184], [520, 112], {
      label: "Level setpoint",
      unit: "m",
      min: 0.2,
      max: 2.6,
      decimals: 1,
      bind: { value: write("Setpoint") },
    });
    add("limit", "setpoint", [64, 336], [520, 112], {
      label: "High level alarm",
      unit: "m",
      min: 2,
      max: 3,
      decimals: 1,
      bind: { value: write("HighLimit") },
    });
    add("mode", "mode-selector", [64, 488], [520, 64], { options: ["Manual", "Auto"], bind: { mode: write("Mode") } });
    add("settings-note", "text", [64, 592], [520, 96], { text: "Verify units and limits before editing." });
    reading("level", [680, 184], 536, "Measured level", "m", 3, "Level");
    trend([680, 336], [536, 288]);
  }
  const values = designValues(model, catalogue);
  const set = (path: string, value: boolean | number | string): void => {
    values[`Example.${path}`] = { value, state: "confirmed" };
  };
  set("Level", 1.8);
  set("Setpoint", 2);
  set("HighLimit", 2.8);
  set("Mode", "Auto");
  for (let index = 0; index < 3; index++) {
    set(`Unit${index}.Running`, index === 0);
    set(`Unit${index}.Fault`, index === 2);
    set(`Unit${index}.Flow`, index === 0 ? 48 : 0);
  }
  set("Pump0.Running", true);
  set("Pump0.Fault", false);
  set("Pump1.Running", false);
  set("Pump1.Fault", true);
  const alarm = {
    path: "Example.Pump1.Trip",
    message: "Transfer pump fault",
    priority: "high",
    state: "active",
    active: true,
    unacknowledged: true,
    shelved: false,
    out_of_service: false,
    suppressed: false,
    first_out: true,
    annunciating: true,
    raise_count: 1,
    since_ms: DESIGN_NOW_MS - 30000,
    equipment: "Example.Pump1",
  };
  return {
    origin: "synthetic",
    size: [1280, 800],
    model,
    values,
    alarms: {
      summary: {
        active: [alarm],
        shelved: [],
        suppressed: [],
        out_of_service: [],
        unacknowledged: 1,
        annunciating: 1,
        newest: alarm,
      },
    },
    missingComponents: [...new Set(model.object.map((object) => object.type))].filter(
      (type) => !catalogue.components.some((component) => component.type === type),
    ),
  };
}
