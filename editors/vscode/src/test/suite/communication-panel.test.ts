import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  buildCommunicationPanelModel,
  COMMUNICATION_COMMAND,
  renderCommunicationPanelHtml,
  resolveCommunicationDocsUri,
} from "../../communication/communicationPanel";
import { COMMUNICATION_PROTOCOLS } from "../../communication/communicationProtocols";
import type { CommCapabilitiesResponse } from "../../communication/capability";
import {
  renderSchemaForm,
  shouldBlockSecretApply,
  validateSchemaValues,
  type CommConfiguredInstance,
  type CommProtocolSchema,
} from "../../communication/schemaForm";
import type { AdsStatusReport } from "../../adsStatusSummary";
import { statusLabel, type CommunicationStatusId } from "../../communication/capability";
import type { RuntimeTarget, RuntimeTargetStatus } from "../../runtimeTarget";

suite("Communication panel", function () {
  test("renders all protocol cards with no runtime chooser", () => {
    const model = buildCommunicationPanelModel(
      onlineRuntime(),
      capabilities(),
      undefined,
      "addDevice"
    );
    const html = renderCommunicationPanelHtml(model, "test-nonce");

    assert.strictEqual(model.cards.length, COMMUNICATION_PROTOCOLS.length);
    assert.ok(html.includes("Which communication do I need?"));
    assert.ok(html.includes("External systems"));
    assert.ok(html.includes("Runtime-to-runtime"));
    assert.ok(html.includes("Devices and fieldbus"));
    assert.ok(html.includes("Telemetry and evidence"));
    assert.ok(html.includes("ADS / TwinCAT"));
    assert.ok(html.includes("Connect to TwinCAT"));
    assert.ok(html.includes("Import symbols"));
    assert.ok(html.includes("Expose to TwinCAT"));
    assert.ok(html.includes("You need"));
    assert.ok(!html.includes("Next: Status"));
    assert.ok(!html.includes("State:"));
    assert.ok(html.includes("ADS logical port 851"));
    assert.ok(!html.includes("48898"));
    assert.ok(html.includes("Configured policy"));
    assert.ok(!html.includes("<select"));
    assert.ok(!html.includes("runtimeSelect"));
  });

  test("ADS card reuses shared ADS status summary", () => {
    const model = buildCommunicationPanelModel(
      onlineRuntime(),
      capabilities(),
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      adsStatusReport()
    );
    const html = renderCommunicationPanelHtml(model, "test-nonce");

    assert.ok(html.includes("ADS: 2 devices · 1 degraded"));
    assert.ok(html.includes('data-ads-action="importSymbols"'));
  });

  test("renders intent rows before protocol cards for first-viewport scanning", () => {
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(onlineRuntime(), capabilities()),
      "test-nonce"
    );

    const chooserIndex = html.indexOf("Which communication do I need?");
    const firstGroupIndex = html.indexOf("External systems");
    assert.ok(chooserIndex >= 0, "missing visible intent chooser");
    assert.ok(firstGroupIndex >= 0, "missing protocol group");
    assert.ok(chooserIndex < firstGroupIndex, "intent chooser must render before cards");
    assert.ok(html.includes("Another truST runtime"));
    assert.ok(html.includes("External software or plant system"));
    assert.ok(html.includes("Local hardware or fieldbus"));
    assert.ok(html.includes('data-group-jump="runtime"'));
    assert.ok(html.includes('data-group-jump="external"'));
    assert.ok(html.includes('data-group-jump="fieldbus"'));
    assert.ok(html.includes("Show runtime options"));
  });

  test("renders all stable status ids without meaningless connected next step", () => {
    const statuses: CommunicationStatusId[] = [
      "not_in_build",
      "not_configured",
      "simulate",
      "runtime_unreachable",
      "connected",
      "degraded",
      "error",
      "configured_policy",
    ];

    for (const status of statuses) {
      const runtime =
        status === "simulate"
          ? simulatedRuntime()
          : status === "runtime_unreachable"
            ? runtimeWithStatus("online_unreachable")
            : onlineRuntime();
      const model = buildCommunicationPanelModel(
        runtime,
        status === "simulate" || status === "runtime_unreachable"
          ? capabilities()
          : capabilitiesForStatus(status)
      );
      const html = renderCommunicationPanelHtml(model, "test-nonce");

      assert.ok(
        html.includes(statusLabel(status)),
        `missing status label ${status}`
      );
      assert.ok(!html.includes("Next:"), `legacy next prefix rendered for ${status}`);
      if (status === "connected") {
        assert.ok(
          !html.includes("Action connected"),
          "connected cards with action=none must not render a fake next step"
        );
      } else {
        assert.ok(
          html.includes(`Action ${status}`) || html.includes("Open Runtime pane"),
          `missing concrete next step for ${status}`
        );
      }
    }
  });

  test("falls back honestly when runtime capabilities are missing", () => {
    const model = buildCommunicationPanelModel(
      onlineRuntime(),
      undefined,
      "unsupported request"
    );
    const html = renderCommunicationPanelHtml(model, "test-nonce");

    assert.ok(html.includes("Runtime capabilities are unavailable"));
    assert.ok(!html.includes(">Connected<"));
  });

  test("simulate mode blocks cards through runtime context", () => {
    const model = buildCommunicationPanelModel(simulatedRuntime(), capabilities());
    const html = renderCommunicationPanelHtml(model, "test-nonce");

    assert.ok(html.includes("Simulate mode"));
    assert.ok(html.includes("Open Runtime pane"));
    assert.ok(!html.includes(">Connected<"));
    assert.ok(!html.includes("<select"));
  });

  test("only offers the Runtime pane for runtime context changes", () => {
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(runtimeWithStatus("online_unreachable"), capabilities()),
      "test-nonce"
    );

    assert.ok(html.includes('data-action="openRuntimePane"'));
    assert.ok(html.includes("Open Runtime pane"));
    for (const forbidden of [
      "runtimeSelect",
      "selectRuntime",
      "changeRuntime",
      "Switch runtime",
    ]) {
      assert.ok(!html.includes(forbidden), `unexpected runtime chooser: ${forbidden}`);
    }
  });

  test("uses nonce-based content security policy", () => {
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(onlineRuntime(), capabilities()),
      "fixed-nonce"
    );

    assert.ok(html.includes("style-src 'nonce-fixed-nonce'"));
    assert.ok(html.includes("script-src 'nonce-fixed-nonce'"));
    assert.ok(html.includes('<style nonce="fixed-nonce">'));
    assert.ok(html.includes('<script nonce="fixed-nonce">'));
    assert.ok(!html.includes("'unsafe-inline'"));
  });

  test("renders runtime schema forms without secret defaults", () => {
    const schema = mqttSchema();
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(
        onlineRuntime(),
        capabilities(),
        undefined,
        undefined,
        {
          schema_version: 1,
          family: "io",
          protocols: [schema],
        },
        "mqtt"
      ),
      "test-nonce"
    );

    assert.ok(html.includes("MQTT setup"));
    assert.ok(html.includes('class="active-setup panel"'));
    assert.ok(html.includes('data-active-setup="mqtt"'));
    assert.ok(html.includes('data-action="commApply"'));
    assert.ok(html.includes('data-field-id="broker"'));
    assert.ok(html.includes('data-params="{&quot;broker&quot;:&quot;127.0.0.1:1883&quot;}"'));
    assert.ok(html.includes('data-field-default="&quot;127.0.0.1:1883&quot;"'));
    assert.ok(html.includes("Remove selected"));
    assert.ok(html.includes('data-apply-action="test"'));
    assert.ok(html.includes('type="password"'));
    assert.ok(!html.includes("hunter2"));
    const gridEnd = html.indexOf("</div>\n    <div class=\"active-setup panel\"");
    assert.ok(gridEnd > 0, "active setup form should render after the card grid, not inside a narrow card cell");
  });

  test("docs actions resolve to local public docs or repository fallback", async () => {
    const repoRoot = vscode.Uri.file(path.resolve(__dirname, "../../../../.."));
    for (const protocol of COMMUNICATION_PROTOCOLS) {
      const localUri = await resolveCommunicationDocsUri(protocol.docsPath, [
        repoRoot,
      ]);
      assert.ok(localUri, `${protocol.id} docs did not resolve`);
      assert.strictEqual(
        localUri.scheme,
        "file",
        `${protocol.id} should resolve to a local public docs file in the repo`
      );
      assert.ok(
        fs.existsSync(localUri.fsPath),
        `${protocol.id} docs file missing: ${localUri.fsPath}`
      );

      const fallbackUri = await resolveCommunicationDocsUri(protocol.docsPath, []);
      assert.ok(fallbackUri, `${protocol.id} docs fallback did not resolve`);
      assert.strictEqual(fallbackUri.scheme, "https");
      assert.ok(
        fallbackUri
          .toString()
          .startsWith(
            "https://github.com/johannesPettersson80/trust-platform/blob/main/docs/public/"
          ),
        `${protocol.id} fallback should point at public repository docs`
      );
    }
  });

  test("renders schema-backed snippet fallback inside VS Code", () => {
    const schema = opcuaSnippetSchema();
    const html = renderSchemaForm(schema, {
      schema_version: 1,
      protocol: "opcua",
      driver: "",
      action: "validate",
      applied: false,
      lifecycle_effect: "deploy_required",
      message: "Configuration validated.",
      config_path: "runtime.toml",
      snippet: "[runtime.opcua]\\nenabled = true\\n",
    });

    assert.ok(html.includes("OPC UA setup"));
    assert.ok(html.includes("Generate snippet"));
    assert.ok(html.includes("runtime.toml"));
    assert.ok(html.includes("[runtime.opcua]"));
  });

  test("renders restart-required apply result as pending, not green connected", () => {
    const html = renderSchemaForm(mqttSchema(), {
      schema_version: 1,
      protocol: "mqtt",
      driver: "mqtt",
      action: "add",
      applied: true,
      lifecycle_effect: "restart_required",
      message: "MQTT saved. Restart the runtime to apply it.",
      config_path: "io.toml",
    });

    assert.ok(html.includes("MQTT saved. Restart the runtime to apply it."));
    assert.ok(html.includes('class="apply-result pending"'));
    assert.ok(!html.includes('class="apply-result connected"'));
  });

  test("uses stronger status badges and problem-card accents", () => {
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(
        onlineRuntime(),
        capabilitiesWithOverrides({
          mqtt: {
            configured: true,
            operational: false,
            health: "error",
            detail: "MQTT broker rejected credentials.",
          },
        })
      ),
      "test-nonce"
    );

    assert.ok(html.includes('data-status="error"'));
    assert.ok(html.includes(".pill.connected"));
    assert.ok(html.includes('.card[data-status="error"]'));
    assert.ok(html.includes('.card[data-status="degraded"]'));
  });

  test("renders non-I/O protocol cards with setup actions, not docs-only placeholders", () => {
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(onlineRuntime(), capabilities()),
      "test-nonce"
    );

    for (const protocol of [
      "opcua",
      "openot",
      "discovery",
      "mesh",
      "realtime_t0",
      "runtime_cloud",
    ]) {
      assert.ok(
        html.includes(`data-action="setupProtocol" data-protocol="${protocol}"`),
        `${protocol} should expose a setup action`
      );
    }
  });

  test("schema snippet fallback uses the same fields and defaults as the setup form", () => {
    const schema = opcuaSnippetSchema();
    const html = renderSchemaForm(schema);

    assert.ok(html.includes('data-field-id="listen"'));
    assert.ok(html.includes('data-field-id="expose"'));
    assert.ok(html.includes('data-field-default="&quot;0.0.0.0:4840&quot;"'));
    assert.ok(html.includes("[\n  &quot;global.*&quot;\n]"));
  });

  test("validates schema values locally before runtime apply", () => {
    const errors = validateSchemaValues(mqttSchema(), {
      broker: "missing-port",
      username: "user",
      password: "",
    });

    assert.ok(errors.some((error) => error.field === "broker"));
    assert.strictEqual(
      validateSchemaValues(mqttSchema(), {
        broker: "127.0.0.1:1883",
        username: "",
        password: "",
      }).length,
      0
    );
    assert.ok(
      validateSchemaValues(opcuaSnippetSchema(), {
        listen: "127.0.0.1:4840",
        expose: "global.*",
      }).some((error) => error.field === "expose")
    );
    assert.ok(
      validateSchemaValues(modbusSchema(), {
        address: "plc.local:502",
      }).some((error) => error.field === "address")
    );
    assert.strictEqual(
      validateSchemaValues(mqttSchema(), {
        broker: "broker.local:1883",
        username: "",
        password: "",
      }).length,
      0
    );
  });

  test("covers Modbus and MQTT invalid and valid setup validation", () => {
    assert.ok(renderSchemaForm(modbusSchema()).includes('data-apply-action="test"'));
    assert.ok(renderSchemaForm(mqttSchema()).includes('data-apply-action="test"'));
    assert.ok(
      validateSchemaValues(modbusSchema(), {
        address: "plc.local:502",
        unit_id: 1,
      }).some((error) => error.field === "address")
    );
    assert.strictEqual(
      validateSchemaValues(modbusSchema(), {
        address: "127.0.0.1:502",
        unit_id: 1,
      }).length,
      0
    );
    assert.ok(
      validateSchemaValues(mqttSchema(), {
        broker: "bad broker:1883",
        username: "",
        password: "",
      }).some((error) => error.field === "broker")
    );
    assert.strictEqual(
      validateSchemaValues(mqttSchema(), {
        broker: "broker.local:1883",
        username: "",
        password: "",
      }).length,
      0
    );
  });

  test("renders platform warnings and simulated/loopback setup forms", () => {
    const platformHtml = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(
        onlineRuntime(),
        capabilitiesWithOverrides({
          ethercat: {
            built: false,
            configured: false,
            operational: false,
            health: "not_in_build",
            detail: "EtherCAT support requires an ethercat-wire build and a real NIC.",
          },
          gpio: {
            built: false,
            configured: false,
            operational: false,
            health: "not_in_build",
            detail: "GPIO setup is Linux only.",
          },
        })
      ),
      "test-nonce"
    );
    assert.ok(platformHtml.includes("EtherCAT support requires"));
    assert.ok(platformHtml.includes("GPIO setup is Linux only"));

    const simulatedHtml = renderSchemaForm(simulatedSchema());
    assert.ok(simulatedHtml.includes("Simulated I/O setup"));
    assert.ok(simulatedHtml.includes('data-field-id="input_count"'));
    assert.ok(simulatedHtml.includes('data-field-id="scan_period_ms"'));

    const loopbackHtml = renderSchemaForm(loopbackSchema());
    assert.ok(loopbackHtml.includes("Loopback I/O setup"));
    assert.ok(loopbackHtml.includes('data-field-id="mode"'));
    assert.strictEqual(
      validateSchemaValues(simulatedSchema(), {
        input_count: 8,
        output_count: 8,
        scan_period_ms: 10,
        mode: "counter",
      }).length,
      0
    );
    assert.strictEqual(
      validateSchemaValues(loopbackSchema(), {
        input_count: 8,
        output_count: 8,
        scan_period_ms: 10,
        mode: "mirror",
      }).length,
      0
    );
  });

  test("renders multi-instance edit and remove controls without losing params", () => {
    const schema = mqttSchema([
      {
        id: "mqtt:0",
        driver: "mqtt",
        display_name: "mqtt broker-a.local:1883",
        params: { broker: "broker-a.local:1883", topic_in: "a/in" },
      },
      {
        id: "mqtt:1",
        driver: "mqtt",
        display_name: "mqtt broker-b.local:1883",
        params: { broker: "broker-b.local:1883", topic_in: "b/in" },
      },
    ]);
    const html = renderSchemaForm(schema);

    assert.ok(html.includes('name="instance_id"'));
    assert.ok(html.includes('value="mqtt:0"'));
    assert.ok(html.includes('value="mqtt:1"'));
    assert.ok(html.includes("broker-a.local:1883"));
    assert.ok(html.includes("broker-b.local:1883"));
    assert.ok(html.includes("Add new MQTT"));
    assert.ok(html.includes('data-apply-action="add"'));
    assert.ok(html.includes("Update selected"));
    assert.ok(html.includes('data-apply-action="edit"'));
    assert.ok(html.includes("Remove selected"));
    assert.ok(html.includes('data-apply-action="remove"'));
    assert.ok(html.includes("Disable selected"));
    assert.ok(html.includes('data-apply-action="disable"'));
    assert.ok(html.includes("topic_in"));
  });

  test("renders refresh affordance for status cadence", () => {
    const html = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(onlineRuntime(), capabilities()),
      "test-nonce"
    );

    assert.ok(html.includes('data-action="refresh"'));
    assert.ok(html.includes("Refresh"));
  });

  test("renders refreshed degraded status from a new capability payload", () => {
    const firstHtml = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(
        onlineRuntime(),
        capabilitiesWithOverrides({
          mqtt: {
            configured: false,
            operational: false,
            health: "not_configured",
            detail: "MQTT is not configured.",
          },
        })
      ),
      "test-nonce"
    );
    const refreshedHtml = renderCommunicationPanelHtml(
      buildCommunicationPanelModel(
        onlineRuntime(),
        capabilitiesWithOverrides({
          mqtt: {
            configured: true,
            operational: false,
            health: "degraded",
            detail: "MQTT broker rejected the last connection attempt.",
            next_action: { kind: "test_connection", label: "Test connection" },
          },
        })
      ),
      "test-nonce"
    );

    assert.ok(firstHtml.includes("MQTT is not configured."));
    assert.ok(!firstHtml.includes("MQTT broker rejected"));
    assert.ok(refreshedHtml.includes("MQTT broker rejected the last connection attempt."));
    assert.ok(refreshedHtml.includes("Degraded"));
  });

  test("blocks secret apply over untrusted runtime channel", () => {
    const schema = mqttSchema();

    assert.strictEqual(
      shouldBlockSecretApply(
        schema,
        { broker: "127.0.0.1:1883", username: "u", password: "hunter2" },
        "untrusted_remote_plain_tcp"
      ),
      true
    );
    assert.strictEqual(
      shouldBlockSecretApply(
        schema,
        { broker: "127.0.0.1:1883", username: "u", password: "hunter2" },
        "trusted_same_host"
      ),
      false
    );
    assert.strictEqual(
      shouldBlockSecretApply(
        schema,
        { broker: "127.0.0.1:1883", username: "u", password: "" },
        "untrusted_remote_plain_tcp"
      ),
      false
    );
  });

  test("package exposes one Communication command and hides ADS command clutter", () => {
    const packageJson = JSON.parse(
      fs.readFileSync(path.resolve(__dirname, "../../../package.json"), "utf8")
    ) as {
      activationEvents?: string[];
      contributes?: {
        commands?: Array<{ command?: string; title?: string; category?: string }>;
        menus?: { commandPalette?: Array<{ command?: string; when?: string }> };
      };
    };
    const command = packageJson.contributes?.commands?.find(
      (entry) => entry.command === COMMUNICATION_COMMAND
    );

    assert.ok(command, "missing Communication command contribution");
    assert.strictEqual(command.title, "Communication");
    assert.strictEqual(command.category, "Structured Text");
    assert.ok(
      (packageJson.activationEvents ?? []).includes(
        `onCommand:${COMMUNICATION_COMMAND}`
      )
    );

    const hidden = new Map(
      (packageJson.contributes?.menus?.commandPalette ?? []).map((entry) => [
        entry.command,
        entry.when,
      ])
    );
    for (const commandId of [
      "trust-lsp.ads.openPanel",
      "trust-lsp.ads.server.openPanel",
      "trust-lsp.ads.addDevice",
      "trust-lsp.ads.diagnose",
      "trust-lsp.ads.importSymbols",
      "trust-lsp.ads.addRoute",
    ]) {
      assert.strictEqual(hidden.get(commandId), "false", `${commandId} visible`);
    }
  });

  test("legacy ADS command ids execute through Communication entry point", async () => {
    const registered = await vscode.commands.getCommands(true);
    for (const commandId of [
      "trust-lsp.ads.openPanel",
      "trust-lsp.ads.server.openPanel",
      "trust-lsp.ads.addDevice",
      "trust-lsp.ads.diagnose",
      "trust-lsp.ads.importSymbols",
      "trust-lsp.ads.addRoute",
    ]) {
      assert.ok(registered.includes(commandId), `${commandId} not registered`);
      await assert.doesNotReject(
        async () => {
          await vscode.commands.executeCommand(commandId);
        },
        `${commandId} should deep-link through Communication`
      );
    }
  });
});

