// The forms contract of the new panel: the form that Rust builds for a connection, for a
// `Runtimes` entry, or for the Add dialog (design §2.1 and §4.5 "D3: forms and the Add dialog").
// The names and the enum spellings are Rust's (`crates/trust-devices/src/view/types/panes.rs`);
// every value is drawn as given and never recomputed in TypeScript. A form writes the user's
// `io.toml`, so this mirrors exactly the JSON that `form.open` returns: `Form`, `FormSection`,
// `FormField`, `FieldKind`, `FieldOption`, `VisibleWhen`, `ListEditor`, `ListProperty`,
// `GhostTile` and `RuntimeRef`. An absent `Option` is omitted, so those fields are optional.

import type { Action } from "./types";

/**
 * A form, which `form.open` returns (panes.rs `Form`). Rust chooses the title, the breadcrumb, the
 * owner line, the sections and their fields, the buttons and the draft tile.
 */
export interface Form {
  /** Its identity, from protocol, runtime and prefill. */
  id: string;
  /** Its protocol. */
  protocol: string;
  /** Its title. */
  title: string;
  /** Its breadcrumb. */
  breadcrumb: string;
  /** The runtime it writes to, in words. */
  owner_line: string;
  /** The protocol's purpose. */
  purpose?: string;
  /** The runtime that `Save` writes to; absent for a form that adds a runtime. */
  runtime?: RuntimeRef;
  /** The runtimes to choose from. */
  runtime_choices: RuntimeRef[];
  /** Its sections: the first without a heading, then `Advanced`. */
  sections: FormSection[];
  /** Its buttons. */
  buttons: Action[];
  /** A note, such as that the endpoint is disabled. */
  note?: string;
  /** The draft tile drawn while the form is open. */
  ghost?: GhostTile;
}

/** A section of a form (panes.rs `FormSection`). */
export interface FormSection {
  /** Its heading; the first section has none. */
  heading?: string;
  /** Its fields. */
  fields: FormField[];
}

/** A field of a form (panes.rs `FormField`). */
export interface FormField {
  /** Its identity. */
  id: string;
  /** Its label. */
  label: string;
  /** Whether it is required. */
  required: boolean;
  /** What it holds. */
  kind: FieldKind;
  /** Its value: the current one, the default or the prefill, chosen by Rust. */
  value: unknown;
  /** Its choices. */
  options: FieldOption[];
  /** Its help. */
  help: string;
  /** Why its value was refused. */
  error?: string;
  /** When it is shown. */
  visible_when?: VisibleWhen;
  /** Its list editor. */
  list?: ListEditor;
  /** The example shown while it is empty. */
  placeholder?: string;
}

/** What a field holds (panes.rs `FieldKind`). */
export type FieldKind =
  | "text"
  | "number"
  | "boolean"
  | "choice"
  | "list"
  | "object"
  | "secret";

/** A choice of a field (panes.rs `FieldOption`). */
export interface FieldOption {
  /** Its value. */
  value: string;
  /** Its label. */
  label: string;
}

/** A field that is shown only while another field has a value (panes.rs `VisibleWhen`). */
export interface VisibleWhen {
  /** The other field. */
  field: string;
  /** The value. */
  equals: string;
}

/** The editor of a list field (panes.rs `ListEditor`). */
export interface ListEditor {
  /** What one item is called. */
  item_noun: string;
  /** The heading of an item, before its number, such as `Peer address` in `Peer address 1`. */
  item_title: string;
  /** The button that adds one. */
  add_label: string;
  /** What shows without items. */
  empty_text: string;
  /** Whether items can be added. */
  can_add: boolean;
  /** What an item holds. */
  item_kind: FieldKind;
  /** A new item. */
  item_template: unknown;
  /** The properties of the items that are objects, in their order. */
  properties: ListProperty[];
}

/** A property of the object items of a list (panes.rs `ListProperty`). */
export interface ListProperty {
  /** Its key in an item. */
  key: string;
  /** Its label. */
  label: string;
  /** What it holds. */
  kind: FieldKind;
  /** Whether an item needs it. */
  required: boolean;
  /** Its choices. */
  options: FieldOption[];
  /**
   * The properties that an item can set instead of this one, of which it sets exactly one, such as
   * `source_cidr` for `source_ip`.
   */
  alternatives: string[];
}

/** The draft tile drawn while a form is open (panes.rs `GhostTile`). */
export interface GhostTile {
  /** Its name. */
  name: string;
  /** Its detail. */
  detail: string;
}

/** A runtime named in a dialog or a form (panes.rs `RuntimeRef`). */
export interface RuntimeRef {
  /** Its identity. */
  id: string;
  /** Its name. */
  name: string;
}
