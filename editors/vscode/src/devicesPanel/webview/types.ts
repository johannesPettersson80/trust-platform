// The slice of the Rust view-model contract (design §2.1) that the new panel's canvas draws: the
// toolbar, the frames, the connection pills, the devices, the links and the mesh buses. The names
// and the enum spellings are Rust's (`crates/trust-devices/src/view/types/`); every value is drawn
// as given and never recomputed in TypeScript. Only the fields the canvas reads are mirrored here
// (design §4.5 D1); fields that D1a's small test views omit are optional.

/** The whole panel, as far as the shell mirrors it. */
export interface DevicesView {
  /** The toolbar's actions. */
  toolbar: Toolbar;
  /** The computers and chips, each with its runtimes. */
  computers: Computer[];
}

/**
 * The toolbar's actions, in the order Rust declares them: `add`, `discover`, `add_host_slot`. The
 * keys are Rust field names, so `Object.values(toolbar)` keeps that order.
 */
export interface Toolbar {
  /** `+ Add`. */
  add: Action;
  /** `Discover`. */
  discover: Action;
  /** The Edit slot after the computers: `Add host`. */
  add_host_slot: Action;
}

/**
 * An action that an item offers. Only the fields the canvas draws are mirrored; the rest of the
 * action (`op`, `reason`, `confirm`, …) travels back to the extension untouched.
 */
export interface Action {
  /** Stable within the item, such as `add`. */
  id: string;
  /** The button's text. */
  label: string;
  /** Whether it can be chosen. */
  enabled: boolean;
  /** How strongly it is drawn. */
  emphasis: Emphasis;
}

/** How strongly an action is drawn. */
export type Emphasis = "primary" | "secondary" | "danger" | "quiet";

/** A computer or a chip, drawn as a frame holding its runtimes. */
export interface Computer {
  /** Its identity. */
  id: string;
  /** `This computer`, `Computer <IP>`, a configured label, or the chip name and address. */
  heading: string;
  /** The board, operating system and architecture, or the chip; drawn on each of its runtime cards. */
  platform?: string;
  /** `Reachable` or `Unreachable`. */
  reachability: Pill;
  /** Every runtime on it. */
  runtimes: Runtime[];
  /** Its Edit slots: `Set up runtime`. */
  slots?: Action[];
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
}

/** A runtime card. */
export interface Runtime {
  /** Its identity. */
  id: string;
  /** Its name. */
  name: string;
  /** Its state. */
  state: Pill;
  /** Its connections, drawn as pills. */
  connections?: Connection[];
  /** Its Edit slots: `Add connection`. */
  slots?: Action[];
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
}

/** A status word with its tone. */
export interface Pill {
  /** The word, such as `Running` or `Reachable`. */
  word: string;
  /** Its colour role. */
  tone: Tone;
}

/** The colour role of a status. */
export type Tone = "ok" | "warning" | "error" | "neutral" | "muted";

/** How an item is drawn: solid, dashed while not proven, or as a draft. */
export type Look = "normal" | "pending" | "draft";

/** A connection pill on its runtime. */
export interface Connection {
  /** Its identity. */
  id: string;
  /** The connection's own name. */
  name: string;
  /** The status dot. */
  dot: Tone;
  /** Whether the dot pulses. */
  live: boolean;
  /** The role word, such as `CLIENT`. */
  role_word: string;
  /** `draft` and `disabled`, exactly as the view gives them. */
  marks: Mark[];
  /** How the pill is drawn. */
  look: Look;
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
}

/** A mark on a connection pill. */
export type Mark = "draft" | "disabled";

/** A device or system at the other end of a connection. */
export interface Device {
  /** Its identity. */
  id: string;
  /** What it is. */
  kind: DeviceKind;
  /** Its name. */
  name: string;
  /** What it presents, such as `MQTT broker`. */
  presents: string;
  /** Its status. */
  status?: Pill;
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
}

/** What a device card shows. */
export type DeviceKind = "external" | "shared" | "sub_device";

/** A link between two items. */
export interface Link {
  /** Its identity. */
  id: string;
  /** One end. */
  from: string;
  /** The other end. */
  to: string;
  /** Solid when proven, dashed otherwise. */
  proven: boolean;
  /** Its tone. */
  tone: Tone;
  /** Where the arrow points. */
  arrow: ArrowAt;
  /** Whether it shows a lock. */
  secure: boolean;
  /** Its status and detail on hover. */
  hover_text?: string;
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
}

/** Where a link's arrow points. */
export type ArrowAt = "from" | "to" | "none";

/** A mesh bus. */
export interface MeshBus {
  /** Its identity. */
  id: string;
  /** Its label, present only when the view gives one. */
  label?: string;
  /** Whether it is a draft. */
  draft: boolean;
  /** Its drops. */
  drops: MeshDrop[];
}

/** A drop of a mesh bus. */
export interface MeshDrop {
  /** The item it reaches. */
  item: string;
  /** Whether it is proven. */
  proven: boolean;
  /** Its tone. */
  tone: Tone;
  /** Its status and detail on hover. */
  hover_text?: string;
}

/** The summary strip. */
export interface Summary {
  /** The line, such as `1 computer · 2 runtimes · 3 connections · 2 running`. */
  text: string;
}

/** The banner. */
export interface Banner {
  /** Its tone. */
  tone: Tone;
  /** Its text. */
  text: string;
  /** Its actions, such as `Retry` and `Open logs`. */
  actions: Action[];
}
