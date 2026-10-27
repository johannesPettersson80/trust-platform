import { useCallback, useState } from "react";
import {
  Connection,
  EdgeChange,
  MarkerType,
  NodeChange,
  XYPosition,
  applyEdgeChanges,
  applyNodeChanges,
} from "@xyflow/react";
import type {
  ParallelNodeData,
  SfcAction,
  SfcGraph,
  SfcNode,
  SfcNodeType,
  SfcParallelNode,
  SfcStepNode,
  SfcTransitionEdge,
  SfcVariable,
  StepNodeData,
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
import { requestEdit, stamp } from "../hostRequests";

// The SFC canvas. The rules of an SFC (the graph of a file and the file of a graph, which
// connections are allowed, branch counts, new nodes and their wiring) are the language server's;
// this hook holds the canvas state and draws: where a transition attaches to a step and where a
// new node is placed.

function isSfcStepNode(node: SfcNode): node is SfcStepNode {
  return node.type === "step";
}

function isSfcParallelNode(node: SfcNode): node is SfcParallelNode {
  return node.type === "parallelSplit" || node.type === "parallelJoin";
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

// Where a node added from the toolbar is placed: below the lowest node.
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

// Where a transition between two steps attaches: straight down to the next step, or by the sides
// when it goes back up or skips steps, so it does not run through other transitions' labels.
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

// An edge of the language server with its arrow.
function withArrow(edge: SfcTransitionEdge): SfcTransitionEdge {
  return {
    ...edge,
    markerEnd: {
      type: MarkerType.ArrowClosed,
    },
  };
}

// An edge of the language server as the canvas draws it: with its arrow, and attached to the step
// handles where the edge names none.
function drawnEdge(edge: SfcTransitionEdge, nodes: SfcNode[]): SfcTransitionEdge {
  const drawn = withArrow(edge);
  applyStepConnectionHandles(drawn, nodes);
  return drawn;
}

/**
 * Custom hook for managing SFC state and operations
 */
export function useSfc() {
  const [nodes, setNodes] = useState<SfcNode[]>(INITIAL_NODES);
  const [edges, setEdges] = useState<SfcTransitionEdge[]>(INITIAL_EDGES);
  const [variables, setVariables] = useState<SfcVariable[]>([]);

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
   * A transition the user draws; the language server decides whether the chart allows it.
   */
  const onConnect = useCallback(
    async (connection: Connection) => {
      const sourceNode = nodes.find((node) => node.id === connection.source);
      const targetNode = nodes.find((node) => node.id === connection.target);
      const fallback =
        sourceNode && targetNode
          ? stepConnectionHandles(sourceNode, targetNode)
          : { sourceHandle: null, targetHandle: null };
      const answer = await requestEdit(
        { nodes, edges },
        {
          op: "connect",
          connection: {
            source: connection.source,
            target: connection.target,
            sourceHandle: connection.sourceHandle,
            targetHandle: connection.targetHandle,
          },
          fallback: {
            sourceHandle: fallback.sourceHandle ?? null,
            targetHandle: fallback.targetHandle ?? null,
          },
          stamp: stamp(),
        }
      );
      const edge = answer.edge;
      if (answer.ok && edge) {
        setEdges((eds) => [...eds, withArrow(edge)]);
      }
    },
    [edges, nodes]
  );

  /**
   * Add a node; one added from the toolbar (no drop position) is placed below the chart, and a
   * step added so is wired onto the chart.
   */
  const addNode = useCallback(
    async (nodeType: SfcNodeType, position?: XYPosition) => {
      const answer = await requestEdit(
        { nodes, edges },
        {
          op: "addNode",
          nodeType,
          position: position ?? nextNodePosition(nodes),
          wire: !position,
          stamp: stamp(),
        }
      );
      const node = answer.node;
      if (!answer.ok || !node) {
        return;
      }
      setNodes((nds) => [...nds, node]);
      const edge = answer.edge;
      if (edge) {
        setEdges((eds) => [...eds, drawnEdge(edge, [...nodes, node])]);
      }
    },
    [edges, nodes]
  );

  const addNewStep = useCallback(
    (position?: XYPosition) => addNode("step", position),
    [addNode]
  );

  const addParallelSplit = useCallback(
    (position?: XYPosition) => addNode("parallelSplit", position),
    [addNode]
  );

  const addParallelJoin = useCallback(
    (position?: XYPosition) => addNode("parallelJoin", position),
    [addNode]
  );

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
   * Update parallel split/join node data; a new branch count is the language server's.
   */
  const updateParallelNodeData = useCallback(
    async (nodeId: string, updates: Partial<ParallelNodeData>) => {
      const { branchCount, ...rest } = updates;
      const setData = (data: Partial<ParallelNodeData>) =>
        setNodes((nds) =>
          nds.map((node) =>
            node.id === nodeId && isSfcParallelNode(node)
              ? { ...node, data: { ...node.data, ...data } }
              : node
          )
        );
      if (Object.keys(rest).length > 0) {
        setData(rest);
      }
      if (branchCount === undefined) {
        return;
      }
      const answer = await requestEdit(
        { nodes, edges },
        { op: "setBranchCount", nodeId, branchCount }
      );
      if (!answer.ok || answer.branchCount === undefined) {
        return;
      }
      setData({ branchCount: answer.branchCount });
      const removed = new Set(answer.removedEdgeIds ?? []);
      if (removed.size > 0) {
        setEdges((eds) => eds.filter((edge) => !removed.has(edge.id)));
      }
    },
    [edges, nodes]
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

          const data = { ...edge.data, ...updates };
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
   * Show the canvas graph of a file, as the language server read it.
   */
  const setGraph = useCallback((graph: SfcGraph) => {
    setNodes(graph.nodes);
    setEdges(graph.edges.map((edge) => drawnEdge(edge, graph.nodes)));
    setVariables(graph.variables ?? []);
  }, []);

  /**
   * The canvas graph, for the language server to save, check or translate.
   */
  const currentGraph = useCallback(
    (): SfcGraph => ({ nodes, edges, variables }),
    [nodes, edges, variables]
  );

  /**
   * Update variables
   */
  const updateVariables = useCallback((newVariables: SfcVariable[]) => {
    setVariables(newVariables);
  }, []);

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
    setGraph,
    currentGraph,
    updateVariables,
    addParallelSplit,
    addParallelJoin,
    setNodes,
    setEdges,
  };
}