function onlineRuntime(): RuntimeTarget {
  return {
    mode: "online",
    endpoint: "tcp://127.0.0.1:9901",
    endpointEnabled: true,
    reachable: true,
    status: "online_reachable",
    label: "line-controller-1",
    credentialChannel: "trusted_same_host",
  };
}

function simulatedRuntime(): RuntimeTarget {
  return {
    mode: "simulate",
    endpointEnabled: true,
    reachable: false,
    status: "simulate",
    label: "Simulated runtime",
    credentialChannel: "unavailable",
  };
}

function runtimeWithStatus(status: RuntimeTargetStatus): RuntimeTarget {
  return {
    ...onlineRuntime(),
    reachable: status === "online_reachable" || status === "auth_failed",
    status,
  };
}

function capabilities(): CommCapabilitiesResponse {
  return capabilitiesWithOverrides({});
}

function capabilitiesWithOverrides(
  overrides: Record<string, Partial<CommCapabilitiesResponse["capabilities"][number]>>
): CommCapabilitiesResponse {
  const ids = [
    "ads",
    "ads_server",
    "opcua",
    "modbus_tcp",
    "mqtt",
    "openot",
    "discovery",
    "mesh",
    "realtime_t0",
    "runtime_cloud",
    "ethercat",
    "gpio",
    "simulated",
    "loopback",
  ];
  return {
    schema_version: 1,
    capabilities: ids.map((id) => ({
      id,
      built: true,
      configured: id === "ads" || id === "runtime_cloud",
      operational: id === "ads",
      health: id === "ads" ? "connected" : id === "runtime_cloud" ? "configured_policy" : "not_configured",
      detail:
        id === "runtime_cloud"
          ? "Runtime cloud/federation policy is configured; it is not an operational live link."
          : `${id} detail`,
      next_action: { kind: "setup", label: "Set up" },
      ...overrides[id],
    })),
  };
}

