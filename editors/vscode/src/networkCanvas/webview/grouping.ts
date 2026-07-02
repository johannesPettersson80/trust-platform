// User-facing Add picker taxonomy from S-09. The backend schema still owns protocol ids,
// fields, and save behavior; this file owns only how the short catalog is presented.

export interface AddPickerProtocol {
  id: string;
  title: string;
  purpose?: string;
  category?: string | null;
}

export interface AddPickerItem<T extends AddPickerProtocol> {
  protocol: T;
  title: string;
  purpose: string;
  badge: string;
}

export interface AddPickerGroup<T extends AddPickerProtocol> {
  key: string;
  label: string;
  advanced: boolean;
  items: Array<AddPickerItem<T>>;
}

interface PickerProtocolCopy {
  title?: string;
  purpose: string;
  badge?: string;
}

interface PickerGroupSpec {
  key: string;
  label: string;
  advanced?: boolean;
  ids: string[];
}

export const ADD_PICKER_GROUPS: ReadonlyArray<PickerGroupSpec> = [
  {
    key: "devices_io",
    label: "Devices and I/O",
    ids: ["modbus_tcp", "modbus", "ethercat", "gpio", "simulated", "loopback"],
  },
  {
    key: "read_tags",
    label: "Read tags from another PLC or server",
    ids: ["opcua_client", "ads"],
  },
  {
    key: "share_values",
    label: "Share truST values",
    ids: ["opcua", "ads_server"],
  },
  {
    key: "messages",
    label: "Send and receive messages",
    ids: ["mqtt"],
  },
  {
    key: "advanced",
    label: "Advanced integrations",
    advanced: true,
    ids: ["mesh", "openot", "realtime_t0", "runtime_cloud", "federation"],
  },
];

const ADD_PICKER_COPY: Record<string, PickerProtocolCopy> = {
  modbus_tcp: {
    title: "Modbus",
    purpose: "Connect PLCs, drives, meters, and remote I/O.",
    badge: "MB",
  },
  modbus: {
    title: "Modbus",
    purpose: "Connect PLCs, drives, meters, and remote I/O.",
    badge: "MB",
  },
  ethercat: {
    purpose: "Connect EtherCAT drives, terminals, or remote I/O.",
    badge: "EC",
  },
  gpio: {
    purpose: "Use Raspberry Pi or Linux controller pins.",
    badge: "IO",
  },
  simulated: {
    purpose: "Practice and test without hardware.",
    badge: "SIM",
  },
  loopback: {
    purpose: "Echo outputs back into inputs for quick checks.",
    badge: "LOOP",
  },
  opcua_client: {
    purpose: "Read selected tags from another OPC UA server.",
    badge: "UA IN",
  },
  ads: {
    purpose: "Read tags from a TwinCAT or ADS PLC.",
    badge: "ADS IN",
  },
  opcua: {
    purpose: "Share truST values with SCADA, HMI, or historians.",
    badge: "UA OUT",
  },
  ads_server: {
    purpose: "Share truST values with TwinCAT or ADS clients.",
    badge: "ADS OUT",
  },
  mqtt: {
    title: "MQTT broker",
    purpose: "Send and receive process values through MQTT topics.",
    badge: "MQTT",
  },
  mesh: {
    title: "Mesh / Zenoh",
    purpose: "Configure a peer network; no live link is active until a runtime reports one.",
  },
  openot: {
    purpose: "Configure OpenOT evidence output; no evidence is published until a runtime reports one.",
  },
  realtime_t0: {
    title: "Realtime T0",
    purpose:
      "Configure deterministic same-host exchange; no live exchange runs until a runtime reports one.",
  },
  runtime_cloud: {
    title: "Runtime cloud",
    purpose: "Configure federation policy; no live link is active until a runtime reports one.",
  },
  federation: {
    title: "Runtime cloud",
    purpose: "Configure federation policy; no live link is active until a runtime reports one.",
  },
};

export function addPickerBadge(protocolId: string, fallback: string): string {
  return ADD_PICKER_COPY[protocolId]?.badge ?? fallback;
}

export function addPickerTitle(protocol: AddPickerProtocol): string {
  return ADD_PICKER_COPY[protocol.id]?.title ?? protocol.title;
}

export function addPickerPurpose(protocol: AddPickerProtocol): string {
  return ADD_PICKER_COPY[protocol.id]?.purpose ?? protocol.purpose ?? "";
}

export function groupForAddPicker<T extends AddPickerProtocol>(items: T[]): AddPickerGroup<T>[] {
  const byId = new Map(items.map((item) => [item.id, item]));
  const used = new Set<string>(["discovery"]);
  const groups: AddPickerGroup<T>[] = [];

  for (const spec of ADD_PICKER_GROUPS) {
    const groupItems: Array<AddPickerItem<T>> = [];
    for (const id of spec.ids) {
      const protocol = byId.get(id);
      if (!protocol) {
        continue;
      }
      used.add(protocol.id);
      groupItems.push({
        protocol,
        title: addPickerTitle(protocol),
        purpose: addPickerPurpose(protocol),
        badge: addPickerBadge(protocol.id, fallbackBadge(protocol.id)),
      });
    }
    if (groupItems.length > 0) {
      groups.push({
        key: spec.key,
        label: spec.label,
        advanced: spec.advanced === true,
        items: groupItems,
      });
    }
  }

  const other = items
    .filter((item) => !used.has(item.id))
    .map((protocol) => ({
      protocol,
      title: addPickerTitle(protocol),
      purpose: addPickerPurpose(protocol),
      badge: addPickerBadge(protocol.id, fallbackBadge(protocol.id)),
    }));
  if (other.length > 0) {
    groups.push({ key: "other", label: "Other choices", advanced: true, items: other });
  }

  return groups;
}

function fallbackBadge(protocolId: string): string {
  return protocolId.replace(/_/g, "").slice(0, 3).toUpperCase();
}
