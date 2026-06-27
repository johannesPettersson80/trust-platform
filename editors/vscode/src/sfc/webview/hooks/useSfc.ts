import { useCallback, useState } from "react";
import {
  Connection,
  EdgeChange,
  MarkerType,
  NodeChange,
  XYPosition,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
} from "@xyflow/react";
import type {
  ParallelNodeData,
  SfcAction,
  SfcNode,
  SfcParallelJoinNode,
  SfcParallelSplitNode,
  SfcStepNode,
  SfcTransitionEdge,
  SfcWorkspace,
  StepNodeData,
  StepType,
  TransitionData,
} from "../types";
import {
  STEP_SOURCE_BOTTOM,
  STEP_SOURCE_LEFT,
  STEP_SOURCE_RIGHT,
  STEP_TARGET_LEFT,
  STEP_TARGET_RIGHT,
  STEP_TARGET_TOP,
} from "../StepNode";

const MIN_PARALLEL_BRANCHES = 2;
const SPLIT_INPUT_HANDLE = "in";
const JOIN_OUTPUT_HANDLE = "out";
const SPLIT_BRANCH_PREFIX = "branch-out-";
const JOIN_BRANCH_PREFIX = "branch-in-";

function isSfcStepNode(node: SfcNode): node is SfcStepNode {
  return node.type === "step";
}

function isSfcParallelSplitNode(node: SfcNode): node is SfcParallelSplitNode {
  return node.type === "parallelSplit";
}

function isSfcParallelJoinNode(node: SfcNode): node is SfcParallelJoinNode {
  return node.type === "parallelJoin";
}

function withActiveState(node: SfcNode, isActive: boolean): SfcNode {
  if (isSfcStepNode(node)) {
    return {
      ...node,
      data: {
        ...node.data,
        isActive,
      },
    };
  }

  if (isSfcParallelSplitNode(node)) {
    return {
      ...node,
      data: {
        ...node.data,
        isActive,
      },
    };
  }

  return {
    ...node,
    data: {
      ...node.data,
      isActive,
    },
  };
}

function buildTransitionData(
  updates: Partial<TransitionData> = {},
  current?: TransitionData
): TransitionData {
  const condition = updates.condition ?? current?.condition ?? "TRUE";

  return {
    condition,
    label: updates.label ?? current?.label ?? condition,
    description: updates.description ?? current?.description,
    priority: updates.priority ?? current?.priority,
  };
}

function normalizeBranchCount(value: number | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return MIN_PARALLEL_BRANCHES;
  }

  return Math.max(MIN_PARALLEL_BRANCHES, Math.floor(value));
}

function parseBranchHandleIndex(
  handle: string | null | undefined,
  prefix: string
): number | null {
  if (!handle || !handle.startsWith(prefix)) {
    return null;
  }

  const parsed = Number.parseInt(handle.slice(prefix.length), 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return null;
  }

  return parsed;
}

function getBranchCount(node: SfcParallelSplitNode | SfcParallelJoinNode): number {
  return normalizeBranchCount(node.data.branchCount);
}

function isSingleHandleTaken(
  connection: Connection,
  existingEdges: SfcTransitionEdge[]
): boolean {
  return existingEdges.some(
    (edge) =>
      edge.source === connection.source &&
      edge.target === connection.target &&
      edge.sourceHandle === connection.sourceHandle &&
      edge.targetHandle === connection.targetHandle
  );
}

const INITIAL_NODES: SfcNode[] = [
  {
    id: "step_init",
    type: "step",
    position: { x: 250, y: 50 },
    data: {
      label: "Init",
      type: "initial",
      actions: [],
    },
  },
];

const INITIAL_EDGES: SfcTransitionEdge[] = [];
const DEFAULT_NODE_X = 250;
const DEFAULT_NODE_Y = 50;
const DEFAULT_NODE_VERTICAL_GAP = 150;

