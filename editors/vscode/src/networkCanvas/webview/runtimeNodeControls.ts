// The controls of a Network Canvas runtime node, as the language server gives them (specification 25
// §5.5; trust-devices `canvas::fleet`), and how the inspector lays them out: the primary control, at
// most two visible secondary actions and the rest behind an overflow disclosure. The rule of which
// controls a node offers lives in Rust.

export interface RuntimeNodeControl {
  // The webview→panel message action. Local lifecycle reuses the existing canvas actions
  // ("startLocalSimulator"/"stopLocalSimulator"/"openRuntimeLogs"/"openRuntimeSettings"); remote uses
  // dedicated messages ("runtimeConnect"/"runtimeDisconnect"). "none" = no-op (disabled progress).
  readonly action:
    | "startLocalSimulator"
    | "stopLocalSimulator"
    | "managedStart"
    | "managedStop"
    | "runtimeConnect"
    | "runtimeDisconnect"
    | "setAuthToken"
    | "setAsRunTarget"
    | "openRuntimeLogs"
    | "openRuntimeSettings"
    | "none";
  readonly label: string;
  readonly kind: "primary" | "secondary";
  readonly enabled: boolean;
}

export interface RuntimeNodeActionItem {
  readonly key: string;
  readonly label: string;
  readonly enabled: boolean;
  /** Why it is disabled. */
  readonly title?: string;
  readonly onClick: () => void;
}

export interface RuntimeNodeControlLayout {
  readonly primary?: RuntimeNodeControl;
  readonly visibleSecondary: RuntimeNodeActionItem[];
  readonly overflowSecondary: RuntimeNodeActionItem[];
  readonly hasOverflow: boolean;
}

export const MAX_VISIBLE_RUNTIME_NODE_SECONDARY = 2;

export function runtimeNodeControlLayout(
  controls: readonly RuntimeNodeControl[] | undefined,
  onControl: ((control: RuntimeNodeControl) => void) | undefined,
  extraSecondary: readonly RuntimeNodeActionItem[] = [],
  showAllSecondary = false
): RuntimeNodeControlLayout {
  if (!controls || !onControl) {
    return {
      primary: undefined,
      visibleSecondary: [],
      overflowSecondary: [],
      hasOverflow: false,
    };
  }
  const secondary = [
    ...controls
      .filter((control) => control.kind === "secondary")
      .map((control) => ({
        key: `${control.action}:${control.label}`,
        label: control.label,
        enabled: control.enabled,
        onClick: () => onControl(control),
      })),
    ...extraSecondary,
  ];
  const visibleSecondary = showAllSecondary
    ? secondary
    : secondary.slice(0, MAX_VISIBLE_RUNTIME_NODE_SECONDARY);
  return {
    primary: controls.find((control) => control.kind === "primary"),
    visibleSecondary,
    overflowSecondary: secondary.slice(MAX_VISIBLE_RUNTIME_NODE_SECONDARY),
    hasOverflow: secondary.length > MAX_VISIBLE_RUNTIME_NODE_SECONDARY,
  };
}
