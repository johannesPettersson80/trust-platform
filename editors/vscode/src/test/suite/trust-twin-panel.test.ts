import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  __testForceTrustTwinPollValues,
  __testForceTrustTwinRefresh,
  __testForceTrustTwinSelectPage,
  __testGetTrustTwinPanelPackageProof,
  __testGetTrustTwinPanelState,
  __testGetTrustTwinPanelWebviewContract,
  __testResetTrustTwinPanelState,
  __testSetTrustTwinControlRequestHandler,
} from "../../trustTwinPanel";

suite("trust-twin VS Code panel", function () {
  this.timeout(30000);

  teardown(() => {
    __testSetTrustTwinControlRequestHandler(undefined);
    __testResetTrustTwinPanelState();
  });

  test("opens a dedicated 3D panel and reconnects through the HMI control transport", async () => {
    const speedId = "resource/RESOURCE/program/Main/field/speed";
    let valuesRequest = 0;
    const requests: string[] = [];
    __testSetTrustTwinControlRequestHandler(async (_endpoint, _auth, requestType, params) => {
      requests.push(requestType);
      if (requestType === "hmi.schema.get") {
        return {
          version: 1,
          mode: "read_write",
          read_only: false,
          resource: "RESOURCE",
          generated_at_ms: Date.now(),
          pages: [
            {
              id: "cell",
              title: "Drive Cell 3D",
              order: 0,
              kind: "scene3d",
              view: "views/drive-cell.view.toml",
              scene_view: {
                node: [
                  {
                    id: "motor-1",
                    primitive: "box",
                    label: "Motor",
                    transform: { position: [0, 0, 0] },
                  },
                ],
                bind3d: [
                  {
                    node: "motor-1",
                    property: "material.emissive",
                    source: "Main.speed",
                    scale: {
                      min: 0,
                      max: 100,
                      output_min: 0,
                      output_max: 1,
                    },
                  },
                ],
              },
            },
          ],
          widgets: [
            {
              id: speedId,
              path: "Main.speed",
              label: "Speed",
              data_type: "REAL",
              access: "read",
              writable: false,
              widget: "gauge",
              source: "program:Main",
              page: "cell",
              group: "Drive",
              order: 0,
            },
          ],
        };
      }
      if (requestType === "hmi.values.get") {
        assert.deepStrictEqual(params, { ids: [speedId] });
        valuesRequest += 1;
        if (valuesRequest === 1) {
          throw new Error("runtime offline");
        }
        return {
          connected: true,
          timestamp_ms: Date.now(),
          freshness_ms: 0,
          values: {
            [speedId]: { v: 42, q: "good", ts_ms: Date.now() },
          },
        };
      }
      throw new Error(`Unexpected request type: ${requestType}`);
    });

    await vscode.commands.executeCommand("trust-lsp.trustTwin.openPanel");
    await __testForceTrustTwinRefresh();
    let state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.hasPanel, true);
    assert.strictEqual(state.activePage?.id, "cell");
    assert.strictEqual(state.activePage?.scene_view?.node?.[0]?.id, "motor-1");
    assert.ok(requests.includes("hmi.schema.get"));

    await __testForceTrustTwinPollValues();
    state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.connected, false);
    assert.match(state.status, /disconnected|failed|offline/i);

    await __testForceTrustTwinPollValues();
    state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.connected, true);
    assert.strictEqual(state.valuesBySource["Main.speed"], 42);
    assert.match(state.status, /connected/i);
  });

  test("declares strict CSP, local resource roots, and package-smoke assets", () => {
    const contract = __testGetTrustTwinPanelWebviewContract(
      vscode.Uri.file("/workspace"),
      vscode.Uri.file("/extension"),
    );
    assert.ok(contract.localResourceRoots.some((entry) => entry.endsWith("media/trust-twin")));
    assert.ok(contract.localResourceRoots.some((entry) => entry.endsWith("hmi")));
    assert.match(contract.csp, /default-src 'none'/);
    assert.match(contract.csp, /script-src 'nonce-\$\{nonce\}' 'wasm-unsafe-eval'/);
    assert.doesNotMatch(contract.csp, /script-src[^;]*https:/);
    assert.doesNotMatch(contract.csp, /connect-src[^;]*https:/);

    const packageProof = __testGetTrustTwinPanelPackageProof();
    assert.strictEqual(packageProof.ok, true, packageProof.missing.join(", "));
    assert.ok(packageProof.assets.includes("media/trust-twin/trust-twin-renderer.wasm"));
    assert.ok(packageProof.assets.includes("media/trust-twin/trust-twin-renderer.js"));
    assert.ok(packageProof.assets.some((entry) => entry.endsWith("components/motor.gltf")));

    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const packageJson = JSON.parse(
      fs.readFileSync(path.join(extensionRoot, "package.json"), "utf8"),
    ) as {
      activationEvents?: string[];
      scripts?: Record<string, string>;
      contributes?: { commands?: Array<{ command?: string }> };
    };
    assert.ok(packageJson.activationEvents?.includes("onCommand:trust-lsp.trustTwin.openPanel"));
    assert.ok(
      packageJson.contributes?.commands?.some(
        (command) => command.command === "trust-lsp.trustTwin.openPanel",
      ),
    );
    assert.ok(packageJson.scripts?.compile?.includes("build:trust-twin"));
    assert.ok(packageJson.scripts?.["package:trust-twin-smoke"]);

    const vscodeIgnore = fs.readFileSync(path.join(extensionRoot, ".vscodeignore"), "utf8");
    assert.match(vscodeIgnore, /!media\/trust-twin\/\*\*/);
  });

  test("navigates HMI pages and surfaces trend and alarm overlays through runtime contracts", async () => {
    const speedId = "resource/RESOURCE/program/Main/field/speed";
    const requests: string[] = [];
    __testSetTrustTwinControlRequestHandler(async (_endpoint, _auth, requestType, params) => {
      requests.push(requestType);
      if (requestType === "hmi.schema.get") {
        return {
          version: 1,
          mode: "read_write",
          read_only: false,
          resource: "RESOURCE",
          generated_at_ms: Date.now(),
          pages: [
            {
              id: "cell",
              title: "Drive Cell",
              order: 0,
              kind: "scene3d",
              view: "views/drive-cell.view.toml",
              scene_view: {
                node: [
                  {
                    id: "motor-1",
                    primitive: "box",
                    label: "Motor",
                    transform: { position: [0, 0, 0] },
                  },
                ],
                bind3d: [
                  {
                    node: "motor-1",
                    property: "transform.rotation.y",
                    source: "Main.speed",
                    scale: { min: 0, max: 100, output_min: 0, output_max: 6.28 },
                  },
                ],
              },
            },
            { id: "trends", title: "Trends", order: 1, kind: "trend", signals: [speedId] },
            { id: "alarms", title: "Alarms", order: 2, kind: "alarm" },
          ],
          widgets: [
            {
              id: speedId,
              path: "Main.speed",
              label: "Speed",
              data_type: "REAL",
              access: "read",
              writable: false,
              widget: "gauge",
              source: "program:Main",
              page: "cell",
              group: "Drive",
              order: 0,
              unit: "rpm",
              min: 0,
              max: 100,
            },
          ],
        };
      }
      if (requestType === "hmi.values.get") {
        assert.deepStrictEqual(params, { ids: [speedId] });
        return {
          connected: true,
          timestamp_ms: 1_700,
          freshness_ms: 0,
          values: {
            [speedId]: { v: 120, q: "good", ts_ms: 1_700 },
          },
        };
      }
      if (requestType === "hmi.trends.get") {
        assert.deepStrictEqual(params, { ids: [speedId], duration_ms: 600000, buckets: 120 });
        return {
          connected: true,
          timestamp_ms: 1_700,
          duration_ms: 600000,
          buckets: 120,
          series: [
            {
              id: speedId,
              label: "Speed",
              unit: "rpm",
              points: [{ ts_ms: 1_700, value: 120, min: 120, max: 120, samples: 1 }],
            },
          ],
        };
      }
      if (requestType === "hmi.alarms.get") {
        return {
          connected: true,
          timestamp_ms: 1_700,
          active: [
            {
              id: "alarm-speed-high",
              widget_id: speedId,
              path: "Main.speed",
              label: "Speed",
              state: "raised",
              acknowledged: false,
              raised_at_ms: 1_700,
              last_change_ms: 1_700,
              value: 120,
              min: 0,
              max: 100,
            },
          ],
          history: [],
        };
      }
      throw new Error(`Unexpected request type: ${requestType}`);
    });

    await vscode.commands.executeCommand("trust-lsp.trustTwin.openPanel", { pageId: "cell" });
    await __testForceTrustTwinRefresh();
    await __testForceTrustTwinPollValues();
    await __testForceTrustTwinSelectPage("trends");
    let state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.activePage?.id, "trends");
    assert.deepStrictEqual(state.breadcrumbs, ["Drive Cell", "Trends"]);
    assert.strictEqual(state.trends?.series[0]?.id, speedId);

    await __testForceTrustTwinSelectPage("alarms");
    state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.activePage?.id, "alarms");
    assert.strictEqual(state.alarms?.active[0]?.id, "alarm-speed-high");
    assert.ok(requests.includes("hmi.trends.get"));
    assert.ok(requests.includes("hmi.alarms.get"));
  });

  test("keeps last robot scene values while disconnected so stale robot freezes", async () => {
    const shoulderId = "resource/RESOURCE/program/Main/field/RobotShoulderAngle";
    let valuesRequest = 0;
    __testSetTrustTwinControlRequestHandler(async (_endpoint, _auth, requestType, params) => {
      if (requestType === "hmi.schema.get") {
        return {
          version: 1,
          mode: "read_write",
          read_only: false,
          resource: "RESOURCE",
          generated_at_ms: Date.now(),
          pages: [
            {
              id: "robot-cell",
              title: "Robot Cell",
              order: 0,
              kind: "scene3d",
              view: "views/robot-cell.view.toml",
              scene_view: {
                node: [
                  {
                    id: "robot.shoulder",
                    primitive: "box",
                    label: "ROBOT-1 shoulder link",
                    transform: { position: [0, 1, 0], rotation: [0, 0, 0] },
                  },
                ],
                bind3d: [
                  {
                    node: "robot.shoulder",
                    property: "transform.rotation.z",
                    source: "Main.RobotShoulderAngle",
                  },
                ],
              },
            },
          ],
          widgets: [
            {
              id: shoulderId,
              path: "Main.RobotShoulderAngle",
              label: "Shoulder angle",
              data_type: "REAL",
              access: "read",
              writable: false,
              widget: "value",
              source: "program:Main",
              page: "robot-cell",
              group: "Robot",
              order: 0,
            },
          ],
        };
      }
      if (requestType === "hmi.values.get") {
        assert.deepStrictEqual(params, { ids: [shoulderId] });
        valuesRequest += 1;
        if (valuesRequest === 1) {
          return {
            connected: true,
            timestamp_ms: 2_000,
            freshness_ms: 0,
            values: {
              [shoulderId]: { v: 0.72, q: "good", ts_ms: 2_000 },
            },
          };
        }
        throw new Error("runtime offline after robot sample");
      }
      if (requestType === "hmi.trends.get") {
        return { connected: true, timestamp_ms: 2_000, duration_ms: 600000, buckets: 120, series: [] };
      }
      if (requestType === "hmi.alarms.get") {
        return { connected: true, timestamp_ms: 2_000, active: [], history: [] };
      }
      throw new Error(`Unexpected request type: ${requestType}`);
    });

    await vscode.commands.executeCommand("trust-lsp.trustTwin.openPanel", { pageId: "robot-cell" });
    await __testForceTrustTwinRefresh();
    await __testForceTrustTwinPollValues();
    let state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.connected, true);
    assert.strictEqual(state.valuesBySource["Main.RobotShoulderAngle"], 0.72);

    await __testForceTrustTwinPollValues();
    state = __testGetTrustTwinPanelState();
    assert.strictEqual(state.connected, false);
    assert.strictEqual(
      state.valuesBySource["Main.RobotShoulderAngle"],
      0.72,
      "stale robot scene should keep the last runtime-driven pose instead of inventing motion",
    );
    assert.match(state.status, /disconnected|offline/i);
  });
});
