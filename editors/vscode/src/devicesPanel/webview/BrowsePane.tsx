// The browse pane of OPC UA nodes, EtherCAT channels and the globals a server exposes, drawn from the
// Rust view model (design §4.5 "D5: browse panes" and D5b; specification 25 §5.4 and §11.2). Rust gives
// every text and every rule: the title, the breadcrumb, the tree with each leaf's key, path, type,
// access and `added` flag, the search box, the error box with its one recovery action, the `Add` button
// with its action and reasons, and the write toggle. This component draws them and keeps only what the
// user sets: the tick boxes, the search text, the write toggle and which groups are open. `Add` posts
// Rust's own action with the ticked raw keys and the toggle as its values, once per click.

import React from "react";

import { InspectorAction } from "./Inspector";
import type { Notice } from "./formTypes";
import {
  actionReasonStyle,
  formNoticeDetailStyle,
  formNoticeStyle,
  formNoticeTextStyle,
  formOwnerStyle,
  inspectorActionStyle,
  pillStyle,
} from "./styles";
import {
  addBodyStyle,
  addCloseStyle,
  addDialogStyle,
  addHeaderStyle,
  addTitleStyle,
} from "./styles/addDialog";
import {
  addRowStyle,
  browseBreadcrumbStyle,
  browseEmptyStyle,
  browseGroupButtonStyle,
  browseGroupNameStyle,
  browseMetaStyle,
  browseNameColumnStyle,
  browseNameStyle,
  browsePathStyle,
  browseRowStyle,
  browseSearchStyle,
  browseTickStyle,
  loadingDotStyle,
  writeReasonStyle,
  writeToggleStyle,
} from "./styles/browse";
import type {
  Action,
  AddButton,
  BrowseView,
  ErrorBox,
  SearchBox,
  Toggle,
  TreeNode,
} from "./types";

/** The values a browse action posts with it: the ticked keys, the write toggle or the typed port. */
export type BrowseValues = Record<string, unknown>;

/** Posts a browse action; a `ui` action carries no values (specification 25 §5.13). */
export type BrowseAction = (action: Action, values?: BrowseValues) => void;

/** The ticked keys of a tree, by identity. */
export type Ticked = ReadonlySet<string>;

/** The ticked keys with `key` ticked or unticked as `checked` says, as a new set. */
export function withTick(ticked: Ticked, key: string, checked: boolean): Ticked {
  const next = new Set(ticked);
  if (checked) {
    next.add(key);
  } else {
    next.delete(key);
  }
  return next;
}

/** The leaves of a tree, in browse order: the rows a search matches (presentation only). */
export function leafNodes(nodes: TreeNode[]): TreeNode[] {
  return nodes.flatMap((node) => (node.children.length > 0 ? leafNodes(node.children) : [node]));
}

/** Whether a leaf matches the search text: a case-insensitive match of its name or its path. */
export function matchesSearch(node: TreeNode, needle: string): boolean {
  return (
    node.name.toLowerCase().includes(needle) || node.path.toLowerCase().includes(needle)
  );
}

/** The error box of a pane or a port: its title, its detail and its one recovery action. */
export function BrowseErrorBox({
  error,
  onAction,
}: {
  /** The error, as the view gives it. */
  error: ErrorBox;
  /** Called with the recovery action, unchanged. */
  onAction: BrowseAction;
}) {
  return (
    <div data-role="browse-error" style={formNoticeStyle("error")}>
      <span data-role="browse-error-title" style={formNoticeTextStyle("error")}>
        {error.title}
      </span>
      <span data-role="browse-error-detail" style={formNoticeDetailStyle}>
        {error.detail}
      </span>
      {error.recovery !== undefined && (
        <InspectorAction action={error.recovery} onAction={(action) => onAction(action)} />
      )}
    </div>
  );
}

/** The words a tree's leaves show, as the view gives them: the Added mark, and the tooltip and accessible
 * name, which are templates that the leaf's own access word and path fill. */
