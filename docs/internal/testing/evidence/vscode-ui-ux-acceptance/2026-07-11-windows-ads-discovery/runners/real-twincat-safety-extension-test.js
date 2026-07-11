const assert = require("assert");
const crypto = require("crypto");
const fs = require("fs");
const net = require("net");
const path = require("path");

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function unixControlRequest(socketPath, type, id) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const socket = net.createConnection({ path: socketPath });
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error(`Timed out waiting for ${type}.`));
    }, 2000);
    socket.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    socket.on("data", (chunk) => {
      buffer += chunk.toString();
      const newline = buffer.indexOf("\n");
      if (newline < 0) return;
      clearTimeout(timer);
      socket.destroy();
      const response = JSON.parse(buffer.slice(0, newline));
      if (!response.ok) {
        reject(new Error(String(response.error || `${type} failed`)));
        return;
      }
      resolve(response.result);
    });
    socket.once("connect", () => {
      socket.write(`${JSON.stringify({ id, type })}\n`);
    });
  });
}

function startStatusOnlyProxy(socketPath, requests) {
  const server = net.createServer((client) => {
    let buffer = "";
    client.on("data", (chunk) => {
      buffer += chunk.toString();
      const newline = buffer.indexOf("\n");
      if (newline < 0) return;
      const line = buffer.slice(0, newline);
      const request = JSON.parse(line);
      requests.push(request.type);
      if (request.type !== "ads.status") {
        client.end(
          `${JSON.stringify({
            id: request.id,
            ok: false,
            error: "The safety proxy blocks every non-status request.",
          })}\n`
        );
        return;
      }
      const upstream = net.createConnection({ path: socketPath });
      let upstreamBuffer = "";
      upstream.once("error", (error) => client.destroy(error));
      upstream.on("data", (upstreamChunk) => {
        upstreamBuffer += upstreamChunk.toString();
        const upstreamNewline = upstreamBuffer.indexOf("\n");
        if (upstreamNewline < 0) return;
        client.end(`${upstreamBuffer.slice(0, upstreamNewline)}\n`);
        upstream.destroy();
      });
      upstream.once("connect", () => upstream.write(`${line}\n`));
    });
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

function listeningPort(server) {
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Safety proxy did not expose a TCP port.");
  }
  return address.port;
}

function closeServer(server) {
  return new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

function adsConnection(report) {
  assert.strictEqual(report.overall, "healthy");
  assert.ok(Array.isArray(report.connections));
  assert.strictEqual(report.connections.length, 1);
  const connection = report.connections[0];
  assert.strictEqual(connection.state, "connected");
  assert.strictEqual(connection.degraded_points, 0);
  return connection;
}

async function run() {
  const repo = path.resolve(process.env.TRUST_REPO);
  const socketPath = path.resolve(process.env.TRUST_REAL_ADS_CONTROL_SOCKET);
  const evidencePath = path.resolve(process.env.TRUST_REAL_ADS_SAFETY_EVIDENCE);
  const controllerSource = path.join(
    repo,
    "editors/vscode/src/networkCanvas/adsServiceProbeController.ts"
  );
  const controllerOut = path.join(
    repo,
    "editors/vscode/out/networkCanvas/adsServiceProbeController.js"
  );
  const runtimeControlClientSource = path.join(
    repo,
    "editors/vscode/src/runtimeControlClient.ts"
  );
  const runtimeControlClientOut = path.join(
    repo,
    "editors/vscode/out/runtimeControlClient.js"
  );
  const candidateRuntime = path.resolve(process.env.TRUST_REAL_RUNTIME);
  const {
    ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE,
    AdsServiceProbeController,
  } = require(controllerOut);

  const requests = [];
  const posts = [];
  const samples = [];
  let proxy;
  try {
    for (let attempt = 0; attempt < 50; attempt += 1) {
      const report = await unixControlRequest(socketPath, "ads.status", 8000 + attempt);
      if (report.overall === "healthy" && report.connections?.[0]?.state === "connected") {
        break;
      }
      await sleep(100);
    }
    adsConnection(await unixControlRequest(socketPath, "ads.status", 8100));

    proxy = await startStatusOnlyProxy(socketPath, requests);
    const endpoint = `tcp://127.0.0.1:${listeningPort(proxy)}`;
    const panel = {
      visible: true,
      webview: {
        postMessage: async (message) => {
          posts.push(message);
          return true;
        },
      },
    };
    const runtime = {
      mode: "online",
      endpoint,
      endpointEnabled: true,
      reachable: true,
      status: "online_reachable",
      label: "Real TwinCAT reader",
      credentialChannel: "trusted_same_host",
    };
    const controller = new AdsServiceProbeController({
      panel: () => panel,
      extensionContext: () => ({}),
      runtimeTargetForOrigin: () => runtime,
      requestIsCurrent: () => true,
    });
    const request = {
      sessionId: "real-twincat-safety",
      requestId: 1,
      origin: "real-twincat-reader",
      candidate: {
        id: "ads:100.67.6.217.1.1",
        label: "TWINCAT-LAPTOP · 100.67.6.217.1.1",
        protocol: "ads",
        source: "ads_identify",
        confidence: "observed",
        originRuntimeId: "real-twincat-reader",
        params: {
          host: "192.168.77.11",
          ams_net_id: "100.67.6.217.1.1",
        },
      },
      ports: [851, 852, 853, 854, 301, 501],
    };

    let probeCompleted = false;
    for (let index = 0; index < 180; index += 1) {
      if (index === 10) {
        await controller.probe(request);
        probeCompleted = true;
      }
      const report = await unixControlRequest(socketPath, "ads.status", 9000 + index);
      const connection = adsConnection(report);
      samples.push({
        index,
        overall: report.overall,
        state: connection.state,
        degraded_points: connection.degraded_points,
        last_good_value_ms: connection.last_good_value_ms,
      });
      await sleep(100);
    }
    controller.cancel();

    assert.strictEqual(probeCompleted, true);
    assert.deepStrictEqual(requests, ["ads.status"]);
    assert.strictEqual(posts.length, 1);
    assert.strictEqual(posts[0].type, "adsServiceProbeResults");
    assert.deepStrictEqual(posts[0].results, []);
    assert.strictEqual(posts[0].error, ACTIVE_ADS_CONNECTION_PROBE_SAFETY_MESSAGE);
    assert.ok(
      samples[samples.length - 1].last_good_value_ms - samples[0].last_good_value_ms > 15000,
      "The real ADS reader must keep receiving fresh values throughout the controller preflight."
    );

    const sampleDigest = crypto
      .createHash("sha256")
      .update(JSON.stringify(samples))
      .digest("hex");
    const evidence = {
      schema_version: 1,
      captured_at: new Date().toISOString(),
      status: "passed",
      purpose:
        "Device-in-loop proof that the exact compiled VS Code ADS service-probe controller fails closed while a selected truST runtime owns a healthy real TwinCAT ADS connection.",
      controller: {
        source: path.relative(repo, controllerSource),
        source_sha256: sha256(controllerSource),
        compiled: path.relative(repo, controllerOut),
        compiled_sha256: sha256(controllerOut),
        runtime_control_client_source: path.relative(
          repo,
          runtimeControlClientSource
        ),
        runtime_control_client_source_sha256: sha256(
          runtimeControlClientSource
        ),
        runtime_control_client_compiled: path.relative(
          repo,
          runtimeControlClientOut
        ),
        runtime_control_client_compiled_sha256: sha256(
          runtimeControlClientOut
        ),
        executed_in_real_vscode_extension_host: true,
      },
      candidate_runtime: {
        path: path.relative(repo, candidateRuntime),
        sha256: sha256(candidateRuntime),
      },
      twin_cat_target: {
        host: "192.168.77.11",
        ams_net_id: "100.67.6.217.1.1",
        computer_name: "TWINCAT-LAPTOP",
        ads_port: 851,
      },
      selected_runtime: {
        control_transport: "status-only loopback TCP proxy to the live Unix control endpoint",
        ads_connection_name: "real_twin_cat_safety_reader",
        polled_symbol: "GVL_truST_Test.TankLevel",
      },
      controller_requests: requests,
      comm_browse_symbols_request_count: requests.filter(
        (type) => type === "comm.browse_symbols"
      ).length,
      controller_result: {
        post_count: posts.length,
        result_count: posts[0].results.length,
        error: posts[0].error,
      },
      continuity: {
        interval_ms: 100,
        sample_count: samples.length,
        all_overall_healthy: samples.every((sample) => sample.overall === "healthy"),
        all_states_connected: samples.every((sample) => sample.state === "connected"),
        all_degraded_points_zero: samples.every(
          (sample) => sample.degraded_points === 0
        ),
        first_last_good_value_ms: samples[0].last_good_value_ms,
        final_last_good_value_ms: samples[samples.length - 1].last_good_value_ms,
        last_good_value_advance_ms:
          samples[samples.length - 1].last_good_value_ms -
          samples[0].last_good_value_ms,
        nonhealthy_samples: samples.filter(
          (sample) =>
            sample.overall !== "healthy" ||
            sample.state !== "connected" ||
            sample.degraded_points !== 0
        ),
        samples_sha256: sampleDigest,
      },
    };
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
    process.stdout.write(
      `REAL_TWINCAT_SAFETY_PASSED ${JSON.stringify(evidence.continuity)}\n`
    );
  } finally {
    if (proxy) await closeServer(proxy);
  }
}

module.exports = { run };
