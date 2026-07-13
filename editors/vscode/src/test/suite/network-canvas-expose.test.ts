import {
  assert,
  buildExposeApplyParams,
} from "./network-canvas-fixtures";

suite("Network Canvas — expose globals apply params", function () {
  test("drops topology-only evidence fields before re-applying ADS/OPC UA server config", () => {
    const { names, params } = buildExposeApplyParams(
      {
        enabled: true,
        listen: "0.0.0.0",
        ams_net_id: "127.0.0.1.1.1",
        port: 48898,
        expose: ["TankLevel"],
        writable: [],
        clients: [],
        clients_count: 0,
        clients_summary: ["127.0.0.1.1.100 (from 127.0.0.1)"],
        username_set: true,
      },
      ["global.Setpoint"],
      true
    );

    assert.deepStrictEqual(names, ["Setpoint"]);
    assert.deepStrictEqual(params.expose, ["TankLevel", "Setpoint"]);
    assert.deepStrictEqual(params.writable, ["Setpoint"]);
    assert.strictEqual(params.clients_count, undefined);
    assert.strictEqual(params.clients_summary, undefined);
    assert.strictEqual(params.username_set, undefined);
  });
});
