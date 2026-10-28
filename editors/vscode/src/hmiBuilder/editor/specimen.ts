// The design specimen (specification 39, §14.1 items 1 and 4): every phase 1 component of the
// catalogue, and each of its variants, drawn in every state a page can show, so the design
// system's text can be measured as the drawing really draws it.

import { drawPage } from "../drawing";
import type {
  AlarmFrame,
  AlarmRowModel,
  CommandState,
  DrawFrame,
  HmiObjectModel,
  HmiPageModel,
  LiveValue,
} from "../drawing/model";
import { withDataState } from "../drawing/previews";
import type { SvgNode } from "../drawing/svg";
import type { Catalogue, ComponentDefinition } from "./catalogue";
import { designValues } from "./designSamples";

export const SPECIMEN_STATES = [
  "normal",
  "unknown",
  "stale",
  "commanded",
  "pending",
  "refused",
  "read-only",
  "missing",
  "alarms",
] as const;
export type SpecimenState = (typeof SPECIMEN_STATES)[number];

const COLUMN = 760;
const ROW = 400;
const COLUMNS = 4;
/** The equipment every specimen path belongs to, so each object shows its equipment's alarms. */
const EQUIPMENT = "Specimen";

/** A size turned to a variant's orientation: a horizontal bar is wide, a vertical slider tall. */
function oriented([width, height]: [number, number], variant?: string): [number, number] {
  const turn = (variant === "horizontal" && height > width) || (variant === "vertical" && width > height);
  return turn ? [height, width] : [width, height];
}

/** One object of a component, with every property bound and every text it can show set. */
function specimenObject(component: ComponentDefinition, index: number, variant?: string): HmiObjectModel {
  const id = variant ? `${component.type}-${variant}` : component.type;
  const bind: HmiObjectModel["bind"] = {};
  for (const property of Object.keys(component.properties ?? {})) {
    bind[property] = { path: `${EQUIPMENT}.${id}.${property}`, direction: "read" };
  }
  if (component.additional) {
    for (const pen of ["pv", "sp"]) bind[pen] = { path: `${EQUIPMENT}.${id}.${pen}`, direction: "read" };
  }
  return {
    id,
    type: component.type,
    ...(variant ? { variant } : {}),
    at: [20 + (index % COLUMNS) * COLUMN, 20 + Math.floor(index / COLUMNS) * ROW],
    size: oriented(component.size, variant),
    rotation: 0,
    label: "Feed pump P101",
    text: "Open trends",
    unit: "rpm",
    decimals: 1,
    min: 0,
    max: 100,
    ...(component.type === "image" ? { image: "pump.png" } : {}),
    ...(component.type === "selector" ? { options: ["Low", "High"] } : {}),
    ...(component.type === "mode-selector" ? { options: ["Manual", "Auto", "Track"] } : {}),
    ...(component.type === "page-button" ? { click: { page: "trends" } } : {}),
    bind,
  };
}

/** The specimen page: each phase 1 component, then each of its other variants. */
export function specimenPage(catalogue: Catalogue): HmiPageModel {
  const objects: HmiObjectModel[] = [];
  for (const component of catalogue.components.filter((candidate) => candidate.phase === 1)) {
    objects.push(specimenObject(component, objects.length));
  }
  for (const component of catalogue.components.filter((candidate) => candidate.phase === 1)) {
    for (const variant of component.variants ?? []) objects.push(specimenObject(component, objects.length, variant));
  }
  if (catalogue.components.some((component) => component.type === "status")) {
    objects.push(
      specimenObject(
        catalogue.components.find((component) => component.type === "status")!,
        objects.length,
        "alarm",
      ),
    );
  }
  return { title: "Specimen", object: objects, pipe: [] };
}

