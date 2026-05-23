// Responsibility: focused trust-twin LM tools module with a single concern.
import { spawn } from "child_process";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  errorResult,
  resolveWorkspaceFolder,
  stableJsonString,
  textResult,
  type InvocationOptions,
  type TrustTwinTopologyProposeParams,
} from "./shared";

type TrustTwinDoctorResult = {
  rule: string;
  passed: boolean;
  message: string;
};

type TrustTwinCompileStats = {
  component_count: number;
  connection_count: number;
  binding_count: number;
  generated_node_count: number;
};

type TrustTwinCompilerDryRunResult = {
  ok: boolean;
  topology_hash?: string | null;
  view_hash?: string | null;
  diagnostics: unknown[];
  doctor_results: TrustTwinDoctorResult[];
  stats?: TrustTwinCompileStats | null;
  error?: string | null;
};

export type TrustTwinCompilerDryRunRequest = {
  rootPath: string;
  topologyPath: string;
  topologySource: string;
};

export type TrustTwinCompilerRunner = (
  request: TrustTwinCompilerDryRunRequest,
  token: vscode.CancellationToken,
) => Promise<TrustTwinCompilerDryRunResult>;

let compilerRunner: TrustTwinCompilerRunner = runTrustTwinCompilerDryRun;

export function __testSetTrustTwinCompilerRunner(
  runner?: TrustTwinCompilerRunner,
): void {
  compilerRunner = runner ?? runTrustTwinCompilerDryRun;
}

type ProposedComponent = {
  id: string;
  kind: string;
  placement: string;
};

export class STTrustTwinTopologyProposeTool {
  async invoke(
    options: InvocationOptions<TrustTwinTopologyProposeParams>,
    token: vscode.CancellationToken,
  ): Promise<unknown> {
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }
    const description = String(options.input.description ?? "").trim();
    if (!description) {
      return errorResult("description is required.");
    }

    const resolved = resolveWorkspaceFolder(options.input.rootPath);
    if (resolved.error || !resolved.folder) {
      return errorResult(resolved.error ?? "Unable to resolve workspace folder.");
    }
    const provider = normalizeProvider(
      vscode.workspace
        .getConfiguration("trust-lsp", resolved.folder.uri)
        .get<string>("trustTwin.aiProvider", "local"),
    );
    if (provider === "disabled") {
      return errorResult(
        "trust-twin AI topology authoring is disabled by trust-lsp.trustTwin.aiProvider.",
      );
    }
    if (provider === "cloud") {
      return errorResult(
        "cloud trust-twin topology authoring is opt-in, but no cloud provider adapter is configured in P4.",
      );
    }

    const topologyPath = topologyPathForReferencePage(options.input.reference_page_id);
    const topologyUri = vscode.Uri.joinPath(
      resolved.folder.uri,
      ...topologyPath.split("/"),
    );
    const existing = await readOptionalUtf8(topologyUri);
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }

    const components = proposeComponents(
      description,
      options.input.component_kind_constraints ?? [],
    );
    const topologyAppend = renderTopologyAppend(components);
    const generatedDiff = renderAdditiveDiff(topologyPath, topologyAppend);
    const proposedTopology = joinTopology(existing.content, topologyAppend);
    const qualityChecks = {
      raw_coordinates: /\bxyz\s*=/.test(topologyAppend),
      bind3d: /\[\[bind3d\]\]/.test(topologyAppend),
    };
    if (qualityChecks.raw_coordinates || qualityChecks.bind3d) {
      return errorResult("generated topology proposal failed trust-twin quality checks.");
    }

    const compiler = await compilerRunner(
      {
        rootPath: resolved.folder.uri.fsPath,
        topologyPath,
        topologySource: proposedTopology,
      },
      token,
    );
    if (token.isCancellationRequested) {
      return textResult("Cancelled.");
    }

    const payload = {
      tool: "trust_twin_topology_propose",
      provider,
      topology_path: topologyPath,
      prompt: {
        description,
        reference_page_id: options.input.reference_page_id ?? null,
        component_kind_constraints: options.input.component_kind_constraints ?? [],
      },
      generated_diff: generatedDiff,
      review: {
        existing_file: existing.exists,
        diff_format: "unified-additive",
        added_components: components.map(({ id, kind }) => ({ id, kind })),
        before: {
          topology_path: topologyPath,
          bytes: existing.content.length,
        },
        after: {
          topology_path: topologyPath,
          added_component_count: components.length,
        },
      },
      compiler,
      doctor_result: compiler.doctor_results,
      quality_checks: qualityChecks,
      artifact_path: null as string | null,
    };

    if (options.input.write_gate_artifact === true) {
      const artifactRoot = resolveGateArtifactRoot(resolved.folder.uri.fsPath);
      const artifactPath = path.join(
        artifactRoot,
        "target",
        "gate-artifacts",
        "trust-twin-p4-ai-author.json",
      );
      await fs.promises.mkdir(path.dirname(artifactPath), { recursive: true });
      payload.artifact_path = artifactPath;
      await fs.promises.writeFile(
        artifactPath,
        `${stableJsonString(payload)}\n`,
        "utf8",
      );
    }

    return textResult(JSON.stringify(payload, null, 2));
  }
}

