// The panel shell: the toolbar above the canvas or the list, the inspector of the selected item, a
// runtime's right-click menu and the issues list, all drawn from the view model Rust returns (design
// §4.5 D1 and D2). `App` holds only view state — the open right-hand pane and whether the canvas or
// the list is shown — and computes no label, state, enabled flag or count: every one of those is a
// field of the view, drawn as given. Only one right-hand pane is open at a time, and Escape closes
// it (specification 25 §5.11).

import React from "react";

import { Canvas } from "./Canvas";
import { ContextMenu } from "./ContextMenu";
import { Inspector } from "./Inspector";
import { IssuesList } from "./IssuesList";
import { ListView } from "./ListView";
import { appStyle, bodyStyle } from "./styles";
import { Toolbar } from "./Toolbar";
import type {
  Action,
  DevicesView,
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
 * the canvas draws. The graph fields are the canvas's own props, so the shell adds no shape of its
 * own; each is present only when Rust gives it.
 */
type ShellView = DevicesView & CanvasGraph;

/** The one open right-hand pane: the inspector of an item, a runtime's right-click menu, or the
 * issues list. */
type OpenPane =
  | { kind: "inspector"; id: string }
  | { kind: "menu"; id: string; x: number; y: number }
  | { kind: "issues" };

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
}: {
  /** The view model, as Rust returned it. */
  view: ShellView;
  /** Posts a message to the extension host; the host owns the transport (design §1.4.1). */
  post: (message: unknown) => void;
}) {
  const [pane, setPane] = React.useState<OpenPane | undefined>(undefined);
  const [panelView, setPanelView] = React.useState<PanelView>("topology");
  const onAction = React.useCallback(
    (action: Action) => {
      post({ type: "action", action });
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
      onAction(action);
      close();
    },
    [onAction, close],
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

  const inspectorId = pane?.kind === "inspector" ? pane.id : undefined;
  const inspector = inspectorId === undefined ? undefined : inspectorFor(view, inspectorId);
  const menuId = pane?.kind === "menu" ? pane.id : undefined;
  const menuRuntime = menuId === undefined ? undefined : runtimeFor(view, menuId);
  const hasList = view.list !== undefined;
  return (
    <div style={appStyle}>
      <Toolbar
        toolbar={view.toolbar}
        issues={view.issues?.button}
        view={hasList ? panelView : undefined}
        onSelectView={setPanelView}
        onOpenIssues={openIssues}
        onAction={onAction}
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
            hideSlots={pane !== undefined}
            selectedRuntime={inspectorId}
            onSelectRuntime={openInspector}
            onSelect={openInspector}
            onContextMenu={openMenu}
            onOpenIssues={openIssues}
            onAction={onAction}
          />
        )}
        {inspectorId !== undefined && inspector !== undefined && (
          <Inspector key={inspectorId} inspector={inspector} onAction={onAction} onClose={close} />
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
          <IssuesList key="issues" issues={view.issues} onAction={onAction} onClose={close} />
        )}
      </div>
    </div>
  );
}
