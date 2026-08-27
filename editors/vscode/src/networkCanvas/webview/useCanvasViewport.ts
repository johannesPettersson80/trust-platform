import React, { useCallback, useEffect, useRef } from "react";
import type { Edge, Node } from "@xyflow/react";

interface CanvasStateStore {
  getState(): unknown;
  setState(state: unknown): void;
}

interface CanvasViewportOptions {
  nodes: Node[];
  nextNodes: Node[];
  nextEdges: Edge[];
  setNodes: React.Dispatch<React.SetStateAction<Node[]>>;
  setEdges: React.Dispatch<React.SetStateAction<Edge[]>>;
  fitView(options: { padding: number; duration: number; maxZoom?: number }): Promise<boolean>;
  focusTargetId: string | undefined;
  clearFocusTarget(): void;
  editSlotsVisible: boolean;
  editMode: boolean;
  activeDrawerWidth: number;
  discoveryOpen: boolean;
  discoveryResultCount: number;
  discoveryProgressCount: number;
  discoveryScanning: boolean;
  browseOpen: boolean;
  browseTreeSize: number;
  browseLoading: boolean;
  applyResultSignature: string;
  builtNodeCount: number;
  stateStore: CanvasStateStore;
}

/** Owns viewport fitting and persisted manual positions; graph-domain state stays in the canvas app. */
export function useCanvasViewport(options: CanvasViewportOptions) {
  const {
    nodes,
    nextNodes,
    nextEdges,
    setNodes,
    setEdges,
    fitView,
    focusTargetId,
    clearFocusTarget,
    editSlotsVisible,
    editMode,
    activeDrawerWidth,
    discoveryOpen,
    discoveryResultCount,
    discoveryProgressCount,
    discoveryScanning,
    browseOpen,
    browseTreeSize,
    browseLoading,
    applyResultSignature,
    builtNodeCount,
    stateStore,
  } = options;
  const fitSignature = useRef("");
  const flowWrapRef = useRef<HTMLDivElement>(null);
  const nodeCount = useRef(0);
  const positions = useRef<Record<string, { x: number; y: number }>>(
    ((stateStore.getState() as { positions?: Record<string, { x: number; y: number }> } | undefined)
      ?.positions) ?? {}
  );
  const layoutMode = useRef(editSlotsVisible);

  // Live refreshes must retain manual top-level positions. Edit mode deliberately reflows all other
  // nodes because the temporary add slots change the host geometry.
  useEffect(() => {
    const modeChanged = layoutMode.current !== editSlotsVisible;
    layoutMode.current = editSlotsVisible;
    setNodes((previous) => {
      const previousPositions = new Map(previous.map((node) => [node.id, node.position]));
      return nextNodes.map((node) => {
        if (node.parentId) {
          return node;
        }
        const position = modeChanged
          ? positions.current[node.id] ?? node.position
          : previousPositions.get(node.id) ?? positions.current[node.id] ?? node.position;
        return { ...node, position };
      });
    });
    setEdges(nextEdges);
  }, [editSlotsVisible, nextEdges, nextNodes, setEdges, setNodes]);

  useEffect(() => {
    if (!focusTargetId || !nodes.some((node) => node.id === focusTargetId)) {
      return;
    }
    const timers = [0, 120, 360].map((delay) =>
      setTimeout(() => void fitView({ duration: 220, padding: 0.2, maxZoom: 1.2 }), delay)
    );
    const clearTimer = setTimeout(clearFocusTarget, 420);
    return () => {
      timers.forEach(clearTimeout);
      clearTimeout(clearTimer);
    };
  }, [clearFocusTarget, fitView, focusTargetId, nodes]);

  // Node identity, rather than live values, controls structural refits so polling cannot yank the view.
  useEffect(() => {
    nodeCount.current = nodes.length;
    const signature = nodes.map((node) => node.id).sort().join("|");
    if (signature && signature !== fitSignature.current) {
      fitSignature.current = signature;
      if (focusTargetId) {
        return;
      }
      const timers = [0, 140, 420].map((delay) =>
        setTimeout(() => void fitView({ padding: 0.2, duration: 220 }), delay)
      );
      return () => timers.forEach(clearTimeout);
    }
  }, [fitView, focusTargetId, nodes]);

  useEffect(() => {
    const element = flowWrapRef.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        if (nodeCount.current > 0) void fitView({ padding: 0.2, duration: 200 });
      }, 140);
    });
    observer.observe(element);
    return () => {
      if (timer) clearTimeout(timer);
      observer.disconnect();
    };
  }, [fitView]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refit = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        if (nodeCount.current > 0) void fitView({ padding: 0.2, duration: 220 });
      }, 120);
    };
    const onVisibility = () => {
      if (!document.hidden) refit();
    };
    window.addEventListener("resize", refit);
    window.addEventListener("focus", refit);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      if (timer) clearTimeout(timer);
      window.removeEventListener("resize", refit);
      window.removeEventListener("focus", refit);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [fitView]);

  // Recover if a host resize or editor split leaves every rendered node outside the viewport.
  useEffect(() => {
    const nodesAreVisible = () => {
      const root = flowWrapRef.current;
      if (!root) return true;
      const rootRect = root.getBoundingClientRect();
      const elements = Array.from(root.querySelectorAll<HTMLElement>(".react-flow__node"));
      if (!elements.length) return nodeCount.current === 0;
      return elements.some((element) => {
        const rect = element.getBoundingClientRect();
        return rect.right > rootRect.left && rect.left < rootRect.right && rect.bottom > rootRect.top && rect.top < rootRect.bottom;
      });
    };
    const id = window.setInterval(() => {
      if (nodeCount.current > 0 && !nodesAreVisible()) {
        void fitView({ padding: 0.2, duration: 220 });
      }
    }, 900);
    return () => window.clearInterval(id);
  }, [fitView]);

  const previousEditMode = useRef(editMode);
  useEffect(() => {
    if (previousEditMode.current === editMode) return;
    previousEditMode.current = editMode;
    const timer = setTimeout(() => void fitView({ padding: 0.2, duration: 300 }), 70);
    return () => clearTimeout(timer);
  }, [editMode, fitView]);

  const previousDrawerWidth = useRef(0);
  useEffect(() => {
    if (previousDrawerWidth.current === activeDrawerWidth) return;
    previousDrawerWidth.current = activeDrawerWidth;
    const timer = setTimeout(() => void fitView({ padding: 0.2, duration: 320 }), 80);
    return () => clearTimeout(timer);
  }, [activeDrawerWidth, fitView]);

  useEffect(() => {
    if (!discoveryOpen || nodeCount.current === 0) return;
    const timer = setTimeout(() => void fitView({ padding: 0.2, duration: 220 }), 120);
    return () => clearTimeout(timer);
  }, [discoveryOpen, discoveryProgressCount, discoveryResultCount, discoveryScanning, fitView]);

  useEffect(() => {
    if (nodeCount.current === 0) return;
    const timers = [140, 460].map((delay) =>
      setTimeout(() => void fitView({ padding: 0.2, duration: 240 }), delay)
    );
    return () => timers.forEach(clearTimeout);
  }, [applyResultSignature, browseLoading, browseOpen, browseTreeSize, builtNodeCount, fitView]);

  const onNodeDragStop = useCallback(
    (_event: React.MouseEvent, node: Node) => {
      if (node.parentId) return;
      positions.current = { ...positions.current, [node.id]: node.position };
      const state = (stateStore.getState() as Record<string, unknown>) ?? {};
      stateStore.setState({ ...state, positions: positions.current });
    },
    [stateStore]
  );

  return { flowWrapRef, onNodeDragStop };
}
