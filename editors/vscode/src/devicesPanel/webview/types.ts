// The slice of the Rust view-model contract (design §2.1) that the new panel draws: the toolbar,
// the frames, the connection pills, the devices, the links, the mesh buses, the inspector and the
// runtime's context menu. The names and the enum spellings are Rust's
// (`crates/trust-devices/src/view/types/`); every value is drawn as given and never recomputed in
// TypeScript. Only the fields the panel reads are mirrored here (design §4.5 D1 and D2); fields that
// the small test views omit are optional.

import type { Point } from "./layout";

/** The whole panel, as far as the shell mirrors it. */
export interface DevicesView {
  /** The toolbar's actions. */
  toolbar: Toolbar;
  /** The computers and chips, each with its runtimes. */
  computers: Computer[];
  /** The issues button and list. */
  issues?: Issues;
  /** The list's columns. */
  list?: ListView;
}

/**
 * The toolbar's actions, in the order Rust declares them: `add`, `discover`, `add_host_slot`, and
 * the `Topology`/`List` switch's two words. The keys are Rust field names, so the panel draws the
 * three actions in that order and the switch beside them.
 */
export interface Toolbar {
  /** `+ Add`. */
  add: Action;
  /** `Discover`. */
  discover: Action;
  /** The Edit slot after the computers: `Add host`. */
  add_host_slot: Action;
  /** The `Topology`/`List` switch's two words (specification 25 §5.11). */
  view_switch?: ViewSwitch;
}

/** The `Topology`/`List` switch: the word of each view. */
export interface ViewSwitch {
  /** `Topology`. */
  topology: string;
  /** `List`. */
  list: string;
}

/**
 * An action that an item offers. Only the fields the panel draws are mirrored; the action's `op`
 * travels back to the extension untouched.
 */
export interface Action {
  /** Stable within the item, such as `add`. */
  id: string;
  /** The button's text. */
  label: string;
  /** Whether it can be chosen. */
  enabled: boolean;
  /** Why it cannot; always present when `enabled` is false (specification 25 §5.1 and §5.4). */
  reason?: string;
  /** How strongly it is drawn. */
  emphasis: Emphasis;
  /** The confirmation of a two-step action. */
  confirm?: Confirm;
}

/** How strongly an action is drawn. */
export type Emphasis = "primary" | "secondary" | "danger" | "quiet";

/** The second step of a two-step action. */
export interface Confirm {
  /** What happens. */
  text: string;
  /** The confirming button. */
  confirm: string;
  /** The cancelling button. */
  cancel: string;
}

/** One line of an inspector section. */
export interface Row {
  /** What the line is about. */
  label: string;
  /** Its value. */
  value: string;
  /** The value's tone, when it carries one. */
  tone?: Tone;
  /** Whether the value takes the whole width. */
  wide: boolean;
  /** The text that the copy button copies, when the line has one. */
  copy?: string;
}

/** A group of rows. */
export interface Section {
  /** The group's heading; the first section has none. */
  heading?: string;
  /** The rows. */
  rows: Row[];
}

/** The inspector of an item: every fact and every action of it (design §2.1). */
export interface Inspector {
  /** `Devices & Connections / <kind>`. */
  breadcrumb: string;
  /** The item's kind. */
  kind: string;
  /** The item's name. */
  title: string;
  /** The word of its state with the tone of its dot (specification 25 §5.11). */
  health?: Pill;
  /** The larger buttons. */
  primary: Action[];
  /** The smaller buttons. */
  secondary: Action[];
  /** The facts. */
  sections: Section[];
  /** The form of `Edit settings`. */
  edit?: string;
}

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
  /** Its inspector: every fact and every action of the computer. */
  inspector: Inspector;
  /** Its Edit slots: `Set up runtime`. */
  slots?: Action[];
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
  /** Its row of the list. */
  list?: ListCells;
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
  /** Its inspector: every fact and every action of the runtime. */
  inspector: Inspector;
  /** Its right-click menu: `Add device or connection…`, `Set as run target`, `Settings` and `Focus`. */
  context_menu?: Action[];
  /** Its Edit slots: `Add connection`. */
  slots?: Action[];
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
  /** Its row of the list. */
  list?: ListCells;
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
  /** Its inspector: every fact and every action of the connection. */
  inspector: Inspector;
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
  /** Its row of the list. */
  list?: ListCells;
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
  /** Its inspector: every fact and every action of the device. */
  inspector: Inspector;
  /** Whether the search dims it. */
  dimmed?: boolean;
  /** Whether the filter hides it. */
  hidden?: boolean;
  /** Its row of the list. */
  list?: ListCells;
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
  /** Opens the issues list (specification 25 §5.14). */
  open_issues?: Action;
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

/** The issues button and the list of every fault and warning (graph.rs `Issues`). */
export interface Issues {
  /** The button. */
  button: IssuesButton;
  /** The issues, grouped by item. */
  groups: IssueGroup[];
}

/** The issues button (specification 25 §5.11). */
export interface IssuesButton {
  /** Its text, such as `4 issues`. */
  text: string;
  /** Its tone. */
  tone: Tone;
  /** The faults. */
  faults: number;
  /** The warnings. */
  warnings: number;
}

/** The issues of one item. */
export interface IssueGroup {
  /** The item; absent for an issue of no shown item. */
  item?: string;
  /** The item's name. */
  item_name: string;
  /** Its issues. */
  issues: Issue[];
}

/** One fault or warning. */
export interface Issue {
  /** Its identity. */
  id: string;
  /** Its severity's word: `Fault`, `Warning` or `Info` (specification 25 §5.11). */
  severity_word: string;
  /** Its severity's tone. */
  severity_tone: Tone;
  /** The text. */
  text: string;
  /** `Show`. */
  show?: Action;
  /** The recovery action. */
  recovery?: Action;
}

/** The list's columns (specification 25 §5.14). */
export interface ListView {
  /** The columns, in their order. */
  columns: Column[];
}

/** A column of the list. */
export interface Column {
  /** The key of its cells. */
  id: string;
  /** Its heading. */
  label: string;
}

/** An item's row of the list, by column key. */
export interface ListCells {
  /** The item's kind: `computer`, `chip`, `runtime`, `connection` or `device`. */
  kind: string;
  /** The text of each column that the item fills, by column key. */
  cells: Record<string, string>;
}

/** Which of the canvas and the list the toolbar's switch shows (specification 25 §5.11). */
export type PanelView = "topology" | "list";

/**
 * The positions message the panel posts when a drag ends and applies when the host sends it back,
 * such as after the panel opens again (design §4.5 D2). The keys are item identities; storing them
 * per project is the host's job, so the webview keeps nothing itself.
 */
export interface PositionsMessage {
  /** The message's kind. */
  type: "positions";
  /** Each dragged item's position, by its identity. */
  positions: Record<string, Point>;
}