function capabilitiesForStatus(status: CommunicationStatusId): CommCapabilitiesResponse {
  return {
    schema_version: 1,
    capabilities: [
      {
        id: "mqtt",
        built: status !== "not_in_build",
        configured: !["not_in_build", "not_configured"].includes(status),
        operational: status === "connected",
        health: status,
        detail: `mqtt ${status}`,
        next_action: { kind: status === "connected" ? "none" : "setup", label: `Action ${status}` },
      },
    ],
  };
}

function adsStatusReport(): AdsStatusReport {
  return {
    overall: "degraded",
    summary: "ADS is degraded.",
    connections: [
      {
        name: "line1",
        state: "connected",
        point_count: 2,
        degraded_points: 0,
        summary: "ok",
      },
      {
        name: "line2",
        state: "connected",
        point_count: 2,
        degraded_points: 1,
        summary: "one stale point",
      },
    ],
  };
}

function mqttSchema(
  instances: CommConfiguredInstance[] = [
    {
      id: "mqtt:0",
      driver: "mqtt",
      display_name: "mqtt 127.0.0.1:1883",
      params: { broker: "127.0.0.1:1883" },
    },
  ]
): CommProtocolSchema {
  return {
    id: "mqtt",
    driver: "mqtt",
    title: "MQTT",
    purpose: "Publish and subscribe process I/O through a broker.",
    apply_mode: "native",
    lifecycle_effect: "restart_required",
    supports_test: true,
    supports_multi_instance: true,
    actions: ["add", "edit", "remove", "disable"],
    instances,
    fields: [
      {
        id: "broker",
        label: "Broker",
        type: "endpoint",
        required: true,
        advanced: false,
        secret: false,
        help: "MQTT broker host and port.",
        default: "127.0.0.1:1883",
      },
      {
        id: "username",
        label: "Username",
        type: "string",
        required: false,
        advanced: false,
        secret: false,
        help: "Broker username.",
        default: "",
      },
      {
        id: "password",
        label: "Password",
        type: "secret",
        required: false,
        advanced: false,
        secret: true,
        help: "Broker password.",
        default: null,
      },
    ],
  };
}

