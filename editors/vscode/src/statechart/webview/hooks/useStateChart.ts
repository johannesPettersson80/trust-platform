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
  XStateConfig,
  StateType,
  ActionMapping,
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

function transitionHandles(
  sourcePosition: { x: number; y: number },
  targetPosition: { x: number; y: number }
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
export const useStateChart = () => {
  const [nodes, setNodes] = useState<StateChartNode[]>([]);
  const [edges, setEdges] = useState<StateChartEdge[]>([]);
  const [actionMappings, setActionMappings] = useState<Record<string, ActionMapping>>({});

  // Handle node changes (position, selection, etc.)
  const onNodesChange = useCallback(
    (changes: NodeChange[]) =>
      setNodes((nds) => applyNodeChanges(changes, nds) as StateChartNode[]),
    []
  );

  // Handle edge changes
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) =>
      setEdges((eds) => applyEdgeChanges(changes, eds) as StateChartEdge[]),
    []
  );

  // Handle new connections
  const onConnect = useCallback(
    (connection: Connection) =>
      setEdges(
        (eds) =>
          addEdge({ ...connection, type: STATE_TRANSITION_EDGE, data: {} }, eds) as StateChartEdge[]
      ),
    []
  );

  /**
   * Add a new state node to the diagram
   */
  const addNewState = useCallback(
    (type: StateType = "normal", position?: { x: number; y: number }) => {
      const id = `state_${Date.now()}`;
      const stateCount = nodes.length;
      const columns = Math.ceil(Math.sqrt(stateCount + 1));

      const newNode: StateChartNode = {
        id,
        type: "stateNode",
        position: position || stateGridPosition(stateCount, columns),
        data: {
          label: `State${stateCount + 1}`,
          type,
          entry: [],
          exit: [],
        },
      };

      setNodes((nds) => [...nds, newNode]);
      return id;
    },
    [nodes.length]
  );

  /**
   * Update data for a specific node
   */
  const updateNodeData = useCallback((nodeId: string, data: Partial<StateNodeData>) => {
    setNodes((nds) =>
      nds.map((node) =>
        node.id === nodeId ? { ...node, data: { ...node.data, ...data } } : node
      )
    );
  }, []);

  /**
   * Update data for a specific edge
   */
  const updateEdgeData = useCallback(
    (edgeId: string, data: StateChartEdge["data"]) => {
      setEdges((eds) =>
        eds.map((edge) =>
          edge.id === edgeId ? { ...edge, data: { ...edge.data, ...data } } : edge
        )
      );
    },
    []
  );

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
    const COLS = Math.ceil(Math.sqrt(nodes.length));

    setNodes((nds) =>
      nds.map((node, index) => ({
        ...node,
        position: stateGridPosition(index, COLS),
      }))
    );
  }, [nodes.length]);

  /**
   * Export to XState JSON format
   */
  const exportToXState = useCallback((): XStateConfig => {
    const states: Record<string, any> = {};
    let initialState: string | undefined;

    // Convert nodes to XState states
    nodes.forEach((node) => {
      const stateConfig: any = {};

      if (node.data.entry && node.data.entry.length > 0) {
        stateConfig.entry = node.data.entry;
      }

      if (node.data.exit && node.data.exit.length > 0) {
        stateConfig.exit = node.data.exit;
      }

      if (node.data.type === "final") {
        stateConfig.type = "final";
      } else if (node.data.type === "compound") {
        stateConfig.type = "compound";
      }

      // Find transitions for this state
      const transitions: Record<string, any> = {};
      edges
        .filter((edge) => edge.source === node.id)
        .forEach((edge) => {
          const event = edge.data?.event || "NEXT";
          const targetNode = nodes.find((n) => n.id === edge.target);

          if (targetNode) {
            const transition: any = { target: targetNode.data.label };

            if (edge.data?.guard) {
              transition.guard = edge.data.guard;
            }

            if (edge.data?.actions && edge.data.actions.length > 0) {
              transition.actions = edge.data.actions;
            }

            if (edge.data?.after !== undefined) {
              transition.after = edge.data.after;
            }

            transitions[event] = transition;
          }
        });

      if (Object.keys(transitions).length > 0) {
        stateConfig.on = transitions;
      }

      states[node.data.label] = stateConfig;

      if (node.data.type === "initial") {
        initialState = node.data.label;
      }
    });

    return {
      id: "stateMachine",
      initial: initialState,
      states,
      actionMappings,
    };
  }, [nodes, edges, actionMappings]);

  /**
   * Import from XState JSON format
   */
  const importFromXState = useCallback((config: XStateConfig) => {
    const newNodes: StateChartNode[] = [];
    const newEdges: StateChartEdge[] = [];
    const statePositions = new Map<string, { x: number; y: number }>();

    // Compact grid layout: keep the full state machine visible beside the
    // shared right inspector at normal VS Code widths.
    const stateNames = Object.keys(config.states);
    const COLS = Math.ceil(Math.sqrt(stateNames.length));

    stateNames.forEach((stateName, index) => {
      statePositions.set(stateName, stateGridPosition(index, COLS));
    });

    // Create nodes
    Object.entries(config.states).forEach(([stateName, stateConfig]) => {
      const isInitial = config.initial === stateName;
      const isFinal = stateConfig.type === "final";
      const isCompound = stateConfig.type === "compound";

      let type: StateType = "normal";
      if (isInitial) type = "initial";
      else if (isFinal) type = "final";
      else if (isCompound) type = "compound";

      const node: StateChartNode = {
        id: `state_${stateName}`,
        type: "stateNode",
        position: statePositions.get(stateName) || { x: 0, y: 0 },
        data: {
          label: stateName,
          type,
          entry: stateConfig.entry || [],
          exit: stateConfig.exit || [],
        },
      };

      newNodes.push(node);

      // Create edges from transitions
      if (stateConfig.on) {
        Object.entries(stateConfig.on).forEach(([event, transition]) => {
          const targetName =
            typeof transition === "string" ? transition : transition.target;
          const targetId = `state_${targetName}`;
          const sourcePosition = statePositions.get(stateName) || { x: 0, y: 0 };
          const targetPosition = statePositions.get(targetName) || { x: 0, y: 0 };

          const edge: StateChartEdge = {
            id: `edge_${node.id}_${targetId}_${event}`,
            source: node.id,
            target: targetId,
            type: STATE_TRANSITION_EDGE,
            ...transitionHandles(sourcePosition, targetPosition),
            label: event,
            data: {
              event,
              guard:
                typeof transition === "object" ? transition.guard : undefined,
              actions:
                typeof transition === "object" ? transition.actions : undefined,
              after:
                typeof transition === "object" ? transition.after : undefined,
            },
          };

          newEdges.push(edge);
        });
      }
    });

    setNodes(newNodes);
    setEdges(newEdges);
    setActionMappings(config.actionMappings || {});
  }, []);

  /**
   * Update action mappings
   */
  const updateActionMappings = useCallback((mappings: Record<string, ActionMapping>) => {
    setActionMappings(mappings);
  }, []);

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
    exportToXState,
    importFromXState,
    setNodes,
    setEdges,
  };
};
