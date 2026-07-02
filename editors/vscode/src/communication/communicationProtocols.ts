export type CommunicationGroup =
  | "external"
  | "runtime"
  | "fieldbus"
  | "telemetry"
  | "enterprise";

export interface CommunicationProtocolDescriptor {
  id: string;
  title: string;
  purpose: string;
  requirements: readonly string[];
  group: CommunicationGroup;
  docsPath: string;
  supportsDiagnose?: boolean;
  supportsTest?: boolean;
}

export const COMMUNICATION_PROTOCOLS: readonly CommunicationProtocolDescriptor[] = [
  {
    id: "ads",
    title: "ADS / TwinCAT",
    purpose: "Connect to TwinCAT symbols or expose truST globals to ADS clients.",
    requirements: [
      "Selected runtime",
      "AMS Net ID",
      "ADS logical port 851",
      "Route and allowlist",
    ],
    group: "external",
    docsPath: "docs/public/connect/external-systems/ads.md",
    supportsDiagnose: true,
  },
  {
    id: "opcua",
    title: "OPC UA",
    purpose: "Let SCADA, HMI, or historian software read and write exposed PLC tags.",
    requirements: ["Listen address", "Security policy", "Globals to expose"],
    group: "external",
    docsPath: "docs/public/connect/external-systems/opc-ua.md",
  },
  {
    id: "modbus_tcp",
    title: "Modbus TCP",
    purpose: "Read and write register-oriented devices or PLC endpoints.",
    requirements: ["Device IP:port", "Unit ID", "Register ranges"],
    group: "external",
    docsPath: "docs/public/connect/external-systems/modbus-tcp.md",
    supportsTest: true,
  },
  {
    id: "mqtt",
    title: "MQTT",
    purpose: "Publish and subscribe process I/O through a broker.",
    requirements: ["Broker host:port", "Topics", "TLS/credentials if needed"],
    group: "external",
    docsPath: "docs/public/connect/external-systems/mqtt.md",
    supportsTest: true,
  },
  {
    id: "discovery",
    title: "Discovery",
    purpose: "Find and pair truST runtimes on the network.",
    requirements: ["Service name", "Network interfaces", "Host group"],
    group: "runtime",
    docsPath: "docs/public/connect/runtime-to-runtime/discovery-and-pairing.md",
  },
  {
    id: "mesh",
    title: "Mesh / Zenoh",
    purpose: "Connect this runtime to selected peers or a Zenoh router. No live link is active until a runtime reports one.",
    requirements: ["Role", "Listen address", "Peer addresses", "Mesh token if used"],
    group: "runtime",
    docsPath: "docs/public/connect/runtime-to-runtime/mesh-zenoh.md",
  },
  {
    id: "realtime_t0",
    title: "Realtime T0",
    purpose: "Check and request host settings for deterministic same-host exchange. No live exchange runs until a suitable host reports one.",
    requirements: ["PREEMPT_RT requirement", "Scheduler policy", "CPU affinity"],
    group: "runtime",
    docsPath: "docs/public/connect/runtime-to-runtime/realtime-t0.md",
  },
  {
    id: "runtime_cloud",
    title: "Runtime cloud / federation",
    purpose: "Configure federation policy and link preferences. No live link is active until a runtime reports one.",
    requirements: ["Profile", "Link preferences", "WAN write policy"],
    group: "runtime",
    docsPath: "docs/public/connect/runtime-to-runtime/runtime-cloud-federation.md",
  },
  {
    id: "ethercat",
    title: "EtherCAT",
    purpose: "Wire deterministic fieldbus I/O through a real NIC.",
    requirements: ["Adapter", "Expected modules", "Cycle budget"],
    group: "fieldbus",
    docsPath: "docs/public/connect/devices-and-fieldbus/ethercat.md",
  },
  {
    id: "gpio",
    title: "GPIO",
    purpose: "Map local Linux/Pi pins to runtime I/O.",
    requirements: ["Backend", "Pin map", "Safe states"],
    group: "fieldbus",
    docsPath: "docs/public/connect/devices-and-fieldbus/gpio.md",
  },
  {
    id: "simulated",
    title: "Simulated I/O",
    purpose: "Try process I/O without hardware.",
    requirements: ["Input count", "Output count", "Optional seed/pattern"],
    group: "fieldbus",
    docsPath: "docs/public/connect/devices-and-fieldbus/simulated-and-loopback.md",
  },
  {
    id: "loopback",
    title: "Loopback I/O",
    purpose: "Echo outputs back into inputs for fast local sanity checks.",
    requirements: ["Input/output count", "Loopback mode"],
    group: "fieldbus",
    docsPath: "docs/public/connect/devices-and-fieldbus/simulated-and-loopback.md",
  },
  {
    id: "openot",
    title: "OpenOT",
    purpose: "Configure OpenOT evidence output. No evidence is published until a runtime reports one.",
    requirements: ["Evidence file", "Record capacity", "Telemetry source"],
    group: "telemetry",
    docsPath: "docs/public/develop/openot-authoring.md",
  },
  {
    id: "enterprise",
    title: "Enterprise systems",
    purpose: "Pick OPC UA, MQTT, or OpenOT for historian, MES, ERP, or audit consumers.",
    requirements: ["Consumer type", "Security posture", "Data direction"],
    group: "enterprise",
    docsPath: "docs/public/connect/enterprise/index.md",
  },
] as const;

export const COMMUNICATION_GROUPS: readonly {
  id: CommunicationGroup;
  title: string;
  purpose: string;
}[] = [
  {
    id: "external",
    title: "External systems",
    purpose: "TwinCAT, SCADA, HMIs, brokers, and register-oriented devices.",
  },
  {
    id: "runtime",
    title: "Runtime-to-runtime",
    purpose: "Discovery, mesh, realtime links, and federation policy.",
  },
  {
    id: "fieldbus",
    title: "Devices and fieldbus",
    purpose: "Physical I/O, local pins, and hardware-free test drivers.",
  },
  {
    id: "telemetry",
    title: "Telemetry and evidence",
    purpose: "Regulated records and typed operational evidence.",
  },
  {
    id: "enterprise",
    title: "Enterprise guidance",
    purpose: "Choose the right protocol for historian and business-system integration.",
  },
] as const;