function simulatedSchema(): CommProtocolSchema {
  return {
    id: "simulated",
    driver: "simulated",
    title: "Simulated I/O",
    purpose: "Try process I/O without hardware.",
    apply_mode: "native",
    lifecycle_effect: "restart_required",
    supports_test: false,
    supports_multi_instance: true,
    actions: ["add", "edit", "remove"],
    fields: [
      {
        id: "input_count",
        label: "Input count",
        type: "number",
        required: true,
        advanced: false,
        secret: false,
        help: "Number of simulated input points.",
        default: 8,
        validation: { kind: "integer_range", min: 0, max: 4096 },
      },
      {
        id: "output_count",
        label: "Output count",
        type: "number",
        required: true,
        advanced: false,
        secret: false,
        help: "Number of simulated output points.",
        default: 8,
        validation: { kind: "integer_range", min: 0, max: 4096 },
      },
      {
        id: "scan_period_ms",
        label: "Scan period",
        type: "number",
        required: true,
        advanced: false,
        secret: false,
        help: "Simulated driver update period.",
        default: 10,
        validation: { kind: "integer_range", min: 1, max: 60000 },
      },
      {
        id: "mode",
        label: "Mode",
        type: "enum",
        required: true,
        advanced: false,
        secret: false,
        help: "How simulated values are produced.",
        default: "static",
        options: ["static", "counter", "random"],
      },
    ],
  };
}

