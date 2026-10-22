// The equipment symbols (specification 39, §14.2, §14.3): two-dimensional and based on P&ID
// symbols; what moves in the plant moves in the symbol. One module per symbol, one shared style.

import type { SvgNode } from "../svg";
import { customSymbol } from "./custom";
import type { CustomSymbolModel } from "./custom";
import type { StateValue, SymbolReader } from "./reading";
import type { PartView, Rule } from "./rules";
import { round } from "./style";
import { CONTROL_VALVE } from "./controlValve";
import { DEVICE } from "./device";
import { DOSING_PUMP } from "./dosingPump";
import { BREAKER, CONTACTOR, DRIVE, VFD } from "./electrical";
import { GRAVITY_TANK } from "./gravityTank";
import {
  ANALYZER,
  CONTROLLER,
  instrument,
  LEVEL_SWITCH,
  PRESSURE_SWITCH,
  SENSOR_PAIR,
  DIGITAL_SWITCH,
} from "./instrument";
import { NETWORK_LINK } from "./link";
import { AXIS_INDICATOR, COUPLING, DRUM, ENCODER, MOTOR, ROTARY_ARM, SERVO_MOTOR, SHAFT } from "./machines";
import { MIXER } from "./mixer";
import { AGITATOR, BLOWER, CONVEYOR, CYLINDER, DAMPER, FAN, MOTOR_VALVE, WEIGH_SCALE } from "./plant";
import { CRUSHER, CYCLONE, FILTER, HEAT_EXCHANGER, SCREEN, VESSEL } from "./process";
import { PUMP } from "./pump";
import { SUPPLY } from "./supply";
import { TANK } from "./tank";
import { TANK_CASCADE } from "./tankCascade";
import {
  COLD_ROOM,
  COOLER,
  COUPLED_VESSELS,
  HEAT_DRAW_OFF,
  HEATER,
  JACKETED_VESSEL,
  REFRIGERATION_UNIT,
} from "./thermal";
import { VALVE } from "./valve";
import { LABEL_HEIGHT, SYMBOL_GEOMETRY, type SymbolLabel, type SymbolPort } from "./geometry";

// The shipped symbols' geometry is the runtime's table (`symbol_geometry.json`), which the language server's routes read too.
export { LABEL_HEIGHT };
export type { SymbolLabel, SymbolPort };

export { SPIN_PERIOD_MS } from "./motion";

/** The project's own symbols, by type (`my:<name>`), as the editor and the display were given them. */
const REGISTERED = new Map<string, SymbolDefinition>();
const TITLES = new Map<string, string>();

/** Makes the project's own symbols known to the drawing (§14.4); a symbol given again replaces
 * the one it had. */
export function registerSymbols(models: CustomSymbolModel[]): void {
  for (const model of models) {
    REGISTERED.set(`my:${model.name}`, customSymbol(model));
    TITLES.set(`my:${model.name}`, model.title);
  }
}

/** Synchronous drawing scope for an isolated project preview. Restore the caller's registry
 * even when drawing fails; no asynchronous work may run inside the callback. */
export function withProjectSymbols<T>(models: CustomSymbolModel[], draw: () => T): T {
  const previous = new Map(REGISTERED),
    titles = new Map(TITLES);
  REGISTERED.clear();
  TITLES.clear();
  try {
    registerSymbols(models);
    return draw();
  } finally {
    REGISTERED.clear();
    TITLES.clear();
    for (const [key, value] of previous) REGISTERED.set(key, value);
    for (const [key, value] of titles) TITLES.set(key, value);
  }
}

/** A project symbol's title, by its type. */
export function symbolTitle(type: string): string | undefined {
  return TITLES.get(type);
}

/** A type's symbol: a shipped one, or one of the project's own. */
export function symbolFor(type: string): SymbolDefinition | undefined {
  return (SYMBOLS as Partial<Record<string, SymbolDefinition>>)[type] ?? REGISTERED.get(type);
}

