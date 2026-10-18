// The panel shell: the toolbar above the canvas or the list, the inspector of the selected item, a
// runtime's right-click menu, the issues list, the Add dialog, and the form pane an action reply
// opens, all drawn from the view model Rust returns (design §4.5 D1, D2 and D3). `App` holds only
// view state — the open right-hand pane and whether the canvas or the list is shown — and computes
// no label, state, enabled flag or count: every one of those is a field of the view, drawn as given.
// Only one right-hand pane is open at a time, and Escape closes it (specification 25 §5.11). It
// passes the host's saved item positions to the canvas and posts the positions a drag produces, so
// the host keeps them for the project (specification 25 §5.11, D2), it opens the Add dialog from the
// `ui` op that `+ Add`, a runtime's `Add connection` slot and its menu carry (§5.14), and it opens
// and closes the form pane from the host's replies, so a result appears in the pane that started it
// (§5.13, D3). It opens a browse pane from the `ui` op a browse carries and draws the ADS tag browser or
// the browse tree that Rust's view says; every action goes through one handler, which carries out the
// `ui` ops and posts the rest with the values their pane gives (design §4.5 D5 and D5b). It opens the
// Discover pane from the toolbar's Discover action and draws the search that the host's `discover` view
// gives (design §4.5 D4).

import React from "react";

import { AddDialog as AddDialogPane } from "./AddDialog";
import type { AddDialog } from "./addTypes";
import { AdsTagBrowser } from "./AdsTagBrowser";
import { BrowsePane } from "./BrowsePane";
import { Canvas } from "./Canvas";
import { ContextMenu } from "./ContextMenu";
import { DiscoverPane } from "./DiscoverPane";
import { FormView, formPaneName, type FormValues } from "./FormView";
import type { ActReply, Form } from "./formTypes";
import { Inspector } from "./Inspector";
import { IssuesList } from "./IssuesList";
import { ListView } from "./ListView";
import type { Point } from "./layout";
import { appStyle, bodyStyle, formPaneStyle } from "./styles";
import { addPaneStyle } from "./styles/addDialog";
import { browsePaneStyle } from "./styles/browse";
import { Toolbar } from "./Toolbar";
import type {
  Action,
  BrowseView,
  DevicesView,
  DiscoverView,
  Inspector as InspectorModel,
  PanelView,
  Runtime,
} from "./types";

/** The graph fields the canvas draws, taken from `Canvas`'s own props so the two always agree. */
type CanvasGraph = Pick<
  React.ComponentProps<typeof Canvas>,
  "summary" | "banner" | "devices" | "links" | "mesh"
>;

/**
 * The view the shell hands on: the fields it mirrors for the toolbar (`DevicesView`) plus the graph
 * the canvas draws and the Add dialog every view carries as `add` (specification 25 §5.14). The
 * graph fields are the canvas's own props, so the shell adds no shape of its own; each is present
 * only when Rust gives it.
 */
type ShellView = DevicesView & CanvasGraph & { add?: AddDialog };

/** The one open right-hand pane: the inspector of an item, a runtime's right-click menu, the issues
 * list, the Add dialog, a browse pane, the Discover pane, or the form a `form.open` reply opened. */
type OpenPane =
  | { kind: "inspector"; id: string }
  | { kind: "menu"; id: string; x: number; y: number }
  | { kind: "issues" }
  | { kind: "add" }
  | { kind: "browse" }
  | { kind: "discover" }
  | { kind: "form"; form: Form };

/** Whether an action's `ui` op opens the pane `pane` in the panel's own view (design §2.1). */
function opensPane(action: Action, pane: string): boolean {
  const op = action.op;
  return op.kind === "ui" && op.ui.kind === "open" && op.ui.pane?.kind === pane;
}

/** Whether an action opens the Add dialog in the panel's own view (specification 25 §5.14). */
function opensAddDialog(action: Action): boolean {
  return opensPane(action, "add");
}

/** Whether an action closes the open pane in the panel's own view (`ui` op `close_inspector`). */
function closesPane(action: Action): boolean {
  return action.op.kind === "ui" && action.op.ui.kind === "close_inspector";
}

/**
 * The pane name Rust gives the pane that started an action, so a notice, a pending action and a
 * `close_pane` are routed to it (`actions/mod.rs` `pane`, specification 25 §5.13).
 */
function paneName(pane: OpenPane, browseId: string | undefined): string {
  switch (pane.kind) {
    case "inspector":
      return `inspector:${pane.id}`;
    case "form":
      return formPaneName(pane.form);
    case "browse":
      return browseId ?? pane.kind;
    default:
      return pane.kind;
  }
}

/** The shell does not keep a form's values itself: a chosen button posts the values it holds. */
function ignoreFormValues(): void {
  // Nothing to do: `FormView` posts its own values with the chosen button.
}

