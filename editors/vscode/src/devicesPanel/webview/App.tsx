// The panel shell: the toolbar above the canvas, both drawn from the view model Rust returns
// (design §4.5 D1). `App` holds only view state — the selected runtime — and computes no label,
// state, enabled flag or count: every one of those is a field of the view, drawn as given.

import React from "react";

import { Canvas } from "./Canvas";
import { appStyle } from "./styles";
import { Toolbar } from "./Toolbar";
import type { Action, DevicesView } from "./types";

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

/** Draws the shell and posts the chosen toolbar, banner or slot action to the extension host. */
export function App({
  view,
  post,
}: {
  /** The view model, as Rust returned it. */
  view: ShellView;
  /** Posts a message to the extension host; the host owns the transport (design §1.4.1). */
  post: (message: unknown) => void;
}) {
  const [selectedRuntime, setSelectedRuntime] = React.useState<string | undefined>(undefined);
  const onAction = React.useCallback(
    (action: Action) => {
      post({ type: "action", action });
    },
    [post],
  );
  return (
    <div style={appStyle}>
      <Toolbar toolbar={view.toolbar} onAction={onAction} />
      <Canvas
        computers={view.computers}
        summary={view.summary}
        banner={view.banner}
        devices={view.devices}
        links={view.links}
        mesh={view.mesh}
        selectedRuntime={selectedRuntime}
        onSelectRuntime={setSelectedRuntime}
        onAction={onAction}
      />
    </div>
  );
}