function row(path: string, priority: string, state: string, extra: Partial<AlarmRowModel> = {}): AlarmRowModel {
  return {
    path: `${EQUIPMENT}.${path}`,
    message: `${priority} alarm of the specimen`,
    priority,
    state,
    active: state !== "rtn_unack",
    unacknowledged: state.includes("unack"),
    shelved: false,
    out_of_service: false,
    suppressed: false,
    first_out: false,
    annunciating: state.includes("unack"),
    raise_count: 1,
    since_ms: 1_000,
    equipment: EQUIPMENT,
    ...extra,
  } as AlarmRowModel;
}

/** Alarms of every priority and state, and a record of every kind. */
function specimenAlarms(): AlarmFrame {
  const active = [
    row("High", "high", "unack", { first_out: true, latching: true }),
    row("Medium", "medium", "ack"),
    row("Low", "low", "rtn_unack"),
  ];
  const kinds = [
    "raised",
    "cleared",
    "acknowledged",
    "shelved",
    "unshelved",
    "suppressed",
    "unsuppressed",
    "out_of_service",
    "in_service",
    "comment",
    "reset",
    "command",
  ];
  return {
    summary: {
      active,
      shelved: [row("Shelved", "medium", "ack", { shelved: true })],
      suppressed: [row("Suppressed", "low", "ack", { suppressed: true })],
      out_of_service: [row("Service", "high", "ack", { out_of_service: true })],
      unacknowledged: 2,
      annunciating: 2,
      newest: active[0],
    },
    records: kinds.map((kind, index) => ({
      seq: index + 1,
      time_ms: 1_000 * (index + 1),
      path: `${EQUIPMENT}.High`,
      kind,
      message: `The specimen's ${kind}`,
      priority: ["high", "medium", "low"][index % 3],
      user: "operator",
      detail: kind === "comment" ? "Checked on site" : undefined,
      outcome: kind === "command" ? "refused" : undefined,
    })),
  };
}

/** The frame of one state. */
function specimenFrame(page: HmiPageModel, catalogue: Catalogue, state: SpecimenState): DrawFrame {
  const samples = designValues(page, catalogue);
  const each = (change: (value: LiveValue) => LiveValue): Record<string, LiveValue> =>
    Object.fromEntries(Object.entries(samples).map(([path, value]) => [path, change(value)]));
  const controls = page.object.filter(
    (object) => catalogue.components.find((component) => component.type === object.type)?.category === "control",
  );
  const commands = (command: CommandState): Record<string, CommandState> =>
    Object.fromEntries(controls.map((object) => [object.id, command]));
  const history: Record<string, Array<[number, number]>> = {};
  for (const path of Object.keys(samples))
    history[path] = Array.from({ length: 30 }, (_, index) => [index * 10_000, 40 + 20 * Math.sin(index / 4)]);
  const base: DrawFrame = {
    size: [COLUMNS * COLUMN + 40, Math.ceil(page.object.length / COLUMNS) * ROW + 40],
    values: samples,
    history,
    nowMs: 300_000,
    asset: (path) => path,
  };
  switch (state) {
    case "normal":
      return base;
    case "unknown":
      return { ...base, values: {}, history: {} };
    case "stale":
      return { ...base, values: each((value) => withDataState(value, "stale")) };
    case "commanded":
      return { ...base, values: each((value) => withDataState(value, "commanded")) };
    case "pending":
      return { ...base, commands: commands({ outcome: "pending" }) };
    case "refused":
      return { ...base, commands: commands({ outcome: "refused", reason: "Interlock: tank level high" }) };
    case "read-only":
      return { ...base, readOnly: true };
    case "missing":
      return { ...base, missing: Object.keys(samples) };
    case "alarms":
      return { ...base, alarms: specimenAlarms() };
  }
}

/** The specimen drawn in every state. */
export function specimenDrawings(catalogue: Catalogue): Array<{ state: SpecimenState; svg: SvgNode }> {
  const page = specimenPage(catalogue);
  return SPECIMEN_STATES.map((state) => ({ state, svg: drawPage(page, specimenFrame(page, catalogue, state)) }));
}
