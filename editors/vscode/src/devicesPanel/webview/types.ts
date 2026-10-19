// The slice of the Rust view-model contract (design §2.1) that the new panel draws: the toolbar,
// the frames, the connection pills, the devices, the links, the mesh buses, the inspector and the
// runtime's context menu. The names and the enum spellings are Rust's
// (`crates/trust-devices/src/view/types/`); every value is drawn as given and never recomputed in
// TypeScript. Only the fields the panel reads are mirrored here (design §4.5 D1 and D2); fields that
// the small test views omit are optional.

import type { FieldOption } from "./formTypes";
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
 * An action that an item offers. The panel draws the fields below and posts the whole action back
 * unchanged; the webview reads `op` only to choose the pane a choice opens, and to send the login of
 * `Create route` with its own action (design §2.1).
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
  /** What choosing it does (design §2.1, `Op`). */
  op: Op;
}

/**
 * What choosing an action does (design §2.1, `Op`): Rust's action, a search request, a VS Code call, or a view
 * change. A search's `request` is the `trust-lsp.devicesSearch` body that Rust gives (specification 25 §2).
 */
export type Op =
  | { kind: "rust"; action: string; payload: unknown }
  | { kind: "search"; request: unknown }
  | { kind: "host"; effect: { kind: string } }
  | { kind: "ui"; ui: { kind: string; pane?: { kind: string } } }
  | { kind: "followed"; first: Op; then: Op };

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

/** The kind of a browse pane, as Rust names it (panes.rs `BrowseKind`). */
export type BrowseKind = "nodes" | "channels" | "expose" | "ads_tags";

/**
 * A browse pane, which `trust-lsp.devicesBrowse` returns as `browse` (panes.rs `BrowseView`). Rust
 * gives the tree, the ADS port cards, the search box, the typed port and every text; the webview
 * keeps only the tick boxes, the write toggle, the search text and the open disclosures.
 */
export interface BrowseView {
  /** Its identity: `browse:<protocol>`. */
  id: string;
  /** Its breadcrumb, above the title. */
  breadcrumb: string;
  /** Its title. */
  title: string;
  /** Its subtitle: the connection's name. */
  subtitle: string;
  /** What it browses. */
  kind: BrowseKind;
  /** Whether the browse still runs. */
  loading: boolean;
  /** The accessible name of the close button. */
  close_label: string;
  /** The word of the mark on a leaf that the project already has. */
  added_label: string;
  /** The tooltip of a leaf's access word: `{access} on the device`, with `{access}` replaced here. */
  access_tooltip: string;
  /** The accessible name of a leaf's Added mark: `Already added: {path}`, with `{path}` replaced here. */
  added_aria_label: string;
  /** The search box, while there is a tree or the ports to search. */
  search?: SearchBox;
  /** What the pane shows instead of its tree: loading, failure or an empty tree. */
  empty_text?: string;
  /** The tree of an OPC UA, EtherCAT or exposed-globals browse. */
  tree: TreeNode[];
  /** The ADS port cards, in port order. */
  ports: AdsPortCard[];
  /** The suggested ADS ports, each with its name. */
  port_suggestions: FieldOption[];
  /** The help under the port suggestions. */
  port_help: string;
  /** The typed ADS port: its toggle, its field and the action that browses it. */
  port_input?: PortInput;
  /** The ADS port summary line. */
  port_summary?: string;
  /** The connection's details, under a disclosure. */
  details?: Details;
  /** What failed. */
  error?: ErrorBox;
  /** `Add`; an ADS pane adds its tags with their own ticks and has none. */
  add?: AddButton;
  /** The write toggle. */
  write_toggle: Toggle;
  /** The label of the footer button that closes an ADS pane. */
  done_label?: string;
}

/** The search box of a browse pane (panes.rs `SearchBox`). */
export interface SearchBox {
  /** The text the empty box shows. */
  placeholder: string;
  /** The text shown when nothing matches. */
  no_match: string;
}

/** The typed ADS port (panes.rs `PortInput`). */
export interface PortInput {
  /** The toggle's text while the field is hidden. */
  show_label: string;
  /** The toggle's text while the field is shown. */
  hide_label: string;
  /** The field's label. */
  label: string;
  /** The text the empty field shows. */
  placeholder: string;
  /** `Browse this port`: it posts the typed text as its `port` value. */
  browse: Action;
}

