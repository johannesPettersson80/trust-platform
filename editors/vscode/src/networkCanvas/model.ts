// The canvas's add flow, as view state: the step the user is on and the protocol of the open form.
// What the canvas draws for them (the drawn simulator, its connections, the draft of the open form,
// the first device, the faults, the banner and the summary) is the server's
// (`trust-lsp.devicesTopology`, crates/trust-devices/src/canvas; specification 25 §5.15).

export type NetworkCanvasStage =
  | "welcome"
  | "runtime_live"
  | "intent"
  | "add_device"
  | "connected";

export interface NetworkCanvasStep {
  readonly id: NetworkCanvasStage;
  readonly label: string;
}

/** The protocols of the drawn simulator's connections, which the add flow's form can set up. */
export type NetworkCanvasProtocolId =
  | "modbus_tcp"
  | "mqtt"
  | "ethercat"
  | "gpio"
  | "simulated"
  | "loopback";

export const NETWORK_CANVAS_STEPS: readonly NetworkCanvasStep[] = [
  { id: "welcome", label: "Welcome" },
  { id: "runtime_live", label: "Runtime live" },
  { id: "intent", label: "What to connect" },
  { id: "add_device", label: "Set up device" },
  { id: "connected", label: "Connected" },
];

const STAGE_ORDER: readonly NetworkCanvasStage[] = NETWORK_CANVAS_STEPS.map(
  (step) => step.id
);

export const NETWORK_CANVAS_IO_PROTOCOLS: readonly NetworkCanvasProtocolId[] = [
  "simulated",
  "loopback",
  "modbus_tcp",
  "mqtt",
  "ethercat",
  "gpio",
];

export function nextNetworkCanvasStage(
  stage: NetworkCanvasStage
): NetworkCanvasStage {
  const index = STAGE_ORDER.indexOf(stage);
  return STAGE_ORDER[Math.min(Math.max(index, 0) + 1, STAGE_ORDER.length - 1)];
}

export function isNetworkCanvasStage(
  value: unknown
): value is NetworkCanvasStage {
  return typeof value === "string" && STAGE_ORDER.includes(value as NetworkCanvasStage);
}