export interface LeafMarks {
  /** The word of the mark on a leaf that the project already has. */
  added: string;
  /** The tooltip of a leaf's access word: `{access} on the device`, with `{access}` replaced. */
  accessTooltip: string;
  /** The accessible name of a leaf's Added mark: `Already added: {path}`, with `{path}` replaced. */
  addedAriaLabel: string;
}

/** The leaf words of a browse view. */
export function leafMarks(browse: BrowseView): LeafMarks {
  return {
    added: browse.added_label,
    accessTooltip: browse.access_tooltip,
    addedAriaLabel: browse.added_aria_label,
  };
}

/** The tree's own props, shared by its groups and leaves. */
interface TreeProps {
  /** The words its leaves show. */
  marks: LeafMarks;
  /** Whether a leaf's tick box is checked. */
  checked: (node: TreeNode) => boolean;
  /** Called with a leaf's tick box and its new state. */
  onTick?: (node: TreeNode, checked: boolean) => void;
  /** Whether the tick boxes cannot change while a Rust action runs. */
  disabled: boolean;
}

/** A browse tree: each group, which opens and closes, and each leaf, with its tick box when Rust says
 * it can be ticked. */
export function TagTree({ nodes, ...props }: { nodes: TreeNode[] } & TreeProps) {
  return (
    <>
      {nodes.map((node) => (
        <TagRow key={node.key} node={node} depth={0} {...props} />
      ))}
    </>
  );
}

/** One row: a group opens to its children, a leaf is one row with its tick box and marks. */
function TagRow({ node, depth, ...props }: { node: TreeNode; depth: number } & TreeProps) {
  if (node.children.length > 0) {
    return <TagGroup node={node} depth={depth} {...props} />;
  }
  return <TagLeaf node={node} depth={depth} {...props} />;
}

/** A group: its own toggle, closed until the user opens it (the old panels' default). */
function TagGroup({ node, depth, ...props }: { node: TreeNode; depth: number } & TreeProps) {
  const [open, setOpen] = React.useState(false);
  return (
    <div data-role="browse-group" data-key={node.key}>
      <button
        type="button"
        data-role="browse-group-toggle"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        style={browseGroupButtonStyle(depth)}
      >
        <span data-role="browse-group-glyph">{open ? "▾" : "▸"}</span>
        <span data-role="browse-name" style={browseGroupNameStyle}>
          {node.name}
        </span>
      </button>
      {open &&
        node.children.map((child) => (
          <TagRow key={child.key} node={child} depth={depth + 1} {...props} />
        ))}
    </div>
  );
}

/** One leaf: its tick box when it can be ticked, its name and path, its type, its access and its mark. */
function TagLeaf({ node, depth, marks, checked, onTick, disabled }: { node: TreeNode; depth: number } & TreeProps) {
  return (
    <div data-role="browse-node" data-key={node.key} data-group="false" style={browseRowStyle(depth)}>
      {node.selectable && (
        <input
          type="checkbox"
          data-role="browse-tick"
          data-key={node.key}
          aria-label={node.select_label}
          checked={checked(node)}
          disabled={disabled}
          onChange={(event) => onTick?.(node, event.currentTarget.checked)}
          style={browseTickStyle}
        />
      )}
      <span style={browseNameColumnStyle}>
        <span data-role="browse-name" style={browseNameStyle}>
          {node.name}
        </span>
        <span data-role="browse-path" style={browsePathStyle}>
          {node.path}
        </span>
      </span>
      {node.type_label !== undefined && (
        <span data-role="browse-type" style={browseMetaStyle}>
          {node.type_label}
        </span>
      )}
      {node.access !== undefined && (
        <span
          data-role="browse-access"
          title={marks.accessTooltip.replace("{access}", node.access)}
          style={browseMetaStyle}
        >
          {node.access}
        </span>
      )}
      {node.added && (
        <span
          data-role="browse-added"
          aria-label={marks.addedAriaLabel.replace("{path}", node.path)}
          style={pillStyle("ok")}
        >
          {marks.added}
        </span>
      )}
    </div>
  );
}

