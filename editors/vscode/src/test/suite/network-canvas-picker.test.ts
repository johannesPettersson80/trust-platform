import {
  assert,
  ADD_PICKER_GROUPS,
  groupForAddPicker,
  protocolName,
} from "./network-canvas-fixtures";

suite("Network Canvas — add picker taxonomy", function () {
  type P = { id: string; title: string; purpose?: string; category?: string };
  const p = (id: string, category?: string): P => ({ id, title: id, purpose: `${id} purpose`, category });

  test("groups protocols by user intent in the S-09 order", () => {
    const protos: P[] = [
      p("opcua", "supervisory_service"),
      p("modbus_tcp", "field_device"),
      p("mqtt", "field_device"),
      p("mesh", "peer_link"),
      p("gpio", "field_device"),
      p("opcua_client", "peer_link"),
      p("ads_server", "supervisory_service"),
      p("ads", "peer_link"),
    ];
    const groups = groupForAddPicker(protos);
    assert.deepStrictEqual(groups.map((g) => g.key), [
      "devices_io",
      "read_tags",
      "share_values",
      "messages",
      "ads_advanced",
      "advanced",
    ]);
    assert.deepStrictEqual(groups.map((g) => g.label), [
      "Devices and I/O",
      "Read tags from another PLC or server",
      "Share truST values",
      "Send and receive messages",
      "ADS advanced setup",
      "Advanced integrations",
    ]);
    assert.deepStrictEqual(groups[0].items.map((item) => item.protocol.id), ["modbus_tcp", "gpio"]);
  });
  test("omits empty groups and keeps advanced choices separate", () => {
    const groups = groupForAddPicker<P>([p("modbus_tcp"), p("mesh")]);
    assert.deepStrictEqual(groups.map((g) => g.key), ["devices_io", "advanced"]);
    assert.strictEqual(groups[1].advanced, true);
  });
  test("does not render runtime discovery as a protocol card", () => {
    const groups = groupForAddPicker<P>([p("discovery"), p("modbus_tcp")]);
    assert.deepStrictEqual(groups.map((g) => g.key), ["devices_io"]);
    assert.deepStrictEqual(groups.flatMap((g) => g.items.map((item) => item.protocol.id)), ["modbus_tcp"]);
  });
  test("routes unknown protocols to a trailing advanced Other choices group and never drops anything", () => {
    const groups = groupForAddPicker<P>([p("modbus_tcp"), p("mystery"), p("blank")]);
    assert.deepStrictEqual(groups.map((g) => g.key), ["devices_io", "other"]);
    assert.strictEqual(groups[1].advanced, true);
    assert.deepStrictEqual(groups[1].items.map((item) => item.protocol.id), ["mystery", "blank"]);
    assert.strictEqual(groups.reduce((n, g) => n + g.items.length, 0), 3);
  });
  test("manual ADS client and server setup are explicit advanced choices", () => {
    const groups = groupForAddPicker<P>([p("opcua"), p("opcua_client"), p("ads_server"), p("ads")]);
    const items = new Map(groups.flatMap((g) => g.items.map((item) => [item.protocol.id, item])));
    assert.strictEqual(items.get("opcua")?.badge, "UA OUT");
    assert.strictEqual(items.get("opcua_client")?.badge, "UA IN");
    assert.strictEqual(items.get("ads_server")?.badge, "ADS SERVER");
    assert.strictEqual(items.get("ads")?.badge, "ADS");
    assert.strictEqual(items.get("ads")?.title, "Connect using a known ADS address");
    assert.strictEqual(
      items.get("ads_server")?.title,
      "Expose this truST runtime as an ADS server",
    );
    assert.ok(items.get("opcua_client")?.purpose.includes("Read selected tags"));
    assert.ok(items.get("opcua")?.purpose.includes("Share truST values"));
  });
  test("advanced picker copy is user-facing and not backend review prose", () => {
    const groups = groupForAddPicker<P>([
      p("mesh"),
      p("openot"),
      p("realtime_t0"),
      p("runtime_cloud"),
    ]);
    const items = new Map(groups.flatMap((g) => g.items.map((item) => [item.protocol.id, item])));
    assert.strictEqual(items.get("mesh")?.title, "Mesh / Zenoh");
    assert.strictEqual(items.get("mesh")?.badge, "MESH");
    assert.strictEqual(items.get("openot")?.badge, "OT");
    assert.strictEqual(items.get("realtime_t0")?.badge, "RT");
    assert.strictEqual(items.get("runtime_cloud")?.badge, "CLOUD");
    assert.ok(items.get("mesh")?.purpose.includes("peer network"));
    assert.ok(items.get("openot")?.purpose.includes("OpenOT evidence"));
    assert.ok(items.get("realtime_t0")?.purpose.includes("deterministic"));
    assert.ok(items.get("runtime_cloud")?.purpose.includes("federation"));
    assert.ok(!items.get("runtime_cloud")?.purpose.includes("pretending"));
  });
  test("local I/O endpoint titles leave the I/O role to the node band", () => {
    assert.strictEqual(protocolName("simulated"), "Simulated I/O");
    assert.strictEqual(protocolName("loopback"), "Loopback I/O");
  });
  test("ADD_PICKER_GROUPS is the canonical S-09 group order", () => {
    assert.deepStrictEqual(ADD_PICKER_GROUPS.map((c) => c.key), [
      "devices_io",
      "read_tags",
      "share_values",
      "messages",
      "ads_advanced",
      "advanced",
    ]);
  });
});
