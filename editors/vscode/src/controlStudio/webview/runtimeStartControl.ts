export interface RuntimeStartControlState {
  runtimeMode: "simulate" | "online";
  runtimeState: "running" | "connected" | "stopped" | "unavailable";
  starting?: boolean;
}

export type RuntimeStartControlResult =
  | { readonly ok: true; readonly message?: string }
  | { readonly ok: false; readonly failure: { readonly message: string } };

export interface RuntimeStartControl {
  present(state: RuntimeStartControlState): void;
  result(result: RuntimeStartControlResult): void;
}

/** Owns only the contextual simulator-start affordance; lifecycle authority remains in the host. */
export function initializeRuntimeStartControl(
  post: (kind: "control-studio.runtime.start") => void,
  owner: Document = document
): RuntimeStartControl {
  const button = owner.querySelector<HTMLButtonElement>("[data-control-studio-start-runtime]");
  let pending = false;
  let current: RuntimeStartControlState | undefined;
  if (!button) return { present: () => undefined, result: () => undefined };

  button.addEventListener("click", () => {
    if (pending || button.hidden) return;
    pending = true;
    button.disabled = true;
    button.textContent = "Starting simulator…";
    button.title = "Starting the local simulator";
    post("control-studio.runtime.start");
  });

  return {
    present(state) {
      current = state;
      const available = state.runtimeMode === "simulate" &&
        (state.runtimeState === "stopped" || state.runtimeState === "unavailable");
      button.hidden = !available;
      if (!available) { pending = false; button.disabled = false; }
      else if (!pending) button.disabled = Boolean(state.starting);
    },
    result(result) {
      pending = false;
      button.disabled = false;
      if (result.ok) {
        button.hidden = true;
        button.textContent = "Start simulator";
        button.removeAttribute("title");
      } else {
        button.hidden = current?.runtimeMode !== "simulate";
        button.textContent = "Retry simulator";
        button.title = result.failure.message;
      }
    },
  };
}
