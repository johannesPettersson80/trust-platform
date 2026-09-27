export type ActiveChartAction = "inspect" | "zoom-in" | "zoom-out" | "reset" | "export-figure";

export interface ActiveChartActionTarget {
  readonly plotIds: readonly string[];
  handle(action: ActiveChartAction, plotId: string): boolean;
}

type RouterState = { readonly targets: Set<ActiveChartActionTarget>; readonly listener: EventListener };
const routers = new WeakMap<Document, RouterState>();

export function initializeActiveChartActionRouter(owner: Document = document): void {
  if (routers.has(owner)) return;
  const targets = new Set<ActiveChartActionTarget>();
  const listener: EventListener = event => {
    const action = (event as CustomEvent<{ action?: string }>).detail?.action;
    if (!isActiveChartAction(action)) return;
    const view = Array.from(owner.querySelectorAll<HTMLElement>("[data-plot-view]"))
      .find(candidate => !candidate.hidden);
    const plotId = view?.dataset.plotView;
    if (!plotId || plotId === "response" || plotId === "recorded-responses") return;
    for (const target of targets) {
      if (target.plotIds.includes(plotId) && target.handle(action, plotId)) {
        event.preventDefault();
        return;
      }
    }
  };
  owner.defaultView?.addEventListener("trust-control-shared-chart-action", listener);
  routers.set(owner, { targets, listener });
}

export function registerActiveChartActionTarget(target: ActiveChartActionTarget, owner: Document = document): () => void {
  initializeActiveChartActionRouter(owner);
  const state = routers.get(owner)!;
  state.targets.add(target);
  return () => state.targets.delete(target);
}

export function disposeActiveChartActionRouter(owner: Document = document): void {
  const state = routers.get(owner);
  if (!state) return;
  owner.defaultView?.removeEventListener("trust-control-shared-chart-action", state.listener);
  routers.delete(owner);
}

function isActiveChartAction(action: string | undefined): action is ActiveChartAction {
  return action === "inspect" || action === "zoom-in" || action === "zoom-out" ||
    action === "reset" || action === "export-figure";
}