/** The connection's details, under a disclosure (panes.rs `Details`). */
export interface Details {
  /** The disclosure's text. */
  summary: string;
  /** The facts. */
  rows: Row[];
}

/** A node of a browse tree (panes.rs `TreeNode`). */
export interface TreeNode {
  /** Its identity in the selection. */
  key: string;
  /** Its name. */
  name: string;
  /** Its path. */
  path: string;
  /** Its type. */
  type_label?: string;
  /** `read/write` or `read-only`; absent for a group. */
  access?: string;
  /** Whether it can be ticked. */
  selectable: boolean;
  /** The accessible name of its tick box, such as `Select MAIN.Counter`. */
  select_label?: string;
  /** Whether the project already has it. */
  added: boolean;
  /** What ticking or unticking it posts (an ADS tag's `ads.tags_set`). */
  action?: Action;
  /** Its children. */
  children: TreeNode[];
}

/** An ADS port of the tag browser (panes.rs `AdsPortCard`). */
export interface AdsPortCard {
  /** The port. */
  port: number;
  /** Its title, such as `ADS port 851`. */
  title: string;
  /** Its state, such as `2 tags` or `Loading…`. */
  state_text: string;
  /** The text of its body when no tree is drawn. */
  empty_text?: string;
  /** Its tags. */
  tags: TreeNode[];
  /** The route help of a port whose route is missing. */
  route?: RouteHelp;
  /** What failed on this port. */
  error?: ErrorBox;
  /** Browses this port, when its tags are not loaded yet. */
  browse?: Action;
}

/** The help of a missing `TwinCAT` route (panes.rs `RouteHelp`). */
export interface RouteHelp {
  /** The text. */
  text: string;
  /** The help lines. */
  help: string[];
  /** One copy action per generated route file. */
  artifacts: Action[];
  /** `Create route`: it opens the login form. */
  create_route: Action;
}

/** A failure in a pane, with its one recovery action (panes.rs `ErrorBox`). */
export interface ErrorBox {
  /** What failed. */
  title: string;
  /** Why. */
  detail: string;
  /** The recovery action. */
  recovery?: Action;
}

/** The `Add` button of a browse pane (panes.rs `AddButton`). */
export interface AddButton {
  /** Its text. */
  label: string;
  /** Its text with the number of ticked symbols: `{count}` stands for the number. */
  counted_label: string;
  /** Whether it can be chosen. */
  enabled: boolean;
  /** Why it cannot. */
  reason?: string;
  /** Why it cannot while nothing is ticked. */
  no_selection_reason: string;
  /** What choosing it posts: `browse.add`, with the ticked keys and the write toggle as values. */
  action: Action;
}

/** A toggle of a browse pane (panes.rs `Toggle`). */
export interface Toggle {
  /** Its text. */
  label: string;
  /** Whether it can be changed. */
  enabled: boolean;
  /** Why it cannot. */
  reason?: string;
}

/**
 * The Discover pane, which the search status answers with `discover` (panes.rs `DiscoverView`, specification
 * 25 §2). Rust gives every text, count and rule; the pane mirrors only the fields it draws.
 */
/** The `Show ignored` toggle (decision O14): its text, whether it is ticked and its change. */
export interface ShowIgnored {
  /** Its text: `Show ignored`. */
  label: string;
  /** Whether it is ticked. */
  checked: boolean;
  /** The change of the toggle, a `search` operation. */
  action: Action;
}

