import React, { memo } from "react";
import { Handle, Position, NodeProps } from "@xyflow/react";
import type { SfcParallelNode } from "./types";

const MIN_PARALLEL_BRANCHES = 2;
const SPLIT_INPUT_HANDLE = "in";
const JOIN_OUTPUT_HANDLE = "out";
const SPLIT_BRANCH_PREFIX = "branch-out-";
const JOIN_BRANCH_PREFIX = "branch-in-";

/**
 * Custom node component for SFC Parallel Split/Join markers
 * Represents IEC 61131-3 parallel divergence and convergence
 */
export const ParallelNode = memo(
  ({ data, selected }: NodeProps<SfcParallelNode>) => {
    const isSplit = data.nodeType === "parallelSplit";
    // The language server keeps the branch count a whole number of at least two.
    const branchCount = data.branchCount ?? MIN_PARALLEL_BRANCHES;
    const width = Math.max(120, branchCount * 52);
    const branchPositions = Array.from({ length: branchCount }, (_, index) => ({
      index,
      left: `${((index + 1) / (branchCount + 1)) * 100}%`,
    }));

    const getNodeStyle = (): React.CSSProperties => {
      return {
        width: `${width}px`,
        padding: "4px 12px",
        borderRadius: "2px",
        border: "3px double",
        borderColor: selected
          ? "var(--vscode-focusBorder)"
          : "var(--vscode-editor-foreground)",
        background: "var(--vscode-editor-background)",
        color: "var(--vscode-editor-foreground)",
        fontFamily: "var(--vscode-font-family)",
        fontSize: "11px",
        fontWeight: 600,
        textAlign: "center",
        position: "relative",
        height: "8px",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        boxSizing: "border-box",
        boxShadow: selected ? "0 0 0 2px var(--vscode-focusBorder)" : "none",
      };
    };

    return (
      <div style={getNodeStyle()}>
        {isSplit ? (
          <Handle
            id={SPLIT_INPUT_HANDLE}
            type="target"
            position={Position.Top}
            style={{
              background: "var(--vscode-editor-foreground)",
              width: 10,
              height: 10,
              top: -5,
            }}
          />
        ) : (
          branchPositions.map(({ index, left }) => (
            <Handle
              key={`join-in-${index}`}
              id={`${JOIN_BRANCH_PREFIX}${index}`}
              type="target"
              position={Position.Top}
              style={{
                background: "var(--vscode-editor-foreground)",
                width: 10,
                height: 10,
                top: -5,
                left,
                transform: "translateX(-50%)",
              }}
            />
          ))
        )}

        {/* Label (optional, shown on hover) */}
        <div
          style={{
            position: "absolute",
            top: "100%",
            left: "50%",
            transform: "translateX(-50%)",
            marginTop: "4px",
            fontSize: "9px",
            opacity: 0.7,
            whiteSpace: "nowrap",
          }}
          title={data.label}
        >
          {isSplit ? "SPLIT" : "JOIN"}
        </div>

        {isSplit ? (
          branchPositions.map(({ index, left }) => (
            <Handle
              key={`split-out-${index}`}
              id={`${SPLIT_BRANCH_PREFIX}${index}`}
              type="source"
              position={Position.Bottom}
              style={{
                background: "var(--vscode-editor-foreground)",
                width: 10,
                height: 10,
                bottom: -5,
                left,
                transform: "translateX(-50%)",
              }}
            />
          ))
        ) : (
          <Handle
            id={JOIN_OUTPUT_HANDLE}
            type="source"
            position={Position.Bottom}
            style={{
              background: "var(--vscode-editor-foreground)",
              width: 10,
              height: 10,
              bottom: -5,
            }}
          />
        )}
      </div>
    );
  }
);

ParallelNode.displayName = "ParallelNode";
