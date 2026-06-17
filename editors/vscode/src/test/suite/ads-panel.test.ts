import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  ADS_COMMANDS,
  buildAdsAddRouteCliArgs,
  buildAdsPanelModel,
  renderAdsPanelHtml,
  type AdsServerStatusSurface,
} from "../../adsPanel";
import type { AdsStatusReport } from "../../adsStatusSummary";
import type { RuntimeTarget } from "../../runtimeTarget";

suite("ADS panel", function () {
  test("simulate mode blocks production actions and only offers the runtime pane for runtime changes", () => {
    const model = buildAdsPanelModel(simulatedRuntime(), undefined, "addDevice");
    const html = renderAdsPanelHtml(model, "test-nonce");

    assert.strictEqual(model.productionActionsEnabled, false);
    assert.ok(model.productionBlockedReason.includes("simulate mode"));
    assert.ok(model.authoringOnlyAvailable);
    assert.ok(html.includes("Authoring only"));
    assert.ok(html.includes('data-action="authoringImport"'));
    assert.ok(html.includes('data-action="openRuntimePane"'));
    assert.ok(!html.includes("<select"));
    assert.ok(!html.includes("runtimeSelect"));
  });

  test("remote plain TCP runtime does not allow credential forwarding", () => {
    const model = buildAdsPanelModel(
      remoteRuntime(),
      adsStatus(),
      "addRoute"
    );
    const html = renderAdsPanelHtml(model, "test-nonce");

    assert.strictEqual(model.productionActionsEnabled, true);
    assert.strictEqual(model.credentialForwardingAllowed, false);
    assert.strictEqual(model.localCliRouteAddAvailable, true);
    assert.ok(model.credentialWarning.includes("must not forward"));
    assert.ok(model.credentialWarning.includes("local CLI"));
    assert.ok(html.includes("remote plain TCP control"));
    assert.ok(html.includes("Add route from this computer"));
    assert.ok(html.includes('data-action="addRouteLocalCli"'));
    assert.ok(!html.includes("routePassword"));
    assert.ok(!html.includes("TwinCAT Password"));
  });

  test("online reachable runtime renders ADS status and setup action", () => {
    const model = buildAdsPanelModel(localRuntime(), adsStatus(), "status");
    const html = renderAdsPanelHtml(model, "test-nonce");

    assert.strictEqual(model.productionActionsEnabled, true);
    assert.strictEqual(model.connectionSummary, "ADS: 2 devices · 1 degraded");
    assert.ok(html.includes("line1"));
    assert.ok(html.includes("line2"));
    assert.ok(html.includes('data-action="openSetup"'));
    assert.ok(html.includes("Production-ready evidence"));
    assert.ok(html.includes("Open Runtime pane"));
    assert.ok(!html.includes("<select"));
  });

  test("online reachable runtime renders ADS server status without a second runtime chooser", () => {
    const model = buildAdsPanelModel(
      localRuntime(),
      adsStatus(),
      "serverStatus",
      undefined,
      adsServerStatus()
    );
    const html = renderAdsPanelHtml(model, "test-nonce");

    assert.strictEqual(
      model.serverSummary,
      "ADS Server: 2 exposed · 1 clients · 1 pending"
    );
    assert.ok(html.includes("ADS Server"));
    assert.ok(html.includes("Self-test ready"));
    assert.ok(html.includes("Discoverable"));
    assert.ok(html.includes("External client verified"));
    assert.ok(html.includes("192.168.10.20.1.1"));
    assert.ok(html.includes("5.23.91.12.1.1"));
    assert.ok(html.includes("source_ip_not_allowed"));
    assert.ok(html.includes("[[runtime.ads_server.clients]]"));
    assert.ok(html.includes("source_ip = &quot;192.168.10.55&quot;"));
    assert.ok(html.includes('data-action="copyServerClient"'));
    assert.ok(html.includes('data-action="serverDoctor"'));
    assert.ok(html.includes('data-action="openSetup"'));
    assert.ok(!html.includes("<select"));
    assert.ok(!html.includes("runtimeSelect"));
  });

  test("unreachable online runtime blocks production actions without laptop fallback", () => {
    const model = buildAdsPanelModel(unreachableRuntime(), undefined, "diagnose");
    const html = renderAdsPanelHtml(model, "test-nonce");

    assert.strictEqual(model.productionActionsEnabled, false);
    assert.ok(model.productionBlockedReason.includes("not reachable"));
    assert.ok(html.includes('data-action="openRuntimePane"'));
    assert.ok(html.includes('data-action="refresh"'));
    assert.ok(html.includes('data-action="authoringImport"'));
    assert.ok(!html.includes("fallback"));
    assert.ok(!html.includes("<select"));
  });

  test("local route command uses password stdin and runtime-host identity values", () => {
    const args = buildAdsAddRouteCliArgs({
      routeName: "trust-line-controller-1",
      targetIp: "192.168.10.5",
      targetNetId: "5.23.91.12.1.1",
      amsPort: 851,
      localIp: "192.168.10.20",
      localNetId: "192.168.10.20.1.1",
      username: "Administrator",
    });

    assert.deepStrictEqual(args, [
      "ads",
      "add-route",
      "--route-name",
      "trust-line-controller-1",
      "--target",
      "192.168.10.5",
      "--target-net-id",
      "5.23.91.12.1.1",
      "--ams-port",
      "851",
      "--local-ip",
      "192.168.10.20",
      "--local-net-id",
      "192.168.10.20.1.1",
      "--username",
      "Administrator",
      "--password-stdin",
      "--json",
    ]);
    assert.ok(args.includes("--password-stdin"));
    assert.ok(!args.includes("not-persisted"));
  });

  test("remote route implementation fetches runtime identity before local CLI add-route", () => {
    const source = fs.readFileSync(
      path.resolve(__dirname, "../../adsPanel.js"),
      "utf8"
    );

    assert.ok(source.includes('"ads.identity"'));
    assert.ok(source.includes('"ads"'));
    assert.ok(source.includes('"add-route"'));
    assert.ok(source.includes('"--password-stdin"'));
    assert.ok(
      source.includes("TwinCAT credentials are sent directly from this computer")
    );
  });

  test("server implementation uses runtime control commands, not ADS framing in TypeScript", () => {
    const source = fs.readFileSync(
      path.resolve(__dirname, "../../adsPanel.js"),
      "utf8"
    );

    assert.ok(source.includes('"ads.server.status"'));
    assert.ok(source.includes('"ads.server.doctor.start"'));
    assert.ok(!source.includes("AmsTcpFrame"));
    assert.ok(!source.includes("ADSIGRP"));
    assert.ok(!source.includes("48898"));
  });

  test("authoring-only import uses dry-run preview before file writes", () => {
    const source = fs.readFileSync(
      path.resolve(__dirname, "../../adsPanel.js"),
      "utf8"
    );

    assert.ok(source.includes('"--dry-run"'));
    assert.ok(source.includes('"vscode.diff"'));
    assert.ok(source.includes("Review the opened diffs before applying."));
    assert.ok(source.includes("writeFile(targetPath"));
  });

  test("package declares ADS commands and activation events", () => {
    const packageJson = JSON.parse(
      fs.readFileSync(
        path.resolve(__dirname, "../../../package.json"),
        "utf8"
      )
    ) as {
      activationEvents?: string[];
      contributes?: { commands?: Array<{ command?: string }> };
    };
    const activationEvents = new Set(packageJson.activationEvents ?? []);
    const commands = new Set(
      (packageJson.contributes?.commands ?? []).map((entry) => entry.command)
    );

    for (const command of Object.values(ADS_COMMANDS)) {
      assert.ok(commands.has(command), `missing contributed command ${command}`);
      assert.ok(
        activationEvents.has(`onCommand:${command}`),
        `missing activation event for ${command}`
      );
    }
  });
});

