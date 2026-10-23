import * as assert from "assert";
import * as path from "path";

import { buildAdsGeneratedImportArgs } from "../../networkCanvas/adsBrowseContract";
import { removeAdsTagFromToml } from "../../networkCanvas/adsTagConfigMutation";
import { planAdsTagAdd } from "../../networkCanvas/adsTagBatch";

suite("ADS tag config mutation", () => {
  test("unchecking removes the tag from matching ADS connections only", () => {
    const before = `[[connections]]
name = "line_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301
local_net_id = "192.168.77.10.1.1"

[[connections.points]]
var = "var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections.points]]
var = "var_2"
symbol = "Task 4.Inputs.Var 2"
type = "USINT"

[[connections]]
name = "duplicate_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301

[[connections.points]]
var = "duplicate_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections]]
name = "line_851"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 851

[[connections.points]]
var = "other_port_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"
`;

    const result = removeAdsTagFromToml(before, {
      host: "192.168.77.11",
      targetNetId: "100.67.6.217.1.1",
      port: 301,
      path: "Task 4.Inputs.Var 1",
    });

    assert.strictEqual(result.removedCount, 2);
    assert.ok(result.text.includes('symbol = "Task 4.Inputs.Var 2"'));
    assert.ok(result.text.includes('name = "line_851"'));
    assert.ok(result.text.includes('var = "other_port_var_1"'));
    assert.ok(!result.text.includes('var = "var_1"\n'));
    assert.ok(!result.text.includes('name = "duplicate_301"'));
  });

  test("tag removal regenerates ADS ST from the updated config and all snapshots", () => {
    const root = path.resolve("ads-removal-project");
    const config = path.join(root, "ads.toml");
    const output = path.join(root, "src", "generated", "ads_generated.st");
    const snapshots = [
      path.join(root, "ads", "snapshots", "line_851.symbols.json"),
      path.join(root, "ads", "snapshots", "line_301.symbols.json"),
    ];

    assert.deepStrictEqual(
      buildAdsGeneratedImportArgs(config, snapshots, output),
      [
        "ads",
        "import",
        "--config",
        config,
        "--snapshot",
        snapshots[0],
        "--snapshot",
        snapshots[1],
        "--output",
        output,
        "--force",
        "--json",
      ],
    );
  });

  test("checking merges with the existing port connection instead of duplicating it", () => {
    const target = {
      name: "PLC_port_301",
      host: "192.168.77.11",
      target_net_id: "100.67.6.217.1.1",
      responding_ads_ports: [301, 851],
    };
    const connections = [
      {
        name: "PLC_port_851",
        host: "192.168.77.11",
        target_net_id: "100.67.6.217.1.1",
        ams_port: 851,
        points: [{ symbol: "GVL.ExistingA" }],
      },
      {
        name: "legacy_duplicate_851",
        host: "192.168.77.11",
        target_net_id: "100.67.6.217.1.1",
        ams_port: 851,
        points: [{ symbol: "GVL.ExistingB" }],
      },
    ];

    assert.deepStrictEqual(
      planAdsTagAdd(connections, target, 851, [
        "GVL.ExistingA",
        "GVL.ExistingB",
        "GVL.NewTag",
      ]),
      {
        connectionName: "PLC_port_851",
        paths: ["GVL.ExistingA", "GVL.ExistingB", "GVL.NewTag"],
      },
    );
    assert.deepStrictEqual(
      planAdsTagAdd(connections, target, 851, ["GVL.ExistingA", "GVL.NewTag"]).paths,
      ["GVL.ExistingA", "GVL.NewTag"],
      "a stale browser connection snapshot must not re-add an unchecked tag",
    );
    assert.strictEqual(
      planAdsTagAdd([], target, 851, ["GVL.NewTag"]).connectionName,
      "PLC_port_851",
      "a new port must replace an existing _port_N suffix instead of appending another one",
    );
  });
});