/** Finds the inspector of the item with this identity, whichever kind of item it is. */
function inspectorFor(view: ShellView, id: string): InspectorModel | undefined {
  for (const computer of view.computers) {
    if (computer.id === id) {
      return computer.inspector;
    }
    for (const runtime of computer.runtimes) {
      if (runtime.id === id) {
        return runtime.inspector;
      }
      for (const connection of runtime.connections ?? []) {
        if (connection.id === id) {
          return connection.inspector;
        }
      }
    }
  }
  for (const device of view.devices ?? []) {
    if (device.id === id) {
      return device.inspector;
    }
  }
  return undefined;
}

/** The runtime with this identity, or `undefined` when the item is not a runtime (spec 25 §5.11). */
function runtimeFor(view: ShellView, id: string): Runtime | undefined {
  for (const computer of view.computers) {
    for (const runtime of computer.runtimes) {
      if (runtime.id === id) {
        return runtime;
      }
    }
  }
  return undefined;
}

/** Draws the shell and posts the chosen action to the extension host. */
export function App({
  view,
  post,
  positions,
  reply,
  browse,
  discover,
}: {
  /** The view model, as Rust returned it. */
  view: ShellView;
  /** Posts a message to the extension host; the host owns the transport (design §1.4.1). */
  post: (message: unknown) => void;
  /** The item positions the host kept for the project, drawn over the layout (design §4.5 D2). */
  positions?: Record<string, Point>;
  /**
   * The browse view the host last answered for `trust-lsp.devicesBrowse` (design §1.4.1): the browse
   * pane draws it once a browse is open (specification 25 §5.4, design §4.5 D5).
   */
  browse?: BrowseView;
  /**
   * The Discover view of the host's last search status (design §1.4.1): the Discover pane draws it once
   * the pane is open (specification 25 §2, design §4.5 D4).
   */
  discover?: DiscoverView;
  /**
   * The latest reply to an action, which the host hands on unchanged (design §1.4.1): a `form`
   * opens in the right-hand pane, a `notice` goes to the pane it names, `close_pane` closes the
   * pane that started the action, and a `pending` action shows its text in that pane until a later
   * reply (specification 25 §5.13). `effects` are the host's, ignored here.
   */
  reply?: ActReply;
}) {
  const [pane, setPane] = React.useState<OpenPane | undefined>(undefined);
  const [panelView, setPanelView] = React.useState<PanelView>("topology");
  // Whether a Rust or host action is awaiting its reply: the browse pane's ticks and `Add` wait for it.
  const [awaiting, setAwaiting] = React.useState(false);
  // Every action the panel offers goes through here (specification 25 §5.13 and §5.14). The `ui` ops
  // are carried out by the shell: the Add dialog and the browse pane open, and `close_inspector` closes
  // the open pane and is never posted. A `ui` op posts no values. A Rust or host action posts with the
  // values its pane gives it: a form's typed values (J07, §5.4), a browse's ticked keys or typed port.
  const chooseAction = React.useCallback(
    (action: Action, values?: FormValues) => {
      if (opensAddDialog(action)) {
        setPane({ kind: "add" });
      }
      if (opensPane(action, "browse")) {
        setPane({ kind: "browse" });
      }
      if (opensPane(action, "discover")) {
        setPane({ kind: "discover" });
      }
      if (closesPane(action)) {
        setPane(undefined);
        return;
      }
      if (action.op.kind === "ui" || values === undefined) {
        post({ type: "action", action });
      } else {
        post({ type: "action", action, values });
      }
      if (action.op.kind !== "ui") {
        setAwaiting(true);
      }
    },
    [post],
  );

  const onPositions = React.useCallback(
    (dragged: Record<string, Point>) => {
      post({ type: "positions", positions: dragged });
    },
    [post],
  );
  const close = React.useCallback(() => {
    setPane(undefined);
  }, []);
  const openInspector = React.useCallback((id: string | undefined) => {
    setPane(id === undefined ? undefined : { kind: "inspector", id });
  }, []);
  const openIssues = React.useCallback(() => {
    setPane({ kind: "issues" });
  }, []);
  const openMenu = React.useCallback(
    (id: string, x: number, y: number) => {
      // Only a runtime has a context menu; a right-click on anything else opens nothing (spec 25 §5.11).
      if (runtimeFor(view, id) === undefined) {
        return;
      }
      setPane({ kind: "menu", id, x, y });
    },
    [view],
  );
  const chooseFromMenu = React.useCallback(
    (action: Action) => {
      chooseAction(action);
      // The menu's `Add device or connection…` opens its own pane; anything else closes the menu.
      if (!opensAddDialog(action)) {
        close();
      }
    },
    [chooseAction, close],
  );
  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setPane(undefined);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
  // A reply opens the form it carries, or closes the pane that started the action; a notice and a
  // pending action are drawn by the pane they name (specification 25 §5.13, design §1.4.1). The
  // effect runs once per reply, because the host hands each reply on as a new object.
  // The browse pane is named by its view's own identity, which its actions name in their notices.
  const browseId = browse?.id;
  const browseIdRef = React.useRef(browseId);
  browseIdRef.current = browseId;
  React.useEffect(() => {
    if (reply === undefined) {
      return;
    }
    setAwaiting(false);
    if (reply.form !== undefined) {
      setPane({ kind: "form", form: reply.form });
      return;
    }
    if (reply.close_pane) {
      const started = reply.notice?.pane ?? reply.pending?.pane;
      setPane((current) => {
        if (current === undefined) {
          return undefined;
        }
        // Close the pane the action started in; a reply that names none closes the open pane.
        return started === undefined || paneName(current, browseIdRef.current) === started
          ? undefined
          : current;
      });
    }
  }, [reply]);

  // The result of a browse action is drawn in the browse pane it names (specification 25 §5.13).
  const browseNotice =
    browse !== undefined && reply?.notice?.pane === browse.id ? reply.notice : undefined;
  // The Discover pane is named `discover`: a result that names it, such as Adopt's, is drawn there (§5.13).
  const discoverNotice =
    discover !== undefined && reply?.notice?.pane === "discover" ? reply.notice : undefined;

  const inspectorId = pane?.kind === "inspector" ? pane.id : undefined;
  const inspector = inspectorId === undefined ? undefined : inspectorFor(view, inspectorId);
  const menuId = pane?.kind === "menu" ? pane.id : undefined;
  const menuRuntime = menuId === undefined ? undefined : runtimeFor(view, menuId);
  const form = pane?.kind === "form" ? pane.form : undefined;
  const hasList = view.list !== undefined;
  return (
    <div style={appStyle}>
      <Toolbar
        toolbar={view.toolbar}
        issues={view.issues?.button}
        view={hasList ? panelView : undefined}
        onSelectView={setPanelView}
        onOpenIssues={openIssues}
        onAction={chooseAction}
      />
      <div style={bodyStyle}>
        {panelView === "list" && view.list !== undefined ? (
          <ListView
            columns={view.list.columns}
            computers={view.computers}
            devices={view.devices}
            onSelectRow={openInspector}
          />
        ) : (
          <Canvas
            computers={view.computers}
            summary={view.summary}
            banner={view.banner}
            devices={view.devices}
            links={view.links}
            mesh={view.mesh}
            ghost={form?.ghost}
            savedPositions={positions}
            onPositions={onPositions}
            hideSlots={pane !== undefined}
            selectedRuntime={inspectorId}
            onSelectRuntime={openInspector}
            onSelect={openInspector}
            onContextMenu={openMenu}
            onOpenIssues={openIssues}
            onAction={chooseAction}
          />
        )}
        {inspectorId !== undefined && inspector !== undefined && (
          <Inspector key={inspectorId} inspector={inspector} onAction={chooseAction} onClose={close} />
        )}
        {pane?.kind === "add" && view.add !== undefined && (
          <aside key="add" data-role="add-pane" style={addPaneStyle}>
            <AddDialogPane dialog={view.add} onAction={chooseAction} onClose={close} />
          </aside>
        )}
        {pane?.kind === "browse" && browse !== undefined && (
          <aside key="browse" data-role="browse-pane" style={browsePaneStyle}>
            {browse.kind === "ads_tags" ? (
              <AdsTagBrowser
                browse={browse}
                notice={browseNotice}
                busy={awaiting}
                onAction={chooseAction}
                onClose={close}
              />
            ) : (
              <BrowsePane
                browse={browse}
                notice={browseNotice}
                busy={awaiting}
                onAction={chooseAction}
                onClose={close}
              />
            )}
          </aside>
        )}
        {pane?.kind === "discover" && discover !== undefined && (
          <aside key="discover" data-role="discover-pane" style={addPaneStyle}>
            <DiscoverPane
              title={view.toolbar.discover.label}
              discover={discover}
              notice={discoverNotice}
              onAction={chooseAction}
              onClose={close}
            />
          </aside>
        )}
        {form !== undefined && (
          <aside key={form.id} data-role="form-pane" style={formPaneStyle}>
            <FormView
              form={form}
              notice={reply?.notice}
              pending={reply?.pending}
              onValues={ignoreFormValues}
              onAction={chooseAction}
            />
          </aside>
        )}
        {menuId !== undefined && menuRuntime !== undefined && pane?.kind === "menu" && (
          <ContextMenu
            key={menuId}
            actions={menuRuntime.context_menu ?? []}
            x={pane.x}
            y={pane.y}
            onAction={chooseFromMenu}
          />
        )}
        {pane?.kind === "issues" && view.issues !== undefined && (
          <IssuesList key="issues" issues={view.issues} onAction={chooseAction} onClose={close} />
        )}
      </div>
    </div>
  );
}