/** What a symbol shows of its equipment's state (§5.3, item 4). */
export interface SymbolView {
  /** The state is unknown: no moving part, the body hatched. */
  unknown: boolean;
  /** The impeller turns, with the delay that continues its turn. */
  turning?: { delayMs: number };
  /** How far the valve's closing element stands open, from 0 closed to 1 open. */
  opening?: number;
  /** The valve body is filled, as a closed valve is on a P&ID. */
  filled?: boolean;
  /** What the visualization rules give each part (§5.3 item 5). */
  parts?: Record<string, PartView>;
  /** The object's variant, for a symbol drawn in more than one form. */
  variant?: string;
  /** Which way something passes the symbol, as heat through a wall. */
  direction?: "left" | "right";
  /** The form a symbol takes in a state, as a network link lost or holding. */
  form?: string;
  /** The object's tag, for a symbol that shows its letters, as a controller's bubble does. */
  tag?: string;
}

/** A state the editor's switcher shows (§5.3, item 6): the values of the bindings that show it. */
export interface SymbolState {
  id: string;
  /** Its name on the switcher. */
  name: string;
  values: Record<string, StateValue>;
  /** The bindings it needs, each written `a|b` when either will do. */
  needs: string[];
}

export interface SymbolDefinition {
  /** The symbol's design size in pixels, without its label. */
  size: [number, number];
  /** Where its label stands; under it when not given. */
  label?: SymbolLabel;
  ports: Record<string, SymbolPort>;
  /** The bindings that decide its state, the main one first. */
  properties: string[];
  /** Those whose value decides the state, so that one unknown makes it unknown. */
  deciding: string[];
  states: SymbolState[];
  /** The state the editor's design samples show. */
  sample: string;
  /** What the bindings say: the state, its word, a fault and the view (§5.3 item 4). */
  read: SymbolReader;
  /** How the bindings drive its parts (§5.3 item 5). */
  rules?: Rule[];
  /**
   * The outline the symbol draws, in its own pixels: path data of absolute commands (M, L, H, V,
   * Q, C, A, Z), each subpath closed, as a vessel's walls with their rounded or dished ends. An
   * instrument attached to the symbol ends its line on this outline (§21.6); a symbol without one
   * is met at the box it stands in.
   */
  outline?: string;
  /** The symbol's parts in its own pixels, for this view. */
  draw(view: SymbolView): SvgNode[];
}

/** Where a symbol stands in its object: scaled to fit above the label, centred (§14.2); or, with
 * its label beside it as an instrument has, at the left, fitting the height and taking at most
 * half the width, so the label keeps its room. */
export function symbolPlacement(
  size: [number, number],
  symbolSize: [number, number],
  label: SymbolLabel = "under",
): { left: number; top: number; scale: number; area: number } {
  const [width, height] = size;
  const [symbolWidth, symbolHeight] = symbolSize;
  if (label === "beside") {
    const fit = Math.max(0, Math.min(height / symbolHeight, width / 2 / symbolWidth));
    return { left: 0, top: round((height - symbolHeight * fit) / 2), scale: fit, area: height };
  }
  const area = Math.max(0, height - LABEL_HEIGHT);
  const scale = Math.max(0, Math.min(width / symbolWidth, area / symbolHeight));
  return { left: round((width - symbolWidth * scale) / 2), top: round((area - symbolHeight * scale) / 2), scale, area };
}

