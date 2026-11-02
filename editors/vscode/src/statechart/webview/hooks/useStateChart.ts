import { useState, useCallback } from "react";
import {
  Node,
  Edge,
  Connection,
  addEdge,
  applyNodeChanges,
  applyEdgeChanges,
  NodeChange,
  EdgeChange,
} from "@xyflow/react";
import {
  StateChartNode,
  StateChartEdge,
  StateNodeData,
  StatechartGraph,
  StateType,
  ActionMapping,
  RequestAddState,
} from "../types";
import {
  STATE_SOURCE_BOTTOM,
  STATE_SOURCE_LEFT,
  STATE_SOURCE_RIGHT,
  STATE_TARGET_LEFT,
  STATE_TARGET_RIGHT,
  STATE_TARGET_TOP,
} from "../StateNode";
import { STATE_TRANSITION_EDGE } from "../StateTransitionEdge";

const STATE_GRID_X = 230;
const STATE_GRID_Y = 230;
const STATE_GRID_ORIGIN_X = 50;
const STATE_GRID_ORIGIN_Y = 50;

function stateGridPosition(index: number, columns: number): { x: number; y: number } {
  return {
    x: (index % columns) * STATE_GRID_X + STATE_GRID_ORIGIN_X,
    y: Math.floor(index / columns) * STATE_GRID_Y + STATE_GRID_ORIGIN_Y,
  };
}

function stateGridColumns(count: number): number {
  return Math.max(1, Math.min(2, Math.ceil(Math.sqrt(count))));
}

function transitionHandles(
  sourcePosition: { x: number; y: number },
  targetPosition: { x: number; y: number },
): Pick<StateChartEdge, "sourceHandle" | "targetHandle"> {
  const sameRow = Math.abs(sourcePosition.y - targetPosition.y) < STATE_GRID_Y / 2;
  if (sameRow && targetPosition.x > sourcePosition.x) {
    return { sourceHandle: STATE_SOURCE_RIGHT, targetHandle: STATE_TARGET_LEFT };
  }
  if (sameRow && targetPosition.x < sourcePosition.x) {
    return { sourceHandle: STATE_SOURCE_LEFT, targetHandle: STATE_TARGET_RIGHT };
  }
  return { sourceHandle: STATE_SOURCE_BOTTOM, targetHandle: STATE_TARGET_TOP };
}

/**
 * Custom hook for managing StateChart editor state and operations
 */