export interface DiscoverView {
  /** The running or last search. */
  search_id?: string;
  /** Whether the search runs. */
  running: boolean;
  /** Its progress. */
  progress_text?: string;
  /** The breadcrumb: `Devices & Connections`. */
  breadcrumb: string;
  /** The accessible name of the close button. */
  close_label: string;
  /** The heading of the scan choices. */
  heading_find: string;
  /** The heading of the origin choices. */
  heading_scan_from: string;
  /** The heading of the known address and subnet inputs. */
  heading_targets: string;
  /** The heading of the advanced settings. */
  heading_advanced: string;
  /** The heading of the runtime's hardware scans. */
  heading_runtime: string;
  /** The help that shows when no discovery is available. */
  unavailable_text?: string;
  /** The help that shows above the scan choices. */
  choose_text?: string;
  /** `Scan from`: this computer, then each runtime. */
  origins: OriginChoice[];
  /** `Only search`: one choice per source. */
  only: OnlyChoice[];
  /** `Known address or subnet`: the inputs' labels and placeholders. */
  targets: TargetsInput;
  /** The state of each source. */
  sources: SourceRow[];
  /** The line that asks to update truST, when a source or protocol was left out. */
  update_text?: string;
  /** `Show ignored`: the toggle of the finds that the project ignores (decision O14). */
  show_ignored: ShowIgnored;
  cards: DeviceCard[];
  /** What shows without finds. */
  empty_text?: string;
  /** The text of a search that failed at a runtime, as the old pane gave it. */
  error_text?: string;
  /** The pane's own actions: `search`, `cancel` while a scan runs, and `configure_ads_ports`. */
  actions: Action[];
}

/** An origin of the search (panes.rs `OriginChoice`). */
export interface OriginChoice {
  /** Its identity. */
  id: string;
  /** Its name. */
  label: string;
  /** Whether it can be chosen. */
  enabled: boolean;
  /** Why it cannot. */
  reason?: string;
  /** Whether it is chosen. */
  selected: boolean;
}

/** A source of `Only search` (panes.rs `OnlyChoice`). */
export interface OnlyChoice {
  /** The source. */
  source: string;
  /** Its name. */
  label: string;
  /** Whether it is ticked. */
  checked: boolean;
  /** Whether it can be ticked. */
  enabled: boolean;
  /** Why it cannot. */
  reason?: string;
  /** The note under its name. */
  note?: string;
  /** The group it is drawn in: `find`, `known_address` or `runtime_hardware`. */
  group: string;
  /** The title of the warning drawn beside its name, such as the EtherCAT title. */
  warning?: string;
  /** The change of its tick, which records it and answers the new view. Absent while it is disabled. */
  action?: Action;
}

/** The inputs of `Known address or subnet` (panes.rs `TargetsInput`). */
export interface TargetsInput {
  /** The address input's label. */
  hosts_label: string;
  /** The subnet input's label. */
  cidr_label: string;
  /** The address input's placeholder. */
  hosts_placeholder: string;
  /** The subnet input's placeholder. */
  cidr_placeholder: string;
}

/** A source's state in the search (panes.rs `SourceRow`). */
export interface SourceRow {
  /** The source. */
  id: string;
  /** Its name. */
  label: string;
  /** `searching`, `{n} found`, `timed out` or `skipped`. */
  word: string;
  /** Its finds. */
  count?: number;
  /** Why it was skipped. */
  reason?: string;
  /** Its tone. */
  tone: Tone;
}

/** A found device (panes.rs `DeviceCard`). */
export interface DeviceCard {
  /** Its identity. */
  id: string;
  /** Its title. */
  title: string;
  /** Its addresses; a runtime shows one `host:port`. */
  addresses: string[];
  /** Every protocol found on it. */
  protocols: CardProtocol[];
  /** Its identity facts. */
  identity: Row[];
  /** When it was last seen. */
  last_seen: string;
  /** The search's warnings about it. */
  warnings: string[];
  /** Its actions. */
  actions: Action[];
  /** Why its actions are disabled on a stale result. */
  stale_reason?: string;
  /** The count of its responding ADS ports, such as `2 responding ADS ports`. */
  responding_ports?: string;
  /** One button per responding ADS port, which browses that port's tags. */
  ports: Action[];
  /** The mark `On canvas`, when the canvas holds the device. */
  on_canvas?: string;
}

/** A protocol found on a device (panes.rs `CardProtocol`). */
export interface CardProtocol {
  /** The protocol's name. */
  name: string;
  /** How it was found. */
  source: string;
  /** Its confidence. */
  confidence: string;
  /** Whether the find is simulated. */
  simulated: boolean;
  /** The visible mark of a simulated find. */
  mark?: string;
}