export type SymbolType =
  | "pump"
  | "valve"
  | "control-valve"
  | "tank"
  | "gravity-tank"
  | "tank-cascade"
  | "dosing-pump"
  | "supply"
  | "mixer"
  | "temperature-sensor"
  | "level-sensor"
  | "flow-sensor"
  | "pressure-sensor"
  | "position-sensor"
  | "speed-sensor"
  | "sensor-pair"
  | "controller"
  | "cold-room"
  | "refrigeration-unit"
  | "heater"
  | "cooler"
  | "jacketed-vessel"
  | "coupled-vessels"
  | "heat-draw-off"
  | "motor"
  | "device"
  | "servo-motor"
  | "shaft"
  | "drum"
  | "coupling"
  | "encoder"
  | "rotary-arm"
  | "axis-indicator"
  | "network-link"
  | "fan"
  | "blower"
  | "agitator"
  | "conveyor"
  | "damper"
  | "cylinder"
  | "motor-valve"
  | "weigh-scale"
  | "analyzer"
  | "level-switch"
  | "digital-switch"
  | "pressure-switch"
  | "vessel"
  | "heat-exchanger"
  | "filter"
  | "cyclone"
  | "crusher"
  | "screen"
  | "vfd"
  | "drive"
  | "contactor"
  | "breaker";

/** What a shipped symbol's module declares: its drawing, its states and their reading, without the geometry the table holds. */
export type SymbolArt = Omit<SymbolDefinition, "size" | "label" | "ports">;

const ART: Record<SymbolType, SymbolArt> = {
  pump: PUMP,
  valve: VALVE,
  "control-valve": CONTROL_VALVE,
  tank: TANK,
  "gravity-tank": GRAVITY_TANK,
  "tank-cascade": TANK_CASCADE,
  "dosing-pump": DOSING_PUMP,
  supply: SUPPLY,
  mixer: MIXER,
  "temperature-sensor": instrument("TT"),
  "level-sensor": instrument("LT"),
  "flow-sensor": instrument("FT"),
  "pressure-sensor": instrument("PT"),
  "position-sensor": instrument("ZT"),
  "speed-sensor": instrument("ST"),
  "sensor-pair": SENSOR_PAIR,
  controller: CONTROLLER,
  "cold-room": COLD_ROOM,
  "refrigeration-unit": REFRIGERATION_UNIT,
  heater: HEATER,
  cooler: COOLER,
  "jacketed-vessel": JACKETED_VESSEL,
  "coupled-vessels": COUPLED_VESSELS,
  "heat-draw-off": HEAT_DRAW_OFF,
  motor: MOTOR,
  device: DEVICE,
  "servo-motor": SERVO_MOTOR,
  shaft: SHAFT,
  drum: DRUM,
  coupling: COUPLING,
  encoder: ENCODER,
  "rotary-arm": ROTARY_ARM,
  "axis-indicator": AXIS_INDICATOR,
  "network-link": NETWORK_LINK,
  fan: FAN,
  blower: BLOWER,
  agitator: AGITATOR,
  conveyor: CONVEYOR,
  damper: DAMPER,
  cylinder: CYLINDER,
  "motor-valve": MOTOR_VALVE,
  "weigh-scale": WEIGH_SCALE,
  analyzer: ANALYZER,
  "level-switch": LEVEL_SWITCH,
  "digital-switch": DIGITAL_SWITCH,
  "pressure-switch": PRESSURE_SWITCH,
  vessel: VESSEL,
  "heat-exchanger": HEAT_EXCHANGER,
  filter: FILTER,
  cyclone: CYCLONE,
  crusher: CRUSHER,
  screen: SCREEN,
  vfd: VFD,
  drive: DRIVE,
  contactor: CONTACTOR,
  breaker: BREAKER,
};

/** A shipped symbol: its drawing, with its design size, label and ports from the runtime's table (§6 item 1, §14.2). */
function shipped(type: SymbolType, art: SymbolArt): SymbolDefinition {
  const geometry = SYMBOL_GEOMETRY[type];
  if (!geometry) throw new Error(`The symbol ${type} has no geometry in the runtime's table.`);
  return { ...art, ...geometry };
}

export const SYMBOLS = Object.fromEntries(
  (Object.entries(ART) as Array<[SymbolType, SymbolArt]>).map(([type, art]) => [type, shipped(type, art)]),
) as Record<SymbolType, SymbolDefinition>;
