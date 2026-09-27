import {
  resolveControlStudioBinding,
  type ControlStudioBindingDiscovery,
  type ControlStudioBindingSelection,
  type ControlStudioBindingSelectionResult,
  type ControlStudioBindingTarget,
} from "./hostBindingDiscovery";
import type { ProjectControlLoopOpenRequest } from "./projectLoop";

export interface ControlStudioBindingScope {
  projectId: string;
  runtimeId: string;
  sessionId: string;
}

export interface WorkspaceControlProject {
  id: string;
  name: string;
  rootUri: string;
}

export interface ControlStudioBindingSessionDependencies {
  scope: ControlStudioBindingScope;
  workspaceProjects: () => readonly WorkspaceControlProject[] | Promise<readonly WorkspaceControlProject[]>;
  selectWorkspaceProject: (projects: readonly WorkspaceControlProject[]) => WorkspaceControlProject | undefined | Promise<WorkspaceControlProject | undefined>;
  discover: (projectRoot: string) => Promise<ControlStudioBindingDiscovery>;
  openNewSession: (value: {
    sessionId: string;
    scope: ControlStudioBindingScope;
    request: ProjectControlLoopOpenRequest;
    binding: Extract<ControlStudioBindingSelectionResult, { kind: "accepted" }>;
  }) => void | Promise<void>;
  newSessionId?: (scope: ControlStudioBindingScope, sequence: number) => string;
}

export type ControlStudioBindingSessionRequest =
  | { schemaVersion: 1; kind: "control-studio.binding.list"; scope: ControlStudioBindingScope; projectRoot?: string }
  | { schemaVersion: 1; kind: "control-studio.binding.select"; scope: ControlStudioBindingScope; selection: ControlStudioBindingSelection };

export interface ControlStudioBindingTargetSummary {
  loopId: string;
  name: string;
  configurationPath: string;
  controller: ControlStudioBindingTarget["controller"];
  inputs: ControlStudioBindingTarget["inputs"];
  outputs: ControlStudioBindingTarget["outputs"];
  controllerCompatibility: ControlStudioBindingTarget["controllerCompatibility"];
}

export type ControlStudioBindingSessionReply =
  | {
      schemaVersion: 1;
      kind: "control-studio.binding.list.reply";
      status: "accepted" | "rejected";
      scope: ControlStudioBindingScope;
      project?: WorkspaceControlProject;
      targets: readonly ControlStudioBindingTargetSummary[];
      rejected: ControlStudioBindingDiscovery["rejected"];
      reason?: string;
    }
  | {
      schemaVersion: 1;
      kind: "control-studio.binding.select.reply";
      status: "accepted" | "rejected";
      scope: ControlStudioBindingScope;
      sessionId?: string;
      request?: ProjectControlLoopOpenRequest;
      binding?: Extract<ControlStudioBindingSelectionResult, { kind: "accepted" }>;
      reason?: string;
    };

interface ListedProject {
  project: WorkspaceControlProject;
  discovery: ControlStudioBindingDiscovery;
}

/**
 * Host-side protocol boundary for the selector UI. Listing is read-only;
 * selection produces a new scoped open request through the supplied callback.
 */