function loopbackSchema(): CommProtocolSchema {
  return {
    id: "loopback",
    driver: "loopback",
    title: "Loopback I/O",
    purpose: "Echo outputs back into inputs for fast local sanity checks.",
    apply_mode: "native",
    lifecycle_effect: "restart_required",
    supports_test: false,
    supports_multi_instance: true,
    actions: ["add", "edit", "remove"],
    fields: [
      {
        id: "input_count",
        label: "Input count",
        type: "number",
        required: true,
        advanced: false,
        secret: false,
        help: "Number of loopback input points.",
        default: 8,
        validation: { kind: "integer_range", min: 0, max: 4096 },
      },
      {
        id: "output_count",
        label: "Output count",
        type: "number",
        required: true,
        advanced: false,
        secret: false,
        help: "Number of loopback output points.",
        default: 8,
        validation: { kind: "integer_range", min: 0, max: 4096 },
      },
      {
        id: "scan_period_ms",
        label: "Scan period",
        type: "number",
        required: true,
        advanced: false,
        secret: false,
        help: "Loopback driver update period.",
        default: 10,
        validation: { kind: "integer_range", min: 1, max: 60000 },
      },
      {
        id: "mode",
        label: "Mode",
        type: "enum",
        required: true,
        advanced: false,
        secret: false,
        help: "How outputs are reflected into inputs.",
        default: "mirror",
        options: ["mirror", "hold_last"],
      },
    ],
  };
}