function normalizeProvider(value: string): "disabled" | "local" | "cloud" {
  if (value === "disabled" || value === "cloud") {
    return value;
  }
  return "local";
}

function topologyPathForReferencePage(referencePageId: string | undefined): string {
  const raw = String(referencePageId ?? "trust-twin-ai").trim();
  const stem = raw
    .replace(/\.toml$/i, "")
    .replace(/\.topology$/i, "")
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
  return `hmi/views/${stem || "trust-twin-ai"}.topology.toml`;
}

function proposeComponents(
  description: string,
  componentKindConstraints: string[],
): ProposedComponent[] {
  const allowed = new Set(
    componentKindConstraints.map((kind) => kind.trim().toLowerCase()).filter(Boolean),
  );
  const chooseKind = (candidates: string[]): string => {
    const kind = candidates.find((candidate) => allowed.size === 0 || allowed.has(candidate));
    if (!kind) {
      throw new Error(
        `component kind constraints do not allow any of: ${candidates.join(", ")}`,
      );
    }
    return kind;
  };

  const lower = description.toLowerCase();
  const components: ProposedComponent[] = [];
  let previousId: string | undefined;
  const add = (id: string, kind: string, explicitPlacement?: string): void => {
    const placement =
      explicitPlacement ?? (previousId ? `right_of = "${previousId}"` : 'grid = "A1"');
    components.push({ id, kind, placement });
    previousId = id;
  };

  if (
    lower.includes("robot") ||
    lower.includes("gripper") ||
    lower.includes("pickup") ||
    lower.includes("drop zone")
  ) {
    add("ROBOT-1", chooseKind(["robot_arm"]), 'grid = "B2"');
    add("PICKUP-1", chooseKind(["pickup_zone"]), 'grid = "A1"');
    add(
      "BOX-1",
      chooseKind(["workpiece"]),
      'attach_to = "PICKUP-1.top", placement = "top_center"',
    );
    add(
      "GRIPPER-1",
      chooseKind(["gripper"]),
      'attach_to = "ROBOT-1.tool", placement = "mount"',
    );
    add("DROP-1", chooseKind(["drop_zone"]), 'grid = "A3"');
    add("LIGHT-1", chooseKind(["safety_light"]), 'right_of = "ROBOT-1"');
    return components;
  }

  if (lower.includes("conveyor")) {
    add("conveyor-1", chooseKind(["motor", "vfd"]));
  }

  const sensorCount = Math.max(
    lower.includes("sensor") ? 1 : 0,
    Number(lower.match(/(\d+)\s+sensors?/)?.[1] ?? 0),
  );
  for (let index = 1; index <= sensorCount; index += 1) {
    add(`sensor-${index}`, chooseKind(["transmitter"]));
  }

  if (lower.includes("pusher")) {
    add("pusher-1", chooseKind(["valve", "pump"]));
  }

  if (components.length === 0) {
    add("unit-1", chooseKind(["tank", "pump", "valve", "motor", "transmitter"]));
  }

  return components;
}

function renderTopologyAppend(components: ProposedComponent[]): string {
  return `${components
    .map(
      (component) => `[[components]]
id = "${component.id}"
kind = "${component.kind}"
at = { ${component.placement} }
`,
    )
    .join("\n")}`;
}

