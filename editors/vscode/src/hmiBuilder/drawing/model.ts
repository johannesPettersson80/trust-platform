// The page model as the language server returns it (specification 39, §15): the file's own keys.

export type HmiBindingDirection = "read" | "write" | "read-write";

export interface HmiBindingModel {
  quality?: string;
  path: string;
  unit?: string;
  /** A trend pen's fixed scale; otherwise the object's range or automatic (§5.9). */
  range?: [number, number];
  direction: HmiBindingDirection;
}

export interface HmiMethodCommand {
  interface: string;
  method: string;
  instance: string;
  arguments: Record<string, boolean | number | string | { input: true }>;
}

export interface HmiObjectModel {
  command?: HmiMethodCommand;
  /** Native FB type of an equipment view; multiple views may share one instance. */
  block?: string;
  id: string;
  tag?: string;
  type: string;
  variant?: string;
  /** For a placed faceplate (type `faceplate`): the faceplate it shows (§8.9 item 3). */
  faceplate?: string;
  /** For a placed faceplate: the instance it is bound to (§8.9 item 3). */
  instance?: string;
  at: [number, number];
  size: [number, number];
  rotation: number;
  label?: string;
  text?: string;
  /** Picture of an image object: a project path under `hmi/assets/`. */
  image?: string;
  unit?: string;
  decimals?: number;
  /** Fallback display thresholds when no bound equipment supplies them (§5.9). */
  limits?: Partial<Record<"low-low" | "low" | "high" | "high-high", number>>;
  min?: number;
  max?: number;
  /** For a trend: the time it shows, in seconds (default 300). */
  span?: number;
  /** For a panel or group: how it rearranges its objects on a smaller screen (§4 item 5). */
  arrange?: "row" | "column" | "grid";
  /** For an arranged panel or group: the space between its objects, in pixels. */
  gap?: number;
  /** For a control: the lowest role that may use it (§12.8). */
  role?: "operator" | "engineer";
  /** For a control: a question asked before its command is sent. */
  confirm?: string;
  /** For a button: what it does to its BOOL target. */
  action?: "set" | "reset" | "toggle";
  /** For a pulse button: how long the pulse lasts, in seconds. */
  pulse?: number;
  /** For a control: the value its feedback reports when the command took effect. */
  expect?: boolean | number | string;
  /** For a control: how long confirmation may take, in seconds. */
  timeout?: number;
  /** For a selector or mode selector: the choices it offers (§5.8). */
  options?: string[];
  /** For an instrument: what it measures, an equipment's id or `pipe:<n>` (§6, item 5). */
  attach?: string;
  /** For a loop controller: the instrument whose measurement it reads, by id, drawn as a signal
   * line to the controller (§6, item 5). */
  measures?: string;
  /** For a loop controller: the equipment it acts on, by id, drawn as a signal line from the
   * controller (§6, item 5). */
  acts_on?: string;
  /** The invisible group it belongs to, by name (§3.3 item 8). */
  group?: string;
  /** Locked in the editor: not selected or moved on the canvas (§3.3 item 8). */
  locked?: boolean;
  bind: Record<string, HmiBindingModel>;
  click?: Record<string, string>;
}

export interface HmiPipeModel {
  /** The port it starts at, `<object id>.<port>` (§6). */
  from: string;
  to: string;
  /** A process line by default, or a signal line (§6, item 7). */
  style?: "process" | "signal";
  /** The bends the user placed, in page pixels (§6, item 3). */
  waypoints?: Array<[number, number]>;
  /** Its flow, a number or a boolean (§6, item 6). */
  bind?: Record<string, HmiBindingModel>;
}

export interface PanelPlacement {
  scope: "manual" | "all" | "selected";
  pages: string[];
  dock: "left" | "right" | "top" | "bottom";
  order: number;
}

export interface PageRegion {
  panel?: string | null;
  at: [number, number];
  size: [number, number];
  objects: string[];
  pipes: number[];
}