function nextNodePosition(nodes: SfcNode[]): XYPosition {
  const maxY = nodes.reduce(
    (current, node) => Math.max(current, node.position.y),
    DEFAULT_NODE_Y - DEFAULT_NODE_VERTICAL_GAP
  );

  return {
    x: DEFAULT_NODE_X,
    y: maxY + DEFAULT_NODE_VERTICAL_GAP,
  };
}

function stepConnectionHandles(
  sourceNode: SfcNode,
  targetNode: SfcNode
): Pick<Connection, "sourceHandle" | "targetHandle"> {
  if (!isSfcStepNode(sourceNode) || !isSfcStepNode(targetNode)) {
    return {
      sourceHandle: null,
      targetHandle: null,
    };
  }

  const deltaY = targetNode.position.y - sourceNode.position.y;
  const expectedVerticalGap = DEFAULT_NODE_VERTICAL_GAP * 1.35;

  if (deltaY < 0) {
    if (Math.abs(deltaY) > expectedVerticalGap) {
      return {
        sourceHandle: STEP_SOURCE_RIGHT,
        targetHandle: STEP_TARGET_RIGHT,
      };
    }

    return {
      sourceHandle: STEP_SOURCE_LEFT,
      targetHandle: STEP_TARGET_LEFT,
    };
  }

  if (deltaY > expectedVerticalGap) {
    return {
      sourceHandle: STEP_SOURCE_RIGHT,
      targetHandle: STEP_TARGET_RIGHT,
    };
  }

  return {
    sourceHandle: STEP_SOURCE_BOTTOM,
    targetHandle: STEP_TARGET_TOP,
  };
}

function applyStepConnectionHandles(edge: SfcTransitionEdge, nodes: SfcNode[]) {
  if (edge.sourceHandle || edge.targetHandle) {
    return;
  }

  const sourceNode = nodes.find((node) => node.id === edge.source);
  const targetNode = nodes.find((node) => node.id === edge.target);
  if (!sourceNode || !targetNode) {
    return;
  }

  const handles = stepConnectionHandles(sourceNode, targetNode);
  edge.sourceHandle = handles.sourceHandle;
  edge.targetHandle = handles.targetHandle;
}

/**
 * Custom hook for managing SFC state and operations
 */