function renderAdditiveDiff(topologyPath: string, topologyAppend: string): string {
  const added = topologyAppend
    .trimEnd()
    .split(/\r?\n/)
    .map((line) => `+${line}`)
    .join("\n");
  return `--- ${topologyPath}\n+++ ${topologyPath}\n@@\n${added}\n`;
}

function joinTopology(existing: string, topologyAppend: string): string {
  const trimmed = existing.trimEnd();
  if (!trimmed) {
    return topologyAppend;
  }
  return `${trimmed}\n\n${topologyAppend}`;
}

async function readOptionalUtf8(
  uri: vscode.Uri,
): Promise<{ exists: boolean; content: string }> {
  try {
    const content = await vscode.workspace.fs.readFile(uri);
    return { exists: true, content: Buffer.from(content).toString("utf8") };
  } catch {
  return { exists: false, content: "" };
  }
}

function resolveGateArtifactRoot(workspaceRoot: string): string {
  const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
  if (
    fs.existsSync(path.join(repoRoot, "Cargo.toml")) &&
    fs.existsSync(path.join(repoRoot, "crates", "trust-twin-compiler"))
  ) {
    return repoRoot;
  }
  return workspaceRoot;
}

async function runTrustTwinCompilerDryRun(
  request: TrustTwinCompilerDryRunRequest,
  token: vscode.CancellationToken,
): Promise<TrustTwinCompilerDryRunResult> {
  const hash = crypto
    .createHash("sha256")
    .update(request.topologyPath)
    .update("\0")
    .update(request.topologySource)
    .digest("hex")
    .slice(0, 16);
  const compilerRoot = resolveGateArtifactRoot(request.rootPath);
  const tempRoot = path.join(compilerRoot, "target", "tmp", `trust-twin-ai-${hash}`);
  await fs.promises.mkdir(tempRoot, { recursive: true });
  const tempTopology = path.join(tempRoot, "proposal.topology.toml");
  await fs.promises.writeFile(tempTopology, request.topologySource, "utf8");

  const candidateExe = path.join(
    compilerRoot,
    "target",
    "debug",
    process.platform === "win32" ? "trust-twin-compiler.exe" : "trust-twin-compiler",
  );
  const command = fs.existsSync(candidateExe) ? candidateExe : "cargo";
  const args = fs.existsSync(candidateExe)
    ? ["--dry-run", "--input", tempTopology, "--json"]
    : [
        "run",
        "-p",
        "trust-twin-compiler",
        "--bin",
        "trust-twin-compiler",
        "--",
        "--dry-run",
        "--input",
        tempTopology,
        "--json",
      ];
  const result = await runProcess(command, args, compilerRoot, token);
  const parsed = parseCompilerOutput(result.stdout);
  if (parsed) {
    return parsed;
  }
  return {
    ok: false,
    diagnostics: [],
    doctor_results: [],
    stats: null,
    error:
      result.stderr.trim() ||
      result.stdout.trim() ||
      `trust-twin compiler exited with ${result.exitCode}`,
  };
}

function parseCompilerOutput(stdout: string): TrustTwinCompilerDryRunResult | undefined {
  const trimmed = stdout.trim();
  if (!trimmed) {
    return undefined;
  }
  try {
    const parsed = JSON.parse(trimmed) as TrustTwinCompilerDryRunResult;
    if (typeof parsed.ok === "boolean") {
      return {
        ok: parsed.ok,
        topology_hash: parsed.topology_hash ?? null,
        view_hash: parsed.view_hash ?? null,
        diagnostics: Array.isArray(parsed.diagnostics) ? parsed.diagnostics : [],
        doctor_results: Array.isArray(parsed.doctor_results)
          ? parsed.doctor_results
          : [],
        stats: parsed.stats ?? null,
        error: parsed.error ?? null,
      };
    }
  } catch {
    return undefined;
  }
  return undefined;
}

function runProcess(
  command: string,
  args: string[],
  cwd: string,
  token: vscode.CancellationToken,
): Promise<{ exitCode: number | null; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd });
    let stdout = "";
    let stderr = "";
    const cancellation = token.onCancellationRequested(() => {
      child.kill();
    });
    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (err) => {
      cancellation.dispose();
      reject(err);
    });
    child.on("close", (exitCode) => {
      cancellation.dispose();
      resolve({ exitCode, stdout, stderr });
    });
  });
}