export interface HmiPageModel {
  /** Shared panel definition and derived layout are separate from authored page objects. */
  panel?: PanelPlacement;
  regions?: PageRegion[];
  /** Saved design work that native bundle validation refuses to deploy. */
  incomplete?: boolean;
  title: string;
  /** The P&ID whose process objects the page shares, `hmi/pid/<name>.toml` (§18 phase 4). */
  pid?: string;
  /** Place in the navigation (§4.6): by order, then by name. */
  order?: number;
  /** One of the shipped navigation icons. */
  icon?: string;
  /** The navigation group the page is listed under. */
  group?: string;
  /** Left out of the navigation. */
  hidden?: boolean;
  /** For a popup (§4.9): its design size. */
  size?: [number, number];
  /** For a popup: whether it may be pinned open beside the page. */
  pinnable?: boolean;
  /** For a faceplate (§8.7): the line under its title. */
  description?: string;
  /** For a faceplate: the fields its objects bind as `$Name`, each with the kinds it takes. */
  fields?: Record<string, { accepts: string[] }>;
  object: HmiObjectModel[];
  pipe: HmiPipeModel[];
}

/** The state of a live value (§5.3). */
export type ValueState = "confirmed" | "commanded" | "estimated" | "unknown";

/** A value as it travels from the program: a single value, an array or a structure. */
export type StructuredValue =
  number | boolean | string | null | StructuredValue[] | { [field: string]: StructuredValue };

/** One bound path's live value. */
export interface LiveValue {
  /** Equipment facts sampled with this value; no write or alarm authority. */
  presentation?: EquipmentPresentationModel;
  value: StructuredValue;
  state: ValueState;
  /** Set when the value is stale: milliseconds since its last update. */
  staleMs?: number;
  /** The commanded target, for a commanded value. */
  target?: number | boolean | string;
  /** For an array: the declared lower bound of each dimension, outermost first (default 0). */
  lowerBounds?: number[];
}

/** What one drawing is made from (§3.4). */
/** A faceplate placed on a page as the page answer gives it, bound to its instance (§8.9). */
export interface PlacedFaceplateModel extends HmiPageModel {
  size: [number, number];
  description?: string;
  settings?: boolean;
  settings_instance?: string;
}

/** What the drawing needs of a placed faceplate: its bound model and its controls' states. */
export interface PlacedFaceplate {
  model: PlacedFaceplateModel;
  commands?: Record<string, CommandState>;
  drafts?: Record<string, number>;
  locked?: Record<string, string>;
}

/** Presentation defaults supplied by the project, separate from authored objects. */
export interface ValueFormatModel {
  unit?: string;
  decimals?: number;
}

export interface EquipmentPresentationModel {
  unit?: string;
  range?: [number, number];
  limits?: HmiObjectModel["limits"];
  priorities?: Partial<Record<"low-low" | "low" | "high" | "high-high", "low" | "medium" | "high">>;
}

export interface DrawFrame {
  currentPage?: string;
  /** Per-value display defaults, keyed by the ASCII lower-case IEC path. */
  formats?: Record<string, ValueFormatModel>;
  /** Design size of the screen in pixels. */
  size: [number, number];
  /** Live values by bound path. */
  values: Record<string, LiveValue>;
  /** The faceplates placed on the page, by object id (§8.9): `null` where the project lacks the
   * faceplate; a placement not listed is not known yet. */
  placed?: Record<string, PlacedFaceplate | null>;
  /** Recent samples by bound path, for trends: [time in ms, value]. */
  history?: Record<string, Array<[number, number]>>;
  /** Turns a project asset path into a URL the display can load. */
  asset?: (path: string) => string;
  /** The time the frame shows, in ms; a trend ends here. Defaults to the newest sample. */
  nowMs?: number;
  /** The latest command outcome on each control, by object id (§12.5). */
  commands?: Record<string, CommandState>;
  /** The controls cannot be used: there is no session, or it ended (§12.4). */
  readOnly?: boolean;
  /** Controls the signed-in role may not use, by object id: the role that may, or "none"
   * (§5.9 item 5). */
  locked?: Record<string, string>;
  /** What each trend shows, by object id, as its operator left it (§5.9 item 1). */
  trendViews?: Record<string, TrendView>;
  /** Bound paths the running program does not have (§16.3). */
  missing?: string[];
  /** A value being entered on a control, by object id, shown until it is sent (§5.8). */
  drafts?: Record<string, number>;
  /** The alarms and records alarm lists and event logs draw (§13.6, §13.7). */
  alarms?: AlarmFrame;
  /** The binding checks' problems by object id, in the editor (§7.6). */
  problems?: Record<string, string[]>;
  /** The quality checks' warnings by object id, in the editor (§14.1 item 4). */
  warnings?: Record<string, string[]>;
  /** The screen's text table in the language shown: each text as written to the text shown (§14.5). */
  texts?: Record<string, string>;
  /** The "longest" preview: every text lengthened by 40 % (§14.5 item 7). */
  longest?: boolean;
  /** The language shown, a BCP 47 tag; numbers, dates and times follow it. English when left out. */
  language?: string;
}