export function useSfc() {
  const [nodes, setNodes] = useState<SfcNode[]>(INITIAL_NODES);
  const [edges, setEdges] = useState<SfcTransitionEdge[]>(INITIAL_EDGES);
  const [variables, setVariables] = useState<SfcWorkspace["variables"]>([]);

  /**
   * Handle node changes (drag, select, etc.)
   */
  const onNodesChange = useCallback((changes: NodeChange<SfcNode>[]) => {
    setNodes((nds) => applyNodeChanges(changes, nds));
  }, []);

  /**
   * Handle edge changes
   */
  const onEdgesChange = useCallback((changes: EdgeChange<SfcTransitionEdge>[]) => {
    setEdges((eds) => applyEdgeChanges(changes, eds));
  }, []);

  /**
   * Handle new connection (transition) between steps
   */
  const onConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) {
        return;
      }

      if (connection.source === connection.target) {
        return;
      }

      const sourceNode = nodes.find((node) => node.id === connection.source);
      const targetNode = nodes.find((node) => node.id === connection.target);
      if (!sourceNode || !targetNode) {
        return;
      }

      if (isSingleHandleTaken(connection, edges)) {
        return;
      }

      if (isSfcParallelSplitNode(sourceNode)) {
        const branchIndex = parseBranchHandleIndex(
          connection.sourceHandle,
          SPLIT_BRANCH_PREFIX
        );
        if (
          branchIndex === null ||
          branchIndex >= getBranchCount(sourceNode) ||
          edges.some(
            (edge) =>
              edge.source === sourceNode.id &&
              edge.sourceHandle === connection.sourceHandle
          )
        ) {
          return;
        }
      }

      if (isSfcParallelJoinNode(sourceNode)) {
        if (
          connection.sourceHandle !== JOIN_OUTPUT_HANDLE ||
          edges.some(
            (edge) =>
              edge.source === sourceNode.id &&
              edge.sourceHandle === JOIN_OUTPUT_HANDLE
          )
        ) {
          return;
        }
      }

      if (isSfcParallelSplitNode(targetNode)) {
        if (
          connection.targetHandle !== SPLIT_INPUT_HANDLE ||
          edges.some(
            (edge) =>
              edge.target === targetNode.id &&
              edge.targetHandle === SPLIT_INPUT_HANDLE
          )
        ) {
          return;
        }
      }

      if (isSfcParallelJoinNode(targetNode)) {
        const branchIndex = parseBranchHandleIndex(
          connection.targetHandle,
          JOIN_BRANCH_PREFIX
        );
        if (
          branchIndex === null ||
          branchIndex >= getBranchCount(targetNode) ||
          edges.some(
            (edge) =>
              edge.target === targetNode.id &&
              edge.targetHandle === connection.targetHandle
          )
        ) {
          return;
        }
      }

      const data = buildTransitionData({ condition: "TRUE", label: "TRUE" });
      const handles = stepConnectionHandles(sourceNode, targetNode);
      const newEdge: SfcTransitionEdge = {
        id: `trans_${Date.now()}`,
        source: connection.source,
        target: connection.target,
        sourceHandle: connection.sourceHandle ?? handles.sourceHandle,
        targetHandle: connection.targetHandle ?? handles.targetHandle,
        type: "transition",
        markerEnd: {
          type: MarkerType.ArrowClosed,
        },
        data,
        label: data.label,
      };

      setEdges((eds) => addEdge(newEdge, eds));
    },
    [edges, nodes]
  );

  /**
   * Add a new step to the diagram
   */
  const addNewStep = useCallback(
    (type: StepType = "normal", position?: XYPosition) => {
      const id = `step_${Date.now()}`;
      setNodes((nds) => {
        const stepCount = nds.filter((node) => node.type === "step").length;
        const newStep: SfcStepNode = {
          id,
          type: "step",
          position: position ?? nextNodePosition(nds),
          data: {
            label: `Step${stepCount + 1}`,
            type,
            actions: [],
          },
        };

        return [...nds, newStep];
      });
      return id;
    },
    []
  );

  /**
   * Add a parallel split node
   */
  const addParallelSplit = useCallback((position?: XYPosition) => {
    const id = `split_${Date.now()}`;
    setNodes((nds) => {
      const data: ParallelNodeData = {
        label: "Parallel Split",
        nodeType: "parallelSplit",
        branchCount: 2,
      };

      const newNode: SfcParallelSplitNode = {
        id,
        type: "parallelSplit",
        position: position ?? nextNodePosition(nds),
        data,
      };

      return [...nds, newNode];
    });
    return id;
  }, []);

  /**
   * Add a parallel join node
   */
  const addParallelJoin = useCallback((position?: XYPosition) => {
    const id = `join_${Date.now()}`;
    setNodes((nds) => {
      const data: ParallelNodeData = {
        label: "Parallel Join",
        nodeType: "parallelJoin",
        branchCount: 2,
      };

      const newNode: SfcParallelJoinNode = {
        id,
        type: "parallelJoin",
        position: position ?? nextNodePosition(nds),
        data,
      };

      return [...nds, newNode];
    });
    return id;
  }, []);

  /**
   * Update node data
   */
  const updateStepNodeData = useCallback(
    (nodeId: string, updates: Partial<StepNodeData>) => {
      setNodes((nds) =>
        nds.map((node) =>
          node.id === nodeId && isSfcStepNode(node)
            ? { ...node, data: { ...node.data, ...updates } }
            : node
        )
      );
    },
    []
  );

  /**
   * Update parallel split/join node data
   */
  const updateParallelNodeData = useCallback(
    (nodeId: string, updates: Partial<ParallelNodeData>) => {
      const normalizedBranchCount =
        updates.branchCount === undefined
          ? undefined
          : normalizeBranchCount(updates.branchCount);

      setNodes((nds) =>
        nds.map((node) => {
          if (
            node.id !== nodeId ||
            (!isSfcParallelSplitNode(node) && !isSfcParallelJoinNode(node))
          ) {
            return node;
          }

          return {
            ...node,
            data: {
              ...node.data,
              ...updates,
              branchCount:
                normalizedBranchCount === undefined
                  ? node.data.branchCount
                  : normalizedBranchCount,
            },
          };
        })
      );

      if (normalizedBranchCount !== undefined) {
        setEdges((eds) =>
          eds.filter((edge) => {
            if (edge.source === nodeId) {
              const splitIndex = parseBranchHandleIndex(
                edge.sourceHandle,
                SPLIT_BRANCH_PREFIX
              );
              if (splitIndex !== null && splitIndex >= normalizedBranchCount) {
                return false;
              }
            }
            if (edge.target === nodeId) {
              const joinIndex = parseBranchHandleIndex(
                edge.targetHandle,
                JOIN_BRANCH_PREFIX
              );
              if (joinIndex !== null && joinIndex >= normalizedBranchCount) {
                return false;
              }
            }
            return true;
          })
        );
      }
    },
    []
  );

  /**
   * Update edge (transition) data
   */
  const updateEdgeData = useCallback(
    (edgeId: string, updates: Partial<TransitionData>) => {
      setEdges((eds) =>
        eds.map((edge) => {
          if (edge.id !== edgeId) {
            return edge;
          }

          const data = buildTransitionData(updates, edge.data);
          return {
            ...edge,
            data,
            label: data.label,
          };
        })
      );
    },
    []
  );

  /**
   * Add action to a step
   */
  const addActionToStep = useCallback((stepId: string, action: SfcAction) => {
    setNodes((nds) =>
      nds.map((node) => {
        if (!(node.id === stepId && isSfcStepNode(node))) {
          return node;
        }

        const currentActions = node.data.actions || [];
        return {
          ...node,
          data: {
            ...node.data,
            actions: [...currentActions, action],
          },
        };
      })
    );
  }, []);

  /**
   * Update action in a step
   */
  const updateAction = useCallback(
    (stepId: string, actionId: string, updates: Partial<SfcAction>) => {
      setNodes((nds) =>
        nds.map((node) => {
          if (!(node.id === stepId && isSfcStepNode(node))) {
            return node;
          }

          const updatedActions =
            node.data.actions?.map((action) =>
              action.id === actionId ? { ...action, ...updates } : action
            ) || [];

          return {
            ...node,
            data: {
              ...node.data,
              actions: updatedActions,
            },
          };
        })
      );
    },
    []
  );

  /**
   * Delete action from a step
   */
  const deleteAction = useCallback((stepId: string, actionId: string) => {
    setNodes((nds) =>
      nds.map((node) => {
        if (!(node.id === stepId && isSfcStepNode(node))) {
          return node;
        }

        const filteredActions =
          node.data.actions?.filter((action) => action.id !== actionId) || [];

        return {
          ...node,
          data: {
            ...node.data,
            actions: filteredActions,
          },
        };
      })
    );
  }, []);

  /**
   * Delete selected nodes and edges
   */
  const deleteSelected = useCallback(
    (selection?: { nodeIds?: string[]; edgeIds?: string[] }) => {
      const selectedNodeIds = new Set(
        selection?.nodeIds && selection.nodeIds.length > 0
          ? selection.nodeIds
          : nodes.filter((node) => node.selected).map((node) => node.id)
      );
      const selectedEdgeIds = new Set(
        selection?.edgeIds && selection.edgeIds.length > 0
          ? selection.edgeIds
          : edges.filter((edge) => edge.selected).map((edge) => edge.id)
      );

      if (selectedNodeIds.size === 0 && selectedEdgeIds.size === 0) {
        return;
      }

      setNodes((nds) => nds.filter((node) => !selectedNodeIds.has(node.id)));
      setEdges((eds) =>
        eds.filter(
          (edge) =>
            !selectedEdgeIds.has(edge.id) &&
            !selectedNodeIds.has(edge.source) &&
            !selectedNodeIds.has(edge.target)
        )
      );
    },
    [edges, nodes]
  );

  /**
   * Auto layout - arrange nodes vertically
   */
  const autoLayout = useCallback(() => {
    setNodes((nds) => {
      const sorted = [...nds].sort((left, right) => {
        const leftInitial = isSfcStepNode(left) && left.data.type === "initial";
        const rightInitial =
          isSfcStepNode(right) && right.data.type === "initial";

        if (leftInitial && !rightInitial) {
          return -1;
        }
        if (!leftInitial && rightInitial) {
          return 1;
        }
        return left.data.label.localeCompare(right.data.label);
      });

      return sorted.map((node, index) => ({
        ...node,
        position: { x: 250, y: 50 + index * 150 },
      }));
    });
  }, []);

  /**
   * Import SFC workspace from JSON
   */
  const importFromJson = useCallback((workspace: SfcWorkspace) => {
    const splitById = new Map(
      (workspace.parallelSplits || []).map((split) => [split.id, split])
    );
    const joinById = new Map(
      (workspace.parallelJoins || []).map((join) => [join.id, join])
    );

    const stepNodes: SfcStepNode[] = workspace.steps.map((step) => ({
      id: step.id,
      type: "step",
      position: { x: step.x || 250, y: step.y || 50 },
      data: {
        label: step.name,
        type: (step.initial ? "initial" : "normal") as StepType,
        actions: step.actions || [],
      },
    }));

    const splitNodes: SfcParallelSplitNode[] = (
      workspace.parallelSplits || []
    ).map((split) => ({
      id: split.id,
      type: "parallelSplit",
      position: { x: split.x || 250, y: split.y || 150 },
      data: {
        label: split.name,
        nodeType: "parallelSplit",
        branchCount: normalizeBranchCount(split.branchIds.length),
      },
    }));

    const joinNodes: SfcParallelJoinNode[] = (workspace.parallelJoins || []).map(
      (join) => ({
        id: join.id,
        type: "parallelJoin",
        position: { x: join.x || 250, y: join.y || 300 },
        data: {
          label: join.name,
          nodeType: "parallelJoin",
          branchCount: normalizeBranchCount(join.branchIds.length),
        },
      })
    );

    const allNodes: SfcNode[] = [...stepNodes, ...splitNodes, ...joinNodes];

    const newEdges: SfcTransitionEdge[] = workspace.transitions.map((trans) => {
      const edge: SfcTransitionEdge = {
        id: trans.id,
        source: trans.sourceStepId,
        target: trans.targetStepId,
        type: "transition",
        markerEnd: {
          type: MarkerType.ArrowClosed,
        },
        data: buildTransitionData({
          condition: trans.condition,
          label: trans.name || trans.condition,
          priority: trans.priority,
        }),
        label: trans.name || trans.condition,
      };

      const sourceSplit = splitById.get(trans.sourceStepId);
      if (sourceSplit) {
        const branchIndex = sourceSplit.branchIds.indexOf(trans.targetStepId);
        if (branchIndex >= 0) {
          edge.sourceHandle = `${SPLIT_BRANCH_PREFIX}${branchIndex}`;
        }
      }

      const sourceJoin = joinById.get(trans.sourceStepId);
      if (sourceJoin) {
        edge.sourceHandle = JOIN_OUTPUT_HANDLE;
      }

      const targetSplit = splitById.get(trans.targetStepId);
      if (targetSplit) {
        edge.targetHandle = SPLIT_INPUT_HANDLE;
      }

      const targetJoin = joinById.get(trans.targetStepId);
      if (targetJoin) {
        const branchIndex = targetJoin.branchIds.indexOf(trans.sourceStepId);
        if (branchIndex >= 0) {
          edge.targetHandle = `${JOIN_BRANCH_PREFIX}${branchIndex}`;
        }
      }

      applyStepConnectionHandles(edge, allNodes);

      return edge;
    });

    setNodes(allNodes);
    setEdges(newEdges);
    setVariables(workspace.variables || []);
  }, []);

  /**
   * Export SFC to workspace JSON
   */
  const exportToJson = useCallback((): SfcWorkspace => {
    const stepNodes = nodes.filter(isSfcStepNode);
    const parallelSplitNodes = nodes.filter(isSfcParallelSplitNode);
    const parallelJoinNodes = nodes.filter(isSfcParallelJoinNode);

    const steps = stepNodes.map((node) => ({
      id: node.id,
      name: node.data.label,
      initial: node.data.type === "initial",
      x: node.position.x,
      y: node.position.y,
      actions: node.data.actions || [],
    }));

    const transitions = edges.map((edge, index) => ({
      id: edge.id,
      name: edge.data.label || `T${index + 1}`,
      condition: edge.data.condition || "TRUE",
      sourceStepId: edge.source,
      targetStepId: edge.target,
      priority: edge.data.priority,
    }));

    const parallelSplits = parallelSplitNodes.map((node) => {
      const branchEdges = edges
        .filter((edge) => edge.source === node.id)
        .map((edge) => ({
          edge,
          index: parseBranchHandleIndex(edge.sourceHandle, SPLIT_BRANCH_PREFIX),
        }))
        .filter((entry) => entry.index !== null)
        .sort((left, right) => (left.index ?? 0) - (right.index ?? 0));

      const branchIds = branchEdges.map((entry) => entry.edge.target);

      return {
        id: node.id,
        name: node.data.label,
        x: node.position.x,
        y: node.position.y,
        branchIds,
      };
    });

    const parallelJoins = parallelJoinNodes.map((node) => {
      const branchEdges = edges
        .filter((edge) => edge.target === node.id)
        .map((edge) => ({
          edge,
          index: parseBranchHandleIndex(edge.targetHandle, JOIN_BRANCH_PREFIX),
        }))
        .filter((entry) => entry.index !== null)
        .sort((left, right) => (left.index ?? 0) - (right.index ?? 0));

      const branchIds = branchEdges.map((entry) => entry.edge.source);

      const outgoingEdge = edges.find(
        (edge) =>
          edge.source === node.id && edge.sourceHandle === JOIN_OUTPUT_HANDLE
      );

      return {
        id: node.id,
        name: node.data.label,
        x: node.position.x,
        y: node.position.y,
        branchIds,
        nextStepId: outgoingEdge?.target,
      };
    });

    return {
      name: "SFC_Program",
      steps,
      transitions,
      parallelSplits: parallelSplits.length > 0 ? parallelSplits : undefined,
      parallelJoins: parallelJoins.length > 0 ? parallelJoins : undefined,
      variables: variables || [],
      metadata: {
        modified: new Date().toISOString(),
        version: "1.0",
      },
    };
  }, [nodes, edges, variables]);

  /**
   * Update variables
   */
  const updateVariables = useCallback((newVariables: SfcWorkspace["variables"]) => {
    setVariables(newVariables);
  }, []);

  /**
   * Highlight active steps during execution
   */
  const highlightActiveSteps = useCallback((activeStepIds: string[]) => {
    setNodes((nds) =>
      nds.map((node) => withActiveState(node, activeStepIds.includes(node.id)))
    );
  }, []);

  /**
   * Update debug state (breakpoints and current debug step)
   */
  const updateDebugState = useCallback(
    (
      breakpoints: string[],
      currentDebugStep: string | null,
      onToggleBreakpoint: (stepId: string) => void
    ) => {
      setNodes((nds) =>
        nds.map((node) => {
          if (!isSfcStepNode(node)) {
            return node;
          }

          return {
            ...node,
            data: {
              ...node.data,
              hasBreakpoint: breakpoints.includes(node.id),
              isCurrentDebugStep: currentDebugStep === node.id,
              onToggleBreakpoint: () => onToggleBreakpoint(node.id),
            },
          };
        })
      );
    },
    []
  );

  return {
    nodes,
    edges,
    variables,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNewStep,
    updateStepNodeData,
    updateParallelNodeData,
    updateEdgeData,
    addActionToStep,
    updateAction,
    deleteAction,
    deleteSelected,
    autoLayout,
    importFromJson,
    exportToJson,
    updateVariables,
    highlightActiveSteps,
    updateDebugState,
    addParallelSplit,
    addParallelJoin,
    setNodes,
    setEdges,
  };
}
