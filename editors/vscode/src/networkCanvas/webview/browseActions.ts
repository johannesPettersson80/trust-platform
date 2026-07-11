// §0.5.2 per-protocol "browse what it exposes". Two shapes:
//  - REMOTE browse  (mode "tags")   — look inside a device's symbols/channels (ADS client, …).
//  - LOCAL picker    (mode "expose") — pick truST's OWN globals to expose/publish (OPC UA / ADS
//                                      server, OpenOT). Source is the local program (LSP-backed).
// Protocols not listed have no browsable tree (Modbus = register-map editor; MQTT = observed
// topics; OpenOT exposes via source/producer config, not a globals list; simulated/loopback =
// none) and get no "browse" button.
export interface BrowseAction {
  label: string; // inspector button label
  title: string; // panel header
  actionLabel: string; // panel add/confirm button
  mode: "tags" | "expose";
  local: boolean; // browse truST's own globals (true) vs a remote device (false)
  route: boolean; // an ADS route is required first
  kind: "symbols" | "channels" | "nodes";
}

export function browseAction(protocol: string): BrowseAction | undefined {
  switch (protocol) {
    case "ads":
      return { label: "Browse variables", title: "Browse variables", actionLabel: "Add variables", mode: "tags", local: false, route: true, kind: "symbols" };
    case "ethercat":
      return { label: "Browse channels", title: "Browse PDO channels", actionLabel: "Add channels", mode: "tags", local: false, route: false, kind: "channels" };
    case "opcua_client":
      // REMOTE browse of an external OPC-UA server's address space; pick nodes to read. Security/cert
      // is handled via the structured browse error (not an ADS route), so route:false.
      return { label: "Browse nodes", title: "Browse OPC UA nodes", actionLabel: "Add nodes", mode: "tags", local: false, route: false, kind: "nodes" };
    case "opcua":
    case "ads_server":
      // Only these carry an `expose` (json_array) field in comm.schema; OpenOT does not.
      return { label: "Choose globals", title: "Choose globals to expose", actionLabel: "Expose selected", mode: "expose", local: true, route: false, kind: "symbols" };
    default:
      return undefined;
  }
}