/**
 * `Add`: enabled when Rust enables it and something is ticked. While nothing is ticked it says Rust's
 * `no_selection_reason`; when Rust disables it, it says Rust's reason. With ticks it reads Rust's counted
 * label, with the number of ticks in its `{count}` place.
 */
export function AddControls({
  add,
  selected,
  busy,
  onAdd,
}: {
  /** The `Add` button, as the view gives it. */
  add: AddButton;
  /** How many keys are ticked. */
  selected: number;
  /** Whether a Rust action is running. */
  busy: boolean;
  /** Called when `Add` is chosen while it can be. */
  onAdd: () => void;
}) {
  const enabled = add.enabled && selected > 0 && !busy;
  const reason = add.enabled ? (selected > 0 ? undefined : add.no_selection_reason) : add.reason;
  const label = selected > 0 ? add.counted_label.replace("{count}", String(selected)) : add.label;
  return (
    <>
      <button
        type="button"
        data-role="browse-add"
        disabled={!enabled}
        title={reason ?? label}
        onClick={() => {
          if (enabled) {
            onAdd();
          }
        }}
        style={inspectorActionStyle("primary", enabled)}
      >
        {label}
      </button>
      {reason !== undefined && (
        <span data-role="browse-add-reason" style={actionReasonStyle}>
          {reason}
        </span>
      )}
    </>
  );
}

/** The write toggle: its own text from Rust, and the reason when it cannot be changed. */
export function WriteToggle({
  toggle,
  writable,
  busy,
  onWritable,
}: {
  /** The toggle, as the view gives it. */
  toggle: Toggle;
  /** Whether the user allows writes. */
  writable: boolean;
  /** Whether a Rust action is running. */
  busy: boolean;
  /** Called with the toggle's new state. */
  onWritable: (writable: boolean) => void;
}) {
  return (
    <>
      <label data-role="browse-write" style={writeToggleStyle}>
        <input
          type="checkbox"
          data-role="browse-write-toggle"
          checked={writable}
          disabled={!toggle.enabled || busy}
          onChange={(event) => onWritable(event.currentTarget.checked)}
        />
        <span data-role="browse-write-label">{toggle.label}</span>
      </label>
      {!toggle.enabled && toggle.reason !== undefined && (
        <span data-role="browse-write-reason" style={writeReasonStyle}>
          {toggle.reason}
        </span>
      )}
    </>
  );
}

/** The search box: its placeholder from Rust, its text kept by the pane. */
export function SearchField({
  search,
  value,
  onChange,
}: {
  /** The search box, as the view gives it. */
  search: SearchBox;
  /** The text typed so far. */
  value: string;
  /** Called with the text as typed. */
  onChange: (value: string) => void;
}) {
  // The text is read from the `input` event as well as from `onChange`, as the form fields are (FormView.tsx):
  // React answers `onChange` to an `input` event only when its input-event check passed as React loaded, and
  // a host without a DOM at that moment fails it, so the typed text would never reach the search.
  return (
    <input
      type="text"
      data-role="browse-search"
      placeholder={search.placeholder}
      value={value}
      onInput={(event) => onChange(event.currentTarget.value)}
      onChange={(event) => onChange(event.currentTarget.value)}
      style={browseSearchStyle}
    />
  );
}

/** The result of the pane's last action, drawn in its footer with the actions it offers. */
export function BrowseNotice({ notice, onAction }: { notice: Notice; onAction: BrowseAction }) {
  return (
    <div data-role="browse-notice" data-tone={notice.tone} style={formNoticeStyle(notice.tone)}>
      <span data-role="browse-notice-text" style={formNoticeTextStyle(notice.tone)}>
        {notice.text}
      </span>
      {notice.detail !== undefined && (
        <span data-role="browse-notice-detail" style={formNoticeDetailStyle}>
          {notice.detail}
        </span>
      )}
      {notice.actions.map((action) => (
        <InspectorAction key={action.id} action={action} onAction={(chosen) => onAction(chosen)} />
      ))}
    </div>
  );
}

