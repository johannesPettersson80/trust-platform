import * as assert from "assert";
import * as path from "path";
import * as vscode from "vscode";

import {
  STTrustTwinTopologyProposeTool,
} from "../../lm-tools";

function toolResultText(result: unknown): string {
  if (typeof result === "string") {
    return result;
  }
  if (!result || typeof result !== "object") {
    return String(result);
  }
  const objectResult = result as Record<string, unknown>;
  if (typeof objectResult.text === "string") {
    return objectResult.text;
  }
  const content = objectResult.content;
  if (Array.isArray(content)) {
    for (const entry of content) {
      if (
        entry &&
        typeof entry === "object" &&
        typeof (entry as { value?: unknown }).value === "string"
      ) {
        return (entry as { value: string }).value;
      }
    }
  }
  const parts = objectResult.parts;
  if (Array.isArray(parts)) {
    for (const part of parts) {
      if (
        part &&
        typeof part === "object" &&
        typeof (part as { value?: unknown }).value === "string"
      ) {
        return (part as { value: string }).value;
      }
      if (
        part &&
        typeof part === "object" &&
        typeof (part as { text?: unknown }).text === "string"
      ) {
        return (part as { text: string }).text;
      }
    }
  }
  return JSON.stringify(result);
}

suite("trust-twin LM tools (VS Code)", function () {
  this.timeout(30000);

  test("topology propose returns reviewed diff and compiler dry-run evidence", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder for extension tests.");

    const prompt = "Conveyor with 2 sensors and a pusher";
    const tool = new STTrustTwinTopologyProposeTool();
    const tokenSource = new vscode.CancellationTokenSource();
    const result = await tool.invoke(
      {
        input: {
          rootPath: workspaceFolder.uri.fsPath,
          description: prompt,
          reference_page_id: "drive-cell",
          component_kind_constraints: ["motor", "transmitter", "valve"],
          write_gate_artifact: true,
        },
      },
      tokenSource.token,
    );

    const payload = JSON.parse(toolResultText(result));
    assert.strictEqual(payload.tool, "trust_twin_topology_propose");
    assert.strictEqual(payload.provider, "local");
    assert.strictEqual(payload.prompt.description, prompt);
    assert.strictEqual(payload.topology_path, "hmi/views/drive-cell.topology.toml");
    assert.ok(payload.generated_diff.includes("--- hmi/views/drive-cell.topology.toml"));
    assert.ok(payload.generated_diff.includes("+++ hmi/views/drive-cell.topology.toml"));
    assert.ok(payload.generated_diff.includes('+kind = "motor"'));
    assert.ok(payload.generated_diff.includes('+kind = "transmitter"'));
    assert.ok(payload.generated_diff.includes('+kind = "valve"'));
    assert.ok(!payload.generated_diff.includes("[[bind3d]]"));
    assert.ok(!payload.generated_diff.includes("xyz"));
    assert.deepStrictEqual(payload.quality_checks, {
      raw_coordinates: false,
      bind3d: false,
    });
    assert.strictEqual(payload.compiler.ok, true);
    assert.strictEqual(payload.compiler.diagnostics.length, 0);
    assert.strictEqual(payload.compiler.topology_hash.length, 64);
    assert.strictEqual(payload.compiler.view_hash.length, 64);
    assert.strictEqual(payload.compiler.stats.component_count, 4);
    assert.strictEqual(payload.review.added_components.length, 4);
    assert.ok(
      String(payload.artifact_path).endsWith(
        "target/gate-artifacts/trust-twin-p4-ai-author.json",
      ),
    );

    const repoRoot = path.resolve(__dirname, "..", "..", "..", "..", "..");
    const artifactUri = vscode.Uri.joinPath(
      vscode.Uri.file(repoRoot),
      "target",
      "gate-artifacts",
      "trust-twin-p4-ai-author.json",
    );
    const artifact = JSON.parse(
      Buffer.from(await vscode.workspace.fs.readFile(artifactUri)).toString("utf8"),
    );
    assert.strictEqual(artifact.prompt.description, prompt);
    assert.strictEqual(artifact.compiler.ok, true);
    assert.strictEqual(artifact.compiler.topology_hash.length, 64);
    assert.strictEqual(artifact.quality_checks.raw_coordinates, false);
    assert.strictEqual(artifact.quality_checks.bind3d, false);
  });

  test("topology propose accepts robot-cell attachment vocabulary", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder for extension tests.");

    const prompt = "Robot cell with pickup table, box, gripper, drop zone and safety light";
    const tool = new STTrustTwinTopologyProposeTool();
    const tokenSource = new vscode.CancellationTokenSource();
    const result = await tool.invoke(
      {
        input: {
          rootPath: workspaceFolder.uri.fsPath,
          description: prompt,
          reference_page_id: "robot-cell",
          component_kind_constraints: [
            "robot_arm",
            "gripper",
            "workpiece",
            "pickup_zone",
            "drop_zone",
            "safety_light",
          ],
        },
      },
      tokenSource.token,
    );

    const payload = JSON.parse(toolResultText(result));
    assert.strictEqual(payload.tool, "trust_twin_topology_propose");
    assert.strictEqual(payload.topology_path, "hmi/views/robot-cell.topology.toml");
    assert.ok(payload.generated_diff.includes('+kind = "robot_arm"'));
    assert.ok(payload.generated_diff.includes('+kind = "workpiece"'));
    assert.ok(payload.generated_diff.includes('+kind = "gripper"'));
    assert.ok(
      payload.generated_diff.includes(
        '+at = { attach_to = "PICKUP-1.top", placement = "top_center" }',
      ),
    );
    assert.ok(
      payload.generated_diff.includes(
        '+at = { attach_to = "ROBOT-1.tool", placement = "mount" }',
      ),
    );
    assert.ok(!payload.generated_diff.includes("[[bind3d]]"));
    assert.ok(!payload.generated_diff.includes("xyz"));
    assert.deepStrictEqual(payload.quality_checks, {
      raw_coordinates: false,
      bind3d: false,
    });
    assert.strictEqual(payload.compiler.ok, true);
    const doctorRules = new Set(
      payload.compiler.doctor_results.map((result: { rule: string }) => result.rule),
    );
    assert.ok(doctorRules.has("attachment-target-exists"));
    assert.ok(doctorRules.has("workpiece-rests-on-surface"));
    assert.ok(doctorRules.has("gripper-approach-sane"));
  });
});