/** One alarm of the runtime's list (specification 40, §5.1). */
export interface AlarmRowModel {
  path: string;
  message: string;
  priority: string;
  state: string;
  active: boolean;
  unacknowledged: boolean;
  shelved: boolean;
  out_of_service: boolean;
  suppressed: boolean;
  first_out: boolean;
  annunciating: boolean;
  raise_count: number;
  latching?: boolean;
  since_ms: number;
  /** The function block instance nearest above it that holds it (§13.6, item 9). */
  equipment?: string;
  /** A limit alarm's value against its limit when it was raised (§5.9 item 2). */
  raised?: { value: number; limit: number; high: boolean; source?: string; unit?: string };
}

/** The runtime's alarm list (§13.6, item 3). */
export interface AlarmSummaryModel {
  active: AlarmRowModel[];
  shelved: AlarmRowModel[];
  suppressed: AlarmRowModel[];
  out_of_service: AlarmRowModel[];
  unacknowledged: number;
  annunciating: number;
  newest: AlarmRowModel | null;
}

/** One record of the runtime's event window (specification 40, §5.1). */
export interface AlarmRecordModel {
  seq: number;
  time_ms: number;
  path: string;
  kind: string;
  message: string;
  priority?: string;
  user?: string;
  detail?: string;
  outcome?: string;
}

/** An alarm list's tabs (§5.7, item 4). */
export type AlarmTab = "active" | "shelved" | "suppressed" | "out_of_service";

/** What a trend shows (§5.9 item 1): its span, and where a paused window ends; without an end
 * it follows the live values. */
export interface TrendView {
  spanMs?: number;
  endMs?: number;
  /** The period of a box being dragged on the plot, to zoom into. */
  box?: { fromMs: number; toMs: number };
  /** The cursor's time, where the legend reads each pen. */
  cursorMs?: number;
  /** The pens hidden from the plot, by name. */
  hidden?: string[];
}

/** An alarm list's columns (§5.9 item 2). */
export type AlarmColumn = "priority" | "time" | "tag" | "message" | "value" | "state";

/** What a list shows: its tab, the first row of its page, and the column it is sorted by, in
 * that column's own order (highest priority, newest, from A) or reversed. */
export interface ListView {
  tab?: AlarmTab;
  first?: number;
  sort?: { column: AlarmColumn; reversed?: boolean };
  filter?: AlarmFilter;
}

/** What an alarm list keeps (§5.9 item 2): the rows whose message, tag or path holds the text, of
 * the priorities chosen (all when none is), unacknowledged only, of one equipment. */
export interface AlarmFilter {
  text?: string;
  priorities?: string[];
  unacknowledged?: boolean;
  equipment?: string;
}

/** The alarms and records a page draws (§13.6, §13.7). */
export interface AlarmFrame {
  summary?: AlarmSummaryModel;
  /** The records, oldest first. */
  records?: AlarmRecordModel[];
  /** The stream is closed or its updates are stale (§13.6, item 8). */
  stale?: boolean;
  /** Each list's view, by object id. */
  views?: Record<string, ListView>;
}

/** A command's outcome as a control shows it (§12.5). */
export interface CommandState {
  outcome: "pending" | "refused" | "failed" | "applied" | "confirmed" | "timed-out" | "unknown";
  /** Why it was refused or not confirmed. */
  reason?: string;
}