function simulatedRuntime(): RuntimeTarget {
  return {
    mode: "simulate",
    endpoint: "tcp://192.168.10.20:9901",
    endpointEnabled: true,
    reachable: false,
    status: "simulate",
    label: "Simulated runtime",
    credentialChannel: "untrusted_remote_plain_tcp",
  };
}

function localRuntime(): RuntimeTarget {
  return {
    mode: "online",
    endpoint: "tcp://127.0.0.1:9901",
    endpointEnabled: true,
    reachable: true,
    status: "online_reachable",
    label: "line-controller-1",
    setupUrl: "http://127.0.0.1:8080/setup/ads",
    credentialChannel: "trusted_same_host",
  };
}

function remoteRuntime(): RuntimeTarget {
  return {
    mode: "online",
    endpoint: "tcp://192.168.10.20:9901",
    endpointEnabled: true,
    reachable: true,
    status: "online_reachable",
    label: "line-controller-1",
    setupUrl: "http://192.168.10.20:8080/setup/ads",
    credentialChannel: "untrusted_remote_plain_tcp",
  };
}

function unreachableRuntime(): RuntimeTarget {
  return {
    mode: "online",
    endpoint: "tcp://192.168.10.20:9901",
    endpointEnabled: true,
    reachable: false,
    status: "online_unreachable",
    label: "line-controller-1",
    credentialChannel: "untrusted_remote_plain_tcp",
  };
}

function adsStatus(): AdsStatusReport {
  return {
    overall: "degraded",
    summary: "2 ADS devices, 1 degraded.",
    connections: [
      {
        name: "line1",
        state: "connected",
        point_count: 4,
        degraded_points: 0,
        summary: "Connected.",
      },
      {
        name: "line2",
        state: "reconnecting",
        point_count: 3,
        degraded_points: 1,
        summary: "Reconnecting.",
      },
    ],
  };
}

function adsServerStatus(): AdsServerStatusSurface {
  return {
    schema_version: 2,
    role: "server",
    enabled: true,
    listen: "192.168.10.20",
    ams_net_id: "192.168.10.20.1.1",
    ads_port: 851,
    exposed_count: 2,
    writable_count: 1,
    allowed_client_count: 1,
    connected_clients: 0,
    proof_status: "self_test_available",
    discoverable: true,
    external_client_verified: true,
    configured_empty: false,
    identity: {
      host_name: "line-controller-1",
      chosen_ip: "192.168.10.20",
      ams_net_id: "192.168.10.20.1.1",
      classification: "lan",
    },
    status: {
      overall: "healthy",
      summary: "ADS server exposes 2 symbols.",
      connections: [
        {
          name: "ads-server",
          state: "connected",
          point_count: 2,
          degraded_points: 0,
          summary: "ADS server exposes 2 symbol(s).",
        },
      ],
    },
    pending_clients: [
      {
        ams_net_id: "5.23.91.12.1.1",
        source_ip: "192.168.10.55",
        reason: "source_ip_not_allowed",
        count: 2,
        suggested_client: {
          ams_net_id: "5.23.91.12.1.1",
          source_ip: "192.168.10.55",
        },
      },
    ],
    recently_refused_clients: [],
  };
}