export const useStateChart = (requestAddState: RequestAddState) => {
  const [nodes, setNodes] = useState<StateChartNode[]>([]);
  const [edges, setEdges] = useState<StateChartEdge[]>([]);
  const [actionMappings, setActionMappings] = useState<Record<string, ActionMapping>>({});

  // Handle node changes (position, selection, etc.)
  const onNodesChange = useCallback(
    (changes: NodeChange[]) => setNodes((nds) => applyNodeChanges(changes, nds) as StateChartNode[]),
    [],
  );

  // Handle edge changes
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => setEdges((eds) => applyEdgeChanges(changes, eds) as StateChartEdge[]),
    [],
  );

  // Handle new connections
  const onConnect = useCallback(
    (connection: Connection) =>
      setEdges((eds) => addEdge({ ...connection, type: STATE_TRANSITION_EDGE, data: {} }, eds) as StateChartEdge[]),
    [],
  );

  /**
   * Update data for a specific node
   */
  const updateNodeData = useCallback((nodeId: string, data: Partial<StateNodeData>) => {
    setNodes((nds) => nds.map((node) => (node.id === nodeId ? { ...node, data: { ...node.data, ...data } } : node)));
  }, []);

  /**
   * Update data for a specific edge
   */
  const updateEdgeData = useCallback((edgeId: string, data: StateChartEdge["data"]) => {
    setEdges((eds) => eds.map((edge) => (edge.id === edgeId ? { ...edge, data: { ...edge.data, ...data } } : edge)));
  }, []);

  /**
   * Delete selected nodes and edges
   */
  const deleteSelected = useCallback(() => {
    setNodes((nds) => nds.filter((node) => !node.selected));
    setEdges((eds) => eds.filter((edge) => !edge.selected));
  }, []);

  /**
   * Apply auto-layout to nodes (simple grid layout for now)
   */
  const autoLayout = useCallback(() => {
    const COLS = stateGridColumns(nodes.length);

    setNodes((nds) =>
      nds.map((node, index) => ({
        ...node,
        position: stateGridPosition(index, COLS),
      })),
    );
  }, [nodes.length]);

  /**
   * The graph the language server writes back as the file (`trust-lsp.visualFormat`): the
   * states and transitions as drawn, in drawing order, and the action mappings.
   */
  const exportGraph = useCallback(
    (): StatechartGraph => ({
      states: nodes.map((node) => ({
        id: node.id,
        label: node.data.label,
        type: node.data.type,
        entry: node.data.entry,
        exit: node.data.exit,
      })),
      transitions: edges.map((edge) => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        event: edge.data?.event,
        guard: edge.data?.guard,
        actions: edge.data?.actions,
        after: edge.data?.after,
      })),
      actionMappings,
    }),
    [nodes, edges, actionMappings],
  );

  /**
   * Adds a state of `type`: the language server names it (`addState`, the smallest free name
   * `State<n>`) or answers why it adds none, and the editor places it on the grid.
   */
  const addNewState = useCallback(
    async (type: StateType = "normal", position?: { x: number; y: number }) => {
      const answer = await requestAddState(exportGraph(), type);
      const state = answer.state;
      if (!answer.ok || !state) {
        console.warn(answer.errors?.[0]?.message ?? "The state was not added.");
        return undefined;
      }
      setNodes((nds) => {
        const columns = stateGridColumns(nds.length + 1);
        const newNode: StateChartNode = {
          id: state.id,
          type: "stateNode",
          position: position || stateGridPosition(nds.length, columns),
          data: { label: state.label, type: state.type, entry: [], exit: [] },
        };
        return [...nds, newNode];
      });
      return state.id;
    },
    [exportGraph, requestAddState],
  );

  /**
   * Draws the graph the language server read from the file (`trust-lsp.visualParse`), placing
   * the states on a compact grid so the whole machine stays visible beside the inspector.
   */
  const importFromXState = useCallback((graph: StatechartGraph) => {
    const columns = stateGridColumns(graph.states.length);
    const positions = new Map<string, { x: number; y: number }>();
    graph.states.forEach((state, index) => {
      positions.set(state.id, stateGridPosition(index, columns));
    });

    const newNodes: StateChartNode[] = graph.states.map((state) => ({
      id: state.id,
      type: "stateNode",
      position: positions.get(state.id) || { x: 0, y: 0 },
      data: {
        label: state.label,
        type: state.type,
        entry: state.entry,
        exit: state.exit,
      },
    }));

    const newEdges: StateChartEdge[] = graph.transitions.map((transition) => ({
      id: transition.id,
      source: transition.source,
      target: transition.target,
      type: STATE_TRANSITION_EDGE,
      ...transitionHandles(
        positions.get(transition.source) || { x: 0, y: 0 },
        positions.get(transition.target) || { x: 0, y: 0 },
      ),
      label: transition.event,
      data: {
        event: transition.event,
        guard: transition.guard,
        actions: transition.actions,
        after: transition.after,
      },
    }));

    setNodes(newNodes);
    setEdges(newEdges);
    setActionMappings(graph.actionMappings);
  }, []);

  /**
   * Update action mappings: the new mappings, or a change of the mappings as they are then.
   */
  const updateActionMappings = useCallback(
    (
      mappings:
        Record<string, ActionMapping> | ((current: Record<string, ActionMapping>) => Record<string, ActionMapping>),
    ) => {
      setActionMappings(mappings);
    },
    [],
  );

  return {
    nodes,
    edges,
    actionMappings,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNewState,
    updateNodeData,
    updateEdgeData,
    updateActionMappings,
    deleteSelected,
    autoLayout,
    exportGraph,
    importFromXState,
    setNodes,
    setEdges,
  };
};