/**
 * The browse pane of a tree: its title, its tree or its empty text, its search, and its footer with
 * `Add`, the write toggle and the result of the last action.
 */
export function BrowsePane({
  browse,
  notice,
  busy = false,
  onAction,
  onClose,
}: {
  /** The browse view, as Rust gives it. */
  browse: BrowseView;
  /** The result of the last action, when it names this pane. */
  notice?: Notice;
  /** Whether a Rust action is running: the ticks, `Add` and the toggle wait for its reply. */
  busy?: boolean;
  /** Called with a chosen action, and the values it carries. */
  onAction: BrowseAction;
  /** Called when the close button is chosen. */
  onClose: () => void;
}) {
  const [ticked, setTicked] = React.useState<Ticked>(() => new Set<string>());
  const [writable, setWritable] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const needle = query.trim().toLowerCase();
  // Only a notice that names this pane is drawn here (specification 25 §5.13).
  const own = notice !== undefined && notice.pane === browse.id ? notice : undefined;
  const add = browse.add;
  const shown =
    needle === ""
      ? undefined
      : leafNodes(browse.tree).filter((node) => matchesSearch(node, needle));
  const treeProps: TreeProps = {
    marks: leafMarks(browse),
    checked: (node) => ticked.has(node.key),
    onTick: (node, checked) => setTicked((current) => withTick(current, node.key, checked)),
    disabled: busy,
  };
  return (
    <div
      data-role="browse-view"
      data-kind={browse.kind}
      data-loading={browse.loading ? "true" : "false"}
      style={addDialogStyle}
    >
      <header style={addHeaderStyle}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <span data-role="browse-breadcrumb" style={browseBreadcrumbStyle}>
            {browse.breadcrumb}
          </span>
          <span data-role="browse-title" title={browse.title} style={addTitleStyle}>
            {browse.title}
          </span>
        </div>
        <button
          type="button"
          data-role="browse-close"
          aria-label={browse.close_label}
          onClick={onClose}
          style={addCloseStyle}
        >
          ✕
        </button>
      </header>
      <div style={addBodyStyle}>
        <span data-role="browse-subtitle" style={formOwnerStyle}>
          {browse.subtitle}
        </span>
        {browse.loading && <span data-role="browse-loading" style={loadingDotStyle} />}
        {browse.error !== undefined && <BrowseErrorBox error={browse.error} onAction={onAction} />}
        {browse.search !== undefined && (
          <SearchField search={browse.search} value={query} onChange={setQuery} />
        )}
        {shown !== undefined ? (
          shown.length > 0 ? (
            <section data-role="browse-tree">
              <TagTree nodes={shown} {...treeProps} />
            </section>
          ) : (
            <p data-role="browse-no-match" style={browseEmptyStyle}>
              {browse.search?.no_match}
            </p>
          )
        ) : browse.tree.length > 0 ? (
          <section data-role="browse-tree">
            <TagTree nodes={browse.tree} {...treeProps} />
          </section>
        ) : (
          browse.empty_text !== undefined && (
            <p data-role="browse-empty" style={browseEmptyStyle}>
              {browse.empty_text}
            </p>
          )
        )}
        <div data-role="browse-footer" style={addRowStyle}>
          {add !== undefined && (
            <AddControls
              add={add}
              selected={ticked.size}
              busy={busy}
              onAdd={() => onAction(add.action, { keys: [...ticked], writable })}
            />
          )}
          <WriteToggle
            toggle={browse.write_toggle}
            writable={writable}
            busy={busy}
            onWritable={setWritable}
          />
          {own !== undefined && <BrowseNotice notice={own} onAction={onAction} />}
        </div>
      </div>
    </div>
  );
}