export function createControlStudioBindingSession(dependencies: ControlStudioBindingSessionDependencies) {
  let listed: ListedProject | undefined;
  let sequence = 0;

  return {
    handle: async (request: ControlStudioBindingSessionRequest): Promise<ControlStudioBindingSessionReply> => {
      if (request.schemaVersion !== 1) return rejectedReply(request, "Unsupported binding-session message version.");
      if (request.kind === "control-studio.binding.list") return list(request);
      return select(request);
    },
  };

  async function list(request: Extract<ControlStudioBindingSessionRequest, { kind: "control-studio.binding.list" }>): Promise<ControlStudioBindingSessionReply> {
    if (!sameScope(request.scope, dependencies.scope)) return listRejected(request, "The binding list belongs to a different Control Studio session.");
    const projects = await dependencies.workspaceProjects();
    const project = request.projectRoot
      ? projects.find(candidate => candidate.rootUri === request.projectRoot)
      : await dependencies.selectWorkspaceProject(projects);
    if (!project) return listRejected(request, "Choose a workspace project before listing Control Studio bindings.");
    if (project.id !== dependencies.scope.projectId) {
      return listRejected(request, `Workspace project ${project.id} is outside the current Control Studio scope.`);
    }
    const discovery = await dependencies.discover(project.rootUri);
    listed = { project, discovery };
    const targets = discovery.targets
      .filter(target => target.loopDefinition.projectId === dependencies.scope.projectId && target.loopDefinition.runtimeId === dependencies.scope.runtimeId)
      .map(targetSummary);
    return {
      schemaVersion: 1, kind: "control-studio.binding.list.reply", status: "accepted", scope: clone(dependencies.scope), project: clone(project),
      targets, rejected: clone(discovery.rejected),
    };
  }

  async function select(request: Extract<ControlStudioBindingSessionRequest, { kind: "control-studio.binding.select" }>): Promise<ControlStudioBindingSessionReply> {
    if (!sameScope(request.scope, dependencies.scope)) return selectRejected(request, "The binding selection belongs to a different Control Studio session.");
    if (!listed) return selectRejected(request, "List the scoped Control Studio bindings before selecting one.");
    const scopedDiscovery: ControlStudioBindingDiscovery = {
      targets: listed.discovery.targets.filter(target => target.loopDefinition.projectId === dependencies.scope.projectId
        && target.loopDefinition.runtimeId === dependencies.scope.runtimeId),
      rejected: listed.discovery.rejected,
    };
    const binding = resolveControlStudioBinding(scopedDiscovery, request.selection);
    if (binding.kind === "rejected") return selectRejected(request, binding.reason);
    const sessionId = dependencies.newSessionId?.(dependencies.scope, sequence + 1)
      ?? `${dependencies.scope.sessionId}:binding:${sequence + 1}`;
    sequence += 1;
    const nextScope = { ...dependencies.scope, sessionId };
    const openRequest: ProjectControlLoopOpenRequest = {
      schemaVersion: 1,
      source: "ConfiguredLoop",
      scope: { projectId: binding.loopDefinition.projectId, runtimeId: binding.loopDefinition.runtimeId },
      loopDefinition: clone(binding.loopDefinition),
    };
    const isolatedBinding = clone(binding);
    await dependencies.openNewSession({ sessionId, scope: nextScope, request: openRequest, binding: isolatedBinding });
    return { schemaVersion: 1, kind: "control-studio.binding.select.reply", status: "accepted", scope: nextScope,
      sessionId, request: clone(openRequest), binding: isolatedBinding };
  }
}

function targetSummary(target: ControlStudioBindingTarget): ControlStudioBindingTargetSummary {
  return {
    loopId: target.loopDefinition.id,
    name: target.loopDefinition.name,
    configurationPath: target.configurationPath,
    controller: clone(target.controller),
    inputs: clone(target.inputs),
    outputs: clone(target.outputs),
    controllerCompatibility: clone(target.controllerCompatibility),
  };
}

function sameScope(left: ControlStudioBindingScope, right: ControlStudioBindingScope): boolean {
  return left.projectId === right.projectId && left.runtimeId === right.runtimeId && left.sessionId === right.sessionId;
}

function listRejected(
  request: Extract<ControlStudioBindingSessionRequest, { kind: "control-studio.binding.list" }>, reason: string,
): ControlStudioBindingSessionReply {
  return { schemaVersion: 1, kind: "control-studio.binding.list.reply", status: "rejected", scope: clone(request.scope), targets: [], rejected: [], reason };
}

function selectRejected(
  request: Extract<ControlStudioBindingSessionRequest, { kind: "control-studio.binding.select" }>, reason: string,
): ControlStudioBindingSessionReply {
  return { schemaVersion: 1, kind: "control-studio.binding.select.reply", status: "rejected", scope: clone(request.scope), reason };
}

function rejectedReply(request: ControlStudioBindingSessionRequest, reason: string): ControlStudioBindingSessionReply {
  return request.kind === "control-studio.binding.list"
    ? listRejected(request, reason)
    : selectRejected(request, reason);
}

function clone<T>(value: T): T { return structuredClone(value); }