function modbusSchema(): CommProtocolSchema {
  return {
    id: "modbus_tcp",
    driver: "modbus-tcp",
    title: "Modbus TCP",
    purpose: "Read and write register-oriented devices.",
    apply_mode: "native",
    lifecycle_effect: "restart_required",
    supports_test: true,
    supports_multi_instance: true,
    actions: ["add", "edit", "remove"],
    fields: [
      {
        id: "address",
        label: "Device address",
        type: "endpoint",
        required: true,
        advanced: false,
        secret: false,
        help: "IP address and TCP port.",
        default: "127.0.0.1:502",
        validation: { kind: "socket_addr" },
      },
    ],
  };
}

function opcuaSnippetSchema(): CommProtocolSchema {
  return {
    id: "opcua",
    driver: "",
    title: "OPC UA",
    purpose: "Let SCADA, HMI, or historian software read and write exposed PLC tags.",
    apply_mode: "snippet",
    lifecycle_effect: "deploy_required",
    supports_test: false,
    supports_multi_instance: false,
    actions: ["validate"],
    fields: [
      {
        id: "listen",
        label: "Listen address",
        type: "endpoint",
        required: true,
        advanced: false,
        secret: false,
        help: "Host and port.",
        default: "0.0.0.0:4840",
      },
      {
        id: "expose",
        label: "Expose globals",
        type: "json_array",
        required: false,
        advanced: false,
        secret: false,
        help: "Glob patterns.",
        default: ["global.*"],
      },
    ],
  };
}
