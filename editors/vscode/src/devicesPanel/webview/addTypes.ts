// The Add dialog contract of the new panel (design §4.5 "D3: forms and the Add dialog",
// specification 25 §5.9 and §5.14). The names and the enum spellings are Rust's
// (`crates/trust-devices/src/view/types/panes.rs`); every value is drawn as given and never
// recomputed in TypeScript. An absent `Option` is omitted, so those fields are optional.

import type { RuntimeRef } from "./formTypes";
import type { Action, Row } from "./types";

/** The add dialog, which every golden view carries as `add` (panes.rs `AddDialog`, specification 25 §5.14). */
export interface AddDialog {
  /** Its title. */
  title: string;
  /** The runtime that a new connection belongs to. */
  target?: RuntimeRef;
  /** The runtimes to choose from. */
  runtime_choices: RuntimeRef[];
  /** `Discover devices and runtimes`. */
  discover: Action;
  /** Its help. */
  discover_help: string;
  /** The heading of the finds, `Found on the network` (Rust's text). */
  found_heading: string;
  /** The finds of the session's last search. */
  found: FoundEntry[];
  /** The `Runtimes` group, first. */
  runtimes: AddGroup;
  /** The other groups, in their canonical order. */
  groups: AddGroup[];
  /** Shows or hides the advanced entries. */
  advanced_toggle?: AdvancedToggle;
  /** The quick starts, while the target runtime has no connections. */
  quick_starts: QuickStart[];
  /** Why the catalog is missing; drawn as the dialog's notice in place of the groups. */
  catalog_state?: string;
  /** The accessible name of the close button (Rust's text). */
  close_label: string;
}

/** A group of the add dialog (panes.rs `AddGroup`). */
export interface AddGroup {
  /** Its name. */
  label: string;
  /** Whether it is advanced. */
  advanced: boolean;
  /** Its entries. */
  entries: AddEntry[];
}

/** An entry of the add dialog (panes.rs `AddEntry`). */
export interface AddEntry {
  /** Its protocol. */
  protocol: string;
  /** Its title. */
  title: string;
  /** Its badge. */
  badge: string;
  /** Its purpose. */
  purpose: string;
  /** What choosing it does. */
  action: Action;
}

/** Shows or hides the advanced entries (panes.rs `AdvancedToggle`). */
export interface AdvancedToggle {
  /** The text that shows them. */
  show_label: string;
  /** The text that hides them. */
  hide_label: string;
  /** How many are hidden. */
  count: number;
}

/** A quick start (panes.rs `QuickStart`). */
export interface QuickStart {
  /** Its title. */
  title: string;
  /** What it adds. */
  description: string;
  /** What choosing it does. */
  action: Action;
}

/** A find of the last search in the add dialog (panes.rs `FoundEntry`). */
export interface FoundEntry {
  /** Its device card. */
  card: string;
  /** Its title. */
  title: string;
  /** Its facts. */
  rows: Row[];
  /** `Add`, or `Adopt` for a runtime. */
  action: Action;
}
